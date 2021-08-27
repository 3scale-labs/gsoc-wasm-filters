use crate::filter::http::CacheFilter;
use crate::{info, warn};
use proxy_wasm::{
    hostcalls::{enqueue_shared_queue, get_shared_data, resolve_shared_queue, set_shared_data},
    types::Status,
};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
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

// This enum is passed to thread-specific MQs to let waiters know how to resume processing.
#[derive(Deserialize, Serialize, Clone)]
pub enum WaiterAction {
    /// Follow the cache hit path with context_id used as inner value.
    HandleCacheHit(u32),
    /// Follow in request failure path with context_id used as inner value.
    HandleFailure(u32),
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
*              ("CL_{CacheKey}", ({SERALIZED_CALLOUT_LOCK_VALUE}, CAS))


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

/// This struct is serialized and stored as the value of a callout lock.
#[derive(Serialize, Deserialize)]
struct CalloutLockValue {
    // Id of the thread who owns the lock.
    pub owned_by: u32,
    // List of contexts that are waiting for lock to be freed.
    pub waiters: Vec<CalloutWaiter>,
}

// Callout lock is acquired by placing a key-value pair inside shared data.
// Return Ok(true) when lock is acquired and Ok(false) when added to waitlist.
pub fn set_callout_lock(context: &CacheFilter) -> Result<bool, UniqueCalloutError> {
    let callout_lock_key = format!("CL_{}", context.cache_key.as_string());
    let context_id = context.context_id;
    let root_id = context.root_id;
    info!(
        context_id,
        "thread(id: {}): trying to set callout-lock for request(key: {})",
        root_id,
        callout_lock_key
    );
    let callout_lock_value = CalloutLockValue {
        owned_by: root_id,
        waiters: Vec::new(),
    };
    // Doing this here just for the sake of performance improvement instead of creating it
    // during every loop run caused by CasMismatch.
    let queue_id = resolve_shared_queue(crate::VM_ID, &root_id.to_string()).unwrap();
    if queue_id.is_none() {
        return Err(UniqueCalloutError::MQResolveFail(root_id));
    }
    let callout_waiter = CalloutWaiter {
        queue_id: queue_id.unwrap(),
        http_context_id: context_id,
    };
    let lock_value_serialized = match bincode::serialize::<CalloutLockValue>(&callout_lock_value) {
        Ok(res) => res,
        Err(e) => {
            return Err(UniqueCalloutError::SerializeFail {
                what: "CalloutLockValue",
                reason: *e,
            })
        }
    };

    loop {
        // check if lock is already acquired or not
        match get_shared_data(&callout_lock_key) {
            Ok((_, None)) => {
                info!(
                    context_id,
                    "thread ({}): trying to acquire lock ({}) the first time",
                    root_id,
                    callout_lock_key
                );
                // Note: CAS is not 'None' here                                   ∨∨∨∨∨∨
                match set_shared_data(&callout_lock_key, Some(&lock_value_serialized), Some(1)) {
                    Ok(()) => {
                        info!(
                            context_id,
                            "thread ({}): callout-lock ({}) acquired", root_id, callout_lock_key
                        );
                        return Ok(true);
                    }
                    Err(Status::CasMismatch) => {
                        info!(
                            context_id,
                            "thread ({}): callout-lock ({}) for already acquired by another thread",
                            root_id,
                            callout_lock_key
                        );
                        continue;
                    }
                    Err(e) => return Err(UniqueCalloutError::ProxyFailure(e)),
                }
            }
            Ok((Some(bytes), Some(cas))) => {
                let mut stored_lock_value = match bincode::deserialize::<CalloutLockValue>(&bytes) {
                    Ok(res) => res,
                    Err(e) => {
                        return Err(UniqueCalloutError::DeserializeFail {
                            what: "CalloutLockValue",
                            reason: *e,
                        })
                    }
                };
                info!(
                    context_id,
                    "thread ({}): callout-lock ({}) already acquired by thread ({}), trying to add to waitlist",
                    root_id,
                    callout_lock_key,
                    stored_lock_value.owned_by
                );
                stored_lock_value.waiters.push(callout_waiter.clone());
                let serialized_updated_lock_value =
                    match bincode::serialize::<CalloutLockValue>(&stored_lock_value) {
                        Ok(res) => res,
                        Err(e) => {
                            return Err(UniqueCalloutError::SerializeFail {
                                what: "(updated) CalloutLockValue",
                                reason: *e,
                            })
                        }
                    };
                if let Err(Status::CasMismatch) = set_shared_data(
                    &callout_lock_key,
                    Some(&serialized_updated_lock_value),
                    Some(cas),
                ) {
                    continue;
                }
                info!(
                    context_id,
                    "thread({}): added to waitlist for callout-lock({})", root_id, callout_lock_key
                );
                WAITING_CONTEXTS.with(|waiters| {
                    waiters
                        .borrow_mut()
                        .insert(context.context_id, context.clone())
                });
                return Ok(false);
            }
            Ok((None, Some(cas))) => {
                info!(
                    context_id,
                    "thread ({}): callout-lock ({}) was already free and trying to acquire again",
                    root_id,
                    callout_lock_key
                );
                match set_shared_data(&callout_lock_key, Some(&lock_value_serialized), Some(cas)) {
                    Ok(()) => {
                        info!(
                            context_id,
                            "thread ({}): callout-lock ({}) successfully acquired",
                            root_id,
                            callout_lock_key
                        );
                        return Ok(true);
                    }
                    Err(Status::CasMismatch) => {
                        info!(
                            context_id,
                            "thread ({}): callout-lock ({}) already acquired by another thread",
                            root_id,
                            callout_lock_key
                        );
                        continue;
                    }
                    Err(e) => return Err(UniqueCalloutError::ProxyFailure(e)),
                }
            }
            Err(e) => return Err(UniqueCalloutError::ProxyFailure(e)),
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
) -> Result<(), UniqueCalloutError> {
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
        return Err(UniqueCalloutError::ProxyFailure(Status::NotFound));
    }

    if let Err(e) = set_shared_data(&callout_key, None, None) {
        warn!(
            context_id,
            "thread ({}): failed to delete the callout-lock ({}) from shared data: {:?}",
            root_id,
            callout_key,
            e
        );
        return Err(UniqueCalloutError::ProxyFailure(e));
    }
    Ok(())
}

pub fn send_action_to_waiters(
    root_id: u32,
    context_id: u32,
    cache_key: &CacheKey,
    mut waiter_action: WaiterAction,
) -> Result<(), UniqueCalloutError> {
    let waiters_key = format!("CW_{}", cache_key.as_string());
    info!(
        context_id,
        "thread({}): trying to resume callout-waiters ({})", root_id, waiters_key
    );

    match get_shared_data(&waiters_key) {
        Ok((Some(bytes), _)) => match bincode::deserialize::<Vec<CalloutWaiter>>(&bytes) {
            Ok(callout_waiters) => {
                for callout_waiter in callout_waiters {
                    match waiter_action {
                        WaiterAction::HandleFailure(ref mut ctxt_id) => {
                            *ctxt_id = callout_waiter.http_context_id
                        }
                        WaiterAction::HandleCacheHit(ref mut ctxt_id) => {
                            *ctxt_id = callout_waiter.http_context_id
                        }
                    }
                    let message = match bincode::serialize::<WaiterAction>(&waiter_action) {
                        Ok(res) => res,
                        Err(e) => {
                            return Err(UniqueCalloutError::SerializeFail {
                                what: "WaiterAction",
                                reason: *e,
                            })
                        }
                    };
                    if let Err(e) = enqueue_shared_queue(callout_waiter.queue_id, Some(&message)) {
                        // There is nothing we can do to signal other threads now and should just
                        // allow them to timeout and maybe add another mechanism for memory clearance
                        warn!(
                            context_id,
                            "thread({}): enqueue failure for queue({}): {:?}",
                            root_id,
                            callout_waiter.queue_id,
                            e
                        );
                    }
                }
                // Note: safe for current SDK implementation.
                set_shared_data(&waiters_key, None, None).unwrap();
            }
            Err(e) => {
                return Err(UniqueCalloutError::DeserializeFail {
                    what: "Vec<CalloutWaiter>",
                    reason: *e,
                });
            }
        },
        Ok((None, _)) => {
            // This can happen either someother thread freed waiting contexts or
            // there was only 1 request for this specific application. If this happens
            // check implementation of acquiring and freeing lock as it's not supposed to happen.
            warn!(
                context_id,
                "thread({}): found no callout-waiters ({})", root_id, waiters_key
            );
        }
        Err(e) => return Err(UniqueCalloutError::ProxyFailure(e)),
    }
    Ok(())
}
