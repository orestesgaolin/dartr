// Dart source: pkg/_fe_analyzer_shared/test/flow_analysis/flow_analysis_test.dart
// (lines 9667-10407: group 'Patterns:', groups 'Switch expression:' and
// 'Switch statement:')

//! Dart group `Patterns:` (third part, second file).

use super::common::*;

mod switch_expression {
    use super::*;

    #[test]
    fn guarded() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_declared_type("int?"),
            switch_expr(
                expr("Object"),
                vec![
                    wildcard().when(x.not_eq(null_literal())).then_expr(second(
                        list_literal(
                            vec![check_reachable(true), check_promoted(x, "int")],
                            "dynamic",
                        ),
                        expr("String"),
                    )),
                    wildcard().then_expr(second(
                        list_literal(
                            vec![check_reachable(true), check_not_promoted(x)],
                            "dynamic",
                        ),
                        expr("String"),
                    )),
                ],
            ),
        ]);
    }

    mod guard_promotes_later_cases {
        use super::*;

        #[test]
        fn when_pattern_fully_covers_the_scrutinee_type() {
            // `case _ when x == null:` promotes `x` to non-null in later cases,
            // because the implicit type of `_` fully covers the scrutinee type.
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_declared_type("int?"),
                switch_expr(
                    expr("Object?"),
                    vec![
                        wildcard()
                            .when(x.eq(null_literal()))
                            .then_expr(int_literal(0)),
                        wildcard().then_expr(second(check_promoted(x, "int"), int_literal(1))),
                    ],
                ),
            ]);
        }

        #[test]
        fn when_pattern_does_not_fully_cover_the_scrutinee_type() {
            // `case String _ when x == null:` does not promote `y` to non-null in
            // later cases, because the type `String` does not fully cover the
            // scrutinee type.
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_declared_type("int?"),
                switch_expr(
                    expr("Object?"),
                    vec![
                        wildcard()
                            .with_declared_type("String")
                            .when(x.eq(null_literal()))
                            .then_expr(int_literal(0)),
                        wildcard().then_expr(second(
                            list_literal(vec![check_not_promoted(x)], "dynamic"),
                            int_literal(1),
                        )),
                    ],
                ),
            ]);
        }
    }

    #[test]
    fn promotes_scrutinee() {
        let mut h = set_up();
        let x = Var::new("x");
        let y = Var::new("y");
        h.run(vec![
            declare(x).with_declared_type("num"),
            switch_expr(
                x,
                vec![
                    y.pattern().with_declared_type("int").then_expr(second(
                        list_literal(
                            vec![check_reachable(true), check_promoted(x, "int")],
                            "dynamic",
                        ),
                        expr("String"),
                    )),
                    wildcard().then_expr(second(
                        list_literal(
                            vec![check_reachable(true), check_not_promoted(x)],
                            "dynamic",
                        ),
                        expr("String"),
                    )),
                ],
            ),
        ]);
    }

    #[test]
    fn reassigned_scrutinee_var_no_longer_promotes() {
        let mut h = set_up();
        let x = Var::new("x");
        // Note that the second `wildcard(type: 'int')` doesn't promote `x`
        // because it's been reassigned.  But it does still promote the
        // scrutinee in the RHS of the `&&`.
        h.run_with(
            vec![
                declare(x).with_initializer(expr("Object")),
                switch_expr(
                    x,
                    vec![
                        wildcard()
                            .with_declared_type("int")
                            .and(
                                wildcard()
                                    .with_expect_inferred_type("int")
                                    .error_id("WILDCARD1"),
                            )
                            .then_expr(second(check_promoted(x, "int"), int_literal(0))),
                        wildcard()
                            .when(second(x.write(expr("Object")), expr("bool")))
                            .then_expr(int_literal(1)),
                        wildcard()
                            .with_declared_type("int")
                            .and(
                                wildcard()
                                    .with_expect_inferred_type("int")
                                    .error_id("WILDCARD2"),
                            )
                            .then_expr(second(check_not_promoted(x), int_literal(2))),
                        wildcard().then_expr(int_literal(3)),
                    ],
                ),
            ],
            errors(&[
                "unnecessaryWildcardPattern(pattern: WILDCARD1, \
                     kind: logicalAndPatternOperand)",
                "unnecessaryWildcardPattern(pattern: WILDCARD2, \
                     kind: logicalAndPatternOperand)",
            ]),
        );
    }

    #[test]
    fn cached_scrutinee_retains_promoted_type_even_if_scrutinee_var_reassigned() {
        let mut h = set_up();
        let x = Var::new("x");
        let y = Var::new("y");
        // `x` is promoted at the time the scrutinee is cached.  Therefore, even
        // though `case _ where f(x = ...)` de-promotes `x`, the promoted type
        // is still used for type inference in the later `case var y`.
        h.run(vec![
            declare(x).with_initializer(expr("Object")),
            x.as_("int"),
            check_promoted(x, "int"),
            switch_expr(
                x,
                vec![
                    wildcard()
                        .when(second(x.write(expr("Object")), expr("bool")))
                        .then_expr(int_literal(0)),
                    y.pattern()
                        .with_expect_inferred_type("int")
                        .then_expr(second(check_not_promoted(x), int_literal(1))),
                ],
            ),
        ]);
    }

    #[test]
    fn no_cases() {
        let mut h = set_up();
        h.run(vec![switch_expr(expr("A"), vec![]), check_reachable(false)]);
    }

    #[test]
    fn error_type_does_not_make_following_cases_unreachable() {
        // We don't know the correct type, so recover by expecting that the
        // following cases still will be useful once the error is fixed.
        let mut h = set_up();
        h.run(vec![switch_expr(
            expr("num"),
            vec![
                wildcard()
                    .with_declared_type("error")
                    .then_expr(second(check_reachable(true), int_literal(0))),
                wildcard().then_expr(second(check_reachable(true), int_literal(1))),
            ],
        )]);
    }
}

