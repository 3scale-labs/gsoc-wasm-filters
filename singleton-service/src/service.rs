pub use crate::configuration::service::ServiceConfig;
use threescale::{
    structs::{ ThreescaleData, Application },
    utils::update_metrics};

use log::{debug, warn, info, error};
use proxy_wasm::{
    traits::{Context, RootContext},
    types::LogLevel,
    hostcalls::{
        register_shared_queue,
        dequeue_shared_queue,
        get_shared_data,
        set_shared_data
    }
};

const QUEUE_NAME: &str = "message_queue"; 
const VM_ID: &str = "my_vm_id";

#[no_mangle]
pub fn _start() {
    proxy_wasm::set_log_level(LogLevel::Trace);
    proxy_wasm::set_root_context(|context_id| -> Box<dyn RootContext> {
        Box::new(SingletonService {
            context_id,
            config: ServiceConfig::default(),
            queue_id: None
        })
    });
}

struct SingletonService {
    context_id: u32,
    config: ServiceConfig,
    queue_id: Option<u32>
}

impl RootContext for SingletonService {

    fn on_vm_start(&mut self,_vm_configuration_size: usize) -> bool {
        if let Ok(q_id) = register_shared_queue(QUEUE_NAME) { self.queue_id = Some(q_id); }
        // TODO : handle MQ failure
        info!("Registered new message queue with id: {}", self.queue_id.unwrap());
        true
         
    }

    fn on_configure(&mut self, _config_size: usize) -> bool {
        //Check for the configuration passed by envoy.yaml
        let configuration: Vec<u8> = match self.get_configuration() {
            Some(c) => c,
            None => {
                warn!("Configuration missing. Please check the envoy.yaml file for filter configuration");
                return false;
            }
        };

        // Parse and store the configuration passed by envoy.yaml
        match serde_json::from_slice::<ServiceConfig>(configuration.as_ref()) {
            Ok(config) => {
                debug!("configuring {}: {:?}", self.context_id, config);
                self.config = config;
                return true;
            }
            Err(e) => {
                warn!("Failed to parse envoy.yaml configuration: {:?}", e);
                return false;
            }
        }
    }

    fn on_queue_ready(&mut self, _queue_id: u32) {
        match dequeue_shared_queue(self.queue_id.unwrap()) {
            Ok(threescale) => {
                info!("Consumed following message from the shared queue: {:?}", threescale)
            },
            Err(error) => info!("Error consuming message from the shared queue: {:?}", error)
        }
    }
}

impl Context for SingletonService {}

impl SingletonService {
    fn update_application_cache(&self, threescale: &ThreescaleData) {
        let cache_key = format!("{}_{}",threescale.app_id,threescale.service_id);
        match self.get_application_from_cache(&cache_key) {
            Some(application) => {
                let is_updated: bool = update_metrics(threescale, &mut application);
                if is_updated {
                    self.set_application_to_cache(&cache_key, &application);
                    // Handle when set_cache fail
                } else {
                    // Handle when update_metrics fail
                }
            },
            // Handle when no app in cache
            None => {}
        }
    }

    fn get_application_from_cache(&self,cache_key: &str) -> Option<Application> {
        match get_shared_data(cache_key) {
            Ok(data) => {
                match data.0 {
                    Some(app) => {
                        // Not safe. Handle deserialization properly.
                        let application : Application = bincode::deserialize(&app).unwrap();
                        return Some(application)
                    },
                    None => None
                }
            },
            Err(error) => {error!("Error reading application from cache {:?}", error)} 
        }
    }

    fn set_application_to_cache(&self,cache_key: &str, app: &Application) -> bool {
        match set_shared_data(cache_key, Some(&bincode::serialize(app).unwrap()), Some(0)) {
            Ok(ok) => true,
            Err(error) => false
        }
    }

    fn update_local_cache(&self) -> bool {
        return true
    }

    fn perform_periodical_flush(&self) -> bool {
        return true
    }
}
