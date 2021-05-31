use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct ServiceConfig {
    /// Threescale cluster name that indicates threescale backend that includes SM API. Should provide the cluster
    /// name of the threescale cluster in the envoy.yaml file.
    threescale_cluster: String,

    /// Authroize call timeout.
    #[serde(with = "serde_humanize_rs")]
    threescale_auth_timeout: Duration,

    /// Size of the local cache container.
    local_cache_container_size: i32,

    /// Minimum tick period for the periodical cache update in case of low traffic.
    #[serde(with = "serde_humanize_rs")]
    minimum_tick: Duration,

    /// Maximum tick period for the periodical cache update in case of low traffic.
    #[serde(with = "serde_humanize_rs")]
    maximum_tick: Duration,

    /// Retry duration in case threescale backend gets offline.
    #[serde(with = "serde_humanize_rs")]
    retry_duration: Duration,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        ServiceConfig {
            threescale_cluster: "threescale_SM_API".to_owned(),
            threescale_auth_timeout: Duration::from_secs(5),
            local_cache_container_size: 100,
            minimum_tick: Duration::from_secs(5),
            maximum_tick: Duration::from_secs(60),
            retry_duration: Duration::from_secs(20),
        }
    }
}
