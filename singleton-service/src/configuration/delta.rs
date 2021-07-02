use serde::Deserialize;
use std::time::Duration;

// Represents the method of cache flush.
// ContainerLimit - Cache update will be performed when the delta store gets filled.
// Periodical - Cache update will be performed after every periodical tick.
// Default - A combination of ContainerLimit and Periodical.
#[derive(Deserialize, Debug, PartialEq, Clone)]
pub enum FlushMode {
    ContainerLimit,
    Periodical,
    Default,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct DeltaStoreConfig {
    /// Capacity of the delta store.
    pub capacity: u64,

    /// Flush duration for periodical cache flush in case of low traffic.
    #[serde(with = "serde_humanize_rs")]
    pub periodical_flush: Duration,

    /// Retry duration in case threescale backend is offline.
    #[serde(with = "serde_humanize_rs")]
    pub retry_duration: Duration,

    /// Capacity of the await queue in case threescale backend is offline.
    pub await_queue_capacity: u64,

    /// FlushMode denotes the strategy used for cache update.
    pub flush_mode: FlushMode,
}

impl Default for DeltaStoreConfig {
    fn default() -> Self {
        DeltaStoreConfig {
            capacity: 100,
            periodical_flush: Duration::from_secs(60),
            retry_duration: Duration::from_secs(30),
            await_queue_capacity: 200,
            flush_mode: FlushMode::Default,
        }
    }
}
