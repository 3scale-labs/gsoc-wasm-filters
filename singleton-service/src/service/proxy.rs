pub use crate::configuration::service::ServiceConfig;
pub use crate::service::deltas::DeltaStore;
use crate::service::report::*;
use threescale::upstream::*;
use log::{debug, info, warn};
use proxy_wasm::{
    hostcalls::{dequeue_shared_queue, register_shared_queue},
    traits::{Context, RootContext},
    types::LogLevel,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;
use threescale::{
    proxy::cache::{get_application_from_cache, set_application_to_cache},
    structs::{Message, ThreescaleData},
    utils::update_metrics,
};

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
                deltas: RefCell::new(HashMap::new()),
            },
        })
    });
}

struct SingletonService {
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
                        self.delta_store.update_delta_store(&threescale);
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
        // This is just a demo of a single Report Call to test the Report call untill bulk requests are implemented.
        info!("onTick triggerd");
        let report: Report = report().unwrap();
        let request = build_report_request(&report).unwrap();
        let (uri, body) = request.uri_and_body();
        info!("request: {:?}", request);
        let headers = request
            .headers
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect::<Vec<_>>();
        let upstream = Upstream {
            name: "3scale-SM-API".to_string(),
            url: "https://su1.3scale.net".parse().unwrap(),
            timeout: Duration::from_millis(5000),
        };
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
                info!("Error: {:?}", e);
                // TODO : Handle error properly with a suitable retry mechanism.
                panic!("Error: {:?}", e)
            }
        };
        info!(
            "threescale_cache_singleton: on_http_request_headers: call token is {}",
            call_token
        );
    }
}

impl Context for SingletonService {}

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
}
