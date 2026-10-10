// Dart source: pkg/linter/lib/src/rules.dart
//! Lint batch B: the rules that need resolution, ported from
//! `pkg/linter/lib/src/rules/<rule>.dart`. Each module has
//! `register(registry, context)`.

use crate::{LinterContext, RuleVisitorRegistry};

pub(crate) mod container_rules;
pub(crate) mod flutter;
pub(crate) mod util;

macro_rules! rules {
    ($($rule:ident),* $(,)?) => {
        $(mod $rule;)*
        pub const RULES: &[&str] = &[$(stringify!($rule)),*];
        pub fn register(
            name: &str,
            registry: &mut RuleVisitorRegistry,
            context: &LinterContext<'_>,
        ) -> bool {
            match name {
                $(stringify!($rule) => {
                    $rule::register(registry, context);
                    true
                })*
                _ => false,
            }
        }
    };
}

rules!(
    provide_deprecation_message,
    prefer_mixin,
    unnecessary_to_list_in_spreads,
    use_rethrow_when_possible,
    unnecessary_null_aware_assignments,
    unawaited_futures,
    unnecessary_string_interpolations,
    use_test_throws_matchers,
    prefer_function_declarations_over_variables,
    type_literal_in_constant_pattern,
    unnecessary_unawaited,
    prefer_const_declarations,
    use_full_hex_values_for_flutter_colors,
    valid_regexps,
    type_init_formals,
    use_if_null_to_convert_nulls_to_bools,
    use_truncating_division,
    prefer_relative_imports,
    sort_child_properties_last,
    sort_unnamed_constructors_first,
    use_decorated_box,
    sized_box_for_whitespace,
    use_colored_box,
    prefer_is_not_empty,
    prefer_iterable_wheretype,
    use_setters_to_change_properties,
    use_to_and_as_if_applicable,
    prefer_conditional_assignment,
    use_is_even_rather_than_modulo,
    use_named_constants,
    unnecessary_constructor_name,
    unnecessary_await_in_return,
    unnecessary_getters_setters,
    recursive_getters,
    prefer_interpolation_to_compose_strings,
    prefer_constructors_over_static_methods,
    prefer_final_parameters,
    prefer_const_literals_to_create_immutables,
    sized_box_shrink_expand,
    prefer_for_elements_to_map_fromiterable,
    unrelated_type_equality_checks,
    prefer_int_literals,
    unnecessary_nullable_for_final_variable_declarations,
    prefer_final_in_for_each,
    tighten_type_of_initializing_formals,
    prefer_const_constructors,
    unnecessary_null_aware_operator_on_extension_on_nullable,
    specify_nonobvious_property_types,
    prefer_foreach,
    simplify_variable_pattern,
    use_key_in_widget_constructors,
    unnecessary_underscores,
    prefer_void_to_null,
    use_null_aware_elements,
    use_primary_constructors,
    specify_nonobvious_local_variable_types,
    prefer_const_constructors_in_immutables,
    switch_on_type,
    use_string_buffers,
    prefer_contains,
    type_annotate_public_apis,
    void_checks,
    prefer_final_fields,
    unnecessary_statements,
    unnecessary_null_checks,
    prefer_final_locals,
    prefer_is_empty,
    use_enums,
);
