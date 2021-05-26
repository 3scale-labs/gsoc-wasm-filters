use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct ServiceConfig {
    /// Envoy cluster name that provides ext_authz service. Should provide the cluster
    /// name of the ext_authz cluster in the envoy.yaml file.
    threescale_cluster: String,

    /// The path to call on the HTTP service for cache
    threescale_auth_basepath: String,

    /// Time duration for the cache update
    #[serde(with = "serde_humanize_rs")]
    threescale_auth_timeout: Duration,

    local_cache_container_size: i32,
    
    minimum_tick: Duration,
    
    maximum_tick: Duration,

    retry_duration: Duration

}

impl Default for ServiceConfig {
    fn default() -> Self {
        ServiceConfig {
            threescale_cluster: "threescale_SM_API".to_owned(),
            threescale_auth_basepath: "authorize.xml".to_owned(),
            threescale_auth_timeout: Duration::from_secs(5),
            local_cache_container_size: 100,
            minimum_tick: Duration::from_secs(5),
            maximum_tick: Duration::from_secs(60),
            retry_duration: Duration::from_secs(20)
        }
    }
}

