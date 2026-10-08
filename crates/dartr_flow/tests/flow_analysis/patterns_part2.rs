// Dart source: pkg/_fe_analyzer_shared/test/flow_analysis/flow_analysis_test.dart
// (lines 7335-8702: group 'Patterns:', the rest of group 'Cast pattern:'
// (from "Doesn't demote"), and groups 'Constant pattern:', 'For-in
// statement:', 'For-in collection element:', 'If-case element:', 'If-case
// statement:', 'Logical-and pattern:', 'Logical-or pattern:', 'List
// pattern:', 'Null-aware map entry:', 'Map pattern:')

//! Dart group `Patterns:` (second part).

use super::common::*;

mod cast_pattern {
    use super::*;

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
                        .and(wildcard().as_("num").error_id("AS_NUM"))
                        .and(y.pattern().with_expect_inferred_type("int")),
                    vec![check_promoted(x, "int")],
                    None,
                ),
            ],
            errors(&[
                "matchedTypeIsSubtypeOfRequired(pattern: AS_NUM, matchedType: int, \
                      requiredType: num)",
            ]),
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
                    record_pattern(vec![wildcard().as_("int").record_field(None)]),
                    vec![check_promoted(x, "(int,)")],
                    None,
                ),
            ]);
        }

        #[test]
        fn supertype_of_matched_value_type() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run_with(
                vec![
                    declare(x).with_initializer(expr("(num,)")),
                    if_case(
                        x,
                        record_pattern(vec![
                            wildcard().as_("Object").error_id("CAST").record_field(None),
                        ]),
                        vec![check_not_promoted(x)],
                        None,
                    ),
                ],
                errors(&[
                    "matchedTypeIsSubtypeOfRequired(pattern: CAST, matchedType: num, \
                          requiredType: Object)",
                ]),
            );
        }

        #[test]
        fn unrelated_to_matched_value_type() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("(num,)")),
                if_case(
                    x,
                    record_pattern(vec![wildcard().as_("String").record_field(None)]),
                    vec![check_not_promoted(x)],
                    None,
                ),
            ]);
        }
    }

    #[test]
    fn error_type_does_not_trigger_unnecessary_cast_warning() {
        let mut h = set_up();
        h.run(vec![if_case(
            expr("int"),
            wildcard().as_("error"),
            vec![],
            None,
        )]);
    }

    #[test]
    fn promotable_property() {
        let mut h = set_up();
        h.add_member("C", "_property", Some("int?"), true, None);
        let c = Var::new("c");
        h.run(vec![
            declare(c).with_initializer(expr("C")),
            if_case(
                c.property("_property", false),
                wildcard().as_("int"),
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
                        .as_("num")
                        .when(expr("bool"))
                        .then(vec![check_promoted(c.property("_property", false), "num")]),
                    wildcard()
                        .when(second(c.write(expr("C")), expr("bool")))
                        .then(vec![]),
                    wildcard()
                        .as_("int")
                        .then(vec![check_not_promoted(c.property("_property", false))]),
                ],
            ),
        ]);
    }

    #[test]
    fn non_promotable_property() {
        let mut h = set_up();
        h.add_member("C", "_property", Some("int?"), false, None);
        let c = Var::new("c");
        h.run(vec![
            declare(c).with_initializer(expr("C")),
            if_case(
                c.property("_property", false),
                wildcard().as_("int"),
                vec![check_not_promoted(c.property("_property", false))],
                None,
            ),
        ]);
    }
}

mod constant_pattern {
    use super::*;

    #[test]
    fn guaranteed_match_due_to_null_type() {
        let mut h = set_up();
        h.run(vec![if_case(
            expr("Null"),
            null_literal().pattern(),
            vec![check_reachable(true)],
            Some(vec![check_reachable(false)]),
        )]);
    }

    #[test]
    fn not_guaranteed_to_match_due_to_null_type_with_old_language_version() {
        let mut h = set_up();
        h.disable_patterns();
        h.run(vec![
            switch_(
                expr("Null"),
                vec![
                    null_literal()
                        .pattern()
                        .then(vec![check_reachable(true), break_(None)]),
                    default_().then(vec![check_reachable(true), break_(None)]),
                ],
            )
            .with_legacy_exhaustive(true),
        ]);
    }

    #[test]
    fn in_the_general_case_may_or_may_not_match() {
        let mut h = set_up();
        h.run(vec![if_case(
            expr("Object?"),
            int_literal(0).pattern(),
            vec![check_reachable(true)],
            Some(vec![check_reachable(true)]),
        )]);
    }

