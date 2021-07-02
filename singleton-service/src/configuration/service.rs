use crate::configuration::delta::DeltaStoreConfig;
use serde::Deserialize;
use std::time::Duration;
use threescale::upstream::Upstream;

#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct ServiceConfig {
    /// Upstream configuration.
    pub upstream_config: Upstream,

    /// Delta store configuration.
    pub delta_store_config: DeltaStoreConfig,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        ServiceConfig {
            upstream_config: Upstream {
                name: "outbound|443||su1.3scale.net".to_owned(),
                url: "https://su1.3scale.net".parse().unwrap(),
                timeout: Duration::from_millis(5000),
            },
            delta_store_config: DeltaStoreConfig::default(),
        }
    }
}
