use std::time::Duration;
use std::cell::RefCell;
use std::collections::HashMap;
use serde::{Serialize,Deserialize};


#[derive(Serialize,Deserialize,Clone)]
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
#[derive(Serialize,Deserialize)]
pub struct PeriodWindow {
  pub start: Duration,
  pub end: Duration,
  pub window_type: Period,
}

#[allow(dead_code)]
#[derive(Serialize,Deserialize)]
pub struct UsageReport {
  pub period_window: PeriodWindow,
  pub left_hits: u32,
  // Required to renew window untill new state is fetched from 3scale.
  pub max_value: u32,
}

// Threescale's Application representation for cache
#[derive(Serialize,Deserialize)]
pub struct Application {
  pub app_id: String,
  pub service_id: String,
  pub local_state: RefCell<HashMap<String,UsageReport>>,
  pub metric_hierarchy: RefCell<HashMap<String,String>>,
  pub unlimited_counter: RefCell<HashMap<String,u32>>,
}

// Request data recieved from previous filters
#[derive(Serialize,Deserialize)]
pub struct ThreescaleData {
  pub app_id: String,
  pub service_id: String,
  pub metrics: RefCell<HashMap<String,u32>>,
}

pub struct Message {
  pub update_cache_from_singleton: bool,
  pub data: ThreescaleData,
}
