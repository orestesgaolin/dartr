// Experimental features of the pinned SDK.
//
// Generated from `pkg/analyzer/lib/src/dart/analysis/experiments.g.dart`
// (`EnableString`, `IsEnabledByDefault`, `IsExpired`). Do not edit.

/// The current language version (`ExperimentStatus.currentVersion`).
pub const CURRENT_LANGUAGE_VERSION: crate::package_config::LanguageVersion =
    crate::package_config::LanguageVersion::new(3, 13);

/// An experimental feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExperimentalFeature {
    /// The flag of `enable-experiment`.
    pub enable_string: &'static str,
    pub is_enabled_by_default: bool,
    pub is_expired: bool,
}

/// All known experimental features, in the order of the analyzer.
pub const KNOWN_FEATURES: &[ExperimentalFeature] = &[
    ExperimentalFeature {
        enable_string: "anonymous-methods",
        is_enabled_by_default: false,
        is_expired: false,
    },
    ExperimentalFeature {
        enable_string: "augmentations",
        is_enabled_by_default: false,
        is_expired: false,
    },
    ExperimentalFeature {
        enable_string: "class-modifiers",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "const-functions",
        is_enabled_by_default: false,
        is_expired: false,
    },
    ExperimentalFeature {
        enable_string: "constant-update-2018",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "constructor-tearoffs",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "control-flow-collections",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "data-assets",
        is_enabled_by_default: false,
        is_expired: false,
    },
    ExperimentalFeature {
        enable_string: "digit-separators",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "dot-shorthands",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "enhanced-enums",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "enhanced-parts",
        is_enabled_by_default: false,
        is_expired: false,
    },
    ExperimentalFeature {
        enable_string: "extension-methods",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "generic-metadata",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "getter-setter-error",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "inference-update-1",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "inference-update-2",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "inference-update-3",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "inference-update-4",
        is_enabled_by_default: false,
        is_expired: false,
    },
    ExperimentalFeature {
        enable_string: "inference-using-bounds",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "inline-class",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "macros",
        is_enabled_by_default: false,
        is_expired: false,
    },
    ExperimentalFeature {
        enable_string: "named-arguments-anywhere",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "native-assets",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "non-nullable",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "nonfunction-type-aliases",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "null-aware-elements",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "patterns",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "primary-constructors",
        is_enabled_by_default: true,
        is_expired: false,
    },
    ExperimentalFeature {
        enable_string: "private-named-parameters",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "record-use",
        is_enabled_by_default: true,
        is_expired: false,
    },
    ExperimentalFeature {
        enable_string: "records",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "sealed-class",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "set-literals",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "sound-flow-analysis",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "spread-collections",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "static-extensions",
        is_enabled_by_default: false,
        is_expired: false,
    },
    ExperimentalFeature {
        enable_string: "super-parameters",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "test-experiment",
        is_enabled_by_default: false,
        is_expired: false,
    },
    ExperimentalFeature {
        enable_string: "this-promotion",
        is_enabled_by_default: false,
        is_expired: false,
    },
    ExperimentalFeature {
        enable_string: "triple-shift",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "unnamed-libraries",
        is_enabled_by_default: true,
        is_expired: true,
    },
    ExperimentalFeature {
        enable_string: "unquoted-imports",
        is_enabled_by_default: false,
        is_expired: false,
    },
    ExperimentalFeature {
        enable_string: "variance",
        is_enabled_by_default: false,
        is_expired: false,
    },
    ExperimentalFeature {
        enable_string: "wildcard-variables",
        is_enabled_by_default: true,
        is_expired: true,
    },
];
