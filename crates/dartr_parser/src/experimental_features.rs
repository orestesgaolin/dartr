// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/experimental_features.dart
// Dart source: pkg/_fe_analyzer_shared/lib/src/experiments/errors.dart
// Dart source: pkg/analyzer/lib/src/dart/analysis/experiments_impl.dart (restrictEnableFlagsToVersion)

//! Which experimental features are enabled during parsing.
//!
//! The analyzer creates the parser with `ExperimentalFeaturesStatus(featureSet)`
//! where `featureSet` is the feature set of the file: the feature set of the
//! package (here: the latest language version), restricted to the version of
//! a `// @dart = x.y` comment when the file has one
//! (`Scanner.configureFeatures`, `FeatureSet.restrictToVersion`).
//! [`ExperimentalFeatures::for_language_version`] computes the same set.

use dartr_diagnostics::cfe::CfeMessage;
use dartr_diagnostics::cfe_codes as diag;
use dartr_syntax::ScannerConfiguration;

pub use crate::experimental_flags::{DEFAULT_LANGUAGE_VERSION, ExperimentalFlag};

/// Dart `ExperimentalFeatures`: the set of enabled experimental flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExperimentalFeatures {
    enabled: u64,
}

impl ExperimentalFeatures {
    /// No flag enabled.
    pub const fn none() -> Self {
        ExperimentalFeatures { enabled: 0 }
    }

    /// Dart `FeatureSet.latestLanguageVersion()`: the released features of
    /// the current language version, no experiments.
    pub fn latest() -> Self {
        Self::for_language_version(DEFAULT_LANGUAGE_VERSION.0, DEFAULT_LANGUAGE_VERSION.1, &[])
    }

    /// Dart `restrictEnableFlagsToVersion` (analyzer): the flags enabled for
    /// a library with language version `major.minor`, with the experiments
    /// in [explicit_enabled] enabled on the command line or in
    /// `analysis_options.yaml`.
    pub fn for_language_version(
        major: u32,
        minor: u32,
        explicit_enabled: &[ExperimentalFlag],
    ) -> Self {
        Self::for_language_version_with_sdk(
            major,
            minor,
            explicit_enabled,
            DEFAULT_LANGUAGE_VERSION,
        )
    }

    /// Dart `restrictEnableFlagsToVersion` with an explicit
    /// `sdkLanguageVersion` (`FeatureSet.fromEnableFlags2(sdkLanguageVersion:
    /// ...)`, used by the formatter): explicitly enabled experiments are on
    /// at [sdk_version] and later.
    pub fn for_language_version_with_sdk(
        major: u32,
        minor: u32,
        explicit_enabled: &[ExperimentalFlag],
        sdk_version: (u32, u32),
    ) -> Self {
        let version = (major, minor);
        let mut enabled = 0u64;
        for &flag in ExperimentalFlag::VALUES {
            // The analyzer `releaseVersion` is the CFE
            // `experimentEnabledVersion` of a flag that is enabled by default.
            if flag.is_enabled_by_default() && version >= flag.experiment_enabled_version() {
                enabled |= 1 << flag as u32;
            }
            if explicit_enabled.contains(&flag)
                && (version >= flag.experiment_released_version() || version >= sdk_version)
            {
                enabled |= 1 << flag as u32;
            }
        }
        ExperimentalFeatures { enabled }
    }

    /// Dart `DefaultExperimentalFeatures`: the flags enabled by default.
    pub fn defaults() -> Self {
        let mut enabled = 0u64;
        for &flag in ExperimentalFlag::VALUES {
            if flag.is_enabled_by_default() {
                enabled |= 1 << flag as u32;
            }
        }
        ExperimentalFeatures { enabled }
    }

    /// Returns a copy with [flag] enabled or disabled.
    pub fn with(mut self, flag: ExperimentalFlag, value: bool) -> Self {
        if value {
            self.enabled |= 1 << flag as u32;
        } else {
            self.enabled &= !(1 << flag as u32);
        }
        self
    }

    /// Dart `isExperimentEnabled`.
    #[inline]
    pub fn is_experiment_enabled(&self, flag: ExperimentalFlag) -> bool {
        self.enabled & (1 << flag as u32) != 0
    }

    /// Dart `ExperimentalFeaturesExtension.buildScannerConfiguration`.
    pub fn build_scanner_configuration(&self) -> ScannerConfiguration {
        ScannerConfiguration {
            enable_triple_shift: self.is_experiment_enabled(ExperimentalFlag::TripleShift),
            enable_augmentations: self.is_experiment_enabled(ExperimentalFlag::Augmentations),
        }
    }
}

impl Default for ExperimentalFeatures {
    fn default() -> Self {
        Self::latest()
    }
}

/// Dart `getExperimentNotEnabledMessage`.
pub fn get_experiment_not_enabled_message(experimental_flag: ExperimentalFlag) -> CfeMessage {
    if experimental_flag.is_enabled_by_default() {
        let (major, minor) = experimental_flag.experiment_enabled_version();
        diag::experiment_not_enabled(experimental_flag.name(), &format!("{major}.{minor}"))
    } else {
        diag::experiment_not_enabled_off_by_default(experimental_flag.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_version_restricts_features() {
        let latest = ExperimentalFeatures::latest();
        assert!(latest.is_experiment_enabled(ExperimentalFlag::Patterns));
        assert!(latest.is_experiment_enabled(ExperimentalFlag::PrimaryConstructors));
        assert!(!latest.is_experiment_enabled(ExperimentalFlag::Variance));
        let v2_19 = ExperimentalFeatures::for_language_version(2, 19, &[]);
        assert!(!v2_19.is_experiment_enabled(ExperimentalFlag::Patterns));
        assert!(v2_19.is_experiment_enabled(ExperimentalFlag::TripleShift));
        let v3_12 = ExperimentalFeatures::for_language_version(3, 12, &[]);
        assert!(!v3_12.is_experiment_enabled(ExperimentalFlag::PrimaryConstructors));
    }
}
