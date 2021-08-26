use crate::{
    configuration::FilterConfig,
    debug, info,
    unique_callout::{
        add_to_callout_waitlist, free_callout_lock, send_action_to_waiters, set_callout_lock,
        WaiterAction,
    },
    utils::{do_auth_call, in_request_failure, request_process_failure},
    warn,
};
use proxy_wasm::{
    traits::{Context, HttpContext},
    types::Action,
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
    stats::*,
    structs::*,
    upstream::*,
    utils::*,
};
use threescalers::response::{Authorization, AuthorizationStatus};

const QUEUE_NAME: &str = "message_queue";
const TIMEOUT_STATUS: &str = "504";

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
    pub root_id: u32,
    pub config: FilterConfig,
    pub update_cache_from_singleton: bool,
    pub cache_key: CacheKey,
    // required for cache miss case
    pub req_data: ThreescaleData,
    pub stats: ThreescaleStats,
}

impl HttpContext for CacheFilter {
    fn on_http_request_headers(&mut self, _: usize) -> Action {
        let mut request_data = match self.get_request_data() {
            Ok(data) => data,
            Err(e) => {
                debug!(self.context_id, "fetching request data failed: {}", e);
                increment_stat(&self.stats.auth_metadata_errors);
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
                    increment_stat(&self.stats.cache_misses);
                    return match set_callout_lock(self.root_id, self.context_id, &self.cache_key) {
                        Ok(true) => do_auth_call(self, self, &request_data),
                        Ok(false) => {
                            if let Err(e) = add_to_callout_waitlist(self) {
                                warn!(
                                    self.context_id,
                                    "failed to add current context to callout-waitlist: {}", e
                                );
                                in_request_failure(self, self);
                            }
                            info!(
                                self.context_id,
                                "successfully added current context to callout-waitlist"
                            );
                            Action::Pause
                        }
                        Err(e) => {
                            warn!(
                                self.context_id,
                                "failed to set callout-lock for request(key: {}): {:?}",
                                self.cache_key.as_string(),
                                e
                            );
                            in_request_failure(self, self)
                        }
                    };
                }
            }
        }

        match get_application_from_cache(&self.cache_key) {
            Ok((mut app, cas)) => match self.handle_cache_hit(&mut app, cas) {
                Ok(()) => Action::Continue,
                Err(e) => {
                    warn!(self.context_id, "cache hit flow failed: {}", e);
                    in_request_failure(self)
                }
            },
            Err(e) => {
                info!(self.context_id, "cache miss: {}", e);
                increment_stat(&self.stats.cache_misses);
                match set_callout_lock(self.root_id, self.context_id, &self.cache_key) {
                    Ok(true) => do_auth_call(self, self, &request_data),
                    Ok(false) => {
                        if let Err(e) = add_to_callout_waitlist(self) {
                            warn!(
                                self.context_id,
                                "failed to add current context to callout-waitlist: {}", e
                            );
                            return in_request_failure(self, self);
                        }
                        info!(
                            self.context_id,
                            "successfully added current context to callout-waitlist"
                        );
                        Action::Pause
                    }
                    Err(e) => {
                        warn!(
                            self.context_id,
                            "failed to set callout-lock for request(key: {}): {:?}",
                            self.cache_key.as_string(),
                            e
                        );
                        in_request_failure(self, self)
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

    // app_cas can be zero if fetched for the first from 3scale
    pub fn handle_cache_hit(
        &mut self,
        app: &mut Application,
        mut app_cas: u32,
    ) -> Result<(), CacheHitError> {
        info!(self.context_id, "cache hit");
        increment_stat(&self.stats.cache_hits);
        let queue_id = self
            .resolve_shared_queue(crate::VM_ID, QUEUE_NAME)
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
                    if app_cas == 0 {
                        increment_stat(&self.stats.cached_apps)
                    }
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
        if let AppIdentifier::UserKey(user_key) = self.cache_key.app_id() {
            self.req_data.app_id = app_identifier.clone();
            set_app_id_to_cache(user_key, &app_id)?;
        }

        self.cache_key = CacheKey::from(&service_id, &app_identifier);

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

        // Freeing of callout-lock requires the cache_key used to set the lock but cache_key
        // attached to 'self' can change inside handle_auth_response (user_key to app_id).
        let prev_cache_key = self.cache_key.clone();

        // Depending on how response is handled here, waiters should resume accordingly.
        // Note: Inner value of this enum is changed to context_id to resume in send_action_to_waiters().
        let mut waiter_action = WaiterAction::HandleCacheHit(0);

        let headers = self.get_http_call_response_headers();
        let status = headers
            .iter()
            .find(|(key, _)| key.as_str() == ":status")
            .map(|(_, value)| value)
            .unwrap();
        if status != TIMEOUT_STATUS {
            match self.get_http_call_response_body(0, body_size) {
                Some(bytes) => {
                    match Authorization::from_str(std::str::from_utf8(&bytes).unwrap()) {
                        Ok(Authorization::Status(response)) => {
                            if response.is_authorized() || response.usage_reports().is_some() {
                                if let Err(e) = self.handle_auth_response(&response) {
                                    warn!(
                                        self.context_id,
                                        "handling auth response failed: {:?}", e
                                    );
                                    waiter_action = WaiterAction::HandleFailure(0);
                                    request_process_failure(self)
                                }
                            } else if cfg!(feature = "visible_logs") {
                                waiter_action = WaiterAction::HandleFailure(0);
                                let (key, val) =
                                    crate::log::visible_logs::get_logs_header_pair(self.context_id);
                                self.send_http_response(
                                    403,
                                    vec![(key.as_ref(), val.as_ref())],
                                    Some(response.reason().unwrap().as_bytes()),
                                );
                            } else {
                                increment_stat(&self.stats.unauthorized);
                                waiter_action = WaiterAction::HandleFailure(0);
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
                            waiter_action = WaiterAction::HandleFailure(0);
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
                            waiter_action = WaiterAction::HandleFailure(0);
                            request_process_failure(self);
                        }
                    }
                }
                None => {
                    info!(
                        self.context_id,
                        "Found nothing in the response with token: {}", token_id
                    );
                    waiter_action = WaiterAction::HandleFailure(0);
                    request_process_failure(self);
                }
            }
        } else {
            info!(
                self.context_id,
                "HTTP request timeout for request with token_id: {}", token_id
            );
            increment_stat(&self.stats.authorize_timeouts);
            request_process_failure(self);
        }
        if let Err(e) = free_callout_lock(self.root_id, self.context_id, &prev_cache_key) {
            warn!(
                self.context_id,
                "failed to free callout-lock after auth response: {}", e
            );
        }
        if let Err(e) = send_action_to_waiters(
            self.root_id,
            self.context_id,
            &prev_cache_key,
            waiter_action,
        ) {
            warn!(
                self.context_id,
                "failed to resume callout-waiters after auth response: {}", e
            );
        }
    }
}
