use crate::{
    configuration::FilterConfig,
    utils::{do_auth_call, in_request_failure, request_process_failure},
};
use log::{debug, info, warn};
use proxy_wasm::{
    traits::{Context, HttpContext},
    types::Action,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::TryInto;
use std::str::FromStr;
use std::time::{Duration, UNIX_EPOCH};
use std::vec;
use threescale::{
    proxy::{
        get_app_id_from_cache, get_application_from_cache, set_app_id_to_cache,
        set_application_to_cache, CacheError, CacheKey,
    },
    structs::*,
    utils::*,
};
use threescalers::response::{Authorization, AuthorizationStatus};

const QUEUE_NAME: &str = "message_queue";
const VM_ID: &str = "my_vm_id";

#[derive(Debug, Clone, thiserror::Error)]
enum CacheHitError {
    #[error("duration since time later than self")]
    TimeConversionErr(#[from] std::time::SystemTimeError),
    #[error("failure of is_rate_limited error")]
    MetricsCheckFail(#[from] UpdateMetricsError),
    #[error("unable to resolve message queue with specified vm_id and q_name")]
    MQNotFound,
}

#[derive(Debug, thiserror::Error)]
enum AuthResponseError {
    #[error("failure to follow cache hit flow")]
    CacheHitErr(#[from] CacheHitError),
    #[error("conversion from i64 time to u64 duration failed")]
    NegativeTimeErr,
    #[error("usage reports from 3scale auth response are missing")]
    UsageNotFound,
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
}

pub struct CacheFilter {
    pub context_id: u32,
    pub config: FilterConfig,
    pub update_cache_from_singleton: bool,
    pub cache_key: CacheKey,
    // required for cache miss case
    pub req_data: ThreescaleData,
}

impl HttpContext for CacheFilter {
    fn on_http_request_headers(&mut self, context_id: usize) -> Action {
        let mut request_data = match self.get_request_data() {
            Ok(data) => data,
            Err(e) => {
                debug!("ctxt {}: fetching request data failed: {}", e, context_id);
                // Send back local response for not providing relevant request data
                self.send_http_response(401, vec![], None);
                return Action::Pause;
            }
        };

        self.cache_key = CacheKey::from(&request_data.service_id, &request_data.app_id);
        self.req_data = request_data.clone();

        if let AppIdentifier::UserKey(ref user_key) = request_data.app_id {
            match get_app_id_from_cache(&user_key) {
                Ok(app_id) => {
                    request_data.app_id = AppIdentifier::from(app_id);
                    self.req_data.app_id = request_data.app_id.clone();
                    self.cache_key.set_app_id(&request_data.app_id);
                }
                Err(e) => {
                    debug!(
                        "ctxt {}: failed to map user_key to app_id: {:?}",
                        context_id, e
                    );
                    // TODO: avoid multiple calls for identical requests
                    return do_auth_call(self, self, &request_data);
                }
            }
        }

        match get_application_from_cache(&self.cache_key) {
            Ok((mut app, _)) => {
                info!("ctxt {}: cache hit", context_id);

                match self.handle_cache_hit(&mut app) {
                    Ok(()) => Action::Continue,
                    Err(e) => {
                        warn!("ctxt {}: cache hit flow failed: {:#?}", context_id, e);
                        in_request_failure(self, self)
                    }
                }
            }
            Err(e) => {
                info!("ctxt {}: cache miss: {}", context_id, e);
                // TODO: Avoid multiple calls for same application
                // saving request data to use when there is response from 3scale
                // fetching new application state using authorize endpoint
                do_auth_call(self, self, &request_data)
            }
        }
    }
}

impl CacheFilter {
    fn report_to_singleton(&self, qid: u32, req_time: &Duration) -> bool {
        let message: Message =
            Message::new(self.update_cache_from_singleton, &self.req_data, req_time);
        if let Err(e) = self.enqueue_shared_queue(qid, Some(&bincode::serialize(&message).unwrap()))
        {
            warn!(
                "ctxt {}: enqueuing to message queue failed: {:?}",
                self.context_id, e
            );
            return false;
        }
        true
    }

    fn handle_cache_hit(&mut self, app: &mut Application) -> Result<(), CacheHitError> {
        let queue_id = self
            .resolve_shared_queue(VM_ID, QUEUE_NAME)
            .ok_or(CacheHitError::MQNotFound)?;

        let current_time = self.get_current_time().duration_since(UNIX_EPOCH)?;

        add_hierarchy_to_metrics(&app.metric_hierarchy, &mut self.req_data.metrics);
        match limit_check_and_update_application(&self.req_data, app, &current_time) {
            Ok(()) => {
                // request is not rate-limited and application is updated
                if !set_application_to_cache(&self.cache_key.as_string(), &app, true, None) {
                    self.update_cache_from_singleton = true;
                }
                info!(
                    "ctxt {}: request is allowed to pass the filter",
                    self.context_id
                );
                if !self.report_to_singleton(queue_id, &current_time) {
                    // TODO: Handle MQ failure here
                    // Update local cache
                    // Report to 3scale and get new state using authrep endpoint
                }
                self.resume_http_request();
            }
            Err(UpdateMetricsError::RateLimited) => {
                info!("ctxt {}: request is rate-limited", self.context_id);
                // TODO: add how many similar requests can be accepted.
                self.send_http_response(429, vec![], Some(b"Request rate-limited.\n"));
            }
            Err(e) => return Err(CacheHitError::MetricsCheckFail(e)),
        }
        Ok(())
    }

    fn handle_auth_response(
        &mut self,
        response: &AuthorizationStatus,
    ) -> Result<(), AuthResponseError> {
        // Form application struct from the response
        let mut state = HashMap::new();
        let reports = response
            .usage_reports()
            .ok_or(AuthResponseError::UsageNotFound)?;
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
                    left_hits: usage.current_value,
                    max_value: usage.max_value,
                },
            );
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

        set_application_to_cache(&self.cache_key.as_string(), &app, true, None);

        match self.handle_cache_hit(&mut app) {
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
        })
    }
}

impl Context for CacheFilter {
    fn on_http_call_response(&mut self, token_id: u32, _: usize, body_size: usize, _: usize) {
        info!(
            "ctxt {}: received response from 3scale: token: {}",
            self.context_id, token_id
        );
        match self.get_http_call_response_body(0, body_size) {
            Some(bytes) => {
                match Authorization::from_str(std::str::from_utf8(&bytes).unwrap()) {
                    Ok(Authorization::Status(response)) => {
                        if response.usage_reports().is_some() {
                            if let Err(e) = self.handle_auth_response(&response) {
                                warn!(
                                    "ctxt {}: handling auth response failed: {:?}",
                                    self.context_id, e
                                );
                                request_process_failure(self, self)
                            }
                        } else {
                            self.send_http_response(
                                403,
                                vec![],
                                Some(response.reason().unwrap().as_bytes()),
                            )
                        }
                    }
                    Ok(Authorization::Error(auth_error)) => {
                        info!("authorization error with code: {}", auth_error.code());
                        request_process_failure(self, self);
                        return;
                    }
                    Err(e) => {
                        info!(
                            "parsing response from 3scale failed: {:#?} with token: {}",
                            e, token_id
                        );
                        request_process_failure(self, self);
                        return;
                    }
                }

                info!(
                    "data received and parsed from callout with token :{}",
                    token_id
                );
            }
            None => {
                info!("Found nothing in the response with token: {}", token_id);
                request_process_failure(self, self);
            }
        }
    }
}
