use crate::configuration::FilterConfig;
use crate::filter::http::CacheFilter;
use crate::utils::request_process_failure;
use crate::{debug, info, warn};
use proxy_wasm::{
    hostcalls::{get_shared_data, resume_http_request, set_effective_context, set_tick_period},
    traits::{Context, HttpContext, RootContext},
    types::{ContextType, LogLevel},
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;
use threescale::{
    proxy::{get_application_from_cache, CacheKey},
    structs::ThreescaleData,
};

#[no_mangle]
pub fn _start() {
    std::panic::set_hook(Box::new(|panic_info| {
        proxy_wasm::hostcalls::log(LogLevel::Critical, &panic_info.to_string()).unwrap();
    }));
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

thread_local! {
    pub static WAITING_CONTEXTS: RefCell<HashMap<String, CacheFilter>> = RefCell::new(HashMap::new());
}

impl RootContext for CacheFilterRoot {
    fn on_vm_start(&mut self, _vm_configuration_size: usize) -> bool {
        info!(self.context_id, "VM started");
        true
    }

    fn on_tick(&mut self) {
        info!(self.context_id, "on_tick called");

        WAITING_CONTEXTS.with(|waiters| {
            waiters.borrow_mut().retain(|callout_key, filter| {
                info!(
                    filter.context_id,
                    "checking callout response for request(key: {})", callout_key
                );
                match get_shared_data(callout_key) {
                    Ok((Some(_), _)) => true, // still waiting for the response.
                    Ok((None, _)) => {
                        match get_application_from_cache(&filter.cache_key) {
                            Ok((mut app, cas)) => {
                                if let Err(e) = set_effective_context(filter.context_id) {
                                    // NOTE: Ideally this should not happen.
                                    warn!(
                                        filter.context_id,
                                        "failed to set effective context in the host: {:?}", e
                                    );
                                } else if let Err(e) = filter.handle_cache_hit(&mut app, cas) {
                                    debug!(filter.context_id, "handle_cache_hit fail: {}", e);
                                    // if there is error from handle_cache_hit, request flow is not changed
                                    // and should be done by code handling the returned error.
                                    request_process_failure(filter);
                                } else {
                                    resume_http_request().unwrap();
                                }
                            },
                            Err(e) => {
                                warn!(filter.context_id, "callout-lock removed but failed to fetch app from shared data: {}", e);
                                request_process_failure(filter);
                            }
                        }
                        false
                    },
                    Err(e) => {
                        warn!(
                            filter.context_id,
                            "failed to find callout-lock in the shared data for {} : {:?}",
                            callout_key,
                            e
                        );

                        if let Err(e) = set_effective_context(filter.context_id) {
                            warn!(
                                filter.context_id,
                                "failed to set effective context in the host: {:?}", e
                            );
                        } else {
                            request_process_failure(filter);
                        }
                        false
                    }
                }
            });

            if waiters.borrow().is_empty() {
                set_tick_period(Duration::from_millis(0)).unwrap();
            }
        })
    }

    fn on_configure(&mut self, _config_size: usize) -> bool {
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
