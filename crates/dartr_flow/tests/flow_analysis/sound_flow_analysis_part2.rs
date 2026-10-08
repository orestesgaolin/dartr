// Dart source: pkg/_fe_analyzer_shared/test/flow_analysis/flow_analysis_test.dart
// (lines 12239-12882: group 'Sound flow analysis:', from group 'Null aware
// field access:' to the end of the group)

//! Dart group `Sound flow analysis:` (second part).

use super::common::*;

mod null_aware_field_access {
    use super::*;

    mod non_cascaded {
        use super::*;

        #[test]
        fn when_disabled_does_not_see_previous_promotions() {
            let mut h = set_up();
            h.disable_sound_flow_analysis();
            h.add_member("A", "_i", Some("int?"), true, None);
            let a = Var::new("a");
            h.run(vec![
                declare(a).with_initializer(expr("A")),
                a.property("_i", false).non_null_assert(),
                // `a._i` is promoted now.
                a.property("_i", false).check_type("int"),
                // But `a?._i` is not.
                a.property("_i", true).check_type("int?"),
            ]);
        }

        #[test]
        fn when_enabled_sees_previous_promotions() {
            let mut h = set_up();
            h.add_member("A", "_i", Some("int?"), true, None);
            let a = Var::new("a");
            h.run(vec![
                declare(a).with_initializer(expr("A")),
                a.property("_i", false).non_null_assert(),
                // `a._i` is promoted now.
                a.property("_i", false).check_type("int"),
                // And so is `a?._i`.
                a.property("_i", true).check_type("int"),
            ]);
        }

        #[test]
        fn when_disabled_cannot_promote() {
            let mut h = set_up();
            h.disable_sound_flow_analysis();
            h.add_member("A", "_i", Some("int?"), true, None);
            let a = Var::new("a");
            h.run(vec![
                declare(a).with_initializer(expr("A")),
                a.property("_i", true).non_null_assert(),
                // `a._i` is not promoted.
                a.property("_i", false).check_type("int?"),
                // But had the field access not been null aware, it would have
                // been promoted.
                a.property("_i", false).non_null_assert(),
                a.property("_i", false).check_type("int"),
            ]);
        }

        #[test]
        fn when_enabled_can_promote() {
            let mut h = set_up();
            h.add_member("A", "_i", Some("int?"), true, None);
            let a = Var::new("a");
            h.run(vec![
                declare(a).with_initializer(expr("A")),
                a.property("_i", false).check_type("int?"),
                a.property("_i", true).non_null_assert(),
                // `a._i` is promoted.
                a.property("_i", false).check_type("int"),
            ]);
        }

        #[test]
        fn in_conditional_expression() {
            let mut h = set_up();
            h.add_member("A", "_i", Some("int?"), true, None);
            let a = Var::new("a");
            h.run(vec![
                declare(a).with_initializer(expr("A")),
                expr("bool").conditional(null_literal(), a.property("_i", true)),
            ]);
        }
    }

    mod cascaded {
        use super::*;

        #[test]
        fn when_disabled_sees_previous_promotions() {
            let mut h = set_up();
            h.disable_sound_flow_analysis();
            h.add_member("A", "_i", Some("int?"), true, None);
            let a = Var::new("a");
            h.run(vec![
                declare(a).with_initializer(expr("A")),
                a.property("_i", false).non_null_assert(),
                // `a._i` is promoted now.
                a.property("_i", false).check_type("int"),
                // And `a?.._i` is promoted.
                a.cascade(
                    vec![Box::new(|placeholder: Node| {
                        placeholder.property("_i", false).check_type("int")
                    })],
                    true,
                ),
            ]);
        }

        #[test]
        fn when_enabled_sees_previous_promotions() {
            let mut h = set_up();
            h.add_member("A", "_i", Some("int?"), true, None);
            let a = Var::new("a");
            h.run(vec![
                declare(a).with_initializer(expr("A")),
                a.property("_i", false).non_null_assert(),
                // `a._i` is promoted now.
                a.property("_i", false).check_type("int"),
                // And `a?.._i` is promoted.
                a.cascade(
                    vec![Box::new(|placeholder: Node| {
                        placeholder.property("_i", false).check_type("int")
                    })],
                    true,
                ),
            ]);
        }
    }
}

