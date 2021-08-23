use crate::{info, warn};
use proxy_wasm::{
    hostcalls::{get_shared_data, set_shared_data},
    types::Status,
};
use threescale::proxy::CacheKey;

thread_local! {
    pub static WAITING_CONTEXTS: RefCell<HashMap<u32, CacheFilter>> = RefCell::new(HashMap::new());
}

#[derive(Debug, thiserror::Error)]
pub enum UniqueCalloutError<'a> {
    #[error("failed to serialize {what:?} due to {reason:?}")]
    SerializeFail {
        what: &'a str,
        reason: bincode::ErrorKind,
    },
    #[error("failed to deserialize {what:?} due to {reason:?}")]
    DeserializeFail {
        what: &'a str,
        reason: bincode::ErrorKind,
    },
    #[error("failure due to proxy's internal issue: {0:?}")]
    ProxyFailure(Status),
    #[error("failed to resolve thread({0}) specific MQ while adding callout-waiter")]
    MQResolveFail(u32),
}

// This struct is serialized and stored in the shared data for callout-lock winner
// to know which thread to wake up and to let waiters know which http context to resume processing.
#[derive(Deserialize, Serialize, Clone)]
pub struct CalloutWaiter {
    /// MQ id of the thread waiting for callout response.
    pub queue_id: u32,
    /// Id of http context waiting for callout response.
    pub http_context_id: u32,
}

/** TD;LR on how lock is acquired by exploiting host implementation:
* cache is essentially a hashmap that maps key to a pair of (value, cas).
* set_shared_data(key, value, cas)'s psuedo-code is as follows:
*   lock_using_a_mutex() // This guarantees only 1 thread executes its instructions at a time.
*   if (key is present in the hashmap)
*      if (cas && cas != cas_of_last_value_added)
*          return CasMisMatch
*      update value with new cas
*   else insert_new_entry_into_the_hashmap
*
* A lock is acquired by a thread when it successfully adds an entry in the cache of the format:
*              ("CL_{CacheKey}", ({THREAD_ID/ROOT_CONTEXT_ID}, CAS))

* Rest of the threads that don't the lock will have their ids stored in the cache with format:
*              ("CW_{CacheKey}", (serialized_vector_of_CalloutWaiter_struct, CAS))

* Here, CL - Callout-Lock, CW - Callout-Waiters

* If you look at Rust SDK spec, when a cache entry is not present, you get (None, None) as result.
* Now imagine a scenario, when two threads (T1 & T2) check for the same lock in the cache and find no lock.
* T1 & T2 tries to set an entry by calling: set_shared_data(key, value, **None**).
* If you go through above-mentioned algo, no matter the order which thread executes first, second thread
* will overwrite the first one because None CAS is translated into 0 in SDK and second-if condition is passed-over.

* Now imagine if T1 & T2 use '1'(any but 0) as CAS, first thread will insert a new entry since it's not
* already present in the hashmap and second one will get CasMismatch in the result due to second-if condition.
* NOTE: Since CAS is a u32 integer, after u32::MAX, CAS resets to 1. What this means for unique-callout
* is that N threads can successfully acquire the lock again when lock was freed with CAS=1. But the chances
* of this happening is worse than winning a lottery and proxy can handle multiple callouts.

* It's good to point that 'None' as CAS was intended for overwriting the exisiting value and that's how
* we free the lock actually, by sending 'None' as value and CAS.

* Please read host implementation (shared_data.cc/h) and Rust SDK/hostcalls.rs for better understanding.
**/

// Callout lock is acquired by placing a key-value pair inside shared data.
pub fn set_callout_lock(
    root_id: u32,
    context_id: u32,
    cache_key: &CacheKey,
) -> Result<bool, Status> {
    let callout_key = format!("CL_{}", cache_key.as_string());
    let root_id_str = root_id.to_string();
    let root_id_str_bytes = root_id_str.as_bytes();
    info!(
        context_id,
        "thread(id: {}): trying to set callout-lock for request(key: {})", root_id, callout_key
    );

    // check if lock is already acquired or not
    match get_shared_data(&callout_key)? {
        (_, None) => {
            info!(
                context_id,
                "thread ({}): trying to acquire lock ({}) the first time", root_id, callout_key
            );
            // Note: CAS is not 'None' here                              ∨∨∨∨∨∨
            match set_shared_data(&callout_key, Some(root_id_str_bytes), Some(1)) {
                Ok(()) => {
                    info!(
                        context_id,
                        "thread ({}): callout-lock ({}) acquired", root_id, callout_key
                    );
                    Ok(true)
                }
                Err(Status::CasMismatch) => {
                    info!(
                        context_id,
                        "thread ({}): callout-lock ({}) for already acquired by another thread",
                        root_id,
                        callout_key
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
                "thread ({}): callout-lock ({}) already acquired by thread ({})",
                root_id,
                callout_key,
                lock_acquired_by
            );
            Ok(false)
        }
        (None, Some(cas)) => {
            info!(
                context_id,
                "thread ({}): callout-lock ({}) was already free and trying to acquire again",
                root_id,
                callout_key
            );
            match set_shared_data(&callout_key, Some(root_id_str_bytes), Some(cas)) {
                Ok(()) => {
                    info!(
                        context_id,
                        "thread ({}): callout-lock ({}) successfully acquired",
                        root_id,
                        callout_key
                    );
                    Ok(true)
                }
                Err(Status::CasMismatch) => {
                    info!(
                        context_id,
                        "thread ({}): callout-lock ({}) already acquired by another thread",
                        root_id,
                        callout_key
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
// Callout-lock is freed by setting null value in the cache for the request key.
pub fn free_callout_lock(
    root_id: u32,
    context_id: u32,
    cache_key: &CacheKey,
    let callout_key = format!("CL_{}", cache_key.as_string());
    info!(
        context_id,
        "thread ({}): trying to free callout-lock ({})", root_id, callout_key
    );

    if let Err(Status::NotFound) = get_shared_data(&callout_key) {
        warn!(
            context_id,
            "thread ({}): trying to free non-existing callout-lock ({})", root_id, callout_key
        );
        return Err(Status::NotFound);
    }

    if let Err(e) = set_shared_data(&callout_key, None, None) {
        warn!(
            context_id,
            "thread ({}): failed to delete the callout-lock ({}) from shared data: {:?}",
            root_id,
            callout_key,
            e
        );
        return Err(e);
    }
    Ok(())
}
