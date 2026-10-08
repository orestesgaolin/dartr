// Dart source: pkg/_fe_analyzer_shared/test/flow_analysis/flow_analysis_test.dart
// (lines 10408-10998: end of group 'Patterns:': groups 'Variable pattern:',
// 'Wildcard pattern:', tests 'Pattern inside guard', 'Error type does not
// trigger unnecessary wildcard warning', group 'Split points:')

//! Dart group `Patterns:` (fourth part).

use super::common::*;

mod variable_pattern {
    use super::*;

    mod covers_matched_type {
        use super::*;

        #[test]
        fn without_promotion_candidate() {
            // In `if(<some int> case num x) ...`, the `else` branch should be
            // unreachable because the type `num` fully covers the type `int`.
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![if_case(
                expr("int"),
                x.pattern().with_declared_type("num"),
                vec![check_reachable(true)],
                Some(vec![check_reachable(false)]),
            )]);
        }

        #[test]
        fn with_promotion_candidate() {
            // In `if(x case num y) ...`, the `else` branch should be unreachable
            // because the type `num` fully covers the type `int`.
            let mut h = set_up();
            let x = Var::new("x");
            let y = Var::new("y");
            h.run(vec![
                declare(x).with_declared_type("int"),
                if_case(
                    x,
                    y.pattern().with_declared_type("num"),
                    vec![check_reachable(true), check_not_promoted(x)],
                    Some(vec![check_reachable(false), check_not_promoted(x)]),
                ),
            ]);
        }

        #[test]
        fn matched_type_is_extension_type() {
            let mut h = set_up();
            h.add_super_interfaces("E", |_| vec![ty("Object?")]);
            h.add_extension_type_erasure("E", "int");
            let x = Var::new("x");
            h.run(vec![if_case(
                expr("E"),
                x.pattern().with_declared_type("int"),
                vec![check_reachable(true)],
                Some(vec![check_reachable(false)]),
            )]);
        }

