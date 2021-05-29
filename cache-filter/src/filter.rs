use crate::{
    configuration::filter::FilterConfig,
    utils::{
        get_request_data,
        handle_cache_miss,
        is_rate_limited,
        report_to_singleton,
    }
};
use threescale::utils::get_application_from_cache;
use log::{debug, info, warn};
use proxy_wasm::{
    traits::{Context, HttpContext, RootContext},
    types::{ContextType, LogLevel, Action},
};

const QUEUE_NAME: &str = "message_queue"; 
const VM_ID: &str = "my_vm_id";

#[no_mangle]
pub fn _start() {
    proxy_wasm::set_log_level(LogLevel::Trace);
    proxy_wasm::set_root_context(|context_id| -> Box<dyn RootContext> {
        Box::new(CacheFilterRoot {
            context_id,
            config: FilterConfig::default(),
        })
    });
}

struct CacheFilterRoot {
    context_id: u32,
    config: FilterConfig,
}

impl RootContext for CacheFilterRoot {
    fn on_vm_start(&mut self, _vm_configuration_size: usize) -> bool {
        info!("VM started");
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
        match serde_json::from_slice::<FilterConfig>(configuration.as_ref()) {
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

    fn create_http_context(&self, _context_id: u32) -> Option<Box<dyn HttpContext>> {
        Some(Box::new(CacheFilter {
            config: self.config.clone(),
        }))
    }

    fn get_type(&self) -> Option<ContextType> {
        Some(ContextType::HttpContext)
    }
}

impl Context for CacheFilterRoot {}

#[allow(dead_code)]
struct CacheFilter {
    config: FilterConfig,
}

impl HttpContext for CacheFilter {
    fn on_http_request_headers(&mut self, context_id: usize) -> Action {
        let current_time = self.get_current_time();
        let queue_id = self.resolve_shared_queue(VM_ID, QUEUE_NAME);
        let request_data = match get_request_data() {
            Some(data) => data,
            None => {
                info!("ctxt {}: Releveant request data not recieved from previous filter", context_id);
                // Send back local response for not providing relevant request data
                self.send_http_response(401,vec![],None);
                return Action::Pause;
            }
        };

        let key = format!("{}_{}",request_data.app_id,request_data.service_id);
        match get_application_from_cache(key.as_str()) {
            Some((mut app,_)) =>
            {
                info!("ctxt {}: Cache Hit",context_id);
                if is_rate_limited(&request_data, &mut app, &current_time)
                {
                    info!("ctxt {}: Request is rate-limited",context_id);
                    // Add some identifier for rate-limit filter
                } 
                else 
                {
                    info!("ctxt {}: Request is allowed to pass",context_id);
                    if report_to_singleton(queue_id,&request_data)
                    {
                        // Handle MQ failure here
                    }
                }
                return Action::Continue;
            },

            None => {
                info!("Cache Miss");
                handle_cache_miss(&request_data);
                return Action::Pause;
            }
        }
    }
}

impl Context for CacheFilter {}

impl CacheFilter {}
