use crate::proxy::{set_application_to_cache, CacheKey};
use crate::structs::{
    Application, Hierarchy, Metrics, Period, RateLimitInfo, RateLimitStatus, ThreescaleData,
};
use std::time::Duration;

#[derive(Debug, Clone, thiserror::Error)]
pub enum UpdateMetricsError {
    #[error("overflow due to two duration addition")]
    DurationOverflow,
    #[error("request is rate-limited")]
    RateLimited,
    #[error("application set was unsuccessful: {0}")]
    CacheUpdateFail(String),
}

// updates application to reflect consumed quota if not rate-limited
// returns Ok() if not rate-limited and faced no problem updating the application
pub fn limit_check_and_update_application(
    data: &ThreescaleData,
    app: &mut Application,
    app_cas: u32,
    current_time: &Duration,
) -> Result<RateLimitStatus, UpdateMetricsError> {
    let mut rate_limit_info = RateLimitInfo::default();

    for (metric, hits) in data.metrics.borrow().iter() {
        // note: we assume missing metrics are not limited until new state is fetched
        if let Some(usage_report) = app.local_state.get_mut(metric) {
            let mut period = &mut usage_report.period_window;

            // taking care of period window expiration
            if period.window != Period::Eternity && period.end < *current_time {
                let time_diff = current_time
                    .checked_sub(period.start)
                    .ok_or(UpdateMetricsError::DurationOverflow)?;

                // This is atleast 1 because current time is higher than window end.
                let num_windows = time_diff.as_secs() / period.window.as_secs();

                // No. of secs to push forward window ends so the current time fall within new window.
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
                rate_limit_info.limit = Some(usage_report.max_value);
                rate_limit_info.remaining = Some(0);
                // Current time is between period window since it's already adjusted.
                rate_limit_info.reset =
                    Some(period.end.checked_sub(*current_time).unwrap().as_secs());
                return Ok(RateLimitStatus::RateLimited(rate_limit_info));
            }
            usage_report.left_hits -= *hits;

            // RateLimit-Limit must convey minimum service-limit.
            if rate_limit_info.limit.is_none()
                || usage_report.max_value < rate_limit_info.limit.unwrap()
            {
                rate_limit_info.limit = Some(usage_report.max_value);
                rate_limit_info.remaining = Some(usage_report.left_hits);
                rate_limit_info.reset =
                    Some(period.end.checked_sub(*current_time).unwrap().as_secs());
            }
        }
    }

    // request is not rate-limited and will be set to cache
    let cache_key = CacheKey::from(&app.service_id, &app.app_id);
    if let Err(e) = set_application_to_cache(&cache_key.as_string(), app, app_cas) {
        return Err(UpdateMetricsError::CacheUpdateFail(e.to_string()));
    }
    Ok(RateLimitStatus::Authorized(rate_limit_info))
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
