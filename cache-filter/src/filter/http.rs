use crate::{
    configuration::FilterConfig,
    utils::{do_auth_call, get_request_data, period_from_response, request_process_failure},
};
use log::{debug, info};
use proxy_wasm::{
    traits::{Context, HttpContext},
    types::Action,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::TryInto;
use std::str::FromStr;
use std::time::{Duration, UNIX_EPOCH};
use threescale::{
    proxy::cache::{get_application_from_cache, set_application_to_cache},
    structs::*,
};
use threescalers::response::{Authorization, UsageReports};

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
            // TODO: Add how many similar requests can be accepted.
            self.send_http_response(429, vec![], Some(b"Request rate-limited.\n"));
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

    fn is_rate_limited(&mut self, app: &RefCell<Application>, current_time: &Duration) -> bool {
        for (metric, hits) in self.req_data.metrics.borrow().iter() {
            if let Some(usage_report) = app.borrow_mut().local_state.get_mut(metric) {
                let mut period = &mut usage_report.period_window;
                if period.window != Period::Eternity {
                    // Taking care of period window expiration
                    while period.end < *current_time {
                        // Get new window and reset left_hits to max value
                        let secs_to_add: u64 = period.window.clone() as u64;
                        period.start = period
                            .start
                            .checked_add(Duration::new(secs_to_add, 0))
                            .unwrap();
                        period.end = period
                            .end
                            .checked_add(Duration::new(secs_to_add, 0))
                            .unwrap();
                        usage_report.left_hits = usage_report.max_value;
                    }

                    if usage_report.left_hits < *hits {
                        return true;
                    }
                    usage_report.left_hits -= *hits;
                }
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
                    Ok(Authorization::Ok(response)) => {
                        // Form application struct from the response
                        let key_split = self.cache_key.split('_').collect::<Vec<_>>();
                        let mut state = HashMap::new();
                        let UsageReports::UsageReports(reports) = response.usage_reports();
                        for usage in reports {
                            state.insert(
                                usage.metric.clone(),
                                UsageReport {
                                    period_window: PeriodWindow {
                                        // i64 to u64 but value will always be positive
                                        start: Duration::from_secs(
                                            usage.period_start.0.try_into().unwrap(),
                                        ),
                                        end: Duration::from_secs(
                                            usage.period_end.0.try_into().unwrap(),
                                        ),
                                        window: period_from_response(&usage.period),
                                    },
                                    left_hits: usage.current_value,
                                    max_value: usage.max_value,
                                },
                            );
                        }
                        let mut hierarchy = HashMap::new();
                        if let Some(metrics) = response.hierarchy() {
                            for (parent, childrens) in metrics.iter() {
                                hierarchy.insert(parent.clone(), childrens.clone());
                            }
                        }
                        let app = Application {
                            app_id: key_split[0].to_string(),
                            service_id: key_split[1].to_string(),
                            local_state: state,
                            metric_hierarchy: hierarchy,
                        };
                        set_application_to_cache(&self.cache_key, &app, true, None);
                    }
                    Ok(Authorization::Denied(denied_auth)) => {
                        info!("Authorization was denied with code: {}", denied_auth.code());
                        request_process_failure(self, self);
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
