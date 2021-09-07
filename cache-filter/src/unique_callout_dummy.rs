use crate::filter::http::CacheFilter;
use threescale::proxy::CacheKey;

/**  Reasoning behind this module:
* There are few modules (like unique_callout) that are configurable through the
* cargo flag features, which means the main functionality of the filter should
* work fine without these modules as well. One way is to add alot of 'if cfg! {}'
* conditions into rest of the code and another way is to define dummy module with
* similar interfaces that do nothing but allow the rest of the code to work as
* if the module was not present at all, without making it hard to read.
**/

/* ========================== UNIQUE-CALLOUT START ========================= */

#[derive(Debug, thiserror::Error)]
pub enum UniqueCalloutError {}

pub enum SetCalloutLockStatus {
    LockAcquired,
    #[allow(dead_code)]
    AddedToWaitlist,
    #[allow(dead_code)]
    ResponseCameFirst,
}

pub enum WaiterAction {
    HandleCacheHit(u32),
    HandleFailure(u32),
}

pub fn set_callout_lock(_: &CacheFilter) -> Result<SetCalloutLockStatus, UniqueCalloutError> {
    Ok(SetCalloutLockStatus::LockAcquired)
}

pub fn free_callout_lock_and_notify_waiters(
    _: u32,
    _: u32,
    _: &CacheKey,
    _: WaiterAction,
) -> Result<(), UniqueCalloutError> {
    Ok(())
}

/* ================================= UNIQUE-CALLOUT END ============================== */