    #[test]
    fn null_pattern_promotes_unchanged_scrutinee() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("int?")),
            if_case(
                x,
                null_literal().pattern(),
                vec![check_reachable(true), check_not_promoted(x)],
                Some(vec![check_reachable(true), check_promoted(x, "int")]),
            ),
        ]);
    }

    #[test]
    fn null_pattern_doesnt_promote_scrutinee_with_old_language_version() {
        let mut h = set_up();
        h.disable_patterns();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("int?")),
            switch_(
                x,
                vec![
                    null_literal().pattern().then(vec![
                        check_reachable(true),
                        check_not_promoted(x),
                        break_(None),
                    ]),
                    default_().then(vec![
                        check_reachable(true),
                        check_not_promoted(x),
                        break_(None),
                    ]),
                ],
            )
            .with_legacy_exhaustive(true),
        ]);
    }

    #[test]
    fn null_pattern_doesnt_promote_changed_scrutinee() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("int?")),
            switch_(
                x,
                vec![
                    wildcard()
                        .when(second(x.write(expr("int?")), expr("bool")))
                        .then(vec![break_(None)]),
                    null_literal()
                        .pattern()
                        .then(vec![check_reachable(true), check_not_promoted(x)]),
                    wildcard()
                        .with_expect_inferred_type("int")
                        .then(vec![check_reachable(true), check_not_promoted(x)]),
                ],
            ),
        ]);
    }

    #[test]
    fn null_pattern_promotes_matched_pattern_var() {
        let mut h = set_up();
        h.run(vec![if_case(
            expr("int?"),
            null_literal()
                .pattern()
                .or(wildcard().with_expect_inferred_type("int")),
            vec![],
            None,
        )]);
    }

    #[test]
    fn demonstrated_type() {
        // The demonstrated type of a constant pattern is the matched value
        // type.  We don't want to promote to the constant type because doing so
        // might be unsound if the user overrides `operator==`.
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("(Object,)")),
            if_case(
                x,
                record_pattern(vec![int_literal(1).pattern().record_field(None)]),
                vec![check_not_promoted(x)],
                None,
            ),
        ]);
    }
}

mod for_in_statement {
    use super::*;

    #[test]
    fn does_not_promote_iterable() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("List<dynamic>")),
            pattern_for_in(wildcard().with_declared_type("int"), x, vec![], false),
            check_not_promoted(x),
        ]);
    }
}

mod for_in_collection_element {
    use super::*;

    #[test]
    fn does_not_promote_iterable() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("List<dynamic>")),
            list_literal(
                vec![pattern_for_in_element(
                    wildcard().with_declared_type("int"),
                    x,
                    expr("Object"),
                    false,
                )],
                "Object",
            ),
            check_not_promoted(x),
        ]);
    }
}

mod if_case_element {
    use super::*;

    #[test]
    fn guarded() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_declared_type("int?"),
            list_literal(
                vec![if_case_element(
                    expr("Object"),
                    wildcard().when(x.not_eq(null_literal())),
                    second(
                        list_literal(
                            vec![check_reachable(true), check_promoted(x, "int")],
                            "dynamic",
                        ),
                        expr("String"),
                    ),
                    second(
                        list_literal(
                            vec![check_reachable(true), check_not_promoted(x)],
                            "dynamic",
                        ),
                        expr("String"),
                    ),
                )],
                "String",
            ),
        ]);
    }

    #[test]
    fn promotes() {
        let mut h = set_up();
        let x = Var::new("x");
        let y = Var::new("y");
        h.run(vec![
            declare(x).with_declared_type("num"),
            list_literal(
                vec![if_case_element(
                    x,
                    y.pattern().with_declared_type("int"),
                    second(
                        list_literal(
                            vec![check_reachable(true), check_promoted(x, "int")],
                            "dynamic",
                        ),
                        expr("String"),
                    ),
                    second(
                        list_literal(
                            vec![check_reachable(true), check_not_promoted(x)],
                            "dynamic",
                        ),
                        expr("String"),
                    ),
                )],
                "String",
            ),
        ]);
    }
}

mod if_case_statement {
    use super::*;

    #[test]
    fn guarded() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_declared_type("int?"),
            if_case(
                expr("Object"),
                wildcard().when(x.not_eq(null_literal())),
                vec![check_reachable(true), check_promoted(x, "int")],
                Some(vec![check_reachable(true), check_not_promoted(x)]),
            ),
        ]);
    }

    #[test]
    fn promotes() {
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
    fn promotion_in_both_pattern_and_guard() {
        let mut h = set_up();
        let x = Var::new("x");
        let y = Var::new("y");
        h.run(vec![
            declare(x).with_declared_type("int?"),
            declare(y).with_declared_type("String?"),
            if_case(
                x,
                wildcard()
                    .with_declared_type("int")
                    .when(y.not_eq(null_literal())),
                vec![
                    check_reachable(true),
                    check_promoted(x, "int"),
                    check_promoted(y, "String"),
                ],
                Some(vec![
                    check_reachable(true),
                    check_not_promoted(x),
                    check_not_promoted(y),
                ]),
            ),
        ]);
    }
}