        #[test]
        fn known_type_is_extension_type() {
            let mut h = set_up();
            h.add_super_interfaces("E", |_| vec![ty("Object?")]);
            h.add_extension_type_erasure("E", "int");
            let x = Var::new("x");
            h.run(vec![if_case(
                expr("int"),
                x.pattern().with_declared_type("E"),
                vec![check_reachable(true)],
                Some(vec![check_reachable(false)]),
            )]);
        }
    }

    mod doesnt_cover_matched_type {
        use super::*;

        #[test]
        fn without_promotion_candidate() {
            // In `if(<some num> case int x) ...`, the `else` branch should be
            // reachable because the type `int` doesn't fully cover the type
            // `num`.
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![if_case(
                expr("num"),
                x.pattern().with_declared_type("int"),
                vec![check_reachable(true)],
                Some(vec![check_reachable(true)]),
            )]);
        }

        mod with_promotion_candidate {
            use super::*;

            #[test]
            fn without_factor() {
                let mut h = set_up();
                let x = Var::new("x");
                let y = Var::new("y");
                h.run(vec![
                    declare(x).with_declared_type("num"),
                    if_case(
                        x,
                        y.pattern().with_declared_type("int"),
                        vec![check_reachable(true), check_promoted(x, "int")],
                        Some(vec![check_reachable(true), check_not_promoted(x)]),
                    ),
                ]);
            }

            #[test]
            fn with_factor() {
                let mut h = set_up();
                let x = Var::new("x");
                let y = Var::new("y");
                h.run(vec![
                    declare(x).with_declared_type("int?"),
                    if_case(
                        x,
                        y.pattern().with_declared_type("Null"),
                        vec![check_reachable(true), check_promoted(x, "Null")],
                        Some(vec![check_reachable(true), check_promoted(x, "int")]),
                    ),
                ]);
            }
        }
    }

    #[test]
    fn subpattern_doesnt_promote_scrutinee() {
        let mut h = set_up();
        let x = Var::new("x");
        let y = Var::new("y");
        h.run(vec![
            declare(x).with_initializer(expr("Object")),
            if_case(
                x,
                object_pattern(
                    "num",
                    vec![
                        y.pattern()
                            .with_declared_type("int")
                            .record_field(Some("sign")),
                    ],
                ),
                vec![
                    check_promoted(x, "num"),
                    // TODO(paulberry): should promote `x.sign` to `int`.
                ],
                None,
            ),
        ]);
    }

    #[test]
    fn doesnt_demote() {
        let mut h = set_up();
        let x = Var::new("x");
        let y = Var::new("y");
        let z = Var::new("z");
        h.run(vec![
            declare(x).with_initializer(expr("Object?")),
            if_case(
                x,
                wildcard()
                    .as_("int")
                    .and(y.pattern().with_declared_type("num"))
                    .and(z.pattern().with_expect_inferred_type("int")),
                vec![check_promoted(x, "int")],
                None,
            ),
        ]);
    }

    #[test]
    fn promotes_to_non_nullable_if_matched_type_is_non_nullable() {
        // When the matched value type is non-nullable, and the variable's
        // declared type is nullable, a successful match promotes the variable.
        // This allows a case pattern of the form `T? x?` to promote `x` to
        // non-nullable `T`.
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![if_case(
            expr("Object"),
            x.pattern().with_declared_type("int?"),
            vec![check_promoted(x, "int")],
            None,
        )]);
    }

    #[test]
    fn does_not_promote_to_non_nullable_if_matched_type_is_null() {
        // Since `Null` is handled specially by `TypeOperations.classifyType`,
        // make sure that we don't accidentally promote the variable to
        // non-nullable when the matched value type is `Null`.
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![if_case(
            expr("Null"),
            x.pattern().with_declared_type("int?"),
            vec![check_not_promoted(x)],
            None,
        )]);
    }

    mod demonstrated_type {
        use super::*;

        #[test]
        fn subtype_of_matched_value_type() {
            let mut h = set_up();
            let x = Var::new("x");
            let y = Var::new("y");
            h.run(vec![
                declare(x).with_initializer(expr("(num,)")),
                if_case(
                    x,
                    record_pattern(vec![
                        y.pattern().with_declared_type("int").record_field(None),
                    ]),
                    vec![check_promoted(x, "(int,)")],
                    None,
                ),
            ]);
        }

        #[test]
        fn supertype_of_matched_value_type() {
            let mut h = set_up();
            let x = Var::new("x");
            let y = Var::new("y");
            h.run(vec![
                declare(x).with_initializer(expr("(num,)")),
                if_case(
                    x,
                    record_pattern(vec![
                        y.pattern().with_declared_type("Object").record_field(None),
                    ]),
                    vec![check_not_promoted(x)],
                    None,
                ),
            ]);
        }

        #[test]
        fn unrelated_to_matched_value_type() {
            let mut h = set_up();
            let x = Var::new("x");
            let y = Var::new("y");
            h.run(vec![
                declare(x).with_initializer(expr("(num,)")),
                if_case(
                    x,
                    record_pattern(vec![
                        y.pattern().with_declared_type("String").record_field(None),
                    ]),
                    vec![check_not_promoted(x)],
                    None,
                ),
            ]);
        }
    }

    #[test]
    fn promotable_property() {
        let mut h = set_up();
        h.add_member("C", "_property", Some("Object"), true, None);
        let c = Var::new("c");
        let x = Var::new("x");
        h.run(vec![
            declare(c).with_initializer(expr("C")),
            if_case(
                c.property("_property", false),
                x.pattern().with_declared_type("int"),
                vec![check_promoted(c.property("_property", false), "int")],
                None,
            ),
        ]);
    }

    #[test]
    fn promotable_property_target_changed() {
        let mut h = set_up();
        h.add_member("C", "_property", Some("Object"), true, None);
        let c = Var::new("c");
        let x = Var::new("x");
        let y = Var::new("y");
        h.run(vec![
            declare(c).with_initializer(expr("C")),
            switch_(
                c.property("_property", false),
                vec![
                    x.pattern()
                        .with_declared_type("int")
                        .when(expr("bool"))
                        .then(vec![check_promoted(c.property("_property", false), "int")]),
                    wildcard()
                        .when(second(c.write(expr("C")), expr("bool")))
                        .then(vec![]),
                    y.pattern()
                        .with_declared_type("int")
                        .then(vec![check_not_promoted(c.property("_property", false))]),
                ],
            ),
        ]);
    }

    #[test]
    fn non_promotable_property() {
        let mut h = set_up();
        h.add_member("C", "_property", Some("Object"), false, None);
        let c = Var::new("c");
        let x = Var::new("x");
        h.run(vec![
            declare(c).with_initializer(expr("C")),
            if_case(
                c.property("_property", false),
                x.pattern().with_declared_type("int"),
                vec![check_not_promoted(c.property("_property", false))],
                None,
            ),
        ]);
    }
}

