// Dart source: pkg/linter/lib/src/rules.dart

use crate::{LinterContext, RuleVisitorRegistry};

mod always_put_required_named_parameters_first;
mod always_specify_types;
mod analyzer_element_model_tracking;
mod analyzer_public_api;
mod annotate_overrides;
mod annotate_redeclares;
mod async_return_with_no_await;
mod avoid_bool_literals_in_conditional_expressions;
mod avoid_catches_without_on_clauses;
mod avoid_catching_errors;
mod avoid_classes_with_only_static_members;
mod avoid_double_and_int_checks;
mod avoid_dynamic_calls;
mod avoid_equals_and_hash_code_on_mutable_classes;
mod avoid_field_initializers_in_const_classes;
mod avoid_function_literals_in_foreach_calls;
mod avoid_futureor_void;
mod avoid_implementing_value_types;
mod avoid_init_to_null;
mod avoid_null_checks_in_equality_operators;
mod avoid_positional_boolean_parameters;
mod avoid_print;
mod avoid_redundant_argument_values;
mod avoid_renaming_method_parameters;
mod avoid_returning_null_for_void;
mod avoid_returning_this;
mod avoid_setters_without_getters;
mod avoid_slow_async_io;
mod helpers;

pub const RULES: &[&str] = &[
    "always_put_required_named_parameters_first",
    "always_specify_types",
    "analyzer_element_model_tracking",
    "analyzer_public_api",
    "annotate_overrides",
    "annotate_redeclares",
    "async_return_with_no_await",
    "avoid_bool_literals_in_conditional_expressions",
    "avoid_catches_without_on_clauses",
    "avoid_catching_errors",
    "avoid_classes_with_only_static_members",
    "avoid_double_and_int_checks",
    "avoid_dynamic_calls",
    "avoid_equals_and_hash_code_on_mutable_classes",
    "avoid_field_initializers_in_const_classes",
    "avoid_function_literals_in_foreach_calls",
    "avoid_futureor_void",
    "avoid_implementing_value_types",
    "avoid_init_to_null",
    "avoid_null_checks_in_equality_operators",
    "avoid_positional_boolean_parameters",
    "avoid_print",
    "avoid_redundant_argument_values",
    "avoid_renaming_method_parameters",
    "avoid_returning_null_for_void",
    "avoid_returning_this",
    "avoid_setters_without_getters",
    "avoid_slow_async_io",
];

pub fn register(
    name: &str,
    registry: &mut RuleVisitorRegistry,
    context: &LinterContext<'_>,
) -> bool {
    match name {
        "always_put_required_named_parameters_first" => {
            always_put_required_named_parameters_first::register(registry, context)
        }
        "always_specify_types" => always_specify_types::register(registry, context),
        "analyzer_element_model_tracking" => {
            analyzer_element_model_tracking::register(registry, context)
        }
        "analyzer_public_api" => analyzer_public_api::register(registry, context),
        "annotate_overrides" => annotate_overrides::register(registry, context),
        "annotate_redeclares" => annotate_redeclares::register(registry, context),
        "async_return_with_no_await" => async_return_with_no_await::register(registry, context),
        "avoid_bool_literals_in_conditional_expressions" => {
            avoid_bool_literals_in_conditional_expressions::register(registry, context)
        }
        "avoid_catches_without_on_clauses" => {
            avoid_catches_without_on_clauses::register(registry, context)
        }
        "avoid_catching_errors" => avoid_catching_errors::register(registry, context),
        "avoid_classes_with_only_static_members" => {
            avoid_classes_with_only_static_members::register(registry, context)
        }
        "avoid_double_and_int_checks" => avoid_double_and_int_checks::register(registry, context),
        "avoid_dynamic_calls" => avoid_dynamic_calls::register(registry, context),
        "avoid_equals_and_hash_code_on_mutable_classes" => {
            avoid_equals_and_hash_code_on_mutable_classes::register(registry, context)
        }
        "avoid_field_initializers_in_const_classes" => {
            avoid_field_initializers_in_const_classes::register(registry, context)
        }
        "avoid_function_literals_in_foreach_calls" => {
            avoid_function_literals_in_foreach_calls::register(registry, context)
        }
        "avoid_futureor_void" => avoid_futureor_void::register(registry, context),
        "avoid_implementing_value_types" => {
            avoid_implementing_value_types::register(registry, context)
        }
        "avoid_init_to_null" => avoid_init_to_null::register(registry, context),
        "avoid_null_checks_in_equality_operators" => {
            avoid_null_checks_in_equality_operators::register(registry, context)
        }
        "avoid_positional_boolean_parameters" => {
            avoid_positional_boolean_parameters::register(registry, context)
        }
        "avoid_print" => avoid_print::register(registry, context),
        "avoid_redundant_argument_values" => {
            avoid_redundant_argument_values::register(registry, context)
        }
        "avoid_renaming_method_parameters" => {
            avoid_renaming_method_parameters::register(registry, context)
        }
        "avoid_returning_null_for_void" => {
            avoid_returning_null_for_void::register(registry, context)
        }
        "avoid_returning_this" => avoid_returning_this::register(registry, context),
        "avoid_setters_without_getters" => {
            avoid_setters_without_getters::register(registry, context)
        }
        "avoid_slow_async_io" => avoid_slow_async_io::register(registry, context),
        _ => return false,
    }
    true
}