mod logical_and_pattern {
    use super::*;

    mod promotion_of_matched_value_type {
        use super::*;

        #[test]
        fn when_scrutinee_is_promotable() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run_with(
                vec![
                    declare(x).with_declared_type("num"),
                    if_case(
                        x,
                        wildcard().with_declared_type("int").and(
                            wildcard()
                                .with_expect_inferred_type("int")
                                .error_id("WILDCARD"),
                        ),
                        vec![check_promoted(x, "int")],
                        None,
                    ),
                ],
                errors(&["unnecessaryWildcardPattern(pattern: WILDCARD, \
                          kind: logicalAndPatternOperand)"]),
            );
        }

        #[test]
        fn when_scrutinee_is_not_promotable() {
            let mut h = set_up();
            h.run_with(
                vec![if_case(
                    expr("num"),
                    wildcard().with_declared_type("int").and(
                        wildcard()
                            .with_expect_inferred_type("int")
                            .error_id("WILDCARD"),
                    ),
                    vec![],
                    None,
                )],
                errors(&["unnecessaryWildcardPattern(pattern: WILDCARD, \
                          kind: logicalAndPatternOperand)"]),
            );
        }
    }

    #[test]
    fn double_promotion_of_matched_value_type() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run_with(
            vec![
                declare(x).with_declared_type("Object"),
                if_case(
                    x,
                    wildcard().with_declared_type("num").and(
                        wildcard().with_declared_type("int").and(
                            wildcard()
                                .with_expect_inferred_type("int")
                                .error_id("WILDCARD"),
                        ),
                    ),
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
        fn lhs_sub_rhs_both_could_promote() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run_with(
                vec![
                    declare(x).with_initializer(expr("(Object,)")),
                    if_case(
                        x,
                        record_pattern(vec![
                            wildcard()
                                .with_declared_type("int")
                                .and(wildcard().with_declared_type("num").error_id("NUM"))
                                .record_field(None),
                        ]),
                        vec![check_promoted(x, "(int,)")],
                        None,
                    ),
                ],
                errors(&["unnecessaryWildcardPattern(pattern: NUM, \
                          kind: logicalAndPatternOperand)"]),
            );
        }

        #[test]
        fn rhs_sub_lhs_both_could_promote() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("(Object,)")),
                if_case(
                    x,
                    record_pattern(vec![
                        wildcard()
                            .with_declared_type("num")
                            .and(wildcard().with_declared_type("int"))
                            .record_field(None),
                    ]),
                    vec![check_promoted(x, "(int,)")],
                    None,
                ),
            ]);
        }

        #[test]
        fn lhs_sub_rhs_rhs_eq_declared_type() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run_with(
                vec![
                    declare(x).with_initializer(expr("(num,)")),
                    if_case(
                        x,
                        record_pattern(vec![
                            wildcard()
                                .with_declared_type("int")
                                .and(wildcard().with_declared_type("num").error_id("NUM"))
                                .record_field(None),
                        ]),
                        vec![check_promoted(x, "(int,)")],
                        None,
                    ),
                ],
                errors(&["unnecessaryWildcardPattern(pattern: NUM, \
                          kind: logicalAndPatternOperand)"]),
            );
        }

        #[test]
        fn rhs_sub_lhs_lhs_eq_declared_type() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run_with(
                vec![
                    declare(x).with_initializer(expr("(num,)")),
                    if_case(
                        x,
                        record_pattern(vec![
                            wildcard()
                                .with_declared_type("num")
                                .error_id("NUM")
                                .and(wildcard().with_declared_type("int"))
                                .record_field(None),
                        ]),
                        vec![check_promoted(x, "(int,)")],
                        None,
                    ),
                ],
                errors(&["unnecessaryWildcardPattern(pattern: NUM, \
                          kind: logicalAndPatternOperand)"]),
            );
        }

        #[test]
        fn lhs_sub_rhs_only_lhs_could_promote() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run_with(
                vec![
                    declare(x).with_initializer(expr("(num,)")),
                    if_case(
                        x,
                        record_pattern(vec![
                            wildcard()
                                .with_declared_type("int")
                                .and(wildcard().with_declared_type("Object").error_id("OBJECT"))
                                .record_field(None),
                        ]),
                        vec![check_promoted(x, "(int,)")],
                        None,
                    ),
                ],
                errors(&["unnecessaryWildcardPattern(pattern: OBJECT, \
                          kind: logicalAndPatternOperand)"]),
            );
        }

        #[test]
        fn rhs_sub_lhs_only_rhs_could_promote() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run_with(
                vec![
                    declare(x).with_initializer(expr("(num,)")),
                    if_case(
                        x,
                        record_pattern(vec![
                            wildcard()
                                .with_declared_type("Object")
                                .error_id("OBJECT")
                                .and(wildcard().with_declared_type("int"))
                                .record_field(None),
                        ]),
                        vec![check_promoted(x, "(int,)")],
                        None,
                    ),
                ],
                errors(&["unnecessaryWildcardPattern(pattern: OBJECT, \
                          kind: logicalAndPatternOperand)"]),
            );
        }

        #[test]
        fn lhs_sub_rhs_neither_could_promote() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run_with(
                vec![
                    declare(x).with_initializer(expr("(int,)")),
                    if_case(
                        x,
                        record_pattern(vec![
                            wildcard()
                                .with_declared_type("num")
                                .error_id("NUM")
                                .and(wildcard().with_declared_type("Object").error_id("OBJECT"))
                                .record_field(None),
                        ]),
                        vec![check_not_promoted(x)],
                        None,
                    ),
                ],
                errors(&[
                    "unnecessaryWildcardPattern(pattern: NUM, \
                     kind: logicalAndPatternOperand)",
                    "unnecessaryWildcardPattern(pattern: OBJECT, \
                     kind: logicalAndPatternOperand)",
                ]),
            );
        }

        #[test]
        fn rhs_sub_lhs_neither_could_promote() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run_with(
                vec![
                    declare(x).with_initializer(expr("(int,)")),
                    if_case(
                        x,
                        record_pattern(vec![
                            wildcard()
                                .with_declared_type("Object")
                                .error_id("OBJECT")
                                .and(wildcard().with_declared_type("num").error_id("NUM"))
                                .record_field(None),
                        ]),
                        vec![check_not_promoted(x)],
                        None,
                    ),
                ],
                errors(&[
                    "unnecessaryWildcardPattern(pattern: OBJECT, \
                     kind: logicalAndPatternOperand)",
                    "unnecessaryWildcardPattern(pattern: NUM, \
                     kind: logicalAndPatternOperand)",
                ]),
            );
        }
    }
}

