use crate::structs::Application;

// Returns Application from shared data with CAS integer
pub fn get_application_from_cache(key: &str) -> Option<(Application,u32)> {
    None
}

// Returns true on set failure
pub fn set_application_to_cache(key: &str, app: &Application) -> bool {
    let max_retries = 10; // We can also allow this, to be set from configuration
    // Add random sleep in-between as well
    true
}
