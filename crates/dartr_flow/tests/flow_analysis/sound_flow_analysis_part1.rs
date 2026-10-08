// Dart source: pkg/_fe_analyzer_shared/test/flow_analysis/flow_analysis_test.dart
// (lines 10999-12238: group 'Sound flow analysis:', up to but excluding
// group 'Null aware field access:')

//! Dart group `Sound flow analysis:` (first part).

use super::common::*;

/// Dart group `<nonNull> as Null:`.
mod non_null_as_null {
    use super::*;

    #[test]
    fn when_enabled_is_guaranteed_to_throw() {
        let mut h = set_up();
        h.run(vec![expr("int").as_("Null"), check_reachable(false)]);
    }

    #[test]
    fn when_disabled_no_effect() {
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![expr("int").as_("Null"), check_reachable(true)]);
    }
}

/// Dart group `<Null> as <nonNullable>:`.
mod null_as_non_nullable {
    use super::*;

    #[test]
    fn when_enabled_is_guaranteed_to_throw() {
        let mut h = set_up();
        h.run(vec![expr("Null").as_("int"), check_reachable(false)]);
    }

    #[test]
    fn when_disabled_no_effect() {
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![expr("Null").as_("int"), check_reachable(true)]);
    }
}

/// Dart group `<nonNull> is Null:`.
mod non_null_is_null {
    use super::*;

    #[test]
    fn when_enabled_is_guaranteed_false() {
        let mut h = set_up();
        h.run(vec![if_else(
            expr("int").is_("Null"),
            vec![check_reachable(false)],
            vec![check_reachable(true)],
        )]);
    }

    #[test]
    fn when_disabled_no_effect() {
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_else(
            expr("int").is_("Null"),
            vec![check_reachable(true)],
            vec![check_reachable(true)],
        )]);
    }
}

/// Dart group `<nonNull> is! Null:`.
mod non_null_is_not_null {
    use super::*;

    #[test]
    fn when_enabled_is_guaranteed_false() {
        let mut h = set_up();
        h.run(vec![if_else(
            expr("int").is_not("Null"),
            vec![check_reachable(true)],
            vec![check_reachable(false)],
        )]);
    }

    #[test]
    fn when_disabled_no_effect() {
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_else(
            expr("int").is_not("Null"),
            vec![check_reachable(true)],
            vec![check_reachable(true)],
        )]);
    }
}

/// Dart group `<Null> is <nonNullable>:`.
mod null_is_non_nullable {
    use super::*;

    #[test]
    fn when_enabled_is_guaranteed_false() {
        let mut h = set_up();
        h.run(vec![if_else(
            expr("Null").is_("int"),
            vec![check_reachable(false)],
            vec![check_reachable(true)],
        )]);
    }

    #[test]
    fn when_disabled_no_effect() {
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_else(
            expr("Null").is_("int"),
            vec![check_reachable(true)],
            vec![check_reachable(true)],
        )]);
    }
}

/// Dart group `<Null> is! <nonNullable>:`.
mod null_is_not_non_nullable {
    use super::*;

    #[test]
    fn when_enabled_is_guaranteed_false() {
        let mut h = set_up();
        h.run(vec![if_else(
            expr("Null").is_not("int"),
            vec![check_reachable(true)],
            vec![check_reachable(false)],
        )]);
    }

    #[test]
    fn when_disabled_no_effect() {
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_else(
            expr("Null").is_not("int"),
            vec![check_reachable(true)],
            vec![check_reachable(true)],
        )]);
    }
}

/// Dart group `<nonNullable> == <Null>:`.
mod non_nullable_eq_null {
    use super::*;

    #[test]
    fn when_enabled_is_guaranteed_false() {
        let mut h = set_up();
        h.run(vec![if_else(
            expr("int").eq(expr("Null")),
            vec![check_reachable(false)],
            vec![check_reachable(true)],
        )]);
    }

    #[test]
    fn when_disabled_no_effect() {
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_else(
            expr("int").eq(expr("Null")),
            vec![check_reachable(true)],
            vec![check_reachable(true)],
        )]);
    }
}

/// Dart group `<nonNullable> != <Null>:`.
mod non_nullable_not_eq_null {
    use super::*;

    #[test]
    fn when_enabled_is_guaranteed_false() {
        let mut h = set_up();
        h.run(vec![if_else(
            expr("int").not_eq(expr("Null")),
            vec![check_reachable(true)],
            vec![check_reachable(false)],
        )]);
    }

