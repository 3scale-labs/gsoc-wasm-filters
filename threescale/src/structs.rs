use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;

#[derive(Serialize, Deserialize, Clone)]
pub enum Period {
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
    Eternity,
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize)]
pub struct PeriodWindow {
    pub start: Duration,
    pub end: Duration,
    pub window_type: Period,
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize)]
pub struct UsageReport {
    pub period_window: PeriodWindow,
    pub left_hits: u64,
    // Required to renew window untill new state is fetched from 3scale.
    pub max_value: u64,
}

// Threescale's Application representation for cache
#[derive(Serialize, Deserialize)]
pub struct Application {
    pub app_id: String,
    pub service_id: String,
    pub local_state: RefCell<HashMap<String, UsageReport>>,
    pub metric_hierarchy: RefCell<HashMap<String, Vec<String>>>,
}

// Request data recieved from previous filters
#[derive(Serialize, Deserialize, Clone)]
pub struct ThreescaleData {
    // TODO: App_key, user_key is also possible as an input
    pub app_id: String,
    pub service_id: String,
    pub service_token: String,
    pub metrics: RefCell<HashMap<String, u64>>,
}

impl Default for ThreescaleData {
    fn default() -> Self {
        ThreescaleData {
            app_id: "".to_owned(),
            service_id: "".to_owned(),
            service_token: "".to_owned(),
            metrics: RefCell::new(HashMap::new()),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Message {
    pub update_cache_from_singleton: bool,
    pub data: ThreescaleData,
}

impl Message {
    pub fn new(update_flag: bool, request_data: &ThreescaleData) -> Message {
        Message {
            update_cache_from_singleton: update_flag,
            data: request_data.clone(),
        }
    }
}