mod logical_or_pattern {
    use super::*;

    mod joins_promotions_of_scrutinee {
        use super::*;

        #[test]
        fn lhs_more_promoted() {
            let mut h = set_up();
            let x = Var::new("x");
            // `(num() && int()) || num()` retains promotion to `num`
            h.run(vec![
                declare(x).with_initializer(expr("Object")),
                if_case(
                    x,
                    object_pattern("num", vec![])
                        .and(object_pattern("int", vec![]))
                        .or(object_pattern("num", vec![])),
                    vec![check_promoted(x, "num")],
                    None,
                ),
            ]);
        }

        #[test]
        fn rhs_more_promoted() {
            let mut h = set_up();
            let x = Var::new("x");
            // `num() || (num() && int())` retains promotion to `num`
            h.run(vec![
                declare(x).with_initializer(expr("Object")),
                if_case(
                    x,
                    object_pattern("num", vec![])
                        .or(object_pattern("num", vec![]).and(object_pattern("int", vec![]))),
                    vec![check_promoted(x, "num")],
                    None,
                ),
            ]);
        }
    }

    mod joins_promotions_of_implicit_temporary_match_variable {
        use super::*;

        #[test]
        fn lhs_more_promoted() {
            let mut h = set_up();
            // `(num() && int()) || num()` retains promotion to `num`
            h.run_with(
                vec![if_case(
                    expr("Object"),
                    object_pattern("num", vec![])
                        .and(object_pattern("int", vec![]))
                        .or(object_pattern("num", vec![]))
                        .and(
                            wildcard()
                                .with_expect_inferred_type("num")
                                .error_id("WILDCARD"),
                        ),
                    vec![],
                    None,
                )],
                errors(&["unnecessaryWildcardPattern(pattern: WILDCARD, \
                          kind: logicalAndPatternOperand)"]),
            );
        }