mod when_disabled_may_promote_to_mutual_subtypes {
    use super::*;

    #[test]
    fn type_cast() {
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("List<Object?>")),
            x.as_("List<dynamic>"),
            check_promoted(x, "List<dynamic>"),
        ]);
    }

    #[test]
    fn type_check() {
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("List<Object?>")),
            if_(x.is_not("List<dynamic>"), vec![return_()]),
            check_promoted(x, "List<dynamic>"),
        ]);
    }

    #[test]
    fn type_of_interest_promotion() {
        // Note: to work around the fact that a full demotion clears types of
        // interest (see https://github.com/dart-lang/language/issues/4380),
        // this test starts with a variable of type `dynamic` and promotes it
        // first to `List<Object?>?` and then to `List<dynamic>`. This ensures
        // that the write that follows (which writes a value of type
        // `List<Object?>?`) does not fully demote the variable, so the types of
        // interest will be preserved.
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("dynamic")),
            x.as_("List<Object?>?"),
            // `x` is now promoted to `List<Object?>?` and `List<Object?>` is a
            // type of interest
            check_promoted(x, "List<Object?>?"),
            x.as_("List<dynamic>"),
            // `x` is now promoted to `List<dynamic>` and `List<dynamic>` is a
            // type of interest.
            check_promoted(x, "List<dynamic>"),
            x.write(expr("List<Object?>?")),
            // `x` is now demoted back to `List<Object?>?`.
            check_promoted(x, "List<Object?>?"),
            x.write(expr("List<Object?>")),
            // `x` is now promoted to `List<Object?>`.
            check_promoted(x, "List<Object?>"),
            x.write(expr("List<void>")),
            // Type of interest promotion rejected `List<Object?>` (because it was
            // the already-promoted type), but accepted `List<dynamic>`.
            check_promoted(x, "List<dynamic>"),
        ]);
    }

    mod finally_clause {
        use super::*;

        #[test]
        fn variable() {
            let mut h = set_up();
            h.disable_sound_flow_analysis();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("Object?")),
                try_(vec![
                    x.as_("List<Object?>"),
                    check_promoted(x, "List<Object?>"),
                ])
                .finally_(vec![
                    check_not_promoted(x),
                    x.as_("List<dynamic>"),
                    check_promoted(x, "List<dynamic>"),
                ]),
                // After the try/finally, the promotions in the try block are
                // layered over the promotions in the finally block (see
                // https://github.com/dart-lang/language/issues/4382), so the
                // promotion to `List<Object?>` layers over the promotion to
                // `List<dynamic>`.
                check_promoted(x, "List<Object?>"),
            ]);
        }

        #[test]
        fn promotable_property_of_unmodified_variable() {
            let mut h = set_up();
            h.disable_sound_flow_analysis();
            h.add_member("C", "_property", Some("Object?"), true, None);
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("C")),
                try_(vec![
                    x.property("_property", false).as_("List<Object?>"),
                    check_promoted(x.property("_property", false), "List<Object?>"),
                ])
                .finally_(vec![
                    check_not_promoted(x),
                    x.property("_property", false).as_("List<dynamic>"),
                    check_promoted(x.property("_property", false), "List<dynamic>"),
                ]),
                // After the try/finally, the promotions in the try block are
                // layered over the promotions in the finally block (see
                // https://github.com/dart-lang/language/issues/4382), so the
                // promotion to `List<Object?>` layers over the promotion to
                // `List<dynamic>`.
                check_promoted(x.property("_property", false), "List<Object?>"),
            ]);
        }

        #[test]
        fn promotable_property_of_modified_variable() {
            let mut h = set_up();
            h.disable_sound_flow_analysis();
            h.add_member("C", "_property", Some("Object?"), true, None);
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("C")),
                try_(vec![
                    x.write(expr("C")),
                    x.property("_property", false).as_("List<dynamic>"),
                    check_promoted(x.property("_property", false), "List<dynamic>"),
                ])
                .finally_(vec![
                    check_not_promoted(x),
                    x.property("_property", false).as_("List<Object?>"),
                    check_promoted(x.property("_property", false), "List<Object?>"),
                ]),
                // After the try/finally, the promotions in the finally block are
                // layered over the promotions in the try block (see
                // https://github.com/dart-lang/language/issues/4382), so the
                // promotion to `List<Object?>` layers over the promotion to
                // `List<dynamic>`.
                check_promoted(x.property("_property", false), "List<Object?>"),
            ]);
        }
    }

    #[test]
    fn boolean_variable() {
        let mut h = set_up();
        h.disable_sound_flow_analysis();
        let x = Var::new("x");
        let b = Var::new("b");
        h.run(vec![
            declare(x).with_initializer(expr("Object?")),
            declare(b).with_initializer(x.is_("List<Object?>")),
            check_not_promoted(x),
            x.as_("List<dynamic>"),
            check_promoted(x, "List<dynamic>"),
            if_(
                b,
                vec![
                    // The promotion to `List<Object?>`, captured at the
                    // declaration site of `b`, is layered over the promotion to
                    // `List<dynamic>`.
                    check_promoted(x, "List<Object?>"),
                ],
            ),
        ]);
    }
}

