use crate::configuration::FilterConfig;
use crate::filter::http::CacheFilter;
use crate::{debug, info, warn};
use proxy_wasm::{
    traits::{Context, HttpContext, RootContext},
    types::{ContextType, LogLevel},
};
use threescale::{proxy::CacheKey, structs::ThreescaleData};

#[no_mangle]
pub fn _start() {
    std::panic::set_hook(Box::new(|panic_info| {
        proxy_wasm::hostcalls::log(LogLevel::Critical, &panic_info.to_string()).unwrap();
    }));
    proxy_wasm::set_root_context(|context_id| -> Box<dyn RootContext> {
        Box::new(CacheFilterRoot {
            context_id: (context_id as usize),
            config: FilterConfig::default(),
        })
    });
}

struct CacheFilterRoot {
    context_id: usize,
    config: FilterConfig,
}

impl RootContext for CacheFilterRoot {
    fn on_vm_start(&mut self, _vm_configuration_size: usize) -> bool {
        info!(context: self.context_id, "VM started");
        true
    }

    fn on_configure(&mut self, _config_size: usize) -> bool {
        //Check for the configuration passed by envoy.yaml
        let configuration: Vec<u8> = match self.get_configuration() {
            Some(c) => c,
            None => {
                warn!(
                    context: self.context_id,
                    "Configuration missing. Please check the envoy.yaml file for filter configuration"
                );
                return true;
            }
        };

        // Parse and store the configuration passed by envoy.yaml
        match serde_json::from_slice::<FilterConfig>(configuration.as_ref()) {
            Ok(config) => {
                debug!(context: self.context_id, "configuring with: {:?}", config);
                self.config = config;
                true
            }
            Err(e) => {
                warn!(context: self.context_id, "Failed to parse envoy.yaml configuration: {:?}", e);
                true
            }
        }
    }

    fn create_http_context(&self, context: u32) -> Option<Box<dyn HttpContext>> {
        Some(Box::new(CacheFilter {
            context_id: context as usize,
            config: self.config.clone(),
            update_cache_from_singleton: false,
            cache_key: CacheKey::default(),
            req_data: ThreescaleData::default(),
        }))
    }

    fn get_type(&self) -> Option<ContextType> {
        Some(ContextType::HttpContext)
    }
}

impl Context for CacheFilterRoot {}
