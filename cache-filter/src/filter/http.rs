use crate::{
    configuration::FilterConfig,
    utils::{do_auth_call, get_request_data, request_process_failure},
};
use log::{debug, info};
use proxy_wasm::{
    traits::{Context, HttpContext},
    types::Action,
};
use std::cell::RefCell;
use std::str::FromStr;
use std::time::{Duration, UNIX_EPOCH};
use threescale::{
    proxy::cache::{get_application_from_cache, set_application_to_cache},
    structs::{Application, Message, ThreescaleData},
};
use threescalers::response::Authorization;

const QUEUE_NAME: &str = "message_queue";
const VM_ID: &str = "my_vm_id";

pub struct CacheFilter {
    pub context_id: u32,
    pub config: FilterConfig,
    pub update_cache_from_singleton: bool,
    pub cache_key: String,
    // This is required for cache miss case
    pub req_data: ThreescaleData,
}

impl HttpContext for CacheFilter {
    fn on_http_request_headers(&mut self, context_id: usize) -> Action {
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
            Some((app, _)) => {
                info!("ctxt {}: Cache Hit", context_id);
                let app_ref = RefCell::new(app);
                if !self.handle_cache_hit(&app_ref) {
                    return Action::Pause;
                }
                Action::Continue
            }
            None => {
                info!("ctxt {}: Cache Miss", context_id);
                // TODO: Avoid multiple calls for same application
                // Saving request data to use when there is response from 3scale
                self.req_data = request_data.clone();
                // Fetching new application state using authorize endpoint
                do_auth_call(self, self, &request_data)
            }
        }
    }
}

impl CacheFilter {
    fn report_to_singleton(&self, queue_id: Option<u32>) -> bool {
        let message: Message = Message::new(self.update_cache_from_singleton, &self.req_data);
        match queue_id {
            Some(qid) => {
                if self
                    .enqueue_shared_queue(qid, Some(&bincode::serialize(&message).unwrap()))
                    .is_err()
                {
                    info!(
                        "ctxt {}: Reporting to singleton failed: MQ with specified id not found",
                        self.context_id
                    );
                    return false;
                }
            }
            None => {
                info!(
                    "ctxt {}: Reporting to singleton failed: Queue id not provided",
                    self.context_id
                );
                return false;
            }
        }
        true
    }

    fn handle_cache_hit(&mut self, app: &RefCell<Application>) -> bool {
        let queue_id = self.resolve_shared_queue(VM_ID, QUEUE_NAME);
        let current_time = match self.get_current_time().duration_since(UNIX_EPOCH) {
            Ok(time) => time,
            Err(e) => {
                debug!("Failed to get time from host due to: {}", e);
                return false;
            }
        };
        if self.is_rate_limited(app, &current_time) {
            info!("ctxt {}: Request is rate-limited", self.context_id);
            // TODO: Add some identifier for rate-limit filter
        } else {
            info!("ctxt {}: Request is allowed to pass", self.context_id);
            if !self.report_to_singleton(queue_id) {
                // TODO: Handle MQ failure here
                // Update local cache
                // Report to 3scale and get new state using authrep endpoint
            }
        }
        true
    }

    fn is_rate_limited(&mut self, app: &RefCell<Application>, _current_time: &Duration) -> bool {
        for (metric, hits) in self.req_data.metrics.borrow().iter() {
            // Check metric is present inside local cache
            if !app.borrow().local_state.borrow().contains_key(metric) {
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
            if ((app
                .borrow()
                .local_state
                .borrow()
                .get(metric)
                .unwrap()
                .left_hits
                - hits) as i32)
                < 0
            {
                return true;
            }
        }

        if !set_application_to_cache(self.cache_key.as_str(), &app.borrow(), true, None) {
            self.update_cache_from_singleton = true;
        }
        false
    }
}

impl Context for CacheFilter {
    fn on_http_call_response(&mut self, token_id: u32, _: usize, body_size: usize, _: usize) {
        info!(
            "ctxt {}: Recieved response from 3scale: token: {}",
            self.context_id, token_id
        );
        match self.get_http_call_response_body(0, body_size) {
            Some(bytes) => {
                match Authorization::from_str(std::str::from_utf8(&bytes).unwrap()) {
                    Ok(_response) => {
                        // Handle cache hit here
                    }
                    Err(e) => {
                        info!(
                            "Parsing response from 3scale failed due to: {} with token: {}",
                            e, token_id
                        );
                        request_process_failure(self, self);
                    }
                }
                info!("Data recived from callout with token :{}", token_id);
            }
            None => {
                info!("Found nothing in the response with token: {}", token_id);
                request_process_failure(self, self);
            }
        }
    }
}
