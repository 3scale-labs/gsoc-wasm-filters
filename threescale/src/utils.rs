use crate::structs::Period;
use crate::structs::{Application, ThreescaleData};
use threescalers::response::Period as ResponsePeriod;

// Perform metrics update based on threescale specific logic
pub fn update_metrics(_new_hits: &ThreescaleData, _application: &mut Application) -> bool {
    true
}

pub fn period_from_response(res_period: &ResponsePeriod) -> Period {
    match res_period {
        ResponsePeriod::Minute => Period::Minute,
        ResponsePeriod::Hour => Period::Hour,
        ResponsePeriod::Day => Period::Day,
        ResponsePeriod::Week => Period::Week,
        ResponsePeriod::Month => Period::Month,
        ResponsePeriod::Year => Period::Year,
        ResponsePeriod::Eternity => Period::Eternity,
    }
}
