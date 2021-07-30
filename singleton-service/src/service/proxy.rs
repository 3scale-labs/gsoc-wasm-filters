use crate::configuration::delta::{DeltaStoreConfig, FlushMode};
use crate::configuration::service::ServiceConfig;
use crate::service::{
    auth::*,
    deltas::{DeltaStore, DeltaStoreState},
    report::*,
};
use anyhow::*;
use log::{debug, info};
use proxy_wasm::{
    hostcalls::{dequeue_shared_queue, register_shared_queue},
    traits::{Context, RootContext},
    types::LogLevel,
};
use std::collections::HashMap;
use std::convert::TryInto;
use std::str::FromStr;
use std::time::Duration;
use thiserror::Error;
use threescale::{
    proxy::{get_application_from_cache, set_application_to_cache, CacheKey},
    structs::{
        AppId, AppIdentifier, AppKey, Application, Message, Period, PeriodWindow, ServiceId,
        ServiceToken, ThreescaleData, UsageReport,
    },
    upstream::*,
    utils::{limit_check_and_update_application, UpdateMetricsError},
};
use threescalers::{http::Request, response::Authorization};
// QUEUE_NAME should be the same as the one in cache filter.
const QUEUE_NAME: &str = "message_queue";
const RATE_LIMIT_STATUS: &str = "409";
const TIMEOUT_STATUS: &str = "504";

#[derive(Error, Debug)]
pub enum SingletonServiceError {
    #[error("Error retrieving local cache entry for cache key: {0}")]
    GetCacheFailure(String),

    #[error("Error settiing local cache entry for cache key: {0}")]
    SetCacheFailure(String),

    #[error("Empty reports for authorize for app_id: {0}")]
    EmptyAuthUsages(String),

    #[error("Authorize response error")]
    AuthResponse,

    #[error("Authorize response processing error")]
    AuthResponseProcess,

    #[error("Authorize response app_keys missing")]
    AuthAppKeysMissing,

    #[error("Conversion from i64 time to u64 duration failed")]
    NegativeTimeErr,

    #[error("limit_check_and_update_application failed")]
    UpdateMetricsFail(String),
}

#[no_mangle]
pub fn _start() {
    proxy_wasm::set_log_level(LogLevel::Info);
    proxy_wasm::set_root_context(|context_id| -> Box<dyn RootContext> {
        Box::new(SingletonService {
            context_id,
            config: ServiceConfig::default(),
            queue_id: None,
            delta_store: DeltaStore {
                last_update: None,
                memory_allocated: 0,
                deltas: HashMap::new(),
                config: DeltaStoreConfig::default(),
            },
            cache_keys: HashMap::new(),
            report_requests: HashMap::new(),
        })
    });
}

struct SingletonService {
    context_id: u32,
    config: ServiceConfig,
    queue_id: Option<u32>,
    delta_store: DeltaStore,
    cache_keys: HashMap<CacheKey, ServiceToken>,
    report_requests: HashMap<u32, Report>,
}

impl RootContext for SingletonService {
    // Message queue will get registered when on_vm_start callback gets called.
    fn on_vm_start(&mut self, _vm_configuration_size: usize) -> bool {
        if let Ok(q_id) = register_shared_queue(QUEUE_NAME) {
            self.queue_id = Some(q_id);
        }
        // TODO : handle MQ failure, change info to trace after dev
        info!(
            "Registered new message queue with id: {}",
            self.queue_id.unwrap()
        );
        true
    }

