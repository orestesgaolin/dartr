// Dart sources: pkg/linter/lib/src/rules/{avoid_type_to_string,
// avoid_types_as_parameter_names,avoid_types_on_closure_parameters,
// avoid_unnecessary_containers,avoid_unused_constructor_parameters,
// avoid_void_async,await_only_futures,cancel_subscriptions,cascade_invocations,
// cast_nullable_to_non_nullable,close_sinks,collection_methods_unrelated_type,
// comment_references,control_flow_in_finally,deprecated_consistency,
// deprecated_member_use_from_same_package,diagnostic_describe_all_properties,
// discarded_futures,do_not_use_environment,erase_dart_type_extension_types,
// exhaustive_cases,hash_and_equals,implementation_imports,
// implicit_call_tearoffs,implicit_reopen,initialize_in_field_declaration,
// invalid_case_patterns,invalid_runtime_check_with_js_interop_types}.dart

use crate::{LinterContext, RuleVisitorRegistry};

mod avoid_type_to_string;
mod avoid_types_as_parameter_names;
mod avoid_types_on_closure_parameters;
mod avoid_unnecessary_containers;
mod avoid_unused_constructor_parameters;
mod avoid_void_async;
mod await_only_futures;
mod cancel_subscriptions;
mod cascade_invocations;
mod cast_nullable_to_non_nullable;
mod close_sinks;
mod collection_methods_unrelated_type;
mod comment_references;
mod control_flow_in_finally;
mod deprecated_consistency;
mod deprecated_member_use_from_same_package;
mod diagnostic_describe_all_properties;
mod discarded_futures;
mod do_not_use_environment;
mod erase_dart_type_extension_types;
mod exhaustive_cases;
mod hash_and_equals;
mod helpers;
mod implementation_imports;
mod implicit_call_tearoffs;
mod implicit_reopen;
mod initialize_in_field_declaration;
mod invalid_case_patterns;
mod invalid_runtime_check_with_js_interop_types;
mod leak_detector;

pub const RULES: &[&str] = &[
    "avoid_type_to_string",
    "avoid_types_as_parameter_names",
    "avoid_types_on_closure_parameters",
    "avoid_unnecessary_containers",
    "avoid_unused_constructor_parameters",
    "avoid_void_async",
    "await_only_futures",
    "cancel_subscriptions",
    "cascade_invocations",
    "cast_nullable_to_non_nullable",
    "close_sinks",
    "collection_methods_unrelated_type",
    "comment_references",
    "control_flow_in_finally",
    "deprecated_consistency",
    "deprecated_member_use_from_same_package",
    "diagnostic_describe_all_properties",
    "discarded_futures",
    "do_not_use_environment",
    "erase_dart_type_extension_types",
    "exhaustive_cases",
    "hash_and_equals",
    "implementation_imports",
    "implicit_call_tearoffs",
    "implicit_reopen",
    "initialize_in_field_declaration",
    "invalid_case_patterns",
    "invalid_runtime_check_with_js_interop_types",
];

pub fn register(
    name: &str,
    registry: &mut RuleVisitorRegistry,
    context: &LinterContext<'_>,
) -> bool {
    match name {
        "avoid_type_to_string" => avoid_type_to_string::register(registry, context),
        "avoid_types_as_parameter_names" => {
            avoid_types_as_parameter_names::register(registry, context)
        }
        "avoid_types_on_closure_parameters" => {
            avoid_types_on_closure_parameters::register(registry, context)
        }
        "avoid_unnecessary_containers" => avoid_unnecessary_containers::register(registry, context),
        "avoid_unused_constructor_parameters" => {
            avoid_unused_constructor_parameters::register(registry, context)
        }
        "avoid_void_async" => avoid_void_async::register(registry, context),
        "await_only_futures" => await_only_futures::register(registry, context),
        "cancel_subscriptions" => cancel_subscriptions::register(registry, context),
        "cascade_invocations" => cascade_invocations::register(registry, context),
        "cast_nullable_to_non_nullable" => {
            cast_nullable_to_non_nullable::register(registry, context)
        }
        "close_sinks" => close_sinks::register(registry, context),
        "collection_methods_unrelated_type" => {
            collection_methods_unrelated_type::register(registry, context)
        }
        "comment_references" => comment_references::register(registry, context),
        "control_flow_in_finally" => control_flow_in_finally::register(registry, context),
        "deprecated_consistency" => deprecated_consistency::register(registry, context),
        "deprecated_member_use_from_same_package" => {
            deprecated_member_use_from_same_package::register(registry, context)
        }
        "diagnostic_describe_all_properties" => {
            diagnostic_describe_all_properties::register(registry, context)
        }
        "discarded_futures" => discarded_futures::register(registry, context),
        "do_not_use_environment" => do_not_use_environment::register(registry, context),
        "erase_dart_type_extension_types" => {
            erase_dart_type_extension_types::register(registry, context)
        }
        "exhaustive_cases" => exhaustive_cases::register(registry, context),
        "hash_and_equals" => hash_and_equals::register(registry, context),
        "implementation_imports" => implementation_imports::register(registry, context),
        "implicit_call_tearoffs" => implicit_call_tearoffs::register(registry, context),
        "implicit_reopen" => implicit_reopen::register(registry, context),
        "initialize_in_field_declaration" => {
            initialize_in_field_declaration::register(registry, context)
        }
        "invalid_case_patterns" => invalid_case_patterns::register(registry, context),
        "invalid_runtime_check_with_js_interop_types" => {
            invalid_runtime_check_with_js_interop_types::register(registry, context)
        }
        _ => return false,
    }
    true
}