    #[test]
    fn when_disabled_no_effect() {
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_else(
            expr("int").not_eq(expr("Null")),
            vec![check_reachable(true)],
            vec![check_reachable(true)],
        )]);
    }
}

/// Dart group `<Null> == <nonNullable>:`.
mod null_eq_non_nullable {
    use super::*;

    #[test]
    fn when_enabled_is_guaranteed_false() {
        let mut h = set_up();
        h.run(vec![if_else(
            expr("Null").eq(expr("int")),
            vec![check_reachable(false)],
            vec![check_reachable(true)],
        )]);
    }

    #[test]
    fn when_disabled_no_effect() {
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_else(
            expr("Null").eq(expr("int")),
            vec![check_reachable(true)],
            vec![check_reachable(true)],
        )]);
    }
}

/// Dart group `<Null> != <nonNullable>:`.
mod null_not_eq_non_nullable {
    use super::*;

    #[test]
    fn when_enabled_is_guaranteed_false() {
        let mut h = set_up();
        h.run(vec![if_else(
            expr("Null").not_eq(expr("int")),
            vec![check_reachable(true)],
            vec![check_reachable(false)],
        )]);
    }

    #[test]
    fn when_disabled_no_effect() {
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_else(
            expr("Null").not_eq(expr("int")),
            vec![check_reachable(true)],
            vec![check_reachable(true)],
        )]);
    }
}

/// Dart group `<Null> == <Null>:`.
mod null_eq_null {
    use super::*;

    #[test]
    fn when_enabled_is_guaranteed_true() {
        let mut h = set_up();
        h.run(vec![if_else(
            expr("Null").eq(expr("Null")),
            vec![check_reachable(true)],
            vec![check_reachable(false)],
        )]);
    }

    #[test]
    fn when_disabled_is_guaranteed_true() {
        // Flow analysis has considered `<Null> == <Null>` as "guaranteed to be
        // true" since its inception.
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_else(
            expr("Null").eq(expr("Null")),
            vec![check_reachable(true)],
            vec![check_reachable(false)],
        )]);
    }
}

/// Dart group `<Null> != <Null>:`.
mod null_not_eq_null {
    use super::*;

    #[test]
    fn when_enabled_is_guaranteed_false() {
        let mut h = set_up();
        h.run(vec![if_else(
            expr("Null").not_eq(expr("Null")),
            vec![check_reachable(false)],
            vec![check_reachable(true)],
        )]);
    }

    #[test]
    fn when_disabled_is_guaranteed_false() {
        // Flow analysis has considered `<Null> != <Null>` as "guaranteed to be
        // false" since its inception.
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_else(
            expr("Null").not_eq(expr("Null")),
            vec![check_reachable(false)],
            vec![check_reachable(true)],
        )]);
    }
}

/// Dart group `== pattern:`.
mod eq_pattern {
    use super::*;

    #[test]
    fn when_enabled_null_pattern_cant_match_non_nullable_types() {
        let mut h = set_up();
        h.run(vec![if_case(
            expr("int"),
            relational_pattern("==", null_literal()),
            vec![check_reachable(false)],
            Some(vec![check_reachable(true)]),
        )]);
    }

    #[test]
    fn when_disabled_null_pattern_can_even_match_non_nullable_types() {
        // Due to mixed mode unsoundness, attempting to match `null` to a
        // non-nullable type can still succeed, so in order to avoid
        // unsoundness escalation, it's important that the matching case is
        // considered reachable.
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_case(
            expr("int"),
            relational_pattern("==", null_literal()),
            vec![check_reachable(true)],
            Some(vec![check_reachable(true)]),
        )]);
    }
}

/// Dart group `!= pattern:`.
mod not_eq_pattern {
    use super::*;

    #[test]
    fn when_enabled_null_pattern_cant_match_non_nullable_types() {
        let mut h = set_up();
        h.run(vec![if_case(
            expr("int"),
            relational_pattern("!=", null_literal()),
            vec![check_reachable(true)],
            Some(vec![check_reachable(false)]),
        )]);
    }

    #[test]
    fn when_disabled_null_pattern_can_even_match_non_nullable_types() {
        // Due to mixed mode unsoundness, attempting to match `null` to a
        // non-nullable type can still succeed, so in order to avoid
        // unsoundness escalation, it's important that the matching case is
        // considered reachable.
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_case(
            expr("int"),
            relational_pattern("!=", null_literal()),
            vec![check_reachable(true)],
            Some(vec![check_reachable(true)]),
        )]);
    }
}

