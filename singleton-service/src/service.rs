pub use crate::configuration::service::ServiceConfig;
use log::{debug, warn};
use proxy_wasm::{
    traits::{Context, RootContext},
    types::LogLevel,
};

#[no_mangle]
pub fn _start() {
    proxy_wasm::set_log_level(LogLevel::Trace);
    proxy_wasm::set_root_context(|context_id| -> Box<dyn RootContext> {
        Box::new(SingletonService {
            context_id,
            config: ServiceConfig::default(),
        })
    });
}

struct SingletonService {
    context_id: u32,
    config: ServiceConfig,
}

impl RootContext for SingletonService {
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
}

impl Context for SingletonService {}
