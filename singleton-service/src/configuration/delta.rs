use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct DeltaStoreConfig {
    /// Capacity of the delta store.
    capacity: u64,

    /// Flush duration for periodical cache flush in case of low traffic.
    #[serde(with = "serde_humanize_rs")]
    periodical_flush: Duration,

    /// Retry duration in case threescale backend is offline.
    #[serde(with = "serde_humanize_rs")]
    retry_duration: Duration,

    /// Capacity of the await queue in case threescale backend is offline.
    await_queue_capacity: u64,
}

impl Default for DeltaStoreConfig {
    fn default() -> Self {
        DeltaStoreConfig {
            capacity: 100,
            periodical_flush: Duration::from_secs(60),
            retry_duration: Duration::from_secs(10),
            await_queue_capacity: 200,
        }
    }
}