mod wildcard_pattern {
    use super::*;

    mod covers_matched_type {
        use super::*;

        #[test]
        fn without_promotion_candidate() {
            // In `if(<some int> case num _) ...`, the `else` branch should be
            // unreachable because the type `num` fully covers the type `int`.
            let mut h = set_up();
            h.run(vec![if_case(
                expr("int"),
                wildcard().with_declared_type("num"),
                vec![check_reachable(true)],
                Some(vec![check_reachable(false)]),
            )]);
        }

        #[test]
        fn with_promotion_candidate() {
            // In `if(x case num _) ...`, the `else` branch should be unreachable
            // because the type `num` fully covers the type `int`.
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_declared_type("int"),
                if_case(
                    x,
                    wildcard().with_declared_type("num"),
                    vec![check_reachable(true), check_not_promoted(x)],
                    Some(vec![check_reachable(false), check_not_promoted(x)]),
                ),
            ]);
        }
    }

    mod doesnt_cover_matched_type {
        use super::*;

        #[test]
        fn without_promotion_candidate() {
            // In `if(<some num> case int _) ...`, the `else` branch should be
            // reachable because the type `int` doesn't fully cover the type
            // `num`.
            let mut h = set_up();
            h.run(vec![if_case(
                expr("num"),
                wildcard().with_declared_type("int"),
                vec![check_reachable(true)],
                Some(vec![check_reachable(true)]),
            )]);
        }

        mod with_promotion_candidate {
            use super::*;

            #[test]
            fn without_factor() {
                let mut h = set_up();
                let x = Var::new("x");
                h.run(vec![
                    declare(x).with_declared_type("num"),
                    if_case(
                        x,
                        wildcard().with_declared_type("int"),
                        vec![check_reachable(true), check_promoted(x, "int")],
                        Some(vec![check_reachable(true), check_not_promoted(x)]),
                    ),
                ]);
            }

            #[test]
            fn with_factor() {
                let mut h = set_up();
                let x = Var::new("x");
                h.run(vec![
                    declare(x).with_declared_type("int?"),
                    if_case(
                        x,
                        wildcard().with_declared_type("Null"),
                        vec![check_reachable(true), check_promoted(x, "Null")],
                        Some(vec![check_reachable(true), check_promoted(x, "int")]),
                    ),
                ]);
            }
        }
    }

    #[test]
    fn subpattern_doesnt_promote_scrutinee() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("Object")),
            if_case(
                x,
                object_pattern(
                    "num",
                    vec![
                        wildcard()
                            .with_declared_type("int")
                            .record_field(Some("sign")),
                    ],
                ),
                vec![
                    check_promoted(x, "num"),
                    // TODO(paulberry): should promote `x.sign` to `int`.
                ],
                None,
            ),
        ]);
    }

    #[test]
    fn doesnt_demote() {
        let mut h = set_up();
        let x = Var::new("x");
        let y = Var::new("y");
        h.run_with(
            vec![
                declare(x).with_initializer(expr("Object?")),
                if_case(
                    x,
                    wildcard()
                        .as_("int")
                        .and(wildcard().with_declared_type("num").error_id("WILDCARD"))
                        .and(y.pattern().with_expect_inferred_type("int")),
                    vec![check_promoted(x, "int")],
                    None,
                ),
            ],
            errors(&["unnecessaryWildcardPattern(pattern: WILDCARD, \
                      kind: logicalAndPatternOperand)"]),
        );
    }

    mod demonstrated_type {
        use super::*;

        #[test]
        fn subtype_of_matched_value_type() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("(num,)")),
                if_case(
                    x,
                    record_pattern(vec![
                        wildcard().with_declared_type("int").record_field(None),
                    ]),
                    vec![check_promoted(x, "(int,)")],
                    None,
                ),
            ]);
        }

        #[test]
        fn supertype_of_matched_value_type() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("(num,)")),
                if_case(
                    x,
                    record_pattern(vec![
                        wildcard().with_declared_type("Object").record_field(None),
                    ]),
                    vec![check_not_promoted(x)],
                    None,
                ),
            ]);
        }

        #[test]
        fn unrelated_to_matched_value_type() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("(num,)")),
                if_case(
                    x,
                    record_pattern(vec![
                        wildcard().with_declared_type("String").record_field(None),
                    ]),
                    vec![check_not_promoted(x)],
                    None,
                ),
            ]);
        }
    }

    #[test]
    fn promotable_property() {
        let mut h = set_up();
        h.add_member("C", "_property", Some("Object"), true, None);
        let c = Var::new("c");
        h.run(vec![
            declare(c).with_initializer(expr("C")),
            if_case(
                c.property("_property", false),
                wildcard().with_declared_type("int"),
                vec![check_promoted(c.property("_property", false), "int")],
                None,
            ),
        ]);
    }

    #[test]
    fn promotable_property_target_changed() {
        let mut h = set_up();
        h.add_member("C", "_property", Some("Object"), true, None);
        let c = Var::new("c");
        h.run(vec![
            declare(c).with_initializer(expr("C")),
            switch_(
                c.property("_property", false),
                vec![
                    wildcard()
                        .with_declared_type("int")
                        .when(expr("bool"))
                        .then(vec![check_promoted(c.property("_property", false), "int")]),
                    wildcard()
                        .when(second(c.write(expr("C")), expr("bool")))
                        .then(vec![]),
                    wildcard()
                        .with_declared_type("int")
                        .then(vec![check_not_promoted(c.property("_property", false))]),
                ],
            ),
        ]);
    }

    #[test]
    fn non_promotable_property() {
        let mut h = set_up();
        h.add_member("C", "_property", Some("Object"), false, None);
        let c = Var::new("c");
        h.run(vec![
            declare(c).with_initializer(expr("C")),
            if_case(
                c.property("_property", false),
                wildcard().with_declared_type("int"),
                vec![check_not_promoted(c.property("_property", false))],
                None,
            ),
        ]);
    }
}

