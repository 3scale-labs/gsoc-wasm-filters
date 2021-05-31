use crate::structs::{Application, PeriodWindow, ThreescaleData};
use std::time::{Duration, SystemTime};

pub fn get_next_period_window(
    old_window: &PeriodWindow,
    _current_time: &SystemTime,
) -> PeriodWindow {
    // TODO: How to calculate next window?
    PeriodWindow {
        window_type: old_window.window_type.clone(),
        start: Duration::new(0, 0),
        end: Duration::new(0, 0),
    }
}

// Perform metrics update based on threescale specific logic
pub fn update_metrics(_new_hits: &ThreescaleData, _application: &mut Application) -> bool {
    true
}
