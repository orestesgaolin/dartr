// Dart source: pkg/_fe_analyzer_shared/test/flow_analysis/flow_analysis_test.dart
// (lines 7038-7333: group 'Patterns:', group 'Assignment:' and the first
// tests of group 'Cast pattern:', up to 'Match failure unreachable')

//! Dart group `Patterns:` (first part).

use super::common::*;

mod assignment {
    use super::*;

    mod demotion {
        use super::*;

        #[test]
        fn demoting() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_declared_type("int?"),
                x.non_null_assert(),
                check_promoted(x, "int"),
                x.pattern().assign(expr("int?")),
                check_not_promoted(x),
            ]);
        }

        #[test]
        fn non_demoting() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_declared_type("num?"),
                x.non_null_assert(),
                check_promoted(x, "num"),
                x.pattern().assign(expr("int")),
                check_promoted(x, "num"),
            ]);
        }
    }

    mod schema {
        use super::*;

        #[test]
        fn not_promoted() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_declared_type("int?"),
                x.pattern().assign(expr("int").check_schema("int?")),
            ]);
        }

        #[test]
        fn promoted() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_declared_type("int?"),
                x.non_null_assert(),
                check_promoted(x, "int"),
                x.pattern().assign(expr("int").check_schema("int")),
            ]);
        }
    }

    mod promotion {
        use super::*;

        #[test]
        fn type_of_interest() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_declared_type("num"),
                if_(x.is_("int"), vec![]),
                check_not_promoted(x),
                x.pattern().assign(expr("int")),
                check_promoted(x, "int"),
            ]);
        }

        #[test]
        fn not_a_type_of_interest() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_declared_type("num"),
                x.pattern().assign(expr("int")),
                check_not_promoted(x),
            ]);
        }

        #[test]
        fn promotes_matched_value() {
            // The code below is equivalent to:
            //     int x;
            //     (x && _!) = ... as dynamic;
            // There should be an "unnecessary !" warning, because the `x`
            // pattern implicitly promotes the matched value to type `int`.
            let mut h = set_up();
            let x = Var::new("x");
            h.run_with(
                vec![
                    declare(x).with_declared_type("int"),
                    x.pattern()
                        .and(wildcard().null_assert().error_id("NULLASSERT"))
                        .assign(expr("dynamic")),
                ],
                errors(&["matchedTypeIsStrictlyNonNullable(pattern: NULLASSERT, \
                          matchedType: int)"]),
            );
        }

        #[test]
        fn does_not_promote_scrutinee() {
            // The code below is equivalent to:
            //     int x;
            //     dynamic y = ...;
            //     (x && _) = y;
            //     // y is *not* promoted to `int`.
            // Although the assignment to `x` performs an implicit downcast,
            // we don't promote `y` because patterns in irrefutable contexts
            // don't trigger scrutinee promotion.
            let mut h = set_up();
            let x = Var::new("x");
            let y = Var::new("y");
            h.run_with(
                vec![
                    declare(x).with_declared_type("int"),
                    declare(y).with_initializer(expr("dynamic")),
                    x.pattern().and(wildcard().error_id("WILDCARD")).assign(y),
                    check_not_promoted(y),
                ],
                errors(&["unnecessaryWildcardPattern(pattern: WILDCARD, \
                          kind: logicalAndPatternOperand)"]),
            );
        }
    }

    #[test]
    fn definite_assignment() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_declared_type("int"),
            check_assigned(x, false),
            x.pattern().assign(expr("int")),
            check_assigned(x, true),
        ]);
    }

    mod boolean_condition {
        use super::*;

        #[test]
        fn as_main_pattern() {
            let mut h = set_up();
            let b = Var::new("b");
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_declared_type("int?"),
                declare(b).with_declared_type("bool"),
                b.pattern().assign(x.not_eq(null_literal())),
                if_(
                    b,
                    vec![
                        // `x` is promoted because `b` is known to equal
                        // `x != null`.
                        check_promoted(x, "int"),
                    ],
                ),
            ]);
        }

        #[test]
        fn as_parenthesized_pattern() {
            let mut h = set_up();
            let b = Var::new("b");
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_declared_type("int?"),
                declare(b).with_declared_type("bool"),
                b.pattern().parenthesized().assign(x.not_eq(null_literal())),
                if_(
                    b,
                    vec![
                        // `x` is promoted because `b` is known to equal
                        // `x != null`.
                        check_promoted(x, "int"),
                    ],
                ),
            ]);
        }

        #[test]
        fn as_subpattern() {
            let mut h = set_up();
            h.add_member("bool", "foo", Some("bool"), false, None);
            let b = Var::new("b");
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_declared_type("int?"),
                declare(b).with_declared_type("bool"),
                object_pattern("bool", vec![b.pattern().record_field(Some("foo"))])
                    .assign(x.not_eq(null_literal())),
                if_(
                    b,
                    vec![
                        // Even though the RHS of the pattern is `x != null`,
                        // `x` is not promoted because the pattern for `b` is
                        // in a subpattern position.
                        check_not_promoted(x),
                    ],
                ),
            ]);
        }
    }

    mod demonstrated_type {
        use super::*;

        #[test]
        fn subtype_of_matched_value_type() {
            let mut h = set_up();
            let x = Var::new("x");
            let y = Var::new("y");
            h.run_with(
                vec![
                    declare(x).with_initializer(expr("(dynamic,)")),
                    declare(y).with_declared_type("int"),
                    record_pattern(vec![y.pattern().record_field(None)])
                        .and(
                            wildcard()
                                .with_expect_inferred_type("(int,)")
                                .error_id("WILDCARD"),
                        )
                        .assign(x),
                    check_not_promoted(x),
                ],
                errors(&["unnecessaryWildcardPattern(pattern: WILDCARD, \
                          kind: logicalAndPatternOperand)"]),
            );
        }

        #[test]
        fn supertype_of_matched_value_type() {
            let mut h = set_up();
            let x = Var::new("x");
            let y = Var::new("y");
            h.run_with(
                vec![
                    declare(x).with_initializer(expr("(int,)")),
                    declare(y).with_declared_type("num"),
                    record_pattern(vec![y.pattern().record_field(None)])
                        .and(
                            wildcard()
                                .with_expect_inferred_type("(int,)")
                                .error_id("WILDCARD"),
                        )
                        .assign(x),
                    check_not_promoted(x),
                ],
                errors(&["unnecessaryWildcardPattern(pattern: WILDCARD, \
                          kind: logicalAndPatternOperand)"]),
            );
        }
    }
}