/// Dart group `null pattern:`.
mod null_pattern {
    use super::*;

    #[test]
    fn when_enabled_null_pattern_cant_match_non_nullable_types() {
        let mut h = set_up();
        h.run(vec![if_case(
            expr("int"),
            null_literal().pattern(),
            vec![check_reachable(false)],
            Some(vec![check_reachable(true)]),
        )]);
    }

    #[test]
    fn when_disabled_null_pattern_can_even_match_non_nullable_types() {
        // Due to mixed mode unsoundness, attempting to match `null` to a
        // non-nullable type can still succeed, so in order to avoid unsoundness
        // escalation, it's important that the matching case is considered
        // reachable.
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_case(
            expr("int"),
            null_literal().pattern(),
            vec![check_reachable(true)],
            Some(vec![check_reachable(true)]),
        )]);
    }
}

/// Dart group `<nonNull>?.foo(<expr>)`.
mod non_null_null_aware_invoke_foo_expr {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_to_execute_expr() {
        let mut h = set_up();
        h.add_member("C", "foo", Some("dynamic"), false, None);
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_declared_type("int"),
            expr("C").invoke_method("foo", nodes![x.write(expr("int"))], true),
            check_assigned(x, true),
        ]);
    }

    #[test]
    fn when_disabled_not_guaranteed_to_execute_expr() {
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.add_member("C", "foo", Some("dynamic"), false, None);
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_declared_type("int"),
            expr("C").invoke_method("foo", nodes![x.write(expr("int"))], true),
            check_assigned(x, false),
        ]);
    }
}

/// Dart group `<nonNull>?..foo(<expr>)`.
mod non_null_null_aware_cascade_foo_expr {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_to_execute_expr() {
        let mut h = set_up();
        h.add_member("C", "foo", Some("dynamic"), false, None);
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_declared_type("int"),
            expr("C").cascade(
                vec![Box::new(move |e: Node| {
                    e.invoke_method("foo", nodes![x.write(expr("int"))], false)
                })],
                true,
            ),
            check_assigned(x, true),
        ]);
    }

    #[test]
    fn when_disabled_not_guaranteed_to_execute_expr() {
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.add_member("C", "foo", Some("dynamic"), false, None);
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_declared_type("int"),
            expr("C").cascade(
                vec![Box::new(move |e: Node| {
                    e.invoke_method("foo", nodes![x.write(expr("int"))], false)
                })],
                true,
            ),
            check_assigned(x, false),
        ]);
    }
}

/// Dart group `<nonNullable> ?? <expr>`.
mod non_nullable_if_null_expr {
    use super::*;

    #[test]
    fn when_enabled_expr_is_dead() {
        let mut h = set_up();
        h.run(vec![expr("int").if_null(check_reachable(false))]);
    }

    #[test]
    fn when_disabled_expr_is_live() {
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![expr("int").if_null(check_reachable(true))]);
    }
}

/// Dart group `{ ?<nonNullable>: <expr> }`.
mod null_aware_map_entry_non_nullable_key {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_to_execute_expr() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_declared_type("int"),
            map_literal(
                vec![map_entry(expr("int"), x.write(expr("int")), true)],
                "dynamic",
                "dynamic",
            ),
            check_assigned(x, true),
        ]);
    }

    #[test]
    fn when_disabled_guaranteed_to_execute_expr() {
        // Flow analysis has considered `{ ?<nonNullable>: <expr> }` as
        // guaranteed to execute <expr> since null-aware map entries were added
        // to the language.
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_declared_type("int"),
            map_literal(
                vec![map_entry(expr("int"), x.write(expr("int")), true)],
                "dynamic",
                "dynamic",
            ),
            check_assigned(x, true),
        ]);
    }
}

/// Dart group `{ ?<Null>: <expr> }`.
mod null_aware_map_entry_null_key {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_to_skip_execution_of_expr() {
        let mut h = set_up();
        h.run(vec![map_literal(
            vec![map_entry(expr("Null"), check_reachable(false), true)],
            "dynamic",
            "dynamic",
        )]);
    }

    #[test]
    fn when_disabled_not_guaranteed_to_skip_execution_of_expr() {
        // Note: it would always have been sound for flow analysis to reason
        // that `{ ?<Null>: <expr> }` was guaranteed to skip execution of
        // `<expr>` (even when flow analysis had to assume that code might be
        // running in unsound null safety mode). But this functionality wasn't
        // implemented. It's been added as part of `sound-flow-analysis`; this
        // test verifies that the old behavior is preserved when
        // `sound-flow-analysis` is disabled.
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![map_literal(
            vec![map_entry(expr("Null"), check_reachable(true), true)],
            "dynamic",
            "dynamic",
        )]);
    }
}

