// Dart source: pkg/linter/lib/src/rules.dart

use crate::{LinterContext, RuleVisitorRegistry};

mod join_return_with_assignment;
mod library_annotations;
mod library_private_types_in_public_api;
mod literal_only_boolean_expressions;
mod matching_super_parameters;
mod missing_whitespace_between_adjacent_strings;
mod no_default_cases;
mod no_duplicate_case_values;
mod no_dynamic_casts;
mod no_leading_underscores_for_local_identifiers;
mod no_literal_bool_comparisons;
mod no_logic_in_create_state;
mod no_raw_types;
mod no_runtime_type_to_string;
mod no_wildcard_variable_uses;
mod noop_primitive_operations;
mod null_check_on_nullable_type_parameter;
mod null_closures;
mod omit_local_variable_types;
mod omit_obvious_local_variable_types;
mod omit_obvious_property_types;
mod one_member_abstracts;
mod only_throw_errors;
mod overridden_fields;
mod parameter_assignments;
mod prefer_asserts_in_initializer_lists;
mod prefer_collection_literals;

pub const RULES: &[&str] = &[
    "join_return_with_assignment",
    "library_annotations",
    "library_private_types_in_public_api",
    "literal_only_boolean_expressions",
    "matching_super_parameters",
    "missing_whitespace_between_adjacent_strings",
    "no_default_cases",
    "no_duplicate_case_values",
    "no_dynamic_casts",
    "no_leading_underscores_for_local_identifiers",
    "no_literal_bool_comparisons",
    "no_logic_in_create_state",
    "no_raw_types",
    "no_runtimetype_tostring",
    "no_wildcard_variable_uses",
    "noop_primitive_operations",
    "null_check_on_nullable_type_parameter",
    "null_closures",
    "omit_local_variable_types",
    "omit_obvious_local_variable_types",
    "omit_obvious_property_types",
    "one_member_abstracts",
    "only_throw_errors",
    "overridden_fields",
    "parameter_assignments",
    "prefer_asserts_in_initializer_lists",
    "prefer_collection_literals",
];

pub fn register(
    name: &str,
    registry: &mut RuleVisitorRegistry,
    context: &LinterContext<'_>,
) -> bool {
    match name {
        "join_return_with_assignment" => join_return_with_assignment::register(registry),
        "library_annotations" => library_annotations::register(registry),
        "library_private_types_in_public_api" => {
            library_private_types_in_public_api::register(registry)
        }
        "literal_only_boolean_expressions" => literal_only_boolean_expressions::register(registry),
        "matching_super_parameters" => matching_super_parameters::register(registry),
        "missing_whitespace_between_adjacent_strings" => {
            missing_whitespace_between_adjacent_strings::register(registry)
        }
        "no_default_cases" => no_default_cases::register(registry),
        "no_duplicate_case_values" => no_duplicate_case_values::register(registry),
        "no_dynamic_casts" => no_dynamic_casts::register(registry),
        "no_leading_underscores_for_local_identifiers" => {
            no_leading_underscores_for_local_identifiers::register(registry)
        }
        "no_literal_bool_comparisons" => no_literal_bool_comparisons::register(registry),
        "no_logic_in_create_state" => no_logic_in_create_state::register(registry),
        "no_raw_types" => no_raw_types::register(registry),
        "no_runtimetype_tostring" => no_runtime_type_to_string::register(registry),
        "no_wildcard_variable_uses" => no_wildcard_variable_uses::register(registry, context),
        "noop_primitive_operations" => noop_primitive_operations::register(registry),
        "null_check_on_nullable_type_parameter" => {
            null_check_on_nullable_type_parameter::register(registry)
        }
        "null_closures" => null_closures::register(registry),
        "omit_local_variable_types" => omit_local_variable_types::register(registry),
        "omit_obvious_local_variable_types" => {
            omit_obvious_local_variable_types::register(registry)
        }
        "omit_obvious_property_types" => omit_obvious_property_types::register(registry),
        "one_member_abstracts" => one_member_abstracts::register(registry),
        "only_throw_errors" => only_throw_errors::register(registry),
        "overridden_fields" => overridden_fields::register(registry),
        "parameter_assignments" => parameter_assignments::register(registry),
        "prefer_asserts_in_initializer_lists" => {
            prefer_asserts_in_initializer_lists::register(registry)
        }
        "prefer_collection_literals" => prefer_collection_literals::register(registry),
        _ => return false,
    }
    true
}