        #[test]
        fn rhs_more_promoted() {
            let mut h = set_up();
            // `num() || (num() && int())` retains promotion to `num`
            h.run_with(
                vec![if_case(
                    expr("Object"),
                    object_pattern("num", vec![])
                        .or(object_pattern("num", vec![]).and(object_pattern("int", vec![])))
                        .and(
                            wildcard()
                                .with_expect_inferred_type("num")
                                .error_id("WILDCARD"),
                        ),
                    vec![],
                    None,
                )],
                errors(&["unnecessaryWildcardPattern(pattern: WILDCARD, \
                          kind: logicalAndPatternOperand)"]),
            );
        }
    }

    mod joins_explicitly_declared_variables {
        use super::*;

        #[test]
        fn lhs_promoted() {
            let mut h = set_up();
            let x1 = Var::new("x").with_identity("x1");
            let x2 = Var::new("x").with_identity("x2");
            let x = Var::join("x", vec![x1, x2]);
            h.run(vec![if_case(
                expr("int?"),
                x1.pattern()
                    .with_declared_type("int?")
                    .null_check()
                    .or(x2.pattern().with_declared_type("int?")),
                vec![check_not_promoted(x)],
                None,
            )]);
        }

        #[test]
        fn rhs_promoted() {
            let mut h = set_up();
            let x1 = Var::new("x").with_identity("x1");
            let x2 = Var::new("x").with_identity("x2");
            let x = Var::join("x", vec![x1, x2]);
            h.run(vec![if_case(
                expr("int?"),
                x1.pattern()
                    .with_declared_type("int?")
                    .or(x2.pattern().with_declared_type("int?").null_check()),
                vec![check_not_promoted(x)],
                None,
            )]);
        }

        #[test]
        fn both_sides_promoted() {
            let mut h = set_up();
            let x1 = Var::new("x").with_identity("x1");
            let x2 = Var::new("x").with_identity("x2");
            let x = Var::join("x", vec![x1, x2]);
            h.run(vec![if_case(
                expr("int?"),
                x1.pattern()
                    .with_declared_type("int?")
                    .null_check()
                    .or(x2.pattern().with_declared_type("int?").null_check()),
                vec![check_promoted(x, "int")],
                None,
            )]);
        }

        #[test]
        fn join_variable_is_promotable() {
            let mut h = set_up();
            let x1 = Var::new("x").with_identity("x1");
            let x2 = Var::new("x").with_identity("x2");
            let x = Var::join("x", vec![x1, x2]);
            h.run(vec![if_case(
                expr("int?"),
                x1.pattern()
                    .with_declared_type("int?")
                    .null_check()
                    .or(x2.pattern().with_declared_type("int?")),
                vec![
                    check_not_promoted(x),
                    x.non_null_assert(),
                    check_promoted(x, "int"),
                ],
                None,
            )]);
        }
    }

    mod sets_join_variable_assigned_even_if_variable_appears_on_only_one_side {
        use super::*;

        #[test]
        fn variable_on_lhs_only() {
            let mut h = set_up();
            let x1 = Var::new("x").with_identity("x1").error_id("X1");
            let x = Var::join("x", vec![x1]);
            // `x` is considered assigned inside the `true` branch (even though
            // it's not actually assigned on both sides of the or-pattern) because
            // this avoids redundant errors.
            h.run_with(
                vec![if_case(
                    expr("num?"),
                    x1.pattern().null_check().or(wildcard()).error_id("OR"),
                    vec![
                        check_assigned(x, true),
                        // Also verify that the join variable is promotable
                        check_not_promoted(x),
                        x.as_("int"),
                        check_promoted(x, "int"),
                    ],
                    None,
                )],
                errors(&[
                    "logicalOrPatternBranchMissingVariable(node: OR, hasInLeft: \
                          true, name: x, variable: X1)",
                ]),
            );
        }

        #[test]
        fn variable_on_rhs_only() {
            let mut h = set_up();
            let x1 = Var::new("x").with_identity("x1").error_id("X1");
            let x = Var::join("x", vec![x1]);
            // `x` is considered assigned inside the `true` branch (even though
            // it's not actually assigned on both sides of the or-pattern) because
            // this avoids redundant errors.
            h.run_with(
                vec![if_case(
                    expr("int?"),
                    wildcard().null_check().or(x1.pattern()).error_id("OR"),
                    vec![
                        check_assigned(x, true),
                        // Also verify that the join variable is promotable
                        check_not_promoted(x),
                        x.non_null_assert(),
                        check_promoted(x, "int"),
                    ],
                    None,
                )],
                errors(&[
                    "logicalOrPatternBranchMissingVariable(node: OR, hasInLeft: \
                          false, name: x, variable: X1)",
                ]),
            );
        }
    }

    mod demonstrated_type {
        use super::*;

