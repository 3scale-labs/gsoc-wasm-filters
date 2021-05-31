use crate::{
    configuration::FilterConfig,
    utils::{get_request_data, handle_cache_miss},
};
use log::info;
use proxy_wasm::{
    traits::{Context, HttpContext},
    types::Action,
};
use std::time::SystemTime;
use threescale::{
    structs::{Application, Message, ThreescaleData},
    utils::{get_application_from_cache, set_application_to_cache},
};

const QUEUE_NAME: &str = "message_queue";
const VM_ID: &str = "my_vm_id";

pub struct CacheFilter {
    pub context_id: u32,
    pub config: FilterConfig,
    pub update_cache_from_singleton: bool,
    pub cache_key: String,
}

impl HttpContext for CacheFilter {
    fn on_http_request_headers(&mut self, context_id: usize) -> Action {
        let current_time = self.get_current_time();
        let queue_id = self.resolve_shared_queue(VM_ID, QUEUE_NAME);

        let request_data = match get_request_data() {
            Some(data) => data,
            None => {
                info!(
                    "ctxt {}: Releveant request data not recieved from previous filter",
                    context_id
                );
                // Send back local response for not providing relevant request data
                self.send_http_response(401, vec![], None);
                return Action::Pause;
            }
        };

        self.cache_key = format!("{}_{}", request_data.app_id, request_data.service_id);
        match get_application_from_cache(self.cache_key.as_str()) {
            Some((mut app, _)) => {
                info!("ctxt {}: Cache Hit", context_id);
                if self.is_rate_limited(&request_data, &mut app, &current_time) {
                    info!("ctxt {}: Request is rate-limited", context_id);
                    // Add some identifier for rate-limit filter
                } else {
                    info!("ctxt {}: Request is allowed to pass", context_id);
                    if self.report_to_singleton(queue_id, &request_data) {
                        // Handle MQ failure here
                    }
                }
                return Action::Continue;
            }

            None => {
                info!("ctxt {}: Cache Miss", context_id);
                handle_cache_miss(&request_data);
                return Action::Pause;
            }
        }
    }
}

impl Context for CacheFilter {}

impl CacheFilter {
    fn report_to_singleton(&self, queue_id: Option<u32>, request_data: &ThreescaleData) -> bool {
        let message: Message = Message::new(self.update_cache_from_singleton, request_data);
        match queue_id {
            Some(qid) => {
                if let Err(_) =
                    self.enqueue_shared_queue(qid, Some(&bincode::serialize(&message).unwrap()))
                {
                    info!(
                        "ctxt {}: Reporting to singleton failed: MQ with specified id not found",
                        self.context_id
                    );
                    return true;
                }
            }
            None => {
                info!(
                    "ctxt {}: Reporting to singleton failed: Queue id not provided",
                    self.context_id
                );
                return true;
            }
        }
        false
    }

    fn is_rate_limited(
        &mut self,
        request_data: &ThreescaleData,
        app: &mut Application,
        _current_time: &SystemTime,
    ) -> bool {
        for (metric, hits) in request_data.metrics.borrow().iter() {
            // Check metric is present inside local cache
            if !app.local_state.borrow().contains_key(metric) {
                continue;
            }

            /* Check period window expiration
            if app.local_state.borrow().get(metric).unwrap().period_window.end < current_time {
                // Period window is expired
                // Update period window using get_new_window for each metric
                // But first confirm how to deal with time sync between host and 3scale
                // Reset left_hits to max value
            }*/

            // If any metric is rate-limited then whole request is restricted
            if app.local_state.borrow().get(metric).unwrap().left_hits - hits < 0 {
                true;
            }
        }

        if !set_application_to_cache(self.cache_key.as_str(), app, true, None) {
            self.update_cache_from_singleton = true;
        }
        false
    }
}
