use log::debug;
use proxy_wasm::hostcalls::{define_metric, increment_metric};
use proxy_wasm::types::MetricType;

/// ThreescaleStat holds a representation of a single metric.
/// u32 - metric identifier assigned depending on the context. (not unique)
/// String - unique metric identifier in reverse DNS format. Globally unique. eg: envoy.3scale.cache.apps
#[derive(Clone, Debug)]
pub struct ThreescaleStat(u32, String);

/// Struct that holds all the threescale specific stats.
#[derive(Clone, Debug)]
pub struct ThreescaleStats {
    // Total number of applications saved in shared data at a time t.
    pub cached_apps: ThreescaleStat,
    // Total number of cache misses.
    pub cache_misses: ThreescaleStat,
    // Total number of cache hits.
    pub cache_hits: ThreescaleStat,
    // Total number of unauthorized responses for authorize requests from cache filter.
    pub unauthorized: ThreescaleStat,
    // TODO: Add stats for cache filter authorize timeouts.
    // Total number of timeouts received for authorize requests (currently only singleton considered)
    pub authorize_timeouts: ThreescaleStat,
    // Total number of error codes due to auth metadata info missing.
    pub auth_metadata_errors: ThreescaleStat,
}

// Helper method to increment a metric by 1.
pub fn increment_stat(metric: &ThreescaleStat) {
    if let Err(error) = increment_metric(metric.0, 1) {
        debug!("Error incrementing {} metric: {:?}", metric.1, error);
    }
}

// Helper method to decrement a metric by 1.
pub fn decrement_stat(metric: &ThreescaleStat) {
    if let Err(error) = increment_metric(metric.0, -1) {
        debug!("Error decrementing {} metric: {:?}", metric.1, error);
    }
}

// Initialize all the stats. With the current implementation of rust-sdk, it's safe to
// directly unwrap define_metric().
pub fn initialize_stats() -> ThreescaleStats {
    ThreescaleStats {
        cached_apps: ThreescaleStat(
            define_metric(MetricType::Counter, "envoy.3scale.cache.apps").unwrap(),
            "envoy.3scale.cache.apps".to_string(),
        ),
        cache_misses: ThreescaleStat(
            define_metric(MetricType::Counter, "envoy.3scale.cache.misses").unwrap(),
            "envoy.3scale.cache.misses".to_string(),
        ),
        cache_hits: ThreescaleStat(
            define_metric(MetricType::Counter, "envoy.3scale.cache.hits").unwrap(),
            "envoy.3scale.cache.hits".to_string(),
        ),
        unauthorized: ThreescaleStat(
            define_metric(MetricType::Counter, "envoy.3scale.cache.unauthorized").unwrap(),
            "envoy.3scale.cache.unauthorized".to_string(),
        ),
        authorize_timeouts: ThreescaleStat(
            define_metric(MetricType::Counter, "envoy.3scale.cache.auth_timeouts").unwrap(),
            "envoy.3scale.cache.timeouts".to_string(),
        ),
        auth_metadata_errors: ThreescaleStat(
            define_metric(
                MetricType::Counter,
                "envoy.3scale.cache.auth_metadata_errors",
            )
            .unwrap(),
            "envoy.3scale.cache.auth_metadata_errors".to_string(),
        ),
    }
}