mod when_enabled_do_not_promote_to_mutual_subtypes {
    use super::*;

    #[test]
    fn type_cast() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("List<Object?>")),
            x.as_("List<dynamic>"),
            check_not_promoted(x),
        ]);
    }

    #[test]
    fn type_check() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("List<Object?>")),
            if_(x.is_not("List<dynamic>"), vec![return_()]),
            check_not_promoted(x),
        ]);
    }

    #[test]
    fn type_of_interest_promotion() {
        // Note: to work around the fact that a full demotion clears types of
        // interest (see https://github.com/dart-lang/language/issues/4380),
        // this test starts with a variable of type `dynamic` and promotes it
        // first to `List<Object?>?` and then to `List<dynamic>`. This ensures
        // that the write that follows (which writes a value of type
        // `List<Object?>?`) does not fully demote the variable, so the types of
        // interest will be preserved.
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("dynamic")),
            x.as_("List<Object?>?"),
            // `x` is now promoted to `List<Object?>?` and `List<Object?>` is a
            // type of interest
            check_promoted(x, "List<Object?>?"),
            x.as_("List<dynamic>"),
            // `x` is now promoted to `List<dynamic>` and `List<dynamic>` is a
            // type of interest.
            check_promoted(x, "List<dynamic>"),
            x.write(expr("List<Object?>?")),
            // `x` is now demoted back to `List<Object?>?`.
            check_promoted(x, "List<Object?>?"),
            x.write(expr("List<Object?>")),
            // `x` is now promoted to `List<Object?>`.
            check_promoted(x, "List<Object?>"),
            x.write(expr("List<void>")),
            // Type of interest promotion rejected `List<Object?>` (because it was
            // the already-promoted type) and `List<dynamic>` (because it is a
            // mutual subtype with the already-promoted type).
            check_promoted(x, "List<Object?>"),
        ]);
    }

    mod finally_clause {
        use super::*;

        #[test]
        fn variable() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("Object?")),
                try_(vec![
                    x.as_("List<Object?>"),
                    check_promoted(x, "List<Object?>"),
                ])
                .finally_(vec![
                    check_not_promoted(x),
                    x.as_("List<dynamic>"),
                    check_promoted(x, "List<dynamic>"),
                ]),
                // After the try/finally, the promotions in the finally block are
                // layered over the promotions in the try block, so the
                // promotion to `List<dynamic>` layers over the promotion to
                // `List<Object?>`. But since the two types are mutual subtypes, the
                // promotion to `List<dynamic>` is discarded, leaving only the
                // promotion to `List<Object?>`.
                check_promoted(x, "List<Object?>"),
            ]);
        }

        #[test]
        fn promotable_property_of_unmodified_variable() {
            let mut h = set_up();
            h.add_member("C", "_property", Some("Object?"), true, None);
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("C")),
                try_(vec![
                    x.property("_property", false).as_("List<Object?>"),
                    check_promoted(x.property("_property", false), "List<Object?>"),
                ])
                .finally_(vec![
                    check_not_promoted(x),
                    x.property("_property", false).as_("List<dynamic>"),
                    check_promoted(x.property("_property", false), "List<dynamic>"),
                ]),
                // After the try/finally, the promotions in the finally block are
                // layered over the promotions in the try block, so the
                // promotion to `List<dynamic>` layers over the promotion to
                // `List<Object?>`. But since the two types are mutual subtypes, the
                // promotion to `List<dynamic>` is discarded, leaving only the
                // promotion to `List<Object?>`.
                check_promoted(x.property("_property", false), "List<Object?>"),
            ]);
        }

        #[test]
        fn promotable_property_of_modified_variable() {
            let mut h = set_up();
            h.add_member("C", "_property", Some("Object?"), true, None);
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("C")),
                try_(vec![
                    x.write(expr("C")),
                    x.property("_property", false).as_("List<dynamic>"),
                    check_promoted(x.property("_property", false), "List<dynamic>"),
                ])
                .finally_(vec![
                    check_not_promoted(x),
                    x.property("_property", false).as_("List<Object?>"),
                    check_promoted(x.property("_property", false), "List<Object?>"),
                ]),
                // After the try/finally, the promotions in the finally block are
                // layered over the promotions in the try block (see
                // https://github.com/dart-lang/language/issues/4382), so the
                // promotion to `List<Object?>` layers over the promotion to
                // `List<dynamic>`. But since the two types are mutual subtypes, the
                // promotion to `List<Object?>` is discarded, leaving only the
                // promotion to `List<dynamic>`.
                check_promoted(x.property("_property", false), "List<dynamic>"),
            ]);
        }
    }

    #[test]
    fn boolean_variable() {
        let mut h = set_up();
        let x = Var::new("x");
        let b = Var::new("b");
        h.run(vec![
            declare(x).with_initializer(expr("Object?")),
            declare(b).with_initializer(x.is_("List<Object?>")),
            check_not_promoted(x),
            x.as_("List<dynamic>"),
            check_promoted(x, "List<dynamic>"),
            if_(
                b,
                vec![
                    // The promotion to `List<Object?>`, captured at the
                    // declaration site of `b`, is layered over the promotion to
                    // `List<dynamic>`. But since the two types are mutual
                    // subtypes, the promotion to `List<Object?>` is discarded,
                    // leaving only the promotion to `List<dynamic>`.
                    check_promoted(x, "List<dynamic>"),
                ],
            ),
        ]);
    }
}