/// Dart group `? pattern applied to non-nullable type`.
mod null_check_pattern_applied_to_non_nullable_type {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_to_match() {
        let mut h = set_up();
        h.run_with(
            vec![if_case(
                expr("int"),
                wildcard().null_check().error_id("nullCheck"),
                vec![check_reachable(true)],
                Some(vec![check_reachable(false)]),
            )],
            errors(&["matchedTypeIsStrictlyNonNullable(pattern: nullCheck, \
                          matchedType: int)"]),
        );
    }

    #[test]
    fn when_disabled_not_guaranteed_to_match() {
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run_with(
            vec![if_case(
                expr("int"),
                wildcard().null_check().error_id("nullCheck"),
                vec![check_reachable(true)],
                Some(vec![check_reachable(true)]),
            )],
            errors(&["matchedTypeIsStrictlyNonNullable(pattern: nullCheck, \
                          matchedType: int)"]),
        );
    }
}

/// Dart group `Map pattern`.
mod map_pattern {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_to_match_non_nullable_map() {
        let mut h = set_up();
        h.run_with(
            vec![if_case(
                expr("Map<int, int>"),
                map_pattern(vec![], None, None).error_id("mapPattern"),
                vec![check_reachable(true)],
                Some(vec![check_reachable(false)]),
            )],
            errors(&["emptyMapPattern(pattern: mapPattern)"]),
        );
    }

    #[test]
    fn when_disabled_not_guaranteed_to_match_non_nullable_map() {
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run_with(
            vec![if_case(
                expr("Map<int, int>"),
                map_pattern(vec![], None, None).error_id("mapPattern"),
                vec![check_reachable(true)],
                Some(vec![check_reachable(true)]),
            )],
            errors(&["emptyMapPattern(pattern: mapPattern)"]),
        );
    }
}

/// Dart group `Null() pattern with non-nullable matched value type`.
mod null_object_pattern_with_non_nullable_matched_value_type {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_not_to_match() {
        let mut h = set_up();
        h.run(vec![if_case(
            expr("int"),
            object_pattern("Null", vec![]),
            vec![check_reachable(false)],
            Some(vec![check_reachable(true)]),
        )]);
    }

    #[test]
    fn when_disabled_might_match() {
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_case(
            expr("int"),
            object_pattern("Null", vec![]),
            vec![check_reachable(true)],
            Some(vec![check_reachable(true)]),
        )]);
    }
}

/// Dart group `Null _ pattern with non-nullable matched value type`.
mod null_wildcard_pattern_with_non_nullable_matched_value_type {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_not_to_match() {
        let mut h = set_up();
        h.run(vec![if_case(
            expr("int"),
            wildcard().with_declared_type("Null"),
            vec![check_reachable(false)],
            Some(vec![check_reachable(true)]),
        )]);
    }

    #[test]
    fn when_disabled_might_match() {
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_case(
            expr("int"),
            wildcard().with_declared_type("Null"),
            vec![check_reachable(true)],
            Some(vec![check_reachable(true)]),
        )]);
    }
}

/// Dart group `Null variable pattern with non-nullable matched value
/// type`.
mod null_variable_pattern_with_non_nullable_matched_value_type {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_not_to_match() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![if_case(
            expr("int"),
            x.pattern().with_declared_type("Null"),
            vec![check_reachable(false)],
            Some(vec![check_reachable(true)]),
        )]);
    }

    #[test]
    fn when_disabled_might_match() {
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        let x = Var::new("x");
        h.run(vec![if_case(
            expr("int"),
            x.pattern().with_declared_type("Null"),
            vec![check_reachable(true)],
            Some(vec![check_reachable(true)]),
        )]);
    }
}

/// Dart group `Non-nullable cast pattern with Null matched value type`.
mod non_nullable_cast_pattern_with_null_matched_value_type {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_not_to_match() {
        let mut h = set_up();
        h.run(vec![if_case(
            expr("Null"),
            wildcard().as_("int"),
            vec![check_reachable(false)],
            Some(vec![check_reachable(false)]),
        )]);
    }

