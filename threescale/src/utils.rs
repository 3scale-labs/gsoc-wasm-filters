use crate::proxy::{set_application_to_cache, CacheKey};
use crate::structs::{AppIdentifier, Application, Hierarchy, Metrics, Period, ThreescaleData};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone, thiserror::Error)]
pub enum UpdateMetricsError {
    #[error("overflow due to two duration addition")]
    DurationOverflow,
    #[error("request is rate-limited")]
    RateLimited,
    #[error("application set was unsuccessful")]
    CacheUpdateFail,
}

/// AllocatedSize trait should be implemented for types that have
/// dynamic memory allocations on the heap and that are taken into
/// effect for memory allocation.
pub trait AllocatedSize {
    fn dynamic_allocated_size(&self) -> usize;
}

// updates application to reflect consumed quota if not rate-limited
// returns Ok() if not rate-limited and faced no problem updating the application
pub fn limit_check_and_update_application(
    data: &ThreescaleData,
    app: &mut Application,
    app_cas: u32,
    current_time: &Duration,
) -> Result<(), UpdateMetricsError> {
    for (metric, hits) in data.metrics.borrow().iter() {
        // note: we assume missing metrics are not limited until new state is fetched
        if let Some(usage_report) = app.local_state.get_mut(metric) {
            let mut period = &mut usage_report.period_window;

            // taking care of period window expiration
            if period.window != Period::Eternity && period.end < *current_time {
                let time_diff = current_time
                    .checked_sub(period.start)
                    .ok_or(UpdateMetricsError::DurationOverflow)?;
                let num_windows = time_diff.as_secs() / period.window.as_secs();
                let seconds_to_add = num_windows * period.window.as_secs();

                // set to new period window
                period.start = period
                    .start
                    .checked_add(Duration::from_secs(seconds_to_add))
                    .ok_or(UpdateMetricsError::DurationOverflow)?;

                period.end = period
                    .end
                    .checked_add(Duration::from_secs(seconds_to_add))
                    .ok_or(UpdateMetricsError::DurationOverflow)?;

                // reset left hits back to max value
                usage_report.left_hits = usage_report.max_value;
            }

            if usage_report.left_hits < *hits {
                return Err(UpdateMetricsError::RateLimited);
            }
            usage_report.left_hits -= *hits;
        }
    }

    // request is not rate-limited and will be set to cache
    let cache_key = CacheKey::from(&app.service_id, &app.app_id);
    if !set_application_to_cache(&cache_key.as_string(), &app, app_cas) {
        return Err(UpdateMetricsError::CacheUpdateFail);
    }
    Ok(())
}

// It takes the provided hierarchy structure, and uses it
// to determine how the metrics, m, are affected, incrementing parent metrics
// based on the value of the parents child/children metrics.
pub fn add_hierarchy_to_metrics(hierarchy: &Hierarchy, metrics: &mut Metrics) {
    for (parent, children) in hierarchy.iter() {
        for (metric, hits) in metrics.borrow().iter() {
            if children.contains(metric) {
                *metrics.borrow_mut().entry(parent.to_string()).or_insert(0) += *hits;
            }
        }
    }
}

/// AllocatedSize impl for String.
impl AllocatedSize for String {
    // Returns the dynamically allocated value of a String on the heap.
    fn dynamic_allocated_size(&self) -> usize {
        self.capacity()
    }
}

/// AllocatedSize impl for AppIdentifier.
impl AllocatedSize for AppIdentifier {
    // Returns the dynamically allocated value of a AppIdentifier on the heap.
    fn dynamic_allocated_size(&self) -> usize {
        match self {
            AppIdentifier::AppId(app_id, None) => app_id.as_ref().to_string().capacity(),
            AppIdentifier::AppId(app_id, Some(app_key)) => {
                app_id.as_ref().to_string().capacity() + app_key.as_ref().to_string().capacity()
            }
            AppIdentifier::UserKey(user_key) => user_key.as_ref().to_string().capacity(),
        }
    }
}

/// AllocatedSize impl for HashMap<String, u64>.
impl AllocatedSize for HashMap<String, u64> {
    // Returns the dynamically allocated value of HashMap<String, u64>. Key is considerd
    // here because u64 has a fixed size.
    fn dynamic_allocated_size(&self) -> usize {
        self.iter()
            .map(|(key, _)| key.dynamic_allocated_size())
            .sum()
    }
}
