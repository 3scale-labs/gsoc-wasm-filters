use std::time::Duration;
use std::cell::RefCell;
use std::collections::HashMap;
use threescale::structs::{UsageReport, ThreescaleData}; 

pub struct CacheDeltas {
  last_update: Duration,
  deltas: RefCell<HashMap<String,UsageReport>>,
  count: i32
}

impl CacheDeltas {
  fn update_cache_deltas(&self, threescale: &ThreescaleData) -> bool {
    return true
  }

  fn flush_deltas(&self) -> bool {
    return true
  }
}