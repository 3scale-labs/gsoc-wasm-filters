use chrono::offset::Utc;
use chrono::DateTime;
use std::cell::RefCell;
use std::collections::HashMap;
use threescale::structs::ThreescaleData;

// getter and setter vs dot
pub struct DeltaStore {
    pub last_update: Option<DateTime<Utc>>,
    pub deltas: RefCell<HashMap<String, HashMap<String, u32>>>,
    pub request_count: u32,
}

impl DeltaStore {
    /// Update delta store with a new entry of type ThreescaleData. If container size reached, then
    /// initiate delta store flush logic.
    pub fn update_delta_store(&self, _threescale: &ThreescaleData) -> bool {
        true
    }

    /// Method to flush delta store to 3scale SM API.
    pub fn flush_deltas(&self) -> bool {
        true
    }
}