mod switch_statement {
    use super::*;

    #[test]
    fn guarded() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_declared_type("int?"),
            switch_(
                expr("Object"),
                vec![
                    switch_statement_member(
                        vec![wildcard().when(x.not_eq(null_literal()))],
                        vec![check_reachable(true), check_promoted(x, "int")],
                        false,
                    ),
                    switch_statement_member(
                        vec![default_()],
                        vec![check_reachable(true), check_not_promoted(x)],
                        false,
                    ),
                ],
            ),
        ]);
    }

    mod guard_promotes_later_cases {
        use super::*;

        #[test]
        fn when_pattern_fully_covers_the_scrutinee_type() {
            // `case _ when x == null:` promotes `x` to non-null in later cases,
            // because the implicit type of `_` fully covers the scrutinee type.
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_declared_type("int?"),
                switch_(
                    expr("Object?"),
                    vec![
                        wildcard()
                            .when(x.eq(null_literal()))
                            .then(vec![break_(None)]),
                        wildcard().then(vec![check_promoted(x, "int")]),
                    ],
                ),
            ]);
        }

        #[test]
        fn when_pattern_does_not_fully_cover_the_scrutinee_type() {
            // `case String _ when x == null:` does not promote `x` to non-null in
            // later cases, because the type `String` does not fully cover the
            // scrutinee type.
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_declared_type("int?"),
                switch_(
                    expr("Object?"),
                    vec![
                        wildcard()
                            .with_declared_type("String")
                            .when(x.eq(null_literal()))
                            .then(vec![break_(None)]),
                        wildcard().then(vec![check_not_promoted(x)]),
                    ],
                ),
            ]);
        }
    }

    #[test]
    fn promotes_scrutinee() {
        let mut h = set_up();
        let x = Var::new("x");
        let y = Var::new("y");
        h.run(vec![
            declare(x).with_declared_type("num"),
            switch_(
                x,
                vec![
                    switch_statement_member(
                        vec![y.pattern().with_declared_type("int")],
                        vec![check_reachable(true), check_promoted(x, "int")],
                        false,
                    ),
                    switch_statement_member(
                        vec![default_()],
                        vec![check_reachable(true), check_not_promoted(x)],
                        false,
                    ),
                ],
            ),
        ]);
    }

    #[test]
    fn implicit_break() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_declared_type("Object"),
            switch_(
                expr("Object"),
                vec![
                    switch_statement_member(
                        vec![wildcard().with_declared_type("int")],
                        vec![x.as_("int")],
                        false,
                    ),
                    switch_statement_member(vec![default_()], vec![return_()], false),
                ],
            ),
            check_reachable(true),
            check_promoted(x, "int"),
        ]);
    }

    mod exhaustiveness {
        use super::*;

        #[test]
        fn exhaustive() {
            let mut h = set_up();
            h.add_exhaustiveness("E", true);
            h.run(vec![
                switch_(
                    expr("E"),
                    vec![switch_statement_member(
                        vec![expr("E").pattern()],
                        vec![return_()],
                        false,
                    )],
                ),
                check_reachable(false),
            ]);
        }

        #[test]
        fn non_exhaustive() {
            let mut h = set_up();
            h.run(vec![
                switch_(
                    expr("int"),
                    vec![switch_statement_member(
                        vec![int_literal(0).pattern()],
                        vec![return_()],
                        false,
                    )],
                ),
                check_reachable(true),
            ]);
        }
    }

    mod pre_patterns_exhaustiveness {
        use super::*;

        #[test]
        fn exhaustive() {
            let mut h = set_up();
            h.disable_patterns();
            h.run(vec![
                switch_(
                    expr("E"),
                    vec![switch_statement_member(
                        vec![expr("E").pattern()],
                        vec![return_()],
                        false,
                    )],
                )
                .with_legacy_exhaustive(true),
                check_reachable(false),
            ]);
        }

        #[test]
        fn non_exhaustive() {
            let mut h = set_up();
            h.disable_patterns();
            h.run(vec![
                switch_(
                    expr("E"),
                    vec![switch_statement_member(
                        vec![expr("E").pattern()],
                        vec![return_()],
                        false,
                    )],
                )
                .with_legacy_exhaustive(false),
                check_reachable(true),
            ]);
        }
    }

    #[test]
    fn empty_exhaustive() {
        // This can happen if a class is marked `sealed` but has no subclasses.
        // Note that exhaustiveness checking of "always exhaustive" types is
        // deferred until a later analysis stage (so that it can take constant
        // evaluation into account), so flow analysis simply assumes that the
        // switch is exhaustive without checking, and sets the
        // `requiresExhaustivenessValidation` flag to let the client know that
        // exhaustiveness checking must be performed later.  Had this been a
        // real compilation (and not just a unit test), exhaustiveness checking
        // would later confirm that the class `C` has no subclasses, or report
        // a compile-time error.
        let mut h = set_up();
        h.add_exhaustiveness("C", true);
        h.run(vec![
            switch_(expr("C"), vec![]).expect_requires_exhaustiveness_validation(true),
            check_reachable(false),
        ]);
    }

    mod nested {
        use super::*;

        #[test]
        fn scrutinee_type() {
            // Verify that the inner switch's matched value type doesn't bleed out
            // to the next case in the outer switch.
            let mut h = set_up();
            h.run(vec![switch_(
                expr("int"),
                vec![
                    wildcard()
                        .with_expect_inferred_type("int")
                        .when(expr("bool"))
                        .then(vec![switch_(
                            expr("String"),
                            vec![wildcard().with_expect_inferred_type("String").then(vec![])],
                        )]),
                    wildcard().with_expect_inferred_type("int").then(vec![]),
                ],
            )]);
        }

        #[test]
        fn scrutinee_reference() {
            // Verify that the inner switch's scrutinee reference is properly
            // distinguished from the outer switch's scrutinee reference.
            let mut h = set_up();
            let x = Var::new("x");
            let y = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("Object")),
                declare(y).with_initializer(expr("Object")),
                switch_(
                    x,
                    vec![
                        wildcard().with_declared_type("num").then(vec![
                            check_promoted(x, "num"),
                            check_not_promoted(y),
                            switch_(
                                y,
                                vec![
                                    wildcard().with_declared_type("int").then(vec![
                                        check_promoted(x, "num"),
                                        check_promoted(y, "int"),
                                    ]),
                                    default_().then(vec![return_()]),
                                ],
                            ),
                            check_promoted(x, "num"),
                            check_promoted(y, "int"),
                        ]),
                        wildcard()
                            .with_declared_type("String")
                            .then(vec![check_promoted(x, "String"), check_not_promoted(y)]),
                    ],
                ),
            ]);
        }
    }

    #[test]
    fn reassigned_scrutinee_var_no_longer_promotes() {
        let mut h = set_up();
        let x = Var::new("x");
        // Note that the second `wildcard(type: 'int')` doesn't promote `x`
        // because it's been reassigned.  But it does still promote the
        // scrutinee in the RHS of the `&&`.
        h.run_with(
            vec![
                declare(x).with_initializer(expr("Object")),
                switch_(
                    x,
                    vec![
                        wildcard()
                            .with_declared_type("int")
                            .and(
                                wildcard()
                                    .with_expect_inferred_type("int")
                                    .error_id("WILDCARD1"),
                            )
                            .then(vec![check_promoted(x, "int")]),
                        wildcard()
                            .when(second(x.write(expr("Object")), expr("bool")))
                            .then(vec![break_(None)]),
                        wildcard()
                            .with_declared_type("int")
                            .and(
                                wildcard()
                                    .with_expect_inferred_type("int")
                                    .error_id("WILDCARD2"),
                            )
                            .then(vec![check_not_promoted(x)]),
                    ],
                ),
            ],
            errors(&[
                "unnecessaryWildcardPattern(pattern: WILDCARD1, \
                     kind: logicalAndPatternOperand)",
                "unnecessaryWildcardPattern(pattern: WILDCARD2, \
                     kind: logicalAndPatternOperand)",
            ]),
        );
    }

    #[test]
    fn cached_scrutinee_retains_promoted_type_even_if_scrutinee_var_reassigned() {
        let mut h = set_up();
        let x = Var::new("x");
        let y = Var::new("y");
        // `x` is promoted at the time the scrutinee is cached.  Therefore, even
        // though `case _ where f(x = ...)` de-promotes `x`, the promoted type
        // is still used for type inference in the later `case var y`.
        h.run(vec![
            declare(x).with_initializer(expr("Object")),
            x.as_("int"),
            check_promoted(x, "int"),
            switch_(
                x,
                vec![
                    wildcard()
                        .when(second(x.write(expr("Object")), expr("bool")))
                        .then(vec![break_(None)]),
                    y.pattern()
                        .with_expect_inferred_type("int")
                        .then(vec![check_not_promoted(x)]),
                ],
            ),
        ]);
    }

    #[test]
    fn synthetic_break_inserted_even_in_unreachable_cases() {
        // In this example, the second case is unreachable, so technically it
        // doesn't matter whether it ends in a synthetic break.  However, to
        // avoid confusion on the part of the CFE and back-end developers, we go
        // ahead and put in the synthetic break anyhow.
        let mut h = set_up();
        h.run(vec![
            switch_(
                expr("Object"),
                vec![
                    wildcard().then(vec![int_literal(0)]),
                    wildcard().then(vec![int_literal(1)]),
                ],
            )
            .check_ir(
                "switch(expr(Object), \
                     case(heads(head(wildcardPattern(matchedType: Object), true, \
                     variables()), variables()), block(stmt(0), synthetic-break())), \
                     case(heads(head(wildcardPattern(matchedType: Object), true, \
                     variables()), variables()), \
                     block(stmt(1), synthetic-break())))",
            ),
        ]);
    }

    #[test]
    fn error_type_does_not_make_following_cases_unreachable() {
        // We don't know the correct type, so recover by expecting that the
        // following cases still will be useful once the error is fixed.
        let mut h = set_up();
        h.run(vec![switch_(
            expr("num"),
            vec![
                wildcard()
                    .with_declared_type("error")
                    .then(vec![check_reachable(true)]),
                wildcard().then(vec![check_reachable(true)]),
            ],
        )]);
    }

    mod joins_promotions_of_scrutinee {
        use super::*;

        #[test]
        fn first_case_more_promoted() {
            let mut h = set_up();
            let x = Var::new("x");
            // ` case num() && int(): case num():` retains promotion to `num`
            h.run(vec![
                declare(x).with_initializer(expr("Object")),
                switch_(
                    x,
                    vec![switch_statement_member(
                        vec![
                            object_pattern("num", vec![]).and(object_pattern("int", vec![])),
                            object_pattern("num", vec![]),
                        ],
                        vec![check_promoted(x, "num")],
                        false,
                    )],
                ),
            ]);
        }

        #[test]
        fn second_case_more_promoted() {
            let mut h = set_up();
            let x = Var::new("x");
            // `case num(): case num() && int():` retains promotion to `num`
            h.run(vec![
                declare(x).with_initializer(expr("Object")),
                switch_(
                    x,
                    vec![switch_statement_member(
                        vec![
                            object_pattern("num", vec![]),
                            object_pattern("num", vec![]).and(object_pattern("int", vec![])),
                        ],
                        vec![check_promoted(x, "num")],
                        false,
                    )],
                ),
            ]);
        }
    }

    mod joins_explicitly_declared_variables {
        use super::*;

        #[test]
        fn first_var_promoted() {
            let mut h = set_up();
            let x1 = Var::new("x").with_identity("x1");
            let x2 = Var::new("x").with_identity("x2");
            let x = Var::join("x", vec![x1, x2]);
            h.run(vec![switch_(
                expr("(int, int?)"),
                vec![switch_statement_member(
                    vec![
                        record_pattern(vec![
                            int_literal(0).pattern().record_field(None),
                            x1.pattern()
                                .with_declared_type("int?")
                                .null_check()
                                .record_field(None),
                        ]),
                        record_pattern(vec![
                            int_literal(1).pattern().record_field(None),
                            x2.pattern().with_declared_type("int?").record_field(None),
                        ]),
                    ],
                    vec![check_not_promoted(x)],
                    false,
                )],
            )]);
        }

        #[test]
        fn second_var_promoted() {
            let mut h = set_up();
            let x1 = Var::new("x").with_identity("x1");
            let x2 = Var::new("x").with_identity("x2");
            let x = Var::join("x", vec![x1, x2]);
            h.run(vec![switch_(
                expr("(int, int?)"),
                vec![switch_statement_member(
                    vec![
                        record_pattern(vec![
                            int_literal(0).pattern().record_field(None),
                            x1.pattern().with_declared_type("int?").record_field(None),
                        ]),
                        record_pattern(vec![
                            int_literal(1).pattern().record_field(None),
                            x2.pattern()
                                .with_declared_type("int?")
                                .null_check()
                                .record_field(None),
                        ]),
                    ],
                    vec![check_not_promoted(x)],
                    false,
                )],
            )]);
        }

        #[test]
        fn both_vars_promoted() {
            let mut h = set_up();
            let x1 = Var::new("x").with_identity("x1");
            let x2 = Var::new("x").with_identity("x2");
            let x = Var::join("x", vec![x1, x2]);
            h.run(vec![switch_(
                expr("int?"),
                vec![switch_statement_member(
                    vec![
                        x1.pattern().with_declared_type("int?").null_check(),
                        x2.pattern().with_declared_type("int?").null_check(),
                    ],
                    vec![check_promoted(x, "int")],
                    false,
                )],
            )]);
        }

        #[test]
        fn promoted_via_when_clause() {
            // Equivalent Dart code:
            //     switch (... as (int, int?)) {
            //       case (0, int? x?):
            //       case (1, int? x) where x != null:
            //         x; // Should be promoted to non-null
            //     }
            let mut h = set_up();
            let x1 = Var::new("x").with_identity("x1");
            let x2 = Var::new("x").with_identity("x2");
            let x = Var::join("x", vec![x1, x2]);
            h.run(vec![switch_(
                expr("(int, int?)"),
                vec![switch_statement_member(
                    vec![
                        record_pattern(vec![
                            int_literal(0).pattern().record_field(None),
                            x1.pattern()
                                .with_declared_type("int?")
                                .null_check()
                                .record_field(None),
                        ]),
                        record_pattern(vec![
                            int_literal(1).pattern().record_field(None),
                            x2.pattern().with_declared_type("int?").record_field(None),
                        ])
                        .when(x2.not_eq(null_literal())),
                    ],
                    vec![check_promoted(x, "int")],
                    false,
                )],
            )]);
        }

        #[test]
        fn complex_example() {
            // This is based on the code sample from
            // https://github.com/dart-lang/sdk/issues/51644, except that the type
            // of the scrutinee has been changed from `dynamic` to `Object?`.
            let mut h = set_up();
            let a1 = Var::new("a").with_identity("a1");
            let a2 = Var::new("a").with_identity("a2");
            let a3 = Var::new("a").with_identity("a3");
            let a = Var::join("a", vec![a1, a2, a3]);
            h.run(vec![switch_(
                expr("Object?"),
                vec![switch_statement_member(
                    vec![
                        a1.pattern()
                            .with_declared_type("String?")
                            .null_check()
                            .when(a1.is_("Never")),
                        a2.pattern()
                            .with_declared_type("String?")
                            .when(a2.not_eq(null_literal())),
                        a3.pattern()
                            .with_declared_type("String?")
                            .null_assert()
                            .when(a3.eq(int_literal(1))),
                    ],
                    vec![check_promoted(a, "String")],
                    false,
                )],
            )]);
        }

        #[test]
        fn join_variable_is_promotable() {
            let mut h = set_up();
            let x1 = Var::new("x").with_identity("x1");
            let x2 = Var::new("x").with_identity("x2");
            let x = Var::join("x", vec![x1, x2]);
            h.run(vec![switch_(
                expr("int?"),
                vec![switch_statement_member(
                    vec![
                        x1.pattern().with_declared_type("int?").null_check(),
                        x2.pattern().with_declared_type("int?"),
                    ],
                    vec![
                        check_not_promoted(x),
                        x.non_null_assert(),
                        check_promoted(x, "int"),
                    ],
                    false,
                )],
            )]);
        }
    }

    mod sets_join_variable_assigned_even_if_variable_doesnt_appear_in_every_case {
        use super::*;

        #[test]
        fn variable_in_first_case_only() {
            let mut h = set_up();
            let x1 = Var::new("x").with_identity("x1");
            let x = Var::join("x", vec![x1]);
            // `x` is considered assigned inside the case body (even though it's
            // not actually assigned by both patterns) because this avoids
            // redundant errors.
            h.run(vec![switch_(
                expr("num?"),
                vec![switch_statement_member(
                    vec![x1.pattern().null_check(), wildcard()],
                    vec![
                        check_assigned(x, true),
                        // Also verify that the join variable is promotable
                        check_not_promoted(x),
                        x.as_("int"),
                        check_promoted(x, "int"),
                    ],
                    false,
                )],
            )]);
        }

        #[test]
        fn variable_in_second_case_only() {
            let mut h = set_up();
            let x1 = Var::new("x").with_identity("x1");
            let x = Var::join("x", vec![x1]);
            // `x` is considered assigned inside the case body (even though it's
            // not actually assigned by both patterns) because this avoids
            // redundant errors.
            h.run(vec![switch_(
                expr("int?"),
                vec![switch_statement_member(
                    vec![wildcard().null_check(), x1.pattern()],
                    vec![
                        check_assigned(x, true),
                        // Also verify that the join variable is promotable
                        check_not_promoted(x),
                        x.non_null_assert(),
                        check_promoted(x, "int"),
                    ],
                    false,
                )],
            )]);
        }
    }

    mod trivial_exhaustiveness {
        // Although flow analysis doesn't attempt to do full exhaustiveness
        // checking on switch statements, it understands that if any single case
        // fully covers the matched value type, the switch statement is
        // exhaustive.  (Such a switch is called "trivially exhaustive").
        //
        // Note that we don't test all possible patterns, because the flow
        // analysis logic for detecting trivial exhaustiveness builds on the
        // logic for tracking the "unmatched" state, which is tested elsewhere.
        use super::*;

        #[test]
        fn exhaustive() {
            let mut h = set_up();
            h.run(vec![
                switch_(expr("Object"), vec![wildcard().then(vec![return_()])]),
                check_reachable(false),
            ]);
        }

        #[test]
        fn exhaustive_but_a_reachable_switch_case_completes() {
            // In this case, even though the switch is trivially exhaustive, the
            // code after the switch is reachable because one of the reachable
            // switch cases completes normally.
            let mut h = set_up();
            h.run(vec![
                switch_(
                    expr("Object"),
                    vec![
                        wildcard()
                            .with_declared_type("int")
                            .then(vec![check_reachable(true)]),
                        wildcard().then(vec![return_()]),
                    ],
                ),
                check_reachable(true),
            ]);
        }

        #[test]
        fn exhaustive_but_an_unreachable_switch_case_completes() {
            // In this case, even though the `int` case completes normally, that
            // case is unreachable, so the code after the switch is unreachable.
            let mut h = set_up();
            h.run(vec![
                switch_(
                    expr("Object"),
                    vec![
                        wildcard().then(vec![return_()]),
                        wildcard()
                            .with_declared_type("int")
                            .then(vec![check_reachable(false)]),
                    ],
                ),
                check_reachable(false),
            ]);
        }

        #[test]
        fn exhaustive_but_a_reachable_switch_case_breaks() {
            // In this case, even though the switch is trivially exhaustive, the
            // code after the switch is reachable because one of the reachable
            // switch cases ends in a break.
            let mut h = set_up();
            h.run(vec![
                switch_(
                    expr("Object"),
                    vec![
                        wildcard()
                            .with_declared_type("int")
                            .then(vec![check_reachable(true), break_(None)]),
                        wildcard().then(vec![return_()]),
                    ],
                ),
                check_reachable(true),
            ]);
        }

        #[test]
        fn exhaustive_but_an_unreachable_switch_case_breaks() {
            // In this case, even though the `int` case breaks, that case is
            // unreachable, so the code after the switch is unreachable.
            let mut h = set_up();
            h.run(vec![
                switch_(
                    expr("Object"),
                    vec![
                        wildcard().then(vec![return_()]),
                        wildcard()
                            .with_declared_type("int")
                            .then(vec![check_reachable(false), break_(None)]),
                    ],
                ),
                check_reachable(false),
            ]);
        }

        #[test]
        fn not_exhaustive() {
            let mut h = set_up();
            h.run(vec![
                switch_(
                    expr("Object"),
                    vec![wildcard().with_declared_type("int").then(vec![return_()])],
                ),
                check_reachable(true),
            ]);
        }
    }
}
