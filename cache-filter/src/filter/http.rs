use crate::{
    configuration::FilterConfig,
    utils::{do_auth_call, in_request_failure, request_process_failure},
};
use crate::{debug, info, warn};
use proxy_wasm::{
    hostcalls::{get_shared_data, set_shared_data, set_tick_period},
    traits::{Context, HttpContext},
    types::{Action, Status},
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::str::FromStr;
use std::time::{Duration, UNIX_EPOCH};
use std::vec;
use threescale::{
    proxy::{
        get_app_id_from_cache, get_application_from_cache, set_app_id_to_cache, CacheError,
        CacheKey,
    },
    structs::*,
    upstream::*,
    utils::*,
};
use threescalers::response::{Authorization, AuthorizationStatus};

const QUEUE_NAME: &str = "message_queue";
const VM_ID: &str = "my_vm_id";
const DEFAULT_TICK_PERIOD_MILLIS: u64 = 200;

#[derive(Debug, thiserror::Error)]
pub enum CacheHitError {
    #[error("duration since time later than self")]
    TimeConversionErr(#[from] std::time::SystemTimeError),
    #[error("failure of is_rate_limited error")]
    MetricsCheckFail(#[from] UpdateMetricsError),
    #[error("unable to resolve message queue with specified vm_id and q_name")]
    MQNotFound,
    #[error("failed to fetch the app during app_set retries")]
    AppFetchFail(#[from] CacheError),
}

#[derive(Debug, thiserror::Error)]
enum AuthResponseError {
    #[error("failure to follow cache hit flow")]
    CacheHitErr(#[from] CacheHitError),
    #[error("conversion from i64 time to u64 duration failed")]
    NegativeTimeErr,
    #[error("list app keys from 3scale auth response are missing")]
    ListKeysMiss,
    #[error("app id field is missing from list keys extension response")]
    ListAppIdMiss,
    #[error("app id field is missing from list keys extension response")]
    ListServiceIdMiss,
    #[error("failed to map user_key to app_id in the cache")]
    AppIdNotMapped(#[from] CacheError),
}

#[derive(Debug, thiserror::Error)]
pub enum RequestDataError {
    #[error("service_token not found inside request metadata")]
    ServiceTokenNotFound,
    #[error("service_id not found inside request metadata")]
    ServiceIdNotFound,
    #[error("usage data not found inside request metadata")]
    UsageNotFound,
    #[error("deserializing usage data failed")]
    DeserializeFail(#[from] serde_json::Error),
    #[error("no authentication pattern provided in the request metadata")]
    AuthKeyMissing,
    #[error("cluster name not found inside request metadata")]
    ClusterNameNotFound,
    #[error("upstream url not found inside request metadata")]
    UpstreamUrlNotFound,
    #[error("failed to parse upstream url")]
    UpstreamUrlParseFail(#[from] url::ParseError),
    #[error("failed to initialize upstream builder")]
    UpstreamBuilderFail(#[from] threescalers::Error),
}

#[derive(Clone)]
pub struct CacheFilter {
    pub context_id: u32,
    pub config: FilterConfig,
    pub update_cache_from_singleton: bool,
    pub cache_key: CacheKey,
    // required for cache miss case
    pub req_data: ThreescaleData,
}

impl HttpContext for CacheFilter {
    fn on_http_request_headers(&mut self, _: usize) -> Action {
        let mut request_data = match self.get_request_data() {
            Ok(data) => data,
            Err(e) => {
                debug!(self.context_id, "fetching request data failed: {}", e);

                // Send back local response for not providing relevant request data
                if cfg!(feature = "visible_logs") {
                    let (key, val) =
                        crate::log::visible_logs::get_logs_header_pair(self.context_id);
                    self.send_http_response(401, vec![(key.as_ref(), val.as_ref())], None);
                } else {
                    self.send_http_response(401, vec![], None);
                }

                return Action::Pause;
            }
        };

        self.cache_key = CacheKey::from(&request_data.service_id, &request_data.app_id);
        self.req_data = request_data.clone();

        if let AppIdentifier::UserKey(ref user_key) = request_data.app_id {
            match get_app_id_from_cache(user_key) {
                Ok(app_id) => {
                    request_data.app_id = AppIdentifier::from(app_id);
                    self.req_data.app_id = request_data.app_id.clone();
                    self.cache_key.set_app_id(&request_data.app_id);
                }
                Err(e) => {
                    debug!(
                        self.context_id,
                        "user_key->app_id mapping not found! considering cache miss: {:?}", e
                    );
                    match self.set_callout_lock(&self.cache_key) {
                        Ok(true) => return do_auth_call(self, self, &request_data),
                        Ok(false) => {
                            self.add_context_to_waitlist();
                            return Action::Pause;
                        }
                        Err(e) => {
                            warn!(
                                self.context_id,
                                "failed to set callout-lock for request(key: {}): {:?}",
                                self.cache_key.as_string(),
                                e
                            );
                            return in_request_failure(self);
                        }
                    }
                }
            }
        }

        match get_application_from_cache(&self.cache_key) {
            Ok((mut app, cas)) => {
                info!(self.context_id, "cache hit");

                match self.handle_cache_hit(&mut app, cas) {
                    Ok(()) => Action::Continue,
                    Err(e) => {
                        warn!(self.context_id, "cache hit flow failed: {}", e);
                        in_request_failure(self)
                    }
                }
            }
            Err(e) => {
                info!(self.context_id, "cache miss: {}", e);
                match self.set_callout_lock(&self.cache_key) {
                    Ok(true) => do_auth_call(self, self, &request_data),
                    Ok(false) => {
                        self.add_context_to_waitlist();
                        Action::Pause
                    }
                    Err(e) => {
                        warn!(
                            self.context_id,
                            "failed to set callout-lock for request(key: {}): {:?}",
                            self.cache_key.as_string(),
                            e
                        );
                        in_request_failure(self)
                    }
                }
            }
        }
    }

    #[cfg(feature = "visible_logs")]
    fn on_http_response_headers(&mut self, _: usize) -> Action {
        let (key, val) = crate::log::visible_logs::get_logs_header_pair(self.context_id);
        self.add_http_response_header(key.as_ref(), val.as_ref());
        Action::Continue
    }
}

impl CacheFilter {
    fn report_to_singleton(&self, qid: u32, req_time: &Duration) -> bool {
        let message: Message =
            Message::new(self.update_cache_from_singleton, &self.req_data, req_time);
        if let Err(e) = self.enqueue_shared_queue(qid, Some(&bincode::serialize(&message).unwrap()))
        {
            warn!(
                self.context_id,
                "enqueuing to message queue failed: {:?}", e
            );
            return false;
        }
        true
    }

    fn add_context_to_waitlist(&self) {
        let callout_key = format!("callout_{}", self.cache_key.as_string());
        crate::filter::root::WAITING_CONTEXTS.with(|waiters| {
            if waiters
                .borrow_mut()
                .insert(callout_key.clone(), self.clone())
                .is_some()
            {
                // should not be possible but just in case.
                warn!(
                    self.context_id,
                    "already added a similar callout(key: {})", callout_key
                );
                self.send_http_response(500, vec![], Some(b"Internal Failure"));
                return;
            }

            info!(
                self.context_id,
                "successfully added context to the callout wait-list",
            );
        });
        if let Err(e) = set_tick_period(Duration::from_millis(DEFAULT_TICK_PERIOD_MILLIS)) {
            warn!(self.context_id, "failed to set tick period: {:?}", e);
            // error due to internal problem.
            self.send_http_response(500, vec![], Some(b"Internal Failure"));

            // remove previously added context since it's now just consuming memory.
            crate::filter::root::WAITING_CONTEXTS.with(|waiters| {
                waiters.borrow_mut().remove(&callout_key);
            })
        }
    }

    // app_cas can be zero if fetched for the first from 3scale
    pub fn handle_cache_hit(
        &mut self,
        app: &mut Application,
        mut app_cas: u32,
    ) -> Result<(), CacheHitError> {
        let queue_id = self
            .resolve_shared_queue(VM_ID, QUEUE_NAME)
            .ok_or(CacheHitError::MQNotFound)?;

        let current_time = self.get_current_time().duration_since(UNIX_EPOCH)?;
        let mut updated_cache = false;
        let mut rate_limited = false;
        let max_tries = self.config.max_tries;

        add_hierarchy_to_metrics(&app.metric_hierarchy, &mut self.req_data.metrics);

        // In case of CAS mismatch, new application needs to be fetched and modified again.
        for num_try in 0..max_tries {
            match limit_check_and_update_application(&self.req_data, app, app_cas, &current_time) {
                Ok(()) => {
                    // App is not rate-limited and updated in cache.
                    info!(self.context_id, "request is allowed to pass the filter");
                    if !self.report_to_singleton(queue_id, &current_time) {
                        // TODO: Handle MQ failure here
                        // Update local cache
                        // Report to 3scale and get new state using authrep endpoint
                    }
                    updated_cache = true;
                    self.resume_http_request();
                    break;
                }
                Err(UpdateMetricsError::RateLimited) => {
                    info!(self.context_id, "request is rate-limited");
                    self.send_http_response(429, vec![], Some(b"Request rate-limited.\n"));
                    // no need to retry if already rate-limted
                    rate_limited = true;
                    break;
                }
                Err(UpdateMetricsError::CacheUpdateFail(reason)) => {
                    info!(
                        self.context_id,
                        "try ({} out of {}): failed to set application to cache: {}",
                        (num_try as u64) + 1,
                        max_tries,
                        reason
                    );
                    if num_try < max_tries {
                        match get_application_from_cache(&self.cache_key) {
                            Ok((new_app, cas)) => {
                                *app = new_app;
                                app_cas = cas;
                            }
                            Err(e) => return Err(CacheHitError::AppFetchFail(e)),
                        }
                        continue;
                    }
                }
                Err(e) => return Err(CacheHitError::MetricsCheckFail(e)),
            }
        }
        // App is not rate-limited and changes are not reflected in the cache yet.
        // Singleton will try to overwrite and in the mean time till singleton receives
        // the message, hopefully, contention will reduce.
        if !updated_cache && !rate_limited {
            self.update_cache_from_singleton = true;
            self.resume_http_request();
        }
        Ok(())
    }

    // Callout lock is acquired by placing a key-value pair inside shared data.
    // Since, only one thread is allowed to access shared data (host uses mutex) thus only one winner;
    fn set_callout_lock(&self, cache_key: &CacheKey) -> Result<bool, Status> {
        let request_key = format!("callout_{}", cache_key.as_string());

        // check if lock is already acquired or not
        match get_shared_data(&request_key)? {
            (None, cas) => {
                // we can also add thread id as value for better debugging.
                match set_shared_data(&request_key, Some(b"lock"), cas) {
                    Ok(()) => Ok(true),                    // lock acquired
                    Err(Status::CasMismatch) => Ok(false), // someone acquired it first
                    Err(e) => Err(e),
                }
            }
            (_, _) => {
                info!(
                    self.context_id,
                    "callout lock for request(key: {}) already acquired by another thread",
                    request_key
                );
                Ok(false)
            }
        }
    }

    // NOTE: Right now, there is no option of deleting the pair instead only the value can be erased,
    // and it requires changes in the ABI so change this because it will lead to better memory usage.
    fn free_callout_lock(&self, cache_key: &CacheKey) -> Result<(), Status> {
        let request_key = format!("callout_{}", cache_key.as_string());

        if let Err(e) = set_shared_data(&request_key, None, None) {
            warn!(
                self.context_id,
                "failed to delete the callout-lock from shared data: {}: {:?}", request_key, e
            );
            return Err(e);
        }
        Ok(())
    }

    fn handle_auth_response(
        &mut self,
        response: &AuthorizationStatus,
    ) -> Result<(), AuthResponseError> {
        // Form application struct from the response
        let mut state = HashMap::new();
        let app_keys = response.app_keys().ok_or(AuthResponseError::ListKeysMiss)?;
        let app_id = AppId::from(
            app_keys
                .app_id()
                .ok_or(AuthResponseError::ListAppIdMiss)?
                .as_ref(),
        );
        let app_identifier = AppIdentifier::from(app_id.clone());
        let service_id = ServiceId::from(
            app_keys
                .service_id()
                .ok_or(AuthResponseError::ListServiceIdMiss)?
                .as_ref(),
        );

        // change user_key to app_id for further processing
        let mut update_cache_key = false;
        if let AppIdentifier::UserKey(user_key) = self.cache_key.app_id() {
            self.req_data.app_id = app_identifier.clone();
            set_app_id_to_cache(user_key, &app_id)?;
            update_cache_key = true;
        }

        let callout_key = format!("callout_{}", self.cache_key.as_string());
        if update_cache_key {
            let new_cache_key = CacheKey::from(&service_id, &app_identifier);
            // required because on_tick should target correct cache_key.
            crate::filter::root::WAITING_CONTEXTS.with(|waiters| {
                waiters
                    .borrow_mut()
                    .entry(callout_key)
                    .and_modify(|filter| (*filter).cache_key = new_cache_key.clone());

                if let Err(e) = self.free_callout_lock(&self.cache_key) {
                    warn!(
                        self.context_id,
                        "failed to delete callout-lock when handling auth resp: {:?}", e
                    )
                }
                self.cache_key = new_cache_key;
            });
        }

        if let Some(reports) = response.usage_reports() {
            for usage in reports {
                state.insert(
                    usage.metric.clone(),
                    UsageReport {
                        period_window: PeriodWindow {
                            start: Duration::from_secs(
                                usage
                                    .period_start
                                    .0
                                    .try_into()
                                    .or(Err(AuthResponseError::NegativeTimeErr))?,
                            ),
                            end: Duration::from_secs(
                                usage
                                    .period_end
                                    .0
                                    .try_into()
                                    .or(Err(AuthResponseError::NegativeTimeErr))?,
                            ),
                            window: Period::from(&usage.period),
                        },
                        left_hits: usage.max_value - usage.current_value,
                        max_value: usage.max_value,
                    },
                );
            }
        }

        let mut hierarchy = HashMap::new();
        if let Some(metrics) = response.hierarchy() {
            for (parent, children) in metrics.iter() {
                hierarchy.insert(parent.clone(), children.clone());
            }
        }
        let keys = app_keys
            .keys()
            .iter()
            .map(|app_key| AppKey::from(app_key.as_ref()))
            .collect::<Vec<_>>();
        let mut app = Application {
            app_id: app_identifier,
            service_id,
            local_state: state,
            metric_hierarchy: hierarchy,
            app_keys: Some(keys),
        };

        // note: we have made an assumption that there is not contention with other threads
        // since it's a fresh application and should not be present in the cache.
        match self.handle_cache_hit(&mut app, 0) {
            Ok(()) => Ok(()),
            Err(e) => Err(AuthResponseError::CacheHitErr(e)),
        }
    }

    // Parse request data and return it back inside the struct
    fn get_request_data(&self) -> Result<ThreescaleData, RequestDataError> {
        // Note: Make changes here when data is fetched from metadata instead of request headers
        let service_token = self
            .get_http_request_header("x-3scale-service-token")
            .ok_or(RequestDataError::ServiceTokenNotFound)?;

        let service_id = self
            .get_http_request_header("x-3scale-service-id")
            .ok_or(RequestDataError::ServiceIdNotFound)?;

        let usage_str = self
            .get_http_request_header("x-3scale-usages")
            .ok_or(RequestDataError::UsageNotFound)?;

        let cluster_name = self
            .get_http_request_header("x-3scale-cluster-name")
            .ok_or(RequestDataError::ClusterNameNotFound)?;

        let upstream_url = self
            .get_http_request_header("x-3scale-upstream-url")
            .ok_or(RequestDataError::UpstreamUrlNotFound)?;
        let parsed_url = url::Url::parse(&upstream_url)?;

        let timeout = match self.get_http_request_header("x-3scale-timeout") {
            Some(time_str) => time_str.parse::<u64>().ok(),
            None => None,
        };

        let upstream_builder = Builder::try_from(parsed_url)?;

        let usages = serde_json::from_str::<std::collections::HashMap<String, u64>>(&usage_str)?;

        let app_id;
        if let Some(user_key) = self.get_http_request_header("x-3scale-user-key") {
            app_id = AppIdentifier::UserKey(UserKey::from(user_key.as_ref()));
        } else {
            app_id = AppIdentifier::appid_from_str(
                &self
                    .get_http_request_header("x-3scale-app-id")
                    .ok_or(RequestDataError::AuthKeyMissing)?,
            );
        }

        Ok(ThreescaleData {
            app_id,
            service_id: ServiceId::from(service_id.as_ref()),
            service_token: ServiceToken::from(service_token.as_ref()),
            metrics: RefCell::new(usages),
            upstream: upstream_builder.build(&cluster_name, timeout),
        })
    }
}

impl Context for CacheFilter {
    fn on_http_call_response(&mut self, token_id: u32, _: usize, body_size: usize, _: usize) {
        info!(
            self.context_id,
            "received response from 3scale: token: {}", token_id
        );
        match self.get_http_call_response_body(0, body_size) {
            Some(bytes) => {
                match Authorization::from_str(std::str::from_utf8(&bytes).unwrap()) {
                    Ok(Authorization::Status(response)) => {
                        if response.is_authorized() || response.usage_reports().is_some() {
                            if let Err(e) = self.handle_auth_response(&response) {
                                warn!(self.context_id, "handling auth response failed: {:?}", e);
                                request_process_failure(self)
                            }
                        } else if cfg!(feature = "visible_logs") {
                            let (key, val) =
                                crate::log::visible_logs::get_logs_header_pair(self.context_id);
                            self.send_http_response(
                                403,
                                vec![(key.as_ref(), val.as_ref())],
                                Some(response.reason().unwrap().as_bytes()),
                            );
                        } else {
                            self.send_http_response(
                                403,
                                vec![],
                                Some(response.reason().unwrap().as_bytes()),
                            )
                        }
                    }
                    Ok(Authorization::Error(auth_error)) => {
                        info!(
                            self.context_id,
                            "authorization error with code: {}",
                            auth_error.code()
                        );
                        request_process_failure(self);
                        return;
                    }
                    Err(e) => {
                        info!(
                            self.context_id,
                            "parsing response from 3scale failed: {:#?} with token: {}",
                            e,
                            token_id
                        );
                        request_process_failure(self);
                        return;
                    }
                }

                info!(
                    self.context_id,
                    "data received and parsed from callout with token :{}", token_id
                );
            }
            None => {
                info!(
                    self.context_id,
                    "Found nothing in the response with token: {}", token_id
                );
                request_process_failure(self);
            }
        }
    }
}