    #[test]
    fn when_disabled_might_match() {
        // Note: it would always have been sound for flow analysis to reason
        // that `_ as int` was guaranteed not to match `Null` (even when flow
        // analysis had to assume that code might be running in unsound null
        // safety mode). But this functionality wasn't implemented. It's been
        // added as part of `sound-flow-analysis`; this test verifies that the
        // old behavior is preserved when `sound-flow-analysis` is disabled.
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_case(
            expr("Null"),
            wildcard().as_("int"),
            vec![check_reachable(true)],
            Some(vec![check_reachable(false)]),
        )]);
    }
}

/// Dart group `Nullable cast pattern with Null matched value type`.
mod nullable_cast_pattern_with_null_matched_value_type {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_to_match() {
        // This is just to check that the logic to handle non-nullable cast
        // patterns is not over-broad.
        let mut h = set_up();
        h.run_with(
            vec![if_case(
                expr("Null"),
                wildcard().as_("int?").error_id("castPattern"),
                vec![check_reachable(true)],
                Some(vec![check_reachable(false)]),
            )],
            errors(&["matchedTypeIsSubtypeOfRequired(pattern: castPattern, \
                          matchedType: Null, requiredType: int?)"]),
        );
    }

    #[test]
    fn when_disabled_guaranteed_to_match() {
        // This is just to check that the logic to handle non-nullable cast
        // patterns is not over-broad.
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run_with(
            vec![if_case(
                expr("Null"),
                wildcard().as_("int?").error_id("castPattern"),
                vec![check_reachable(true)],
                Some(vec![check_reachable(false)]),
            )],
            errors(&["matchedTypeIsSubtypeOfRequired(pattern: castPattern, \
                          matchedType: Null, requiredType: int?)"]),
        );
    }
}

/// Dart group `Null cast pattern with Null matched value type`.
mod null_cast_pattern_with_null_matched_value_type {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_to_match() {
        // This is just to check that the logic to handle non-nullable cast
        // patterns is not over-broad.
        let mut h = set_up();
        h.run_with(
            vec![if_case(
                expr("Null"),
                wildcard().as_("Null").error_id("castPattern"),
                vec![check_reachable(true)],
                Some(vec![check_reachable(false)]),
            )],
            errors(&["matchedTypeIsSubtypeOfRequired(pattern: castPattern, \
                          matchedType: Null, requiredType: Null)"]),
        );
    }

    #[test]
    fn when_disabled_guaranteed_to_match() {
        // This is just to check that the logic to handle non-nullable cast
        // patterns is not over-broad.
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run_with(
            vec![if_case(
                expr("Null"),
                wildcard().as_("Null").error_id("castPattern"),
                vec![check_reachable(true)],
                Some(vec![check_reachable(false)]),
            )],
            errors(&["matchedTypeIsSubtypeOfRequired(pattern: castPattern, \
                          matchedType: Null, requiredType: Null)"]),
        );
    }
}

/// Dart group `Non-nullable variable pattern with Null matched value
/// type`.
mod non_nullable_variable_pattern_with_null_matched_value_type {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_not_to_match() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![if_case(
            expr("Null"),
            x.pattern().with_declared_type("int"),
            vec![check_reachable(false)],
            Some(vec![check_reachable(true)]),
        )]);
    }

    #[test]
    fn when_disabled_might_match() {
        // Note: it would always have been sound for flow analysis to reason
        // that `int x` was guaranteed not to match `Null` (even when flow
        // analysis had to assume that code might be running in unsound null
        // safety mode). But this functionality wasn't implemented. It's been
        // added as part of `sound-flow-analysis`; this test verifies that the
        // old behavior is preserved when `sound-flow-analysis` is disabled.
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        let x = Var::new("x");
        h.run(vec![if_case(
            expr("Null"),
            x.pattern().with_declared_type("int"),
            vec![check_reachable(true)],
            Some(vec![check_reachable(true)]),
        )]);
    }
}

/// Dart group `Nullable variable pattern with Null matched value type`.
mod nullable_variable_pattern_with_null_matched_value_type {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_to_match() {
        // This is just to check that the logic to handle non-nullable variable
        // patterns is not over-broad.
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![if_case(
            expr("Null"),
            x.pattern().with_declared_type("int?"),
            vec![check_reachable(true)],
            Some(vec![check_reachable(false)]),
        )]);
    }

    #[test]
    fn when_disabled_guaranteed_to_match() {
        // This is just to check that the logic to handle non-nullable variable
        // patterns is not over-broad.
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        let x = Var::new("x");
        h.run(vec![if_case(
            expr("Null"),
            x.pattern().with_declared_type("int?"),
            vec![check_reachable(true)],
            Some(vec![check_reachable(false)]),
        )]);
    }
}

