//! Decoding of `enable-experiment` flags (`decodeExplicitFlags` and
//! `restrictEnableFlagsToVersion` of
//! `pkg/analyzer/lib/src/dart/analysis/experiments_impl.dart`).

#[path = "experiments_table.rs"]
mod table;

pub use table::{CURRENT_LANGUAGE_VERSION, ExperimentalFeature, KNOWN_FEATURES};

/// Returns the feature with the flag [enable_string].
pub fn feature(enable_string: &str) -> Option<&'static ExperimentalFeature> {
    KNOWN_FEATURES
        .iter()
        .find(|f| f.enable_string == enable_string)
}

/// Returns the experimental features (not enabled by default, not expired)
/// that the flags enable. A `no-` prefix disables a feature; the last flag for
/// a feature wins; unknown and expired flags are ignored. The result is
/// sorted.
pub fn enabled_experiments(flags: &[String]) -> Vec<&'static str> {
    let mut requested: Vec<(&'static str, bool)> = Vec::new();
    for flag in flags {
        let (name, value) = match flag.strip_prefix("no-") {
            Some(name) => (name, false),
            None => (flag.as_str(), true),
        };
        if let Some(feature) = feature(name)
            && !feature.is_expired
        {
            requested.retain(|(n, _)| *n != feature.enable_string);
            requested.push((feature.enable_string, value));
        }
    }
    let mut result: Vec<&'static str> = requested
        .into_iter()
        .filter(|(name, value)| *value && feature(name).is_some_and(|f| !f.is_enabled_by_default))
        .map(|(name, _)| name)
        .collect();
    result.sort();
    result
}
