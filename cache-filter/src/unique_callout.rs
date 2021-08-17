use crate::{info, warn};
use proxy_wasm::{
    hostcalls::{get_shared_data, set_shared_data},
    types::Status,
};
use threescale::proxy::CacheKey;

// Callout lock is acquired by placing a key-value pair inside shared data.
// Since, only one thread is allowed to access shared data (host uses mutex) thus only one winner;
pub fn set_callout_lock(
    root_id: u32,
    context_id: u32,
    cache_key: &CacheKey,
) -> Result<bool, Status> {
    let request_key = format!("callout_{}", cache_key.as_string());
    let root_id_str = root_id.to_string();
    let root_id_str_bytes = root_id_str.as_bytes();
    info!(
        context_id,
        "thread(id: {}): trying to set callout-lock for request(key: {})", root_id, request_key
    );

    // check if lock is already acquired or not
    match get_shared_data(&request_key)? {
        (_, None) => {
            // lock not acquired yet, try to set lock.
            // Note: Since two threads can get None for CAS at the same time, to avoid
            // overwriting of lock by another thread, use any value for CAS other than 0.
            match set_shared_data(&request_key, Some(root_id_str_bytes), Some(1)) {
                Ok(()) => {
                    info!(
                        context_id,
                        "thread(id: {}): callout-lock acquired for request(key: {})",
                        root_id,
                        request_key
                    );
                    Ok(true)
                }
                Err(Status::CasMismatch) => {
                    info!(
                        context_id,
                        "thread(id: {}): callout-lock for request(key:{}) already acquired by another thread",
                        root_id,
                        request_key
                    );
                    Ok(false)
                }
                Err(e) => Err(e),
            }
        }
        (Some(bytes), Some(_)) => {
            let lock_acquired_by =
                std::str::from_utf8(&bytes).unwrap_or("thread id failed to deserialize");
            info!(
                context_id,
                "thread(id: {}): callout-lock for request(key: {}) already acquired by thread(id: {})",
                root_id,
                request_key,
                lock_acquired_by
            );
            Ok(false)
        }
        (None, Some(cas)) => {
            info!(
                context_id,
                "thread(id: {}): callout-lock was already free and trying to acquire now for request(key: {})",
                root_id,
                request_key
            );
            match set_shared_data(&request_key, Some(root_id_str_bytes), Some(cas)) {
                Ok(()) => {
                    info!(
                        context_id,
                        "thread(id: {}): callout-lock acquired for request(key: {})",
                        root_id,
                        request_key
                    );
                    Ok(true)
                }
                Err(Status::CasMismatch) => {
                    info!(
                        context_id,
                        "thread(id: {}): callout-lock for request(key: {}) already acquired by another thread",
                        root_id,
                        request_key
                    );
                    Ok(false)
                }
                Err(e) => Err(e),
            }
        }
    }
}

// NOTE: Right now, there is no option of deleting the pair instead only the value can be erased,
// and it requires changes in the ABI so change this because it will lead to better memory usage.
pub fn free_callout_lock(
    root_id: u32,
    context_id: u32,
    cache_key: &CacheKey,
) -> Result<(), Status> {
    let request_key = format!("callout_{}", cache_key.as_string());
    info!(
        context_id,
        "thread(id: {}): trying to free callout-lock (key: {})", root_id, request_key
    );

    if let Err(Status::NotFound) = get_shared_data(&request_key) {
        warn!(
            context_id,
            "thread(id: {}): trying to free non-existing callout-lock (key: {})",
            root_id,
            request_key
        );
        return Err(Status::NotFound);
    }

    if let Err(e) = set_shared_data(&request_key, None, None) {
        warn!(
            context_id,
            "thread(id: {}): failed to delete the callout-lock (key: {}) from shared data: {:?}",
            root_id,
            request_key,
            e
        );
        return Err(e);
    }
    Ok(())
}
