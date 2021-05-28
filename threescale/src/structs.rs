use std::time::Duration;
use std::cell::RefCell;
use std::collections::HashMap;
use serde::{Serialize,Deserialize};


#[derive(Serialize,Deserialize)]
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
  start: Duration,
  end: Duration,
  window_type: Period,
}

#[allow(dead_code)]
#[derive(Serialize,Deserialize)]
pub struct UsageReport {
  period_window: PeriodWindow,
  left_hits: u32,
  // Required to renew window untill new state is fetched from 3scale.
  max_value: u32,
}

// Threescale's Application representation for cache
#[allow(dead_code)]
#[derive(Serialize,Deserialize)]
pub struct Application {
  app_id: String,
  service_id: String,
  timestamp: Duration,
  local_state: RefCell<HashMap<String,UsageReport>>,
  metric_hierarchy: RefCell<HashMap<String,String>>,
  unlimited_counter: RefCell<HashMap<String,u32>>,
}

// Request data recieved from previous filters
pub struct ThreescaleData {
  pub app_id: String,
  pub service_id: String,
  pub metrics: RefCell<HashMap<String,u32>>,
}
