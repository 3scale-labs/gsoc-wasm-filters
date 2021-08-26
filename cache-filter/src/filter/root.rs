use crate::configuration::FilterConfig;
use crate::filter::http::CacheFilter;
use crate::rand::thread_rng::{thread_rng_init_fallible, ThreadRng};
use crate::unique_callout::{WaiterAction, WAITING_CONTEXTS};
use crate::utils::request_process_failure;
use crate::{debug, info, warn};
use proxy_wasm::{
    hostcalls::set_effective_context,
    traits::{Context, HttpContext, RootContext},
    types::{ContextType, LogLevel},
};
use threescale::{
    proxy::{get_app_id_from_cache, get_application_from_cache, CacheKey},
    stats::*,
    structs::{AppIdentifier, ThreescaleData},
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
            stats: initialize_stats(),
            rng: ThreadRng,
            id: 0,
        })
    });
}

struct CacheFilterRoot {
    context_id: u32,
    config: FilterConfig,
    stats: ThreescaleStats,
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
        self.rng = match thread_rng_init_fallible(self, self.context_id) {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    self.context_id,
                    "FATAL: failed to initialize thread pseudo RNG: {}", e
                );
                panic!("failed to initialize thread pseudo RNG: {}", e);
            }
        };

        self.id = self.rng.next_u32();
        info!(self.context_id, "root initialized with id: {}", self.id);

        // Initializing a thread-specific message queue
        let queue_id = self.register_shared_queue(&self.id.to_string());
        info!(
            self.context_id,
            "root({}): registered thread-specific MQ ({})", self.id, queue_id
        );

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

    fn on_queue_ready(&mut self, queue_id: u32) {
        info!(
            self.context_id,
            "thread({}): on_queue called on the filter side", self.id
        );
        match self.dequeue_shared_queue(queue_id) {
            Ok(Some(bytes)) => {
                let message = match bincode::deserialize::<WaiterAction>(&bytes) {
                    Ok(res) => res,
                    Err(e) => {
                        warn!(
                            self.context_id,
                            "thread({}): unrecoverable err: deserializing failure: {}", self.id, e
                        );
                        return;
                    }
                };
                let context_to_resume: u32 = match message {
                    WaiterAction::HandleCacheHit(ctxt_id) => ctxt_id,
                    WaiterAction::HandleFailure(ctxt_id) => ctxt_id,
                };

                WAITING_CONTEXTS.with(|refcell| {
                    let mut waiters = refcell.borrow_mut();
                    let context = waiters.get_mut(&context_to_resume);
                    if context.is_none() {
                        warn!(
                            self.context_id,
                            "thread({}): unrecoverable err: http context({}) not found while resuming after callout response",
                            self.id,
                            context_to_resume
                        );
                        return;
                    }

                    let context = context.unwrap();

                    if let Err(e) = set_effective_context(context_to_resume) {
                        // NOTE: Ideally this should *not* happen.
                        warn!(
                            context_to_resume,
                            "thread({}): unrecoverable err: failed to set effective context in the host: {:?}", self.id, e
                        );
                        waiters.remove(&context_to_resume);
                        return;
                    }

                    if let WaiterAction::HandleFailure(_) = message {
                        // This can happen either there was no response from 3scale (e.g. timeout) or handling
                        // response failed (e.g. parsing).
                        info!(context_to_resume, "thread({}): handling auth callout failure for this waiting context", self.id);
                        request_process_failure(context, context);
                        waiters.remove(&context_to_resume);
                        return;
                    }

                    // Waiting contexts can have cache_key with user_key pattern but cache stores
                    // application only with app_id pattern so change if required before accessing it.
                    if let AppIdentifier::UserKey(ref user_key) = context.cache_key.app_id() {
                        match get_app_id_from_cache(user_key) {
                            Ok(app_id) => {
                                context.cache_key.set_app_id(&AppIdentifier::from(app_id))
                            }
                            Err(e) => {
                                // This is unlikely since mapping is defined when auth response is handled properly.
                                // Or someone made a flow error above, allowing this instruction to execute.
                                warn!(
                                    self.context_id,
                                    "failed to map user_key to app_id cache key pattern: {:?}", e
                                );
                                waiters.remove(&context_to_resume);
                                return;
                            }
                        }
                    }

                    match get_application_from_cache(&context.cache_key) {
                        Ok((mut app, cas)) => {
                            if let Err(e) = context.handle_cache_hit(&mut app, cas) {
                                debug!(context_to_resume, "handle_cache_hit fail: {}", e);
                                // if there is error from handle_cache_hit, request flow is not changed
                                // and should be done by the code handling the returned error.
                                request_process_failure(context, context);
                            } else {
                                context.resume_http_request();
                            }
                        }
                        Err(e) => warn!(
                            context_to_resume,
                            "failed to fetch application from cache: {:?}", e
                        ),
                    }
                    waiters.remove(&context_to_resume);
                })
            }
            Ok(None) => warn!(
                self.context_id,
                "thread({}): on_queue called but found nothing in the MQ", self.id
            ),
            Err(e) => warn!(
                self.context_id,
                "thread({}): failed to dequeue from thread-specific MQ: {:?}", self.id, e
            ),
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
            stats: self.stats.clone(),
        }))
    }

    fn get_type(&self) -> Option<ContextType> {
        Some(ContextType::HttpContext)
    }
}

impl Context for CacheFilterRoot {}
