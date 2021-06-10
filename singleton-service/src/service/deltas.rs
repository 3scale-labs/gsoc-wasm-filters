use chrono::offset::Utc;
use chrono::DateTime;
use std::collections::HashMap;
use threescale::structs::ThreescaleData;

#[derive(Clone)]
pub struct AppDelta {
    pub key_type: String,
    pub usages: HashMap<String, u32>,
}

// getter and setter vs dot
pub struct DeltaStore {
    pub last_update: Option<DateTime<Utc>>,
    pub deltas: HashMap<String, HashMap<String, AppDelta>>,
    pub request_count: u32,
    pub capacity: u32,
}

#[derive(PartialEq)]
pub enum DeltaStoreState {
    Flush,
    Ok,
}

impl DeltaStore {
    /// Update delta store with a new entry of type ThreescaleData. If container size reached, then
    /// initiate delta store flush logic.
    pub fn update_delta_store(
        &mut self,
        threescale: &ThreescaleData,
    ) -> Result<DeltaStoreState, anyhow::Error> {
        match self.get_service(&threescale.service_id, &threescale.service_token) {
            Some(service) => match DeltaStore::get_app_delta(&threescale.app_id, service) {
                Some(app) => {
                    DeltaStore::update_app_delta(app, threescale);
                }
                None => {
                    DeltaStore::add_app_delta(service, threescale);
                }
            },
            None => {
                let mut usages: HashMap<String, AppDelta> = HashMap::new();
                usages.insert(
                    threescale.app_id.clone(),
                    AppDelta {
                        key_type: "user_key".to_string(),
                        usages: threescale.metrics.borrow().clone(),
                    },
                );
                let delta_key = format!("{}_{}", threescale.service_id, threescale.service_token);
                self.deltas.insert(delta_key, usages);
            }
        }
        self.request_count += 1;
        if self.request_count == self.capacity {
            Ok(DeltaStoreState::Flush)
        } else {
            Ok(DeltaStoreState::Ok)
        }
    }

    /// Method to flush delta store to 3scale SM API.
    // #[allow(dead_code)]
    // pub fn flush_deltas(&mut self) -> bool {
    //     for (service_key, apps) in self.deltas.drain() {
    //         let report: Report = report(&service_key, &apps).unwrap();
    //         let request = build_report_request(&report).unwrap();
    //         dispatch_http_call(, headers: Vec<(&str, &str)>, body: Option<&[u8]>, trailers: Vec<(&str, &str)>, timeout: Duration)
    //     }
    //     true
    // }

    fn get_app_delta<'a>(
        app_key: &'a str,
        service: &'a mut HashMap<String, AppDelta>,
    ) -> Option<&'a mut AppDelta> {
        service.get_mut(app_key)
    }

    fn get_service(
        &mut self,
        service_id: &str,
        service_token: &str,
    ) -> Option<&mut HashMap<String, AppDelta>> {
        let key = format!("{}_{}", service_id, service_token);
        self.deltas.get_mut(&key)
    }

    fn update_app_delta(app_delta: &mut AppDelta, threescale: &ThreescaleData) -> bool {
        for (metric, value) in threescale.metrics.borrow().iter() {
            if app_delta.usages.contains_key(metric) {
                *app_delta.usages.get_mut(metric).unwrap() += value;
            } else {
                app_delta.usages.insert(metric.to_string(), *value);
            }
        }
        true
    }

    fn add_app_delta(service: &mut HashMap<String, AppDelta>, threescale: &ThreescaleData) -> bool {
        service.insert(
            threescale.app_id.clone(),
            AppDelta {
                key_type: "user_key".to_string(),
                usages: threescale.metrics.borrow().clone(),
            },
        );
        true
    }
}
