use chrono::offset::Utc;
use chrono::DateTime;
use std::cell::RefCell;
use std::collections::HashMap;
use threescale::structs::ThreescaleData;

// getter and setter vs dot
pub struct DeltaStore {
  last_update: DateTime<Utc>,
  deltas: RefCell<HashMap<String, HashMap<String, u32>>>,
  request_count: u32,
}

impl DeltaStore {
  pub fn update_delta_store(&self, threescale: &ThreescaleData) -> bool {

    return true;
  }

  pub fn flush_deltas(&self) -> bool {
    return true;
  }

}
