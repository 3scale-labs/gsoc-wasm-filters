use serde::Deserialize;
use std::time::Duration;
use threescale::upstream::Upstream;

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct FilterConfig {
    /// Upstream config
    pub upstream: Upstream,
    /// Behaviour in case of a cache miss and authorize call gets failed.
    pub failure_mode_deny: bool,
    /// Number of retries for setting data to cache
    pub max_tries: u32,
}

impl Default for FilterConfig {
    fn default() -> Self {
        FilterConfig {
            upstream: Upstream {
                name: "outbound|443||su1.3scale.net".to_owned(),
                url: "http://0.0.0.0:3000/".parse().unwrap(),
                timeout: Duration::from_millis(5000),
            },
            failure_mode_deny: true,
            max_tries: 5,
        }
    }
}
