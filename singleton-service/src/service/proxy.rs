pub use crate::configuration::service::ServiceConfig;
pub use crate::service::deltas::DeltaStore;
use crate::service::deltas::{AppDelta, DeltaStoreState};
use crate::service::report::*;
use log::{debug, info, warn};
use proxy_wasm::{
    hostcalls::{dequeue_shared_queue, register_shared_queue},
    traits::{Context, RootContext},
    types::LogLevel,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;
use threescale::upstream::*;
use threescale::{
    proxy::cache::{get_application_from_cache, set_application_to_cache},
    structs::{Message, ThreescaleData},
    utils::update_metrics,
};
use threescalers::http::Request;

// QUEUE_NAME should be the same as the one in cache filter.
const QUEUE_NAME: &str = "message_queue";

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
                request_count: 0,
                capacity: 2, // TODO : Re-implement config parsing after finalizing the config structs.
                deltas: HashMap::new(),
            },
        })
    });
}

pub struct SingletonService {
    context_id: u32,
    config: ServiceConfig,
    queue_id: Option<u32>,
    delta_store: DeltaStore,
}

impl RootContext for SingletonService {
    /// Message queue will get registered when on_vm_start callback gets called.
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
                true
            }
            Err(e) => {
                warn!(
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
                // TODO : Handle local cache update based on the flag passed from cache filter (when it's implemented)
                info!(
                    "Consumed following message from the shared queue: {:?}",
                    queue_entry
                );
                match queue_entry {
                    Some(message) => {
                        // TODO: Hanlde deserialization safely.
                        let message_received: Message = bincode::deserialize(&message).unwrap();
                        // TODO: Handle update failure
                        let threescale: ThreescaleData = message_received.data;
                        if message_received.update_cache_from_singleton {
                            self.update_application_cache(&threescale);
                        }
                        // TODO : Handle delta store update failure.
                        self.delta_store.update_delta_store(&threescale).unwrap();
                    }
                    None => {
                        info!("No application found from the message queue entry")
                    }
                }
            }
            Err(error) => info!("Error consuming message from the shared queue: {:?}", error),
        }
    }

    /// When on_tick gets triggered, is_flush_required will check whether delta store flush is required or not.
    /// Delta store flush is required in case of a low traffic where it takes a long time to fill the delta store
    /// container.
    fn on_tick(&mut self) {
        // This is just a demo of adding delta entries to delta store and flushing them.
        info!("onTick triggerd. starting test scenario....");
        let metrics1: HashMap<String, u32> = [
            ("hits".to_string(), 1_u32),
            ("hits.79419".to_string(), 1_u32),
        ]
        .iter()
        .cloned()
        .collect();
        let threescale1 = ThreescaleData {
            app_id: "46de54605a1321aa3838480c5fa91bcc".to_string(),
            service_id: "2555417902188".to_string(),
            service_token: "6705c7d02e9a899d4db405dc1413361611e4250dfd12ec3dcbcea8c3de7cdd29"
                .to_string(),
            metrics: RefCell::new(metrics1),
        };
        let metrics2: HashMap<String, u32> = [
            ("hits".to_string(), 1_u32),
            ("hits.73545".to_string(), 1_u32),
        ]
        .iter()
        .cloned()
        .collect();
        let threescale2 = ThreescaleData {
            app_id: "de90b3d58dc5449572d2fdb7ae0af61a".to_string(),
            service_id: "2555417889374".to_string(),
            service_token: "e1abc8f29e6ba7dfed3fcc9c5399be41f7a881f85fa11df68b93a5d800c3c07a"
                .to_string(),
            metrics: RefCell::new(metrics2),
        };
        let state1 = self.delta_store.update_delta_store(&threescale1).unwrap();
        if state1 == DeltaStoreState::Flush {
            info!("Cache flush required. flushing....");
            self.flush_local_cache();
        }
        let state2 = self.delta_store.update_delta_store(&threescale2).unwrap();
        if state2 == DeltaStoreState::Flush {
            info!("Cache flush required. flushing....");
            self.flush_local_cache();
        }
    }
}

impl Context for SingletonService {
    fn on_http_call_response(
        &mut self,
        token_id: u32,
        _num_headers: usize,
        _body_size: usize,
        _num_trailers: usize,
    ) {
        info!("3scale SM API response for call token :{}", token_id);
    }
}

impl SingletonService {
    /// update_application_cache method updates the local application cache if the cache update
    /// fails from the cache filter for a particular request
    fn update_application_cache(&self, threescale: &ThreescaleData) -> bool {
        let cache_key = format!("{}_{}", threescale.app_id, threescale.service_id);
        match get_application_from_cache(&cache_key) {
            Some((mut application, _)) => {
                let is_updated: bool = update_metrics(threescale, &mut application);
                if is_updated {
                    set_application_to_cache(&cache_key, &application, false, None);
                    // TODO: Handle when set_cache fail
                    true
                } else {
                    false
                    // TODO: Handle when update_metrics fail
                }
            }
            //TODO: Handle when no app in cache
            None => {
                info!("No app in shared data");
                false
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
    fn perform_http_call(&self, request: &Request) {
        // TODO: read upstream from config when configuration parsing is re-implemented.
        let upstream = Upstream {
            name: "3scale-SM-API".to_string(),
            url: "https://su1.3scale.net".parse().unwrap(),
            timeout: Duration::from_millis(5000),
        };
        let (uri, body) = request.uri_and_body();
        let headers = request
            .headers
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect::<Vec<_>>();
        let call_token = match upstream.call(
            self,
            uri.as_ref(),
            request.method.as_str(),
            headers,
            body.map(str::as_bytes),
            None,
            None,
        ) {
            Ok(call_token) => call_token,
            Err(e) => {
                panic!("Error occured performing http call : {:?}", e)
            }
        };
        info!("http call performed. call token : {}", call_token)
    }

    /// This method flush the deltas in the deltastore by making a clone and then
    /// by emptying the deltastore hashmap.  
    fn flush_delta_store(&mut self) -> HashMap<String, HashMap<String, AppDelta>> {
        let deltas_cloned = self.delta_store.deltas.clone();
        self.delta_store.deltas.clear();
        assert!(self.delta_store.deltas.is_empty());
        deltas_cloned
    }

    /// This method will flush the local cache to the 3scale SM API by sending a report call per each service.
    /// This method uses flush_delta_store(), build_report_request() and perform_http_call() helper methods to
    /// flush local cache. This will be called when delta store is full or when timer based cache flush is required.
    fn flush_local_cache(&mut self) {
        let deltas = self.flush_delta_store();
        for (key, apps) in deltas {
            let report: Report = report(&key, &apps).unwrap();
            let request = build_report_request(&report).unwrap();
            info!("report : {:?}", report);
            self.perform_http_call(&request);
        }
    }
}
