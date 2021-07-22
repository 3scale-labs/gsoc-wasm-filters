use crate::configuration::delta::DeltaStoreConfig;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct ServiceConfig {
    /// Delta store configuration.
    pub delta_store_config: DeltaStoreConfig,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        ServiceConfig {
            delta_store_config: DeltaStoreConfig::default(),
        }
    }
}