#[test]
fn pattern_inside_guard() {
    // Roughly equivalent Dart code:
    //     FutureOr<int> x = ...;
    //     FutureOr<String> y = ...;
    //     if (x case int _ when f(() {
    //           if (y case String _) {
    //             /* x promoted to `int` */
    //             /* y promoted to `String` */
    //           } else {
    //             /* x promoted to `int` */
    //             /* y promoted to `Future<String>` */
    //           }
    //         }, throw ...)) {
    //       /* unreachable (due to `throw`) */
    //     } else {
    //       /* x promoted to `Future<int>` */
    //     }
    // For this to be analyzed correctly, flow analysis needs to avoid mixing
    // up the "unmatched" state from the outer and inner pattern matches.
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    h.run(vec![
        declare(x).with_initializer(expr("FutureOr<int>")),
        declare(y).with_initializer(expr("FutureOr<String>")),
        if_case(
            x,
            wildcard().with_declared_type("int").when(second(
                local_function(vec![if_case(
                    y,
                    wildcard().with_declared_type("String"),
                    vec![check_promoted(x, "int"), check_promoted(y, "String")],
                    Some(vec![
                        check_promoted(x, "int"),
                        check_promoted(y, "Future<String>"),
                    ]),
                )]),
                throw_(expr("Object")),
            )),
            vec![check_reachable(false)],
            Some(vec![
                check_reachable(true),
                check_promoted(x, "Future<int>"),
            ]),
        ),
    ]);
}

