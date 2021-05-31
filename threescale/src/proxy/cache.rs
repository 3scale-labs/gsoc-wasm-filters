use crate::structs::Application;
use log::info;
use proxy_wasm::hostcalls::{get_shared_data, set_shared_data};

// Returns Application from shared data with CAS integer
pub fn get_application_from_cache(key: &str) -> Option<(Application, u32)> {
    match get_shared_data(&key) {
        Ok((Some(bytes), Some(cas))) => {
            Some((bincode::deserialize::<Application>(&bytes).unwrap(), cas))
        }
        Ok((_bytes, _cas)) => None,
        Err(e) => {
            info!("Fetching application from cache failed due to: {:?}", e);
            None
        }
    }
}

fn get_cas_from_cache(key: &str) -> Option<u32> {
    let (_b, cas) = get_shared_data(&key).unwrap();
    cas
}

// Returns false on set failure
pub fn set_application_to_cache(
    key: &str,
    app: &Application,
    overwrite: bool,
    num_tries: Option<u32>,
) -> bool {
    let mut cas = None; // Default case is set to overwrite

    if !overwrite {
        cas = get_cas_from_cache(key);
    }

    for num_try in 1..(1 + num_tries.unwrap_or(1)) {
        info!("Try {}: Setting application with key: {}", num_try, key);
        match set_shared_data(
            &key,
            Some(&bincode::serialize::<Application>(&app).unwrap()),
            cas,
        ) {
            Ok(()) => return true,
            Err(e) => info!(
                "Try {}: Set operation failed for key: {} due to: {:?}",
                num_try, key, e
            ),
        }
        cas = get_cas_from_cache(key);
    }
    false
}