mod try_finally_layering_order {
    use super::*;

    mod local_variables {
        use super::*;

        /// Dart `setUp` of the group: `x = Var('x'); y = Var('y');`.
        fn vars() -> (Var, Var) {
            (Var::new("x"), Var::new("y"))
        }

        /// Dart `checkPromotionsAfterTryFinally(expectations)`.
        fn check_promotions_after_try_finally(
            h: &mut FlowAnalysisTestHarness,
            x: Var,
            y: Var,
            expectations: Vec<Node>,
        ) {
            let mut statements = vec![
                declare(x).with_initializer(expr("Object")),
                declare(y).with_initializer(expr("Object")),
                try_(vec![
                    x.as_("num"),
                    y.as_("int"),
                    check_promoted(x, "num"),
                    check_promoted(y, "int"),
                ])
                .finally_(vec![
                    // Neither `x` nor `y` is promoted at this point, because in
                    // principle an exception could have occurred at any point in
                    // the `try` block.
                    check_not_promoted(x),
                    check_not_promoted(y),
                    x.as_("int"),
                    y.as_("num"),
                    check_promoted(x, "int"),
                    check_promoted(y, "num"),
                ]),
            ];
            statements.extend(expectations);
            h.run(statements);
        }

        #[test]
        fn when_disabled_promotions_in_finally_applied_first() {
            let mut h = set_up();
            let (x, y) = vars();
            h.disable_sound_flow_analysis();
            check_promotions_after_try_finally(
                &mut h,
                x,
                y,
                vec![
                    // After the try/finally, both `x` and `y` are fully
                    // promoted to `int`. But since the promotions from the `try`
                    // block are layered over the promotions from the `finally`
                    // block, `x` has promotion chain `[int]`, whereas `y` has
                    // promotion chain `[num, int]`.
                    check_promotion_chain(x, &["int"]),
                    check_promotion_chain(y, &["num", "int"]),
                ],
            );
        }