/// Dart group `Null variable pattern with Null matched value type`.
mod null_variable_pattern_with_null_matched_value_type {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_to_match() {
        // This is just to check that the logic to handle non-nullable variable
        // patterns is not over-broad.
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![if_case(
            expr("Null"),
            x.pattern().with_declared_type("Null"),
            vec![check_reachable(true)],
            Some(vec![check_reachable(false)]),
        )]);
    }

    #[test]
    fn when_disabled_guaranteed_to_match() {
        // This is just to check that the logic to handle non-nullable variable
        // patterns is not over-broad.
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        let x = Var::new("x");
        h.run(vec![if_case(
            expr("Null"),
            x.pattern().with_declared_type("Null"),
            vec![check_reachable(true)],
            Some(vec![check_reachable(false)]),
        )]);
    }
}

/// Dart group `List pattern with Null matched value type`.
mod list_pattern_with_null_matched_value_type {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_not_to_match() {
        let mut h = set_up();
        h.run(vec![if_case(
            expr("Null"),
            list_pattern(vec![], None),
            vec![check_reachable(false)],
            Some(vec![check_reachable(true)]),
        )]);
    }

    #[test]
    fn when_disabled_might_match() {
        // Note: it would always have been sound for flow analysis to reason
        // that `[]` was guaranteed not to match `Null` (even when flow analysis
        // had to assume that code might be running in unsound null safety
        // mode). But this functionality wasn't implemented. It's been added as
        // part of `sound-flow-analysis`; this test verifies that the old
        // behavior is preserved when `sound-flow-analysis` is disabled.
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_case(
            expr("Null"),
            list_pattern(vec![], None),
            vec![check_reachable(true)],
            Some(vec![check_reachable(true)]),
        )]);
    }
}

/// Dart group `Map pattern with Null matched value type`.
mod map_pattern_with_null_matched_value_type {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_not_to_match() {
        let mut h = set_up();
        h.run_with(
            vec![if_case(
                expr("Null"),
                map_pattern(vec![], None, None).error_id("mapPattern"),
                vec![check_reachable(false)],
                Some(vec![check_reachable(true)]),
            )],
            errors(&["emptyMapPattern(pattern: mapPattern)"]),
        );
    }

    #[test]
    fn when_disabled_might_match() {
        // Note: it would always have been sound for flow analysis to reason
        // that `{}` was guaranteed not to match `Null` (even when flow analysis
        // had to assume that code might be running in unsound null safety
        // mode). But this functionality wasn't implemented. It's been added as
        // part of `sound-flow-analysis`; this test verifies that the old
        // behavior is preserved when `sound-flow-analysis` is disabled.
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run_with(
            vec![if_case(
                expr("Null"),
                map_pattern(vec![], None, None).error_id("mapPattern"),
                vec![check_reachable(true)],
                Some(vec![check_reachable(true)]),
            )],
            errors(&["emptyMapPattern(pattern: mapPattern)"]),
        );
    }
}

/// Dart group `Non-nullable object pattern with Null matched value type`.
mod non_nullable_object_pattern_with_null_matched_value_type {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_not_to_match() {
        let mut h = set_up();
        h.run(vec![if_case(
            expr("Null"),
            object_pattern("int", vec![]),
            vec![check_reachable(false)],
            Some(vec![check_reachable(true)]),
        )]);
    }

    #[test]
    fn when_disabled_might_match() {
        // Note: it would always have been sound for flow analysis to reason
        // that `int()` was guaranteed not to match `Null` (even when flow
        // analysis had to assume that code might be running in unsound null
        // safety mode). But this functionality wasn't implemented. It's been
        // added as part of `sound-flow-analysis`; this test verifies that the
        // old behavior is preserved when `sound-flow-analysis` is disabled.
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_case(
            expr("Null"),
            object_pattern("int", vec![]),
            vec![check_reachable(true)],
            Some(vec![check_reachable(true)]),
        )]);
    }
}

/// Dart group `Nullable object pattern with Null matched value type`.
mod nullable_object_pattern_with_null_matched_value_type {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_to_match() {
        // This is just to check that the logic to handle non-nullable object
        // patterns is not over-broad.
        let mut h = set_up();
        h.run(vec![if_case(
            expr("Null"),
            object_pattern("dynamic", vec![]),
            vec![check_reachable(true)],
            Some(vec![check_reachable(false)]),
        )]);
    }

