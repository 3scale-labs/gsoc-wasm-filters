use std::collections::HashMap;
use std::cell::RefCell;
pub mod cache;

// Request data recieved from previous filters
#[allow(dead_code)]
pub struct ThreescaleData<'b> {
    app_id: String,
    service_id: String,
    metrics: RefCell<HashMap<&'b str,u32>>,
}
  