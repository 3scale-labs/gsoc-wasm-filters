use crate::upstream::Upstream;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::time::Duration;
use threescalers::response::Period as ResponsePeriod;

pub type Hierarchy = HashMap<String, Vec<String>>;
pub type Metrics = RefCell<HashMap<String, u64>>;

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum Period {
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
    Eternity,
}

impl Period {
    pub fn as_secs(&self) -> u64 {
        match *self {
            Period::Minute => 60,
            Period::Hour => 3600,
            Period::Day => 86400,
            Period::Week => 604800,
            Period::Month => 2592000,
            Period::Year => 31536000,
            Period::Eternity => u64::MAX,
        }
    }
}

impl From<&ResponsePeriod> for Period {
    fn from(res_period: &ResponsePeriod) -> Self {
        match res_period {
            ResponsePeriod::Minute => Period::Minute,
            ResponsePeriod::Hour => Period::Hour,
            ResponsePeriod::Day => Period::Day,
            ResponsePeriod::Week => Period::Week,
            ResponsePeriod::Month => Period::Month,
            ResponsePeriod::Year => Period::Year,
            ResponsePeriod::Eternity => Period::Eternity,
            _ => Period::Eternity,
        }
    }
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug)]
pub struct PeriodWindow {
    pub start: Duration,
    pub end: Duration,
    pub window: Period,
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug)]
pub struct UsageReport {
    pub period_window: PeriodWindow,
    pub left_hits: u64,
    // Required to renew window untill new state is fetched from 3scale.
    pub max_value: u64,
}

#[repr(transparent)]
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
pub struct AppId(String);
#[repr(transparent)]
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
pub struct AppKey(String);
#[repr(transparent)]
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
pub struct UserKey(String);

impl AsRef<str> for AppId {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl AsRef<str> for AppKey {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl AsRef<str> for UserKey {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl From<&str> for UserKey {
    fn from(a: &str) -> Self {
        Self(a.to_string())
    }
}

impl From<&str> for AppId {
    fn from(a: &str) -> Self {
        Self(a.to_string())
    }
}

impl From<&str> for AppKey {
    fn from(a: &str) -> Self {
        Self(a.to_string())
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceToken(String);
#[repr(transparent)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ServiceId(String);

impl AsRef<str> for ServiceToken {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl AsRef<str> for ServiceId {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl From<&str> for ServiceToken {
    fn from(a: &str) -> Self {
        Self(a.to_string())
    }
}

impl From<&str> for ServiceId {
    fn from(a: &str) -> Self {
        Self(a.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq)]
pub enum AppIdentifier {
    AppId(AppId, Option<AppKey>),
    UserKey(UserKey),
}

impl From<AppId> for AppIdentifier {
    fn from(a: AppId) -> Self {
        AppIdentifier::AppId(a, None)
    }
}

impl From<(AppId, AppKey)> for AppIdentifier {
    fn from(a: (AppId, AppKey)) -> Self {
        AppIdentifier::AppId(a.0, Some(a.1))
    }
}

impl From<UserKey> for AppIdentifier {
    fn from(u: UserKey) -> Self {
        AppIdentifier::UserKey(u)
    }
}

impl AsRef<str> for AppIdentifier {
    fn as_ref(&self) -> &str {
        match self {
            AppIdentifier::AppId(AppId(id), _key) => id.as_str(),
            // Unreachable condition once we map user_key to app_id.
            AppIdentifier::UserKey(UserKey(user_key)) => user_key.as_str(),
        }
    }
}

impl AppIdentifier {
    pub fn appid_from_str(s: &str) -> AppIdentifier {
        let v: Vec<&str> = s.split(':').collect();
        if v.len() == 2 {
            return AppIdentifier::AppId(AppId(v[0].to_owned()), Some(AppKey(v[1].to_owned())));
        }
        AppIdentifier::AppId(AppId(v[0].to_owned()), None)
    }
}

impl Hash for AppIdentifier {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state);
    }
}

impl PartialEq for AppIdentifier {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}

// Threescale's Application representation for cache
#[derive(Serialize, Deserialize, Debug)]
pub struct Application {
    pub app_id: AppIdentifier,
    pub service_id: ServiceId,
    pub local_state: HashMap<String, UsageReport>,
    pub metric_hierarchy: Hierarchy,
    pub app_keys: Option<Vec<AppKey>>,
}

// Request data recieved from previous filters
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ThreescaleData {
    // TODO: App_key, user_key is also possible as an input
    pub app_id: AppIdentifier,
    pub service_id: ServiceId,
    pub service_token: ServiceToken,
    pub metrics: Metrics,
    pub upstream: Upstream,
}

impl Default for ThreescaleData {
    fn default() -> Self {
        ThreescaleData {
            app_id: AppIdentifier::UserKey(UserKey("".to_owned())),
            service_id: ServiceId("".to_owned()),
            service_token: ServiceToken("".to_owned()),
            metrics: RefCell::new(HashMap::new()),
            upstream: Upstream {
                name: "".to_owned(),
                url: url::Url::parse("https://su1.su1.3scale.net/").unwrap(),
                timeout: Duration::from_millis(1000),
            },
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Message {
    pub update_cache_from_singleton: bool,
    pub data: ThreescaleData,
    pub req_time: Duration,
}

impl Message {
    pub fn new(update_flag: bool, request_data: &ThreescaleData, time: &Duration) -> Message {
        Message {
            update_cache_from_singleton: update_flag,
            data: request_data.clone(),
            req_time: *time,
        }
    }
}
