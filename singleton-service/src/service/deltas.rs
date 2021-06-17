use chrono::offset::Utc;
use chrono::DateTime;
use std::collections::HashMap;
use threescale::structs::{AppIdentifier, ThreescaleData};

/// DeltaStore is an in-memory storage built using nested hashmaps to store deltas for different
/// metrics related to different applications until they gets flushed. Data stored in delta store
/// is structured in a way that is favourable cache flush operation.
/// (minimal computations required before flushing)
pub struct DeltaStore {
    // Represents the previous cache flush time in UTC. This will be used to flush cache in case of
    // a low network traffic scenario.
    pub last_update: Option<DateTime<Utc>>,

    // Represents a hierarchical storage of deltas.
    // Hierarchy => - Service
    //                - Application
    //                  - Metric : Value
    pub deltas: HashMap<String, HashMap<AppIdentifier, HashMap<String, u64>>>,

    // Represents the request count. Gets incremented by 1 for each request passing
    // through the proxy. Used together with the capacity attribute to implement a
    // container filling mechansim for cache flush in case of high network traffic scenarios.
    pub request_count: u32,

    // Represents the capacity of the cache container.
    pub capacity: u32,
}

/// DeltaStoreState represents the state of the delta store. Singleton service uses this
/// state to initiate cache flush when delta store gets filled.
#[derive(PartialEq)]
pub enum DeltaStoreState {
    Flush,
    Ok,
}

impl DeltaStore {
    /// Method to update delta store. Handles scenarios like updating existing metrics,
    /// adding new services, applications with new metrics. Gets called for each message
    /// received through the message queue.
    pub fn update_delta_store(
        &mut self,
        threescale: &ThreescaleData,
    ) -> Result<DeltaStoreState, anyhow::Error> {
        match self.get_mut_service(
            threescale.service_id.as_ref(),
            threescale.service_token.as_ref(),
        ) {
            Some(service) => match DeltaStore::get_mut_app_delta(&threescale.app_id, service) {
                Some(app) => {
                    DeltaStore::update_app_delta(app, threescale);
                }
                None => {
                    DeltaStore::add_app_delta(service, threescale);
                }
            },
            None => {
                let mut usages: HashMap<AppIdentifier, HashMap<String, u64>> = HashMap::new();
                usages.insert(
                    threescale.app_id.clone(),
                    threescale.metrics.borrow().clone(),
                );
                let delta_key = format!(
                    "{}_{}",
                    threescale.service_id.as_ref(),
                    threescale.service_token.as_ref()
                );
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

    fn get_mut_app_delta<'a>(
        app: &'a AppIdentifier,
        service: &'a mut HashMap<AppIdentifier, HashMap<String, u64>>,
    ) -> Option<&'a mut HashMap<String, u64>> {
        service.get_mut(app)
    }

    fn get_mut_service(
        &mut self,
        service_id: &str,
        service_token: &str,
    ) -> Option<&mut HashMap<AppIdentifier, HashMap<String, u64>>> {
        let key = format!("{}_{}", service_id, service_token);
        self.deltas.get_mut(&key)
    }

    fn update_app_delta(app_delta: &mut HashMap<String, u64>, threescale: &ThreescaleData) -> bool {
        for (metric, value) in threescale.metrics.borrow().iter() {
            if app_delta.contains_key(metric) {
                *app_delta.get_mut(metric).unwrap() += value;
            } else {
                app_delta.insert(metric.to_string(), *value);
            }
        }
        true
    }

    fn add_app_delta(
        service: &mut HashMap<AppIdentifier, HashMap<String, u64>>,
        threescale: &ThreescaleData,
    ) -> bool {
        service.insert(
            threescale.app_id.clone(),
            threescale.metrics.borrow().clone(),
        );
        true
    }
}