        #[test]
        fn when_enabled_promotions_in_try_applied_first() {
            let mut h = set_up();
            let (x, y) = vars();
            check_promotions_after_try_finally(
                &mut h,
                x,
                y,
                vec![
                    // After the try/finally, both `x` and `y` are fully
                    // promoted to `int`. But since the promotions from the
                    // `finally` block are layered over the promotions from the
                    // `try` block, `x` has promotion chain `[num, int]`, whereas
                    // `y` has promotion chain `[int]`.
                    check_promotion_chain(x, &["num", "int"]),
                    check_promotion_chain(y, &["int"]),
                ],
            );
        }
    }

    mod fields_of_unmodified_local_variables {
        use super::*;

        /// Dart `setUp` of the group: `x = Var('x'); y = Var('y');`.
        fn vars() -> (Var, Var) {
            (Var::new("x"), Var::new("y"))
        }

        /// Dart `checkPromotionsAfterTryFinally(expectations)`.
        fn check_promotions_after_try_finally(
            h: &mut FlowAnalysisTestHarness,
            x: Var,
            y: Var,
            expectations: Vec<Node>,
        ) {
            h.add_member("C", "_f", Some("Object"), true, None);
            let mut statements = vec![
                declare(x).with_initializer(expr("C")),
                declare(y).with_initializer(expr("C")),
                try_(vec![
                    x.property("_f", false).as_("num"),
                    y.property("_f", false).as_("int"),
                    check_promoted(x.property("_f", false), "num"),
                    check_promoted(y.property("_f", false), "int"),
                ])
                .finally_(vec![
                    // Neither `x._f` nor `y._f` is promoted at this point,
                    // because in principle an exception could have occurred at
                    // any point in the `try` block.
                    check_not_promoted(x.property("_f", false)),
                    check_not_promoted(y.property("_f", false)),
                    x.property("_f", false).as_("int"),
                    y.property("_f", false).as_("num"),
                    check_promoted(x.property("_f", false), "int"),
                    check_promoted(y.property("_f", false), "num"),
                ]),
            ];
            statements.extend(expectations);
            h.run(statements);
        }

        #[test]
        fn when_disabled_promotions_in_finally_applied_first() {
            let mut h = set_up();
            let (x, y) = vars();
            h.disable_sound_flow_analysis();
            check_promotions_after_try_finally(
                &mut h,
                x,
                y,
                vec![
                    // After the try/finally, both `x._f` and `y._f` are fully
                    // promoted to `int`. But since the promotions from the `try`
                    // block are layered over the promotions from the `finally`
                    // block, `x._f` has promotion chain `[int]`, whereas `y._f`
                    // has promotion chain `[num, int]`.
                    check_promotion_chain(x.property("_f", false), &["int"]),
                    check_promotion_chain(y.property("_f", false), &["num", "int"]),
                ],
            );
        }

        #[test]
        fn when_enabled_promotions_in_try_applied_first() {
            let mut h = set_up();
            let (x, y) = vars();
            check_promotions_after_try_finally(
                &mut h,
                x,
                y,
                vec![
                    // After the try/finally, both `x._f` and `y._f` are fully
                    // promoted to `int`. But since the promotions from the
                    // `finally` block are layered over the promotions from the
                    // `try` block, `x._f` has promotion chain `[num, int]`,
                    // whereas `y._f` has promotion chain `[int]`.
                    check_promotion_chain(x.property("_f", false), &["num", "int"]),
                    check_promotion_chain(y.property("_f", false), &["int"]),
                ],
            );
        }
    }

    mod fields_of_local_variables_modified_in_try_clause {
        use super::*;

        /// Dart `setUp` of the group: `x = Var('x'); y = Var('y');`.
        fn vars() -> (Var, Var) {
            (Var::new("x"), Var::new("y"))
        }

