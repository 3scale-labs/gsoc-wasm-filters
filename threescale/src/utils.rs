use crate::structs::{ Application, PeriodWindow };
use proxy_wasm::hostcalls::{ set_shared_data, get_shared_data};
use log::info;
use std::time::{ SystemTime, Duration };

// Returns Application from shared data with CAS integer
pub fn get_application_from_cache(key: &str) -> Option<(Application,u32)> {
    match get_shared_data(&key).unwrap()
    {
        (bytes,cas) => {
            match bytes {
                Some(data) => Some((bincode::deserialize::<Application>(&data).unwrap(),cas.unwrap())),
                None => return None,
            }
        },
    } 
}

fn get_cas_from_cache(key: &str) -> Option<u32> {
    match get_shared_data(&key).unwrap()
    {
        (_b, cas_o) => {
            match cas_o {
                Some(_c) => cas_o,
                None => None,
            }
        }
    }
}

// Returns false on set failure
pub fn set_application_to_cache(key: &str, app: &Application, overwrite: bool, num_tries: Option<u32>) -> bool {
    let mut cas = Some(0);
    
    if !overwrite {
        cas = get_cas_from_cache(key);
    }

    for num_try in 1..(1+num_tries.unwrap_or(1)) {
        info!("Try {}: Setting application with key: {}", num_try, key);
        match set_shared_data(&key,Some(&bincode::serialize::<Application>(&app).unwrap()), cas)
        {
            Ok(()) => return true,
            Err(e) => info!("Try {}: Set operation failed for key: {} due to: {:?}", num_try, key, e),
        }
        cas = get_cas_from_cache(key);
    }
    false
}

pub fn get_next_period_window(old_window: &PeriodWindow, current_time: &SystemTime) -> PeriodWindow {
    // TODO: How to calculate next window?
    PeriodWindow {
        window_type: old_window.window_type.clone(),
        start: Duration::new(0,0),
        end: Duration::new(0,0),
    }
}