mod cast_pattern {
    use super::*;

    #[test]
    fn subtype() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_declared_type("Object?"),
            if_case(
                x,
                wildcard().with_expect_inferred_type("String").as_("String"),
                vec![check_promoted(x, "String")],
                None,
            ),
        ]);
    }

    #[test]
    fn supertype() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run_with(
            vec![
                declare(x).with_declared_type("num"),
                if_case(
                    x,
                    wildcard()
                        .with_expect_inferred_type("Object")
                        .as_("Object")
                        .error_id("PATTERN"),
                    vec![check_not_promoted(x)],
                    None,
                ),
            ],
            errors(&["matchedTypeIsSubtypeOfRequired(pattern: PATTERN, \
                      matchedType: num, requiredType: Object)"]),
        );
    }

    #[test]
    fn unrelated_type() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_declared_type("num"),
            if_case(
                x,
                wildcard().with_expect_inferred_type("String").as_("String"),
                vec![check_not_promoted(x)],
                None,
            ),
        ]);
    }

    #[test]
    fn inner_promotions_have_no_effect() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run_with(
            vec![
                declare(x).with_declared_type("Object?"),
                if_case(
                    x,
                    object_pattern("int", vec![]).as_("num").and(
                        wildcard()
                            .with_expect_inferred_type("num")
                            .error_id("WILDCARD"),
                    ),
                    vec![check_promoted(x, "num")],
                    None,
                ),
            ],
            errors(&["unnecessaryWildcardPattern(pattern: WILDCARD, \
                      kind: logicalAndPatternOperand)"]),
        );
    }

    #[test]
    fn match_failure_unreachable() {
        // Cast patterns don't fail; they throw exceptions. So the "match
        // failure" code path should be unreachable.
        let mut h = set_up();
        h.run(vec![if_case(
            expr("Object?"),
            wildcard().as_("int"),
            vec![check_reachable(true)],
            Some(vec![check_reachable(false)]),
        )]);
    }
}