#[test]
fn error_type_does_not_trigger_unnecessary_wildcard_warning() {
    let mut h = set_up();
    h.run(vec![if_case(
        expr("num"),
        wildcard()
            .with_declared_type("int")
            .and(wildcard().with_declared_type("error")),
        vec![],
        None,
    )]);
}

mod split_points {
    use super::*;

    #[test]
    fn guarded() {
        // This test verifies that for a guarded pattern, the join of the two
        // "unmatched" control flow paths corresponds to a split point at the
        // beginning of the pattern.
        let mut h = set_up();
        let i = Var::new("i");
        h.run(vec![
            declare(i).with_initializer(expr("int?")),
            if_case(
                second(throw_(expr("Object")), expr("int")).check_type("int"),
                object_pattern("int", vec![]).when(i.eq(null_literal())),
                vec![],
                Some(vec![
                    // There is a join point here, joining the flow control paths
                    // where (a) the pattern `int()` failed to match and (b) the
                    // guard `i == null` was not satisfied. Since the scrutinee has
                    // type `int`, and the pattern is `int()`, the pattern is
                    // guaranteed to match, so path (a) is unreachable. Path (b) is
                    // also unreachable due to the fact that the scrutinee throws,
                    // but since the split point is the beginning of the pattern,
                    // path (b) is reachable from the split point. So the promotion
                    // implied by (b) is preserved after the join.
                    check_promoted(i, "int"),
                    // Note that due to the `throw` in the scrutinee, this code is
                    // unreachable.
                    check_reachable(false),
                ]),
            ),
        ]);
    }

    #[test]
    fn logical_or() {
        // This test verifies that for a logical-or pattern, the join of the two
        // "matched" control flow paths corresponds to a split point at the
        // beginning of the top level pattern.
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![if_case(
            expr("(Null, Null, int?)"),
            record_pattern(vec![
                relational_pattern("!=", null_literal()).record_field(None),
                wildcard().record_field(None),
                wildcard().record_field(None),
            ])
            // At this point, control flow is unreachable due to the fact
            // that the `!= null` pattern in the first field of the
            // record pattern above can never match the type `Null`.
            .and(
                record_pattern(vec![
                    wildcard().record_field(None),
                    relational_pattern("!=", null_literal()).record_field(None),
                    wildcard().record_field(None),
                ])
                // At this point, control flow is unreachable for a
                // second reason: because the `!= null` pattern in the
                // second field of the record pattern above can never
                // match the type `Null`.
                .or(
                    record_pattern(vec![
                        wildcard().record_field(None),
                        wildcard().record_field(None),
                        wildcard().null_check().record_field(None),
                    ]),
                    // At this point, the third field of the scrutinee
                    // is promoted from `int?` to `int`, due to the
                    // null check pattern.
                ),
                // At this point, there is a control flow join between the
                // two branches of the logical-or pattern. Since the split
                // point corresponding to the control flow join is at the
                // beginning of the top level pattern, both branches are
                // considered unreachable, so neither is favored in the
                // join, and therefore, the promotion from the second
                // branch is lost.
            )
            .and(
                // The record pattern below matches `x` to the unpromoted
                // type of the third field of the scrutinee, so we just
                // have to verify that it has the expected type of `int?`.
                record_pattern(vec![
                    wildcard().record_field(None),
                    wildcard().record_field(None),
                    x.pattern()
                        .with_expect_inferred_type("int?")
                        .record_field(None),
                ]),
            ),
            vec![
                // As a sanity check, confirm that the overall pattern
                // can't ever match.
                check_reachable(false),
            ],
            None,
        )]);
    }
}
