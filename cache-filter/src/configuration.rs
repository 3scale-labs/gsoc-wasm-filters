use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct FilterConfig {
    /// Behaviour in case of a cache miss and authorize call gets failed.
    pub failure_mode_deny: bool,
    /// Number of retries for setting data to cache
    pub max_tries: u32,
    /// Max memory in bytes that shared data is allowed to use.
    pub max_shared_memory_bytes: u64,
}

impl Default for FilterConfig {
    fn default() -> Self {
        FilterConfig {
            failure_mode_deny: true,
            max_tries: 5,
            max_shared_memory_bytes: 4294967296, // equivalent to 4GB
        }
    }
}
