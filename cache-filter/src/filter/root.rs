use crate::configuration::FilterConfig;
use crate::filter::http::CacheFilter;
use crate::rand::thread_rng::{thread_rng_init_fallible, ThreadRng};
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
            context_id,
            config: FilterConfig::default(),
            rng: ThreadRng,
        })
    });
}

struct CacheFilterRoot {
    context_id: u32,
    config: FilterConfig,
    rng: ThreadRng,
    id: u32,
}

impl RootContext for CacheFilterRoot {
    fn on_vm_start(&mut self, _vm_configuration_size: usize) -> bool {
        info!(self.context_id, "VM started");
        true
    }

    fn on_configure(&mut self, _config_size: usize) -> bool {
        // Initialize the PRNG for this thread in the root context
        // This only needs to happen once per thread. Since we are
        // single-threaded, this means it just needs to happen once.
        self.rng = match thread_rng_init_fallible(self, context_id) {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    context_id,
                    "FATAL: failed to initialize thread pseudo RNG: {}", e
                );
                panic!("failed to initialize thread pseudo RNG: {}", e);
            }
        };

        self.id = self.rng.next_u32();
        info!(self.context_id, "root initialized with id: {}", self.id);

        //Check for the configuration passed by envoy.yaml
        let configuration: Vec<u8> = match self.get_configuration() {
            Some(c) => c,
            None => {
                warn!(
                    self.context_id,
                    "Configuration missing. Please check the envoy.yaml file for filter configuration"
                );
                return true;
            }
        };

        // Parse and store the configuration passed by envoy.yaml
        match serde_json::from_slice::<FilterConfig>(configuration.as_ref()) {
            Ok(config) => {
                debug!(self.context_id, "configuring with: {:?}", config);
                self.config = config;
                true
            }
            Err(e) => {
                warn!(
                    self.context_id,
                    "Failed to parse envoy.yaml configuration: {:?}", e
                );
                true
            }
        }
    }

    fn create_http_context(&self, context: u32) -> Option<Box<dyn HttpContext>> {
        Some(Box::new(CacheFilter {
            context_id: context,
            root_id: self.id,
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
