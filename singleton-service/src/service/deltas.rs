use crate::configuration::delta::{DeltaStoreConfig, FlushMode};
use chrono::offset::Utc;
use chrono::DateTime;
use log::info;
use std::collections::HashMap;
use threescale::{
    structs::{AppIdentifier, ThreescaleData},
    utils::*,
};

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
    pub memory_allocated: usize,

    // DeltaStoreConfig contains all the configurations related for delta store.
    pub config: DeltaStoreConfig,
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
        let delta_increase: usize;
        match self.get_mut_service(
            threescale.service_id.as_ref(),
            threescale.service_token.as_ref(),
        ) {
            Some(service) => match DeltaStore::get_mut_app_delta(&threescale.app_id, service) {
                Some(app) => {
                    info!(
                        "Application {} found for service {}",
                        &threescale.app_id.as_ref(),
                        &threescale.service_id.as_ref()
                    );
                    delta_increase = DeltaStore::update_app_delta(app, threescale);
                }
                None => {
                    info!(
                        "No application found for service {}",
                        &threescale.service_id.as_ref()
                    );
                    delta_increase = DeltaStore::add_app_delta(service, threescale);
                }
            },
            None => {
                info!("No service and application found for the given key combination");
                // Current allocation of the services hashmap before new entry.
                let prev_alloc = self.deltas.capacity();
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
                // Entry bytes denotes the dynamic heap allocation for the entry.
                let entry_bytes = delta_key.capacity()
                    + threescale.app_id.dynamic_allocated_size()
                    + threescale.metrics.borrow().dynamic_allocated_size();
                // transitive_alloc denotes the allocations of the inner hashmaps due to
                // new entry.
                let transitive_alloc = usages.capacity()
                    * (std::mem::size_of::<HashMap<String, u64>>()
                        + std::mem::size_of::<AppIdentifier>())
                    + threescale.metrics.borrow().capacity()
                        * (std::mem::size_of::<HashMap<String, u64>>());
                self.deltas.insert(delta_key, usages);
                // new_alloc denotes the new allocation of the services hashmap.
                let new_alloc = self.deltas.capacity();
                let direct_alloc = (new_alloc - prev_alloc)
                    * (std::mem::size_of::<String>()
                        + std::mem::size_of::<HashMap<AppIdentifier, HashMap<String, u64>>>());
                // Total value of the delta store memory allocation increase is equal to direct_alloc of the
                // services hashmap + transitive allocation of the nested hashmaps + dynamic allocation of the entry.
                delta_increase = direct_alloc + transitive_alloc + entry_bytes;
            }
        }
        if self.config.flush_mode != FlushMode::Periodical {
            info!(
                "Delta store memory allocation increased by: {}",
                delta_increase
            );
            self.memory_allocated += delta_increase;
            if self.memory_allocated >= self.config.capacity as usize {
                Ok(DeltaStoreState::Flush)
            } else {
                Ok(DeltaStoreState::Ok)
            }
        } else {
            Ok(DeltaStoreState::Ok)
        }
    }

    // TODO: Handle app_id -> app_id + app_key scenario.
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

    fn update_app_delta(
        app_delta: &mut HashMap<String, u64>,
        threescale: &ThreescaleData,
    ) -> usize {
        let mut entry_bytes: usize = 0;
        // Current allocation of the metrics hashmap before new entry insertion.
        let prev_alloc = app_delta.capacity();
        for (metric, value) in threescale.metrics.borrow().iter() {
            if app_delta.contains_key(metric) {
                *app_delta.get_mut(metric).unwrap() += value;
            } else {
                app_delta.insert(metric.to_string(), *value);
                entry_bytes += metric.to_string().dynamic_allocated_size();
            }
        }
        // If no new entry, that means updating existing metrics. No difference in memory allocation.
        if entry_bytes == 0 {
            0
        } else {
            let new_alloc = app_delta.capacity();
            entry_bytes
                + (new_alloc - prev_alloc)
                    * (std::mem::size_of::<String>() + std::mem::size_of::<u64>())
        }
    }

    fn add_app_delta(
        service: &mut HashMap<AppIdentifier, HashMap<String, u64>>,
        threescale: &ThreescaleData,
    ) -> usize {
        // Previous allocation of the application hashmap before new entry.
        let prev_alloc = service.capacity();
        service.insert(
            threescale.app_id.clone(),
            threescale.metrics.borrow().clone(),
        );
        // New allocation of the application hashmap after new entry.
        let new_alloc = service.capacity();
        let direct_alloc = (new_alloc - prev_alloc)
            * (std::mem::size_of::<AppIdentifier>() + std::mem::size_of::<HashMap<String, u64>>());
        // Transitive allocation of the metrics hashmap.
        let transitive_alloc = threescale.metrics.borrow().capacity()
            * (std::mem::size_of::<String>() + std::mem::size_of::<u64>());
        // Dynamic allocation of the entry.
        let entry_bytes = threescale.app_id.dynamic_allocated_size()
            + threescale.metrics.borrow().dynamic_allocated_size();
        // Total memory allocation as a summation of direct_alloc, entry_bytes and transitive_alloc
        direct_alloc + entry_bytes + transitive_alloc
    }
}