        /// Dart `checkPromotionsAfterTryFinally(expectations)`.
        fn check_promotions_after_try_finally(
            h: &mut FlowAnalysisTestHarness,
            x: Var,
            y: Var,
            expectations: Vec<Node>,
        ) {
            h.add_member("C", "_f", Some("Object"), true, None);
            let mut statements = vec![
                declare(x).with_initializer(expr("C")),
                declare(y).with_initializer(expr("C")),
                try_(vec![
                    x.write(expr("C")),
                    y.write(expr("C")),
                    x.property("_f", false).as_("num"),
                    y.property("_f", false).as_("int"),
                    check_promoted(x.property("_f", false), "num"),
                    check_promoted(y.property("_f", false), "int"),
                ])
                .finally_(vec![
                    // Neither `x._f` nor `y._f` is promoted at this point,
                    // because in principle an exception could have occurred at
                    // any point in the `try` block.
                    check_not_promoted(x.property("_f", false)),
                    check_not_promoted(y.property("_f", false)),
                    x.property("_f", false).as_("int"),
                    y.property("_f", false).as_("num"),
                    check_promoted(x.property("_f", false), "int"),
                    check_promoted(y.property("_f", false), "num"),
                ]),
            ];
            statements.extend(expectations);
            h.run(statements);
        }

        #[test]
        fn when_disabled_promotions_in_try_applied_first() {
            let mut h = set_up();
            let (x, y) = vars();
            h.disable_sound_flow_analysis();
            check_promotions_after_try_finally(
                &mut h,
                x,
                y,
                vec![
                    // After the try/finally, both `x._f` and `y._f` are fully
                    // promoted to `int`. But since the promotions from the
                    // `finally` block are layered over the promotions from the
                    // `try` block, `x._f` has promotion chain `[num, int]`,
                    // whereas `y._f` has promotion chain `[int]`.
                    check_promotion_chain(x.property("_f", false), &["num", "int"]),
                    check_promotion_chain(y.property("_f", false), &["int"]),
                ],
            );
        }

        #[test]
        fn when_enabled_promotions_in_try_applied_first() {
            let mut h = set_up();
            let (x, y) = vars();
            check_promotions_after_try_finally(
                &mut h,
                x,
                y,
                vec![
                    // After the try/finally, both `x._f` and `y._f` are fully
                    // promoted to `int`. But since the promotions from the
                    // `finally` block are layered over the promotions from the
                    // `try` block, `x._f` has promotion chain `[num, int]`,
                    // whereas `y._f` has promotion chain `[int]`.
                    check_promotion_chain(x.property("_f", false), &["num", "int"]),
                    check_promotion_chain(y.property("_f", false), &["int"]),
                ],
            );
        }
    }
}

#[test]
fn when_disabled_full_demotion_clears_types_of_interest() {
    let mut h = set_up();
    let x = Var::new("x");
    h.disable_sound_flow_analysis();
    h.run(vec![
        declare(x).with_initializer(expr("Object")),
        x.as_("num"),
        check_promoted(x, "num"),
        x.write(expr("String")),
        check_not_promoted(x),
        x.write(expr("num")),
        check_not_promoted(x),
    ]);
}

#[test]
fn when_enabled_full_demotion_preserves_types_of_interest() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare(x).with_initializer(expr("Object")),
        x.as_("num"),
        check_promoted(x, "num"),
        x.write(expr("String")),
        check_not_promoted(x),
        x.write(expr("num")),
        check_promoted(x, "num"),
    ]);
}

mod false_branch_for_trivially_satisfied_is_test {
    use super::*;

    mod when_enabled_sets_unreachable {
        use super::*;

        #[test]
        fn promotable_target() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("int")),
                if_(
                    x.is_not("int"),
                    vec![check_not_promoted(x), check_reachable(false)],
                ),
            ]);
        }

        #[test]
        fn non_promotable_target() {
            let mut h = set_up();
            h.run(vec![if_(
                expr("int").is_not("int"),
                vec![check_reachable(false)],
            )]);
        }
    }

    mod when_disabled_leaves_reachable {
        use super::*;

        #[test]
        fn promotable_target() {
            let mut h = set_up();
            h.disable_sound_flow_analysis();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("int")),
                if_(
                    x.is_not("int"),
                    vec![check_not_promoted(x), check_reachable(true)],
                ),
            ]);
        }

        #[test]
        fn non_promotable_target() {
            let mut h = set_up();
            h.disable_sound_flow_analysis();
            h.run(vec![if_(
                expr("int").is_not("int"),
                vec![check_reachable(true)],
            )]);
        }
    }
}
