use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct FilterConfig {
    /// Behaviour in case of a cache miss and authorize call gets failed.
    pub failure_mode_deny: bool,
    /// Number of retries for setting data to cache
    pub max_tries: u32,
}

impl Default for FilterConfig {
    fn default() -> Self {
        FilterConfig {
            failure_mode_deny: true,
            max_tries: 5,
        }
    }
}