        fn check(scrutinee_type: &str, lhs: &str, rhs: &str, promoted: Option<&str>) {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr(scrutinee_type)),
                if_case(
                    x,
                    record_pattern(vec![
                        wildcard()
                            .with_declared_type(lhs)
                            .or(wildcard().with_declared_type(rhs))
                            .record_field(None),
                    ]),
                    vec![match promoted {
                        Some(t) => check_promoted(x, t),
                        None => check_not_promoted(x),
                    }],
                    None,
                ),
            ]);
        }

        #[test]
        fn lhs_sub_rhs_both_could_promote() {
            // In the circumstance where the LHS and RHS of the logical-or pattern
            // promote the matched value to different types, we don't retain any
            // promotion.  This is similar to how we don't retain any promotion
            // for a test like `if (x is int || x is num)`.
            check("(Object,)", "int", "num", None);
        }

        #[test]
        fn rhs_sub_lhs_both_could_promote() {
            // In the circumstance where the LHS and RHS of the logical-or pattern
            // promote the matched value to different types, we don't retain any
            // promotion.  This is similar to how we don't retain any promotion
            // for a test like `if (x is int || x is num)`.
            check("(Object,)", "num", "int", None);
        }

        #[test]
        fn lhs_eq_rhs_could_promote() {
            // In the circumstance where the LHS and RHS of the logical-or pattern
            // promote the matched value to the same type, we do retain the
            // promotion.  This is similar to how we retain the promotion for a
            // test like `if (x is num || x is num)`.
            check("(Object,)", "num", "num", Some("(num,)"));
        }

        #[test]
        fn lhs_sub_rhs_only_lhs_could_promote() {
            check("(num,)", "int", "Object", None);
        }

        #[test]
        fn rhs_sub_lhs_only_rhs_could_promote() {
            check("(num,)", "Object", "int", None);
        }

        #[test]
        fn lhs_sub_rhs_neither_could_promote() {
            check("(int,)", "num", "Object", None);
        }

        #[test]
        fn rhs_sub_lhs_neither_could_promote() {
            check("(int,)", "Object", "num", None);
        }

        #[test]
        fn does_not_promote_to_lub() {
            // `if (x case (int _ || double _,)` doesn't promote `x` to `(num,)`.
            // Rationale: we want to be consistent with the behavior of
            // `if (x case int _ || double _)`, which doesn't promote to `num`.
            check("(Object?,)", "int", "double", None);
        }
    }
}

mod list_pattern {
    use super::*;

    mod not_guaranteed_to_match {
        use super::*;

        mod empty_list {
            use super::*;

            #[test]
            fn matched_value_type_is_non_nullable_list() {
                let mut h = set_up();
                h.run(vec![switch_(
                    expr("List<Object>"),
                    vec![
                        list_pattern(vec![], None).then(vec![break_(None)]),
                        default_().then(vec![check_reachable(true)]),
                    ],
                )]);
            }

            #[test]
            fn matched_value_type_is_nullable_list() {
                let mut h = set_up();
                let x = Var::new("x");
                h.run(vec![
                    declare(x).with_initializer(expr("List<Object?>?")),
                    switch_(
                        x,
                        vec![
                            list_pattern(vec![], None).then(vec![
                                check_reachable(true),
                                check_promoted(x, "List<Object?>"),
                            ]),
                            default_().then(vec![check_reachable(true), check_not_promoted(x)]),
                        ],
                    ),
                ]);
            }
        }

        #[test]
        fn single_non_rest_element() {
            let mut h = set_up();
            h.run(vec![switch_(
                expr("List<Object>"),
                vec![
                    list_pattern(vec![wildcard()], None).then(vec![break_(None)]),
                    default_().then(vec![check_reachable(true)]),
                ],
            )]);
        }

        #[test]
        fn rest_pattern_with_subpattern_that_may_fail_to_match() {
            let mut h = set_up();
            h.run(vec![switch_(
                expr("List<Object>"),
                vec![
                    list_pattern(vec![rest_pattern(list_pattern(vec![], None))], None)
                        .then(vec![break_(None)]),
                    default_().then(vec![check_reachable(true)]),
                ],
            )]);
        }
    }

    mod guaranteed_to_match {
        use super::*;

        #[test]
        fn rest_pattern_with_no_subpattern() {
            let mut h = set_up();
            h.run(vec![switch_(
                expr("List<Object>"),
                vec![
                    list_pattern(vec![rest_pattern(None)], None).then(vec![break_(None)]),
                    default_().then(vec![check_reachable(false)]),
                ],
            )]);
        }

