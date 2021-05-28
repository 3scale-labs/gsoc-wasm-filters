use std::time::Duration;
use std::cell::RefCell;
use std::collections::HashMap;

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
pub struct PeriodWindow {
  start: Duration,
  end: Duration,
  window_type: Period,
}

#[allow(dead_code)]
pub struct UsageReport {
  period_window: PeriodWindow,
  left_hits: u32,
}

// Threescale's Application representation for cache
#[allow(dead_code)]
pub struct Application<'a> {
  app_id: String,
  service_id: String,
  timestamp: Duration,
  local_state: RefCell<HashMap<&'a str,UsageReport>>,
  metric_hierarchy: RefCell<HashMap<&'a str,&'a str>>,
  unlimited_counter: RefCell<HashMap<&'a str,u32>>,
}
