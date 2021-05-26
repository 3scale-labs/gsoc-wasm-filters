use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct FilterConfig {
    /// Envoy cluster name that provides ext_authz service. Should provide the cluster
    /// name of the ext_authz cluster in the envoy.yaml file.
    threescale_cluster: String,

    /// The path to call on the HTTP service for ext_authz
    threescale_auth_basepath: String,

    /// External auth request authority header
    threescale_auth_timeout: Duration,

    failure_mode_deny: bool
}

impl Default for FilterConfig {
    fn default() -> Self {
        FilterConfig {
            threescale_cluster: "threescale_SM_API".to_owned(),
            threescale_auth_basepath: "authorize.xml".to_owned(),
            threescale_auth_timeout: Duration::from_secs(5),
            failure_mode_deny: true
        }
    }
}