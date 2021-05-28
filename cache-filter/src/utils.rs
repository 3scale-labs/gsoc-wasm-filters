use threescale::structs::{ ThreescaleData, Application };
use std::collections::HashMap;
use std::cell::RefCell;
use std::time::SystemTime;

// Parse request data and return it back inside the struct
pub fn get_request_data() -> Option<ThreescaleData> {
    // Note: Confirm whether request data is recieved from metadata or headers?
    Some(ThreescaleData {
        app_id: "App".to_owned(),
        service_id: "Service".to_owned(),
        metrics: RefCell::new(HashMap::new()),
    })
}

pub fn handle_cache_miss(request_data: &ThreescaleData) {
    // Send response to 3scale
    // NOTE: Need to create a callback mechanism so we can refer to the same call and take any actions
}

pub fn is_rate_limited(request_data: &ThreescaleData, app: &mut Application, current_time: &SystemTime) -> bool {
    // NOTE: Handle period window expiration as well
    false
}

pub fn report_to_singleton(qid: Option<u32>, request_data: &ThreescaleData) -> bool {
    false
}
