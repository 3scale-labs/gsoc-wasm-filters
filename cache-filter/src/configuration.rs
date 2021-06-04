use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct FilterConfig {
    /// Threescale cluster name that indicates threescale backend that includes SM API. Should provide the cluster
    /// name of the threescale cluster in the envoy.yaml file.
    pub threescale_cluster: String,

    /// Authroize call timeout.
    #[serde(with = "serde_humanize_rs")]
    pub threescale_auth_timeout: Duration,

    /// Behaviour in case of a cache miss and authorize call gets failed.
    pub failure_mode_deny: bool,
}

impl Default for FilterConfig {
    fn default() -> Self {
        FilterConfig {
            threescale_cluster: "outbound|443||su1.3scale.net".to_owned(),
            threescale_auth_timeout: Duration::from_millis(5000),
            failure_mode_deny: true,
        }
    }
}
