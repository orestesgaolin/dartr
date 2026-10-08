//! Metadata for registered lint rules in the pinned Dart SDK.
//!
//! Extracted from `pkg/linter/lib/src/rules/*.dart` at Dart SDK 3.13.3.
//! To audit an SDK update, list state declarations with
//! `rg 'RemovedAnalysisRule|state:.*(deprecated|removed)'` and list conflict
//! declarations with `rg -l 'incompatibleRules'`, then compare every matching
//! rule file with the two tables below. Keep this in sync with `lint_rules.rs`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuleStateKind {
    Deprecated,
    Removed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RuleState {
    pub(crate) name: &'static str,
    pub(crate) kind: RuleStateKind,
    pub(crate) since: Option<(u16, u16, u16)>,
    pub(crate) replacement: Option<&'static str>,
}

pub(crate) const RULE_STATES: &[RuleState] = &[
    RuleState {
        name: "always_require_non_null_named_parameters",
        kind: RuleStateKind::Removed,
        since: Some((3, 3, 0)),
        replacement: None,
    },
    RuleState {
        name: "avoid_as",
        kind: RuleStateKind::Removed,
        since: Some((2, 12, 0)),
        replacement: None,
    },
    RuleState {
        name: "avoid_null_checks_in_equality_operators",
        kind: RuleStateKind::Deprecated,
        since: Some((3, 11, 0)),
        replacement: None,
    },
    RuleState {
        name: "avoid_private_typedef_functions",
        kind: RuleStateKind::Deprecated,
        since: Some((3, 13, 0)),
        replacement: None,
    },
    RuleState {
        name: "avoid_returning_null",
        kind: RuleStateKind::Removed,
        since: Some((3, 3, 0)),
        replacement: None,
    },
    RuleState {
        name: "avoid_returning_null_for_future",
        kind: RuleStateKind::Removed,
        since: Some((3, 3, 0)),
        replacement: None,
    },
    RuleState {
        name: "avoid_unstable_final_fields",
        kind: RuleStateKind::Removed,
        since: None,
        replacement: None,
    },
    RuleState {
        name: "enable_null_safety",
        kind: RuleStateKind::Removed,
        since: Some((3, 0, 0)),
        replacement: None,
    },
    RuleState {
        name: "invariant_booleans",
        kind: RuleStateKind::Removed,
        since: Some((3, 0, 0)),
        replacement: None,
    },
    RuleState {
        name: "iterable_contains_unrelated_type",
        kind: RuleStateKind::Removed,
        since: Some((3, 3, 0)),
        replacement: None,
    },
    RuleState {
        name: "list_remove_unrelated_type",
        kind: RuleStateKind::Removed,
        since: Some((3, 3, 0)),
        replacement: None,
    },
    RuleState {
        name: "one_member_abstracts",
        kind: RuleStateKind::Deprecated,
        since: Some((3, 13, 0)),
        replacement: None,
    },
    RuleState {
        name: "package_api_docs",
        kind: RuleStateKind::Removed,
        since: Some((3, 7, 0)),
        replacement: None,
    },
    RuleState {
        name: "prefer_bool_in_asserts",
        kind: RuleStateKind::Removed,
        since: Some((3, 0, 0)),
        replacement: None,
    },
    RuleState {
        name: "prefer_equal_for_default_values",
        kind: RuleStateKind::Removed,
        since: Some((3, 0, 0)),
        replacement: None,
    },
    RuleState {
        name: "prefer_final_parameters",
        kind: RuleStateKind::Deprecated,
        since: Some((3, 11, 0)),
        replacement: None,
    },
    RuleState {
        name: "super_goes_last",
        kind: RuleStateKind::Removed,
        since: Some((3, 0, 0)),
        replacement: None,
    },
    RuleState {
        name: "unnecessary_await_in_return",
        kind: RuleStateKind::Deprecated,
        since: Some((3, 13, 0)),
        replacement: None,
    },
    RuleState {
        name: "unsafe_html",
        kind: RuleStateKind::Removed,
        since: Some((3, 7, 0)),
        replacement: None,
    },
    RuleState {
        name: "use_if_null_to_convert_nulls_to_bools",
        kind: RuleStateKind::Deprecated,
        since: Some((3, 11, 0)),
        replacement: None,
    },
];

pub(crate) fn rule_state(name: &str) -> Option<&'static RuleState> {
    RULE_STATES.iter().find(|state| state.name == name)
}

pub(crate) fn incompatible_rules(name: &str) -> &'static [&'static str] {
    match name {
        "always_specify_types" => &[
            "avoid_types_on_closure_parameters",
            "omit_local_variable_types",
            "omit_obvious_local_variable_types",
            "omit_obvious_property_types",
        ],
        "always_use_package_imports" => &["prefer_relative_imports"],
        "avoid_final_parameters" => &["prefer_final_parameters"],
        "avoid_types_on_closure_parameters" => &["always_specify_types"],
        "omit_local_variable_types" => &[
            "always_specify_types",
            "specify_nonobvious_local_variable_types",
        ],
        "omit_obvious_local_variable_types" => &["always_specify_types"],
        "omit_obvious_property_types" => &["always_specify_types", "type_annotate_public_apis"],
        "prefer_double_quotes" => &["prefer_single_quotes"],
        "prefer_final_in_for_each" => &["unnecessary_final"],
        "prefer_final_locals" => &["unnecessary_final"],
        "prefer_final_parameters" => &["unnecessary_final", "avoid_final_parameters"],
        "prefer_relative_imports" => &["always_use_package_imports"],
        "prefer_single_quotes" => &["prefer_double_quotes"],
        "specify_nonobvious_local_variable_types" => &["omit_local_variable_types"],
        "type_annotate_public_apis" => &["omit_obvious_property_types"],
        "unnecessary_final" => &[
            "prefer_final_locals",
            "prefer_final_parameters",
            "prefer_final_in_for_each",
        ],
        _ => &[],
    }
}