    /// Configuration passed by envoy.yaml will get deserialized to ServiceConfig. If there's an issue with the
    /// passed configuration, default configuration will be used.
    fn on_configure(&mut self, _config_size: usize) -> bool {
        // Check for the configuration passed by envoy.yaml
        self.set_tick_period(Duration::from_secs(5));
        let configuration: Vec<u8> = match self.get_configuration() {
            Some(c) => c,
            None => {
                info!("Configuration missing. Please check the envoy.yaml file for filter configuration.
                Using default configuration.");
                return true;
            }
        };

        // Parse and store the configuration passed by envoy.yaml
        match serde_json::from_slice::<ServiceConfig>(configuration.as_ref()) {
            Ok(config) => {
                debug!("configuring {}: {:?}", self.context_id, config);
                self.config = config;
                self.set_tick_period(self.config.delta_store_config.periodical_flush);
                self.delta_store.config = self.config.delta_store_config.clone();
                true
            }
            Err(e) => {
                info!(
                    "Failed to parse envoy.yaml configuration: {:?}. Using default configuration.",
                    e
                );
                true
            }
        }
    }

    /// on_queue_ready will get triggered when cache filter enqueue data. dequeue_shared_queue() is used
    /// to dequeu data from the queue. For each entry the following functions are performed.
    ///     * If local cache update is required, update metrics and perform local cache update.
    ///     * Add the entry to delta store.
    fn on_queue_ready(&mut self, _queue_id: u32) {
        match dequeue_shared_queue(self.queue_id.unwrap()) {
            Ok(queue_entry) => {
                match queue_entry {
                    Some(message) => {
                        // TODO: Hanlde deserialization safely.
                        let message_received: Message = bincode::deserialize(&message).unwrap();
                        // TODO: Handle update failure
                        info!(
                            "Consumed following message from the shared queue: {:?}",
                            message_received
                        );
                        let threescale: ThreescaleData = message_received.data;
                        let req_time: Duration = message_received.req_time;
                        if message_received.update_cache_from_singleton {
                            self.update_application_cache(&threescale, &req_time)
                                .unwrap();
                        }
                        // TODO : Handle delta store update failure.
                        self.cache_keys
                            .entry(CacheKey::from(&threescale.service_id, &threescale.app_id))
                            .or_insert_with(|| threescale.service_token.clone());
                        let delta_store_state =
                            self.delta_store.update_delta_store(&threescale).unwrap();
                        if delta_store_state == DeltaStoreState::Flush {
                            self.flush_local_cache();
                        }
                    }
                    None => {
                        info!("No application found from the message queue entry")
                    }
                }
            }
            Err(error) => info!(
                "Error consuming message from the message queue: {:?}",
                error
            ),
        }
    }

    /// Delta store flush is required in case of a low traffic where it takes a long time to fill the delta store
    /// container.
    // TODO: Consider requirements of a dynamic tick and timestamp based flush if it makes a significant
    // improvement.
    fn on_tick(&mut self) {
        info!(
            "onTick triggerd. Current tick duration: {:?}",
            self.config.delta_store_config.periodical_flush
        );
        // Perform cache update based on the cache flush type defined by the user. For Default,
        // other than the container limit this will trigger the cache update.
        // For periodical only this onTick method will trigger the cache update.
        // For ContainerLimit, no effect here.
        if self.delta_store.config.flush_mode != FlushMode::ContainerLimit {
            self.flush_local_cache()
        }
    }
}

impl Context for SingletonService {
    fn on_http_call_response(
        &mut self,
        token_id: u32,
        _num_headers: usize,
        body_size: usize,
        _num_trailers: usize,
    ) {
        info!("3scale SM API response for call token :{}", token_id);
        let headers = self.get_http_call_response_headers();
        let status = headers
            .iter()
            .find(|(key, _)| key.as_str() == ":status")
            .map(|(_, value)| value)
            .unwrap();
        if status != TIMEOUT_STATUS {
            match self.get_http_call_response_body(0, body_size) {
                Some(bytes) => {
                    info!("Auth response");
                    // TODO : Handle auth processing.
                    #[allow(unused_must_use)]
                    {
                        self.handle_auth_response(bytes, status);
                    }
                }
                None => {
                    if self.report_requests.contains_key(&token_id) {
                        info!("Report response");
                        self.handle_report_response(status, &token_id);
                    }
                }
            }
        } else {
            info!(
                "HTTP request timeout for request with token_id: {}",
                token_id
            );
            if self.report_requests.contains_key(&token_id) {
                self.handle_report_response(status, &token_id);
            }
        }
    }
}

impl SingletonService {
    /// update_application_cache method updates the local application cache if the cache update
    /// fails from the cache filter for a particular request.
    fn update_application_cache(
        &self,
        threescale: &ThreescaleData,
        req_time: &Duration,
    ) -> Result<(), anyhow::Error> {
        let cache_key = CacheKey::from(&threescale.service_id, &threescale.app_id);
        match get_application_from_cache(&cache_key) {
            Ok((mut application, cas)) => {
                match limit_check_and_update_application(
                    threescale,
                    &mut application,
                    cas,
                    req_time,
                ) {
                    Ok(()) => Ok(()),
                    Err(UpdateMetricsError::CacheUpdateFail) => {
                        anyhow::bail!(SingletonServiceError::SetCacheFailure(
                            cache_key.as_string()
                        ))
                    }
                    Err(e) => {
                        anyhow::bail!(SingletonServiceError::UpdateMetricsFail(e.to_string()))
                    }
                }
            }
            Err(_) => {
                info!("No app in shared data");
                anyhow::bail!(SingletonServiceError::GetCacheFailure(
                    cache_key.as_string()
                ))
            }
        }
    }