    #[test]
    fn when_disabled_guaranteed_to_match() {
        // This is just to check that the logic to handle non-nullable object
        // patterns is not over-broad.
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_case(
            expr("Null"),
            object_pattern("dynamic", vec![]),
            vec![check_reachable(true)],
            Some(vec![check_reachable(false)]),
        )]);
    }
}

/// Dart group `Null object pattern with Null matched value type`.
mod null_object_pattern_with_null_matched_value_type {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_to_match() {
        // This is just to check that the logic to handle non-nullable object
        // patterns is not over-broad.
        let mut h = set_up();
        h.run(vec![if_case(
            expr("Null"),
            object_pattern("Null", vec![]),
            vec![check_reachable(true)],
            Some(vec![check_reachable(false)]),
        )]);
    }

    #[test]
    fn when_disabled_guaranteed_to_match() {
        // This is just to check that the logic to handle non-nullable object
        // patterns is not over-broad.
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_case(
            expr("Null"),
            object_pattern("Null", vec![]),
            vec![check_reachable(true)],
            Some(vec![check_reachable(false)]),
        )]);
    }
}

/// Dart group `Record pattern with Null matched value type`.
mod record_pattern_with_null_matched_value_type {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_not_to_match() {
        let mut h = set_up();
        h.run(vec![if_case(
            expr("Null"),
            record_pattern(vec![wildcard().record_field(None)]),
            vec![check_reachable(false)],
            Some(vec![check_reachable(true)]),
        )]);
    }

    #[test]
    fn when_disabled_might_match() {
        // Note: it would always have been sound for flow analysis to reason
        // that `(_,)` was guaranteed not to match `Null` (even when flow
        // analysis had to assume that code might be running in unsound null
        // safety mode). But this functionality wasn't implemented. It's been
        // added as part of `sound-flow-analysis`; this test verifies that the
        // old behavior is preserved when `sound-flow-analysis` is disabled.
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_case(
            expr("Null"),
            record_pattern(vec![wildcard().record_field(None)]),
            vec![check_reachable(true)],
            Some(vec![check_reachable(true)]),
        )]);
    }
}

/// Dart group `Non-nullable wildcard pattern with Null matched value
/// type`.
mod non_nullable_wildcard_pattern_with_null_matched_value_type {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_not_to_match() {
        let mut h = set_up();
        h.run(vec![if_case(
            expr("Null"),
            wildcard().with_declared_type("int"),
            vec![check_reachable(false)],
            Some(vec![check_reachable(true)]),
        )]);
    }

    #[test]
    fn when_disabled_might_match() {
        // Note: it would always have been sound for flow analysis to reason
        // that `int _` was guaranteed not to match `Null` (even when flow
        // analysis had to assume that code might be running in unsound null
        // safety mode). But this functionality wasn't implemented. It's been
        // added as part of `sound-flow-analysis`; this test verifies that the
        // old behavior is preserved when `sound-flow-analysis` is disabled.
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_case(
            expr("Null"),
            wildcard().with_declared_type("int"),
            vec![check_reachable(true)],
            Some(vec![check_reachable(true)]),
        )]);
    }
}

/// Dart group `Nullable wildcard pattern with Null matched value type`.
mod nullable_wildcard_pattern_with_null_matched_value_type {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_to_match() {
        // This is just to check that the logic to handle non-nullable wildcard
        // patterns is not over-broad.
        let mut h = set_up();
        h.run(vec![if_case(
            expr("Null"),
            wildcard().with_declared_type("int?"),
            vec![check_reachable(true)],
            Some(vec![check_reachable(false)]),
        )]);
    }

    #[test]
    fn when_disabled_guaranteed_to_match() {
        // This is just to check that the logic to handle non-nullable wildcard
        // patterns is not over-broad.
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_case(
            expr("Null"),
            wildcard().with_declared_type("int?"),
            vec![check_reachable(true)],
            Some(vec![check_reachable(false)]),
        )]);
    }
}

/// Dart group `Null wildcard pattern with Null matched value type`.
mod null_wildcard_pattern_with_null_matched_value_type {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_to_match() {
        // This is just to check that the logic to handle non-nullable wildcard
        // patterns is not over-broad.
        let mut h = set_up();
        h.run(vec![if_case(
            expr("Null"),
            wildcard().with_declared_type("Null"),
            vec![check_reachable(true)],
            Some(vec![check_reachable(false)]),
        )]);
    }