        #[test]
        fn rest_pattern_with_subpattern_that_always_matches() {
            let mut h = set_up();
            h.run(vec![switch_(
                expr("List<Object>"),
                vec![
                    list_pattern(vec![rest_pattern(wildcard())], None).then(vec![break_(None)]),
                    default_().then(vec![check_reachable(false)]),
                ],
            )]);
        }
    }

    #[test]
    fn promotes() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("Object?")),
            if_case(
                x,
                list_pattern(vec![wildcard()], Some("int")),
                vec![check_promoted(x, "List<int>")],
                None,
            ),
        ]);
    }

    #[test]
    fn doesnt_demote() {
        let mut h = set_up();
        let x = Var::new("x");
        let y = Var::new("y");
        h.run(vec![
            declare(x).with_initializer(expr("Object?")),
            if_case(
                x,
                wildcard()
                    .as_("List<int>")
                    .and(list_pattern(vec![], Some("num")))
                    .and(y.pattern().with_expect_inferred_type("List<int>")),
                vec![check_promoted(x, "List<int>")],
                None,
            ),
        ]);
    }

    #[test]
    fn reachability() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("List<int>")),
            if_case(
                x,
                list_pattern(vec![], Some("int")),
                vec![check_reachable(true)],
                Some(vec![check_reachable(true)]),
            ),
        ]);
    }

    mod demonstrated_type {
        use super::*;

        #[test]
        fn subtype_of_matched_value_type() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("(Iterable<Object>,)")),
                if_case(
                    x,
                    record_pattern(vec![
                        list_pattern(vec![wildcard().with_declared_type("int")], Some("num"))
                            .record_field(None),
                    ]),
                    vec![check_promoted(x, "(List<num>,)")],
                    None,
                ),
            ]);
        }

        #[test]
        fn supertype_of_matched_value_type() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("(List<num>,)")),
                if_case(
                    x,
                    record_pattern(vec![
                        list_pattern(vec![wildcard().with_declared_type("int")], Some("Object"))
                            .record_field(None),
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
                declare(x).with_initializer(expr("(List<int?>,)")),
                if_case(
                    x,
                    record_pattern(vec![
                        list_pattern(vec![wildcard().with_declared_type("int")], Some("num"))
                            .record_field(None),
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
                list_pattern(vec![], None),
                vec![check_promoted(
                    c.property("_property", false),
                    "List<Object?>",
                )],
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
                    list_pattern(vec![], None)
                        .when(expr("bool"))
                        .then(vec![check_promoted(
                            c.property("_property", false),
                            "List<Object?>",
                        )]),
                    wildcard()
                        .when(second(c.write(expr("C")), expr("bool")))
                        .then(vec![]),
                    list_pattern(vec![], None)
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
                list_pattern(vec![], None),
                vec![check_not_promoted(c.property("_property", false))],
                None,
            ),
        ]);
    }
}

mod null_aware_map_entry {
    use super::*;

    #[test]
    fn promotes_key_within_value() {
        let mut h = set_up();
        let a = Var::new("a");

        h.run(vec![
            declare(a)
                .with_declared_type("String?")
                .with_initializer(expr("String?")),
            map_literal(
                vec![map_entry(a, check_promoted(a, "String"), true)],
                "String",
                "dynamic",
            ),
            check_not_promoted(a),
        ]);
    }

    #[test]
    fn non_null_aware_key() {
        let mut h = set_up();
        let a = Var::new("a");

        h.run(vec![
            declare(a)
                .with_declared_type("String?")
                .with_initializer(expr("String?")),
            map_literal(
                vec![map_entry(a, check_not_promoted(a), false)],
                "String?",
                "dynamic",
            ),
            check_not_promoted(a),
        ]);
    }

    #[test]
    fn promotes() {
        let mut h = set_up();
        let a = Var::new("a");
        let x = Var::new("x");

        h.run(vec![
            declare(a)
                .with_declared_type("String")
                .with_initializer(expr("String")),
            declare(x)
                .with_declared_type("num")
                .with_initializer(expr("num")),
            map_literal(vec![map_entry(a, x.as_("int"), true)], "String", "dynamic"),
            check_promoted(x, "int"),
        ]);
    }

    #[test]
    fn affects_promotion() {
        let mut h = set_up();
        let a = Var::new("a");
        let x = Var::new("x");

        h.run(vec![
            declare(a)
                .with_declared_type("String?")
                .with_initializer(expr("String?")),
            declare(x)
                .with_declared_type("num")
                .with_initializer(expr("num")),
            map_literal(vec![map_entry(a, x.as_("int"), true)], "String", "dynamic"),
            check_not_promoted(x),
        ]);
    }

    #[test]
    fn unreachable() {
        let mut h = set_up();
        let a = Var::new("a");
        h.run(vec![
            declare(a)
                .with_declared_type("String")
                .with_initializer(expr("String")),
            map_literal(
                vec![map_entry(a, throw_(expr("Object")), true)],
                "String",
                "dynamic",
            ),
            check_reachable(false),
        ]);
    }

    #[test]
    fn reachable() {
        let mut h = set_up();
        let a = Var::new("a");
        h.run(vec![
            declare(a)
                .with_declared_type("String?")
                .with_initializer(expr("String?")),
            map_literal(
                vec![map_entry(a, throw_(expr("Object")), true)],
                "String",
                "dynamic",
            ),
            check_reachable(true),
        ]);
    }
}

mod map_pattern {
    use super::*;

    #[test]
    fn promotes() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("Object?")),
            if_case(
                x,
                map_pattern(
                    vec![map_pattern_entry(int_literal(0), wildcard())],
                    Some("int"),
                    Some("String"),
                ),
                vec![check_promoted(x, "Map<int, String>")],
                None,
            ),
        ]);
    }

    #[test]
    fn match_failure_reachable() {
        let mut h = set_up();
        h.run(vec![if_case(
            expr("Object?"),
            map_pattern(
                vec![map_pattern_entry(expr("Object"), wildcard())],
                None,
                None,
            ),
            vec![check_reachable(true)],
            Some(vec![check_reachable(true)]),
        )]);
    }

    #[test]
    fn doesnt_demote() {
        let mut h = set_up();
        let x = Var::new("x");
        let y = Var::new("y");
        h.run(vec![
            declare(x).with_initializer(expr("Object?")),
            if_case(
                x,
                wildcard()
                    .as_("Map<int, int>")
                    .and(map_pattern(
                        vec![map_pattern_entry(expr("Object"), wildcard())],
                        Some("num"),
                        Some("num"),
                    ))
                    .and(y.pattern().with_expect_inferred_type("Map<int, int>")),
                vec![check_promoted(x, "Map<int, int>")],
                None,
            ),
        ]);
    }

    #[test]
    fn reachability() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("Map<int, int>")),
            if_case(
                x,
                map_pattern(
                    vec![map_pattern_entry(expr("Object"), wildcard())],
                    Some("int"),
                    Some("int"),
                ),
                vec![check_reachable(true)],
                Some(vec![check_reachable(true)]),
            ),
        ]);
    }

    mod demonstrated_type {
        use super::*;

        #[test]
        fn subtype_of_matched_value_type() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("(Map<num?, Object>?,)")),
                if_case(
                    x,
                    record_pattern(vec![
                        map_pattern(
                            vec![map_pattern_entry(
                                int_literal(0),
                                wildcard().with_declared_type("int"),
                            )],
                            Some("int?"),
                            Some("num"),
                        )
                        .record_field(None),
                    ]),
                    vec![check_promoted(x, "(Map<int?, num>,)")],
                    None,
                ),
            ]);
        }

        #[test]
        fn supertype_of_matched_value_type() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("(Map<int, num>,)")),
                if_case(
                    x,
                    record_pattern(vec![
                        map_pattern(
                            vec![map_pattern_entry(
                                int_literal(0),
                                wildcard().with_declared_type("int"),
                            )],
                            Some("int"),
                            Some("Object"),
                        )
                        .record_field(None),
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
                declare(x).with_initializer(expr("(Map<int, int?>,)")),
                if_case(
                    x,
                    record_pattern(vec![
                        map_pattern(
                            vec![map_pattern_entry(
                                int_literal(0),
                                wildcard().with_declared_type("int"),
                            )],
                            Some("int"),
                            Some("num"),
                        )
                        .record_field(None),
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
                map_pattern(
                    vec![map_pattern_entry(int_literal(0), wildcard())],
                    None,
                    None,
                ),
                vec![check_promoted(
                    c.property("_property", false),
                    "Map<Object?, Object?>",
                )],
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
                    map_pattern(
                        vec![map_pattern_entry(int_literal(0), wildcard())],
                        None,
                        None,
                    )
                    .when(expr("bool"))
                    .then(vec![check_promoted(
                        c.property("_property", false),
                        "Map<Object?, Object?>",
                    )]),
                    wildcard()
                        .when(second(c.write(expr("C")), expr("bool")))
                        .then(vec![]),
                    map_pattern(
                        vec![map_pattern_entry(int_literal(0), wildcard())],
                        None,
                        None,
                    )
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
                map_pattern(
                    vec![map_pattern_entry(int_literal(0), wildcard())],
                    None,
                    None,
                ),
                vec![check_not_promoted(c.property("_property", false))],
                None,
            ),
        ]);
    }
}