    /// is_flush_required gets executed when on_tick() gets triggered. It will initiate delta store flush
    /// if it is required.
    #[allow(dead_code)]
    fn is_flush_required(&self) -> bool {
        true
    }

    /// This is a helper method to send http requests. Both Report and Auth calls will use this method to
    /// send http requests after building relevant threescalers request type.
    /// TODO : Handle http callout failure from proxy side.
    fn perform_http_call(&self, request: &Request) -> Result<u32, anyhow::Error> {
        // TODO: read upstream from config when configuration parsing is re-implemented.
        let upstream = Upstream {
            name: "outbound|443||su1.3scale.net".to_string(),
            url: "https://su1.3scale.net".parse().unwrap(),
            timeout: Duration::from_millis(5000),
        };
        let (uri, body) = request.uri_and_body();
        let headers = request
            .headers
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect::<Vec<_>>();
        let call_token = upstream.call(
            self,
            uri.as_ref(),
            request.method.as_str(),
            headers,
            body.map(str::as_bytes),
            None,
            None,
        )?;
        info!("http call performed. call token : {}", call_token);
        Ok(call_token)
    }

    /// This method flush the deltas in the deltastore by making a clone and then
    /// by emptying the deltastore hashmap.  
    fn flush_delta_store(
        &mut self,
    ) -> HashMap<String, HashMap<AppIdentifier, HashMap<String, u64>>> {
        let deltas_cloned = self.delta_store.deltas.clone();
        self.delta_store.deltas.clear();
        self.delta_store.memory_allocated = 0;
        assert!(self.delta_store.deltas.is_empty());
        deltas_cloned
    }

    /// This method will flush the local cache to the 3scale SM API by sending a report call per each service.
    /// This method uses flush_delta_store(), build_report_request() and perform_http_call() helper methods to
    /// flush local cache. This will be called when delta store is full or when timer based cache flush is required.
    fn flush_local_cache(&mut self) {
        let deltas = self.flush_delta_store();
        let mut auth_keys = HashMap::new();
        for (key, apps) in deltas {
            let report: Report = report(&key, &apps).unwrap();
            let request = build_report_request(&report).unwrap();
            let app_keys = apps.keys().cloned().collect::<Vec<_>>();
            auth_keys.insert(key, app_keys);
            info!("report : {:?}", report);
            // TODO: Handle http local failure
            match self.perform_http_call(&request) {
                Ok(token_id) => {
                    self.report_requests.insert(token_id, report);
                }
                Err(err) => {
                    info!("Error: {}", err);
                }
            }
        }
        self.update_local_cache();
    }