    #[test]
    fn when_disabled_guaranteed_to_match() {
        // This is just to check that the logic to handle non-nullable wildcard
        // patterns is not over-broad.
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_case(
            expr("Null"),
            wildcard().with_declared_type("Null"),
            vec![check_reachable(true)],
            Some(vec![check_reachable(false)]),
        )]);
    }
}

/// Dart group `Declared variable pattern with matching non-nullable
/// types`.
mod declared_variable_pattern_with_matching_non_nullable_types {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_to_match() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![if_case(
            expr("int"),
            x.pattern().with_declared_type("int"),
            vec![check_reachable(true)],
            Some(vec![check_reachable(false)]),
        )]);
    }

    #[test]
    fn when_disabled_guaranteed_to_match() {
        // Flow analysis has considered `int x` as guaranteed to match a value
        // with static type `int` since patterns were added to the language
        // (even though that was not technically guaranteed to be the case when
        // running in unsound null safety mode).
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        let x = Var::new("x");
        h.run(vec![if_case(
            expr("int"),
            x.pattern().with_declared_type("int"),
            vec![check_reachable(true)],
            Some(vec![check_reachable(false)]),
        )]);
    }
}

/// Dart group `List pattern`.
mod list_pattern {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_to_match_non_nullable_list() {
        let mut h = set_up();
        h.run(vec![if_case(
            expr("List<int>"),
            list_pattern(vec![rest_pattern(None)], None),
            vec![check_reachable(true)],
            Some(vec![check_reachable(false)]),
        )]);
    }

    #[test]
    fn when_disabled_guaranteed_to_match_non_nullable_list() {
        // Flow analysis has considered a list pattern as guaranteed to match a
        // value with static type `List` since patterns were added to the
        // language (even though that was not technically guaranteed to be the
        // case when running in unsound null safety mode).
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_case(
            expr("List<int>"),
            list_pattern(vec![rest_pattern(None)], None),
            vec![check_reachable(true)],
            Some(vec![check_reachable(false)]),
        )]);
    }
}

/// Dart group `Object pattern with matching non-nullable types`.
mod object_pattern_with_matching_non_nullable_types {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_to_match() {
        let mut h = set_up();
        h.run(vec![if_case(
            expr("int"),
            object_pattern("int", vec![]),
            vec![check_reachable(true)],
            Some(vec![check_reachable(false)]),
        )]);
    }

    #[test]
    fn when_disabled_guaranteed_to_match() {
        // Flow analysis has considered an object pattern as guaranteed to match
        // a value with a matching static type since patterns were added to the
        // language (even though that was not technically guaranteed to be the
        // case when running in unsound null safety mode).
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_case(
            expr("int"),
            object_pattern("int", vec![]),
            vec![check_reachable(true)],
            Some(vec![check_reachable(false)]),
        )]);
    }
}

/// Dart group `Record pattern with matching non-nullable type`.
mod record_pattern_with_matching_non_nullable_type {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_to_match() {
        let mut h = set_up();
        h.run(vec![if_case(
            expr("(int,)"),
            record_pattern(vec![
                wildcard().with_declared_type("int").record_field(None),
            ]),
            vec![check_reachable(true)],
            Some(vec![check_reachable(false)]),
        )]);
    }

    #[test]
    fn when_disabled_guaranteed_to_match() {
        // Flow analysis has considered a record pattern as guaranteed to match
        // a value with a matching static type since patterns were added to the
        // language (even though that was not technically guaranteed to be the
        // case when running in unsound null safety mode).
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_case(
            expr("(int,)"),
            record_pattern(vec![
                wildcard().with_declared_type("int").record_field(None),
            ]),
            vec![check_reachable(true)],
            Some(vec![check_reachable(false)]),
        )]);
    }
}

/// Dart group `Wildcard pattern with matching non-nullable types`.
mod wildcard_pattern_with_matching_non_nullable_types {
    use super::*;

    #[test]
    fn when_enabled_guaranteed_to_match() {
        let mut h = set_up();
        h.run(vec![if_case(
            expr("int"),
            wildcard().with_declared_type("int"),
            vec![check_reachable(true)],
            Some(vec![check_reachable(false)]),
        )]);
    }

    #[test]
    fn when_disabled_guaranteed_to_match() {
        // Flow analysis has considered `int _` as guaranteed to match a value
        // with static type `int` since patterns were added to the language
        // (even though that was not technically guaranteed to be the case when
        // running in unsound null safety mode).
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        h.run(vec![if_case(
            expr("int"),
            wildcard().with_declared_type("int"),
            vec![check_reachable(true)],
            Some(vec![check_reachable(false)]),
        )]);
    }
}
