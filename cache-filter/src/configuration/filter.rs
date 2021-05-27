use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct FilterConfig {
    /// Threescale cluster name that indicates threescale backend that includes SM API. Should provide the cluster
    /// name of the threescale cluster in the envoy.yaml file.
    threescale_cluster: String,

    /// The basepath for the authorize endpoint.
    threescale_auth_basepath: String,

    /// Authroize call timeout.
    #[serde(with = "serde_humanize_rs")]
    threescale_auth_timeout: Duration,

    /// Behaviour in case of a cache miss and authorize call gets failed.
    failure_mode_deny: bool,
}

impl Default for FilterConfig {
    fn default() -> Self {
        FilterConfig {
            threescale_cluster: "threescale_SM_API".to_owned(),
            threescale_auth_basepath: "authorize.xml".to_owned(),
            threescale_auth_timeout: Duration::from_secs(5),
            failure_mode_deny: true,
        }
    }
}