    /// Update the local cache by sending authorize requests to 3scale SM API.
    fn update_local_cache(&self) {
        for (cache_key, service_token) in self.cache_keys.iter() {
            if let Ok(auth_request) = auth(
                cache_key.service_id().as_ref().to_string(),
                service_token.as_ref().to_string(),
                cache_key.app_id().clone(),
            )
            .and_then(|auth_app| build_auth_request(&auth_app))
            {
                // TODO : Handle local failure.
                self.perform_http_call(&auth_request).unwrap();
            } else {
                // TODO : Handle threescalers auth request creation.
                info!("Error creating Auth request")
            }
        }
    }

    /// Handle Authorize response received from the 3scale SM API. Depending on the response,
    /// several operations will be performed like cache update.
    fn handle_auth_response(&self, response: Vec<u8>, status: &str) -> Result<(), anyhow::Error> {
        // TODO : Handle cache update after enabling list keys extension for both 200 and 409.
        match Authorization::from_str(std::str::from_utf8(&response).unwrap()) {
            Ok(Authorization::Status(data)) => {
                info!("auth response : {:?}", data);
                if data.is_authorized() || status == RATE_LIMIT_STATUS {
                    let app_keys = data
                        .app_keys()
                        .ok_or(SingletonServiceError::AuthAppKeysMissing)?;
                    let app_id =
                        AppIdentifier::from(AppId::from(app_keys.app_id().unwrap().as_ref()));
                    let service_id = ServiceId::from(app_keys.service_id().unwrap().as_ref());
                    let mut new_app_state = HashMap::new();
                    let reports = data.usage_reports().ok_or_else(|| {
                        SingletonServiceError::EmptyAuthUsages(app_id.as_ref().to_string())
                    })?;
                    for usage in reports {
                        new_app_state.insert(
                            usage.metric.clone(),
                            UsageReport {
                                period_window: PeriodWindow {
                                    start: Duration::from_secs(
                                        usage
                                            .period_start
                                            .0
                                            .try_into()
                                            .or(Err(SingletonServiceError::NegativeTimeErr))?,
                                    ),
                                    end: Duration::from_secs(
                                        usage
                                            .period_end
                                            .0
                                            .try_into()
                                            .or(Err(SingletonServiceError::NegativeTimeErr))?,
                                    ),
                                    window: Period::from(&usage.period),
                                },
                                left_hits: usage.max_value - usage.current_value,
                                max_value: usage.max_value,
                            },
                        );
                    }
                    let mut hierarchy = HashMap::new();
                    if let Some(metrics) = data.hierarchy() {
                        for (parent, children) in metrics.iter() {
                            hierarchy.insert(parent.clone(), children.clone());
                        }
                    }

                    let app;
                    if let Some(app_keys) = data.app_keys() {
                        let keys = app_keys
                            .keys()
                            .iter()
                            .map(|app_key| AppKey::from(app_key.as_ref()))
                            .collect::<Vec<_>>();
                        app = Application {
                            app_id: app_id.clone(),
                            service_id: service_id.clone(),
                            local_state: new_app_state,
                            metric_hierarchy: hierarchy,
                            app_keys: Some(keys),
                        };
                    } else {
                        app = Application {
                            app_id: app_id.clone(),
                            service_id: service_id.clone(),
                            local_state: new_app_state,
                            metric_hierarchy: hierarchy,
                            app_keys: None,
                        };
                    }

                    set_application_to_cache(
                        CacheKey::from(&service_id, &app_id).as_string().as_ref(),
                        &app,
                        0,
                    );
                    Ok(())
                } else {
                    anyhow::bail!(SingletonServiceError::AuthResponse)
                }
            }
            Ok(Authorization::Error(error)) => {
                info!("auth error response: {:?}", error);
                anyhow::bail!(SingletonServiceError::AuthResponse)
            }
            Err(e) => {
                info!("error processing auth response: {:?}", e);
                anyhow::bail!(SingletonServiceError::AuthResponseProcess)
            }
        }
    }

    /// Handle Report response received from the 3scale SM API. Depending on the response received
    /// several operations will take place.
    fn handle_report_response(&mut self, status: &str, token_id: &u32) {
        // TODO : Handle report failure.
        info!("Report status : {} {}", status, token_id);
        self.report_requests.remove(token_id);
    }
}
