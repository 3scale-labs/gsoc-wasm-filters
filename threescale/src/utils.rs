use crate::structs::{Application, Period, ThreescaleData};
use std::time::Duration;

#[derive(Debug, Clone, thiserror::Error)]
pub enum UpdateMetricsError {
    #[error("overflow due to two duration addition")]
    DurationOverflow,
    #[error("request is rate-limited")]
    RateLimited,
}

// updates application to reflect consumed quota if not rate-limited
// returns Ok() if not rate-limited and faced no problem updating the application
pub fn limit_check_and_update_application(
    data: &ThreescaleData,
    app: &mut Application,
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
    Ok(())
}
