// Dart source: pkg/_fe_analyzer_shared/test/flow_analysis/flow_analysis_test.dart
// (lines 8703-9666: group 'Patterns:', groups 'Null-assert:', 'Null-check:',
// 'Object pattern:', 'Pattern assignment:', 'Pattern variable declaration:',
// 'Record pattern:' and 'Relational pattern:')

//! Dart group `Patterns:` (third part, first file).

use super::common::*;

mod null_assert {
    use super::*;

    #[test]
    fn throws_if_not_null() {
        let mut h = set_up();
        h.run(vec![if_case(
            expr("Object?"),
            wildcard().null_assert(),
            vec![],
            Some(vec![check_reachable(false)]),
        )]);
    }

    mod scrutinee_promotion {
        use super::*;

        #[test]
        fn if_changed() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("Object?")),
                switch_(
                    x,
                    vec![
                        wildcard()
                            .when(second(x.write(expr("Object?")), expr("bool")))
                            .then(vec![break_(None)]),
                        wildcard().null_assert().then(vec![check_not_promoted(x)]),
                    ],
                ),
            ]);
        }

        #[test]
        fn if_unchanged() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("Object?")),
                if_case(
                    x,
                    wildcard().null_assert(),
                    vec![check_promoted(x, "Object")],
                    Some(vec![check_not_promoted(x)]),
                ),
            ]);
        }

        #[test]
        fn if_subpattern() {
            // Equivalent Dart code:
            //     typedef T = int?;
            //     extension on T {
            //       dynamic get foo { ... }
            //     }
            //     f(Object? x) {
            //       if (x case T(foo: _!)) {
            //         // x still might be `null`
            //       }
            //     }
            let mut h = set_up();
            TypeRegistry::add_interface_type_name("T");
            h.add_downward_infer("T", "Object?", "int?");
            h.add_member("int?", "foo", Some("dynamic"), false, None);
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("Object?")),
                if_case(
                    x,
                    object_pattern(
                        "T",
                        vec![wildcard().null_assert().record_field(Some("foo"))],
                    ),
                    vec![check_promoted(x, "int?")],
                    None,
                ),
            ]);
        }

        #[test]
        fn if_promotable_property() {
            let mut h = set_up();
            h.add_member("C", "_property", Some("int?"), true, None);
            let c = Var::new("c");
            h.run(vec![
                declare(c).with_initializer(expr("C")),
                if_case(
                    c.property("_property", false),
                    wildcard().null_assert(),
                    vec![check_promoted(c.property("_property", false), "int")],
                    None,
                ),
            ]);
        }

        #[test]
        fn if_promotable_property_target_changed() {
            let mut h = set_up();
            h.add_member("C", "_property", Some("int?"), true, None);
            let c = Var::new("c");
            h.run_with(
                vec![
                    declare(c).with_initializer(expr("C")),
                    switch_(
                        c.property("_property", false),
                        vec![
                            wildcard()
                                .null_assert()
                                .when(expr("bool"))
                                .then(vec![check_promoted(c.property("_property", false), "int")]),
                            wildcard()
                                .when(second(c.write(expr("C")), expr("bool")))
                                .then(vec![]),
                            wildcard()
                                .null_assert()
                                .error_id("SECOND_NULL_ASSERT")
                                .then(vec![check_not_promoted(c.property("_property", false))]),
                        ],
                    ),
                ],
                errors(&["matchedTypeIsStrictlyNonNullable(\
                              pattern: SECOND_NULL_ASSERT, matchedType: int)"]),
            );
        }

        #[test]
        fn if_non_promotable_property() {
            let mut h = set_up();
            h.add_member("C", "_property", Some("int?"), false, None);
            let c = Var::new("c");
            h.run(vec![
                declare(c).with_initializer(expr("C")),
                if_case(
                    c.property("_property", false),
                    wildcard().null_assert(),
                    vec![check_not_promoted(c.property("_property", false))],
                    None,
                ),
            ]);
        }
    }

    #[test]
    fn promotes_temporary_variable() {
        let mut h = set_up();
        h.run_with(
            vec![if_case(
                expr("Object?"),
                wildcard().null_assert().and(
                    wildcard()
                        .with_expect_inferred_type("Object")
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
    fn unreachable_if_null() {
        let mut h = set_up();
        h.run(vec![if_case(
            expr("Null"),
            wildcard().null_assert(),
            vec![check_reachable(false)],
            None,
        )]);
    }

    #[test]
    fn reachable_otherwise() {
        let mut h = set_up();
        h.run(vec![if_case(
            expr("Object?"),
            wildcard().null_assert(),
            vec![check_reachable(true)],
            None,
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
                    .as_("int?")
                    .and(wildcard().null_assert())
                    .and(y.pattern().with_expect_inferred_type("int")),
                vec![check_promoted(x, "int")],
                None,
            ),
        ]);
    }

    #[test]
    fn demonstrated_type() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("(int?,)")),
            if_case(
                x,
                record_pattern(vec![wildcard().null_assert().record_field(None)]),
                vec![check_promoted(x, "(int,)")],
                None,
            ),
        ]);
    }
}

mod null_check {
    use super::*;

    #[test]
    fn might_not_match() {
        let mut h = set_up();
        h.run(vec![if_case(
            expr("Object?"),
            wildcard().null_check(),
            vec![],
            Some(vec![check_reachable(true)]),
        )]);
    }

    mod scrutinee_promotion {
        use super::*;

        #[test]
        fn if_changed() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("Object?")),
                switch_(
                    x,
                    vec![
                        wildcard()
                            .when(second(x.write(expr("Object?")), expr("bool")))
                            .then(vec![break_(None)]),
                        wildcard().null_check().then(vec![check_not_promoted(x)]),
                    ],
                ),
            ]);
        }

        #[test]
        fn if_unchanged() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("Object?")),
                if_case(
                    x,
                    wildcard().null_check(),
                    vec![check_promoted(x, "Object")],
                    Some(vec![check_not_promoted(x)]),
                ),
            ]);
        }

        #[test]
        fn if_subpattern() {
            // Equivalent Dart code:
            //     typedef T = int?;
            //     extension on T {
            //       dynamic get foo { ... }
            //     }
            //     f(Object? x) {
            //       if (x case T(foo: _?)) {
            //         // x still might be `null`
            //       }
            //     }
            let mut h = set_up();
            TypeRegistry::add_interface_type_name("T");
            h.add_downward_infer("T", "Object?", "int?");
            h.add_member("int?", "foo", Some("dynamic"), false, None);
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("Object?")),
                if_case(
                    x,
                    object_pattern("T", vec![wildcard().null_check().record_field(Some("foo"))]),
                    vec![check_promoted(x, "int?")],
                    None,
                ),
            ]);
        }

        #[test]
        fn if_promotable_property() {
            let mut h = set_up();
            h.add_member("C", "_property", Some("int?"), true, None);
            let c = Var::new("c");
            h.run(vec![
                declare(c).with_initializer(expr("C")),
                if_case(
                    c.property("_property", false),
                    wildcard().null_check(),
                    vec![check_promoted(c.property("_property", false), "int")],
                    None,
                ),
            ]);
        }

        #[test]
        fn if_promotable_property_target_changed() {
            let mut h = set_up();
            h.add_member("C", "_property", Some("int?"), true, None);
            let c = Var::new("c");
            h.run(vec![
                declare(c).with_initializer(expr("C")),
                switch_(
                    c.property("_property", false),
                    vec![
                        wildcard()
                            .null_check()
                            .when(expr("bool"))
                            .then(vec![check_promoted(c.property("_property", false), "int")]),
                        wildcard()
                            .when(second(c.write(expr("C")), expr("bool")))
                            .then(vec![]),
                        wildcard()
                            .null_check()
                            .then(vec![check_not_promoted(c.property("_property", false))]),
                    ],
                ),
            ]);
        }

        #[test]
        fn if_non_promotable_property() {
            let mut h = set_up();
            h.add_member("C", "_property", Some("int?"), false, None);
            let c = Var::new("c");
            h.run(vec![
                declare(c).with_initializer(expr("C")),
                if_case(
                    c.property("_property", false),
                    wildcard().null_check(),
                    vec![check_not_promoted(c.property("_property", false))],
                    None,
                ),
            ]);
        }
    }

    #[test]
    fn promotes_temporary_variable() {
        let mut h = set_up();
        h.run_with(
            vec![if_case(
                expr("Object?"),
                wildcard().null_check().and(
                    wildcard()
                        .with_expect_inferred_type("Object")
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
    fn unreachable_if_null() {
        let mut h = set_up();
        h.run(vec![if_case(
            expr("Null"),
            wildcard().null_check(),
            vec![check_reachable(false)],
            None,
        )]);
    }

    #[test]
    fn reachable_otherwise() {
        let mut h = set_up();
        h.run(vec![if_case(
            expr("Object?"),
            wildcard().null_check(),
            vec![check_reachable(true)],
            None,
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
                    .as_("int?")
                    .and(wildcard().null_check())
                    .and(y.pattern().with_expect_inferred_type("int")),
                vec![check_promoted(x, "int")],
                None,
            ),
        ]);
    }

    #[test]
    fn demonstrated_type() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("(int?,)")),
            if_case(
                x,
                record_pattern(vec![wildcard().null_check().record_field(None)]),
                vec![check_promoted(x, "(int,)")],
                None,
            ),
        ]);
    }
}

mod object_pattern {
    use super::*;

    #[test]
    fn promotes() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("Object?")),
            if_case(
                x,
                object_pattern("int", vec![]),
                vec![check_promoted(x, "int")],
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
                    .as_("int")
                    .and(object_pattern("num", vec![]))
                    .and(y.pattern().with_expect_inferred_type("int")),
                vec![check_promoted(x, "int")],
                None,
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
                declare(x).with_initializer(expr("(num,)")),
                if_case(
                    x,
                    record_pattern(vec![object_pattern("int", vec![]).record_field(None)]),
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
                    record_pattern(vec![object_pattern("Object", vec![]).record_field(None)]),
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
                    record_pattern(vec![object_pattern("String", vec![]).record_field(None)]),
                    vec![check_not_promoted(x)],
                    None,
                ),
            ]);
        }
    }

    #[test]
    fn read_of_never_typed_getter_makes_unreachable() {
        let mut h = set_up();
        h.add_downward_infer("A", "Object", "A");
        h.add_member("A", "foo", Some("Never"), false, None);
        h.run(vec![if_case(
            expr("Object"),
            object_pattern(
                "A",
                vec![Var::new("foo").pattern().record_field(Some("foo"))],
            ),
            vec![check_reachable(false)],
            None,
        )]);
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
                object_pattern("int", vec![]),
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
                    object_pattern("int", vec![])
                        .when(expr("bool"))
                        .then(vec![check_promoted(c.property("_property", false), "int")]),
                    wildcard()
                        .when(second(c.write(expr("C")), expr("bool")))
                        .then(vec![]),
                    object_pattern("int", vec![])
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
                object_pattern("int", vec![]),
                vec![check_not_promoted(c.property("_property", false))],
                None,
            ),
        ]);
    }
}

mod pattern_assignment {
    use super::*;

    #[test]
    fn does_not_promote_rhs() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("num")),
            wildcard().as_("int").assign(x),
            check_not_promoted(x),
        ]);
    }
}

mod pattern_variable_declaration {
    use super::*;

    #[test]
    fn does_not_promote_rhs() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("num")),
            pattern_variable_declaration(wildcard().as_("int"), x, false),
            check_not_promoted(x),
        ]);
    }
}

mod record_pattern {
    use super::*;

    #[test]
    fn simple_promotion() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("Object?")),
            if_case(
                x,
                record_pattern(vec![wildcard().record_field(None)]),
                vec![check_promoted(x, "(Object?,)")],
                None,
            ),
        ]);
    }

    mod promote_to_demonstrated_type {
        use super::*;

        #[test]
        fn unnamed_fields() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("Object?")),
                if_case(
                    x,
                    record_pattern(vec![
                        wildcard().with_declared_type("int").record_field(None),
                        wildcard().with_declared_type("String").record_field(None),
                    ]),
                    vec![check_promoted(x, "(int, String)")],
                    None,
                ),
            ]);
        }

        #[test]
        fn named_fields() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("Object?")),
                if_case(
                    x,
                    record_pattern(vec![
                        wildcard().with_declared_type("int").record_field(Some("i")),
                        wildcard()
                            .with_declared_type("String")
                            .record_field(Some("s")),
                    ]),
                    vec![check_promoted(x, "({int i, String s})")],
                    None,
                ),
            ]);
        }
    }

    #[test]
    fn required_type_is_a_type_of_interest() {
        // The required type is `(Object?,)`.  Since that's the type used in the
        // desugared type test, it's considered a type of interest even though
        // the scrutinee is initially promoted to `(int,)`.
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("Object")),
            if_case(
                x,
                record_pattern(vec![
                    wildcard().with_declared_type("int").record_field(None),
                ]),
                vec![
                    check_promoted(x, "(int,)"),
                    x.write(expr("(num,)")),
                    check_promoted(x, "(Object?,)"),
                ],
                None,
            ),
        ]);
    }

    #[test]
    fn promotion_to_demonstrated_type_cannot_fail() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("(Object?,)")),
            if_case(
                x,
                record_pattern(vec![wildcard().as_("int").record_field(None)]),
                vec![check_promoted(x, "(int,)")],
                Some(vec![check_reachable(false)]),
            ),
        ]);
    }

    #[test]
    fn match_failure_reachable() {
        let mut h = set_up();
        h.run(vec![if_case(
            expr("Object?"),
            record_pattern(vec![wildcard().record_field(None)]),
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
                    .as_("(int,)")
                    .and(record_pattern(vec![wildcard().record_field(None)]))
                    .and(y.pattern().with_expect_inferred_type("(int,)")),
                vec![check_promoted(x, "(int,)")],
                None,
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
                declare(x).with_initializer(expr("((num,),)")),
                if_case(
                    x,
                    record_pattern(vec![
                        record_pattern(vec![
                            wildcard().with_declared_type("int").record_field(None),
                        ])
                        .record_field(None),
                    ]),
                    vec![check_promoted(x, "((int,),)")],
                    None,
                ),
            ]);
        }

        #[test]
        fn supertype_of_matched_value_type() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("Never")),
                if_case(
                    x,
                    record_pattern(vec![
                        record_pattern(vec![
                            wildcard().with_declared_type("num").record_field(None),
                        ])
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
                declare(x).with_initializer(expr("String")),
                if_case(
                    x,
                    record_pattern(vec![
                        record_pattern(vec![
                            wildcard().with_declared_type("num").record_field(None),
                        ])
                        .record_field(None),
                    ]),
                    vec![check_not_promoted(x)],
                    None,
                ),
            ]);
        }
    }

    #[test]
    fn error_type_does_not_alter_previous_reachability_conclusions() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("(Null, Object?)")),
            if_case(
                x,
                record_pattern(vec![
                    relational_pattern("!=", null_literal()).record_field(None),
                    wildcard().with_declared_type("error").record_field(None),
                ]),
                vec![check_reachable(false)],
                Some(vec![check_reachable(true)]),
            ),
        ]);
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
                record_pattern(vec![]),
                vec![check_promoted(c.property("_property", false), "()")],
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
                    record_pattern(vec![])
                        .when(expr("bool"))
                        .then(vec![check_promoted(c.property("_property", false), "()")]),
                    wildcard()
                        .when(second(c.write(expr("C")), expr("bool")))
                        .then(vec![]),
                    record_pattern(vec![])
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
                record_pattern(vec![]),
                vec![check_not_promoted(c.property("_property", false))],
                None,
            ),
        ]);
    }
}

mod relational_pattern {
    use super::*;

    mod eq {
        use super::*;

        #[test]
        fn guaranteed_match_due_to_null_type() {
            let mut h = set_up();
            h.run(vec![if_case(
                expr("Null"),
                relational_pattern("==", null_literal()),
                vec![check_reachable(true)],
                Some(vec![check_reachable(false)]),
            )]);
        }

        #[test]
        fn guaranteed_match_due_to_null_type_in_subpattern() {
            let mut h = set_up();
            h.run(vec![if_case(
                expr("(Null,)"),
                record_pattern(vec![
                    relational_pattern("==", null_literal()).record_field(None),
                ]),
                vec![check_reachable(true)],
                Some(vec![check_reachable(false)]),
            )]);
        }

        #[test]
        fn in_the_general_case_may_or_may_not_match() {
            let mut h = set_up();
            h.run(vec![if_case(
                expr("Object?"),
                relational_pattern("==", int_literal(0)),
                vec![check_reachable(true)],
                Some(vec![check_reachable(true)]),
            )]);
        }

        #[test]
        fn using_dot_shorthands_in_relational_pattern() {
            let mut h = set_up();
            h.add_member("C", "field", Some("C"), false, None);
            h.run(vec![if_case(
                expr("C"),
                relational_pattern("==", dot_shorthand_head("field").dot_shorthand()),
                vec![check_reachable(true)],
                None,
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
                    relational_pattern("==", null_literal()),
                    vec![check_reachable(true), check_not_promoted(x)],
                    Some(vec![check_reachable(true), check_promoted(x, "int")]),
                ),
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
                        relational_pattern("==", null_literal())
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
                relational_pattern("==", null_literal())
                    .or(wildcard().with_expect_inferred_type("int")),
                vec![],
                None,
            )]);
        }

        mod demonstrated_type {
            use super::*;

            #[test]
            fn eq_value() {
                // The demonstrated type of a relational pattern using `==` is the
                // matched value type.
                let mut h = set_up();
                let x = Var::new("x");
                h.run(vec![
                    declare(x).with_initializer(expr("(Object?,)")),
                    if_case(
                        x,
                        record_pattern(vec![
                            relational_pattern("==", expr("Object")).record_field(None),
                        ]),
                        vec![check_not_promoted(x)],
                        None,
                    ),
                ]);
            }

            #[test]
            fn eq_null() {
                // The demonstrated type of a relational pattern using `==` is the
                // matched value type, even in the case of `== null`, because we
                // don't promote to the `Null` type.
                let mut h = set_up();
                let x = Var::new("x");
                h.run(vec![
                    declare(x).with_initializer(expr("(Object?,)")),
                    if_case(
                        x,
                        record_pattern(vec![
                            relational_pattern("==", null_literal()).record_field(None),
                        ]),
                        vec![check_not_promoted(x)],
                        None,
                    ),
                ]);
            }
        }
    }

    mod not_eq {
        use super::*;

        #[test]
        fn guaranteed_mismatch_due_to_null_type() {
            let mut h = set_up();
            h.run(vec![if_case(
                expr("Null"),
                relational_pattern("!=", null_literal()),
                vec![check_reachable(false)],
                Some(vec![check_reachable(true)]),
            )]);
        }

        #[test]
        fn in_the_general_case_may_or_may_not_match() {
            let mut h = set_up();
            h.run(vec![if_case(
                expr("Object?"),
                relational_pattern("!=", int_literal(0)),
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
                    relational_pattern("!=", null_literal()),
                    vec![check_reachable(true), check_promoted(x, "int")],
                    Some(vec![check_reachable(true), check_not_promoted(x)]),
                ),
            ]);
        }

        #[test]
        fn null_pattern_doesnt_promote_changed_scrutinee() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run_with(
                vec![
                    declare(x).with_initializer(expr("int?")),
                    switch_(
                        x,
                        vec![
                            wildcard()
                                .when(second(x.write(expr("int?")), expr("bool")))
                                .then(vec![break_(None)]),
                            relational_pattern("!=", null_literal())
                                .and(
                                    wildcard()
                                        .with_expect_inferred_type("int")
                                        .error_id("WILDCARD"),
                                )
                                .then(vec![check_reachable(true), check_not_promoted(x)]),
                        ],
                    ),
                ],
                errors(&["unnecessaryWildcardPattern(pattern: WILDCARD, \
                              kind: logicalAndPatternOperand)"]),
            );
        }

        #[test]
        fn null_pattern_promotes_matched_pattern_var() {
            let mut h = set_up();
            h.run_with(
                vec![if_case(
                    expr("int?"),
                    relational_pattern("!=", null_literal()).and(
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

        mod demonstrated_type {
            use super::*;

            #[test]
            fn not_eq_value() {
                // The demonstrated type of a relational pattern using `!=` is
                // usually the matched value type.
                let mut h = set_up();
                let x = Var::new("x");
                h.run(vec![
                    declare(x).with_initializer(expr("(Object?,)")),
                    if_case(
                        x,
                        record_pattern(vec![
                            relational_pattern("!=", expr("Object")).record_field(None),
                        ]),
                        vec![check_not_promoted(x)],
                        None,
                    ),
                ]);
            }

            #[test]
            fn not_eq_null() {
                // The demonstrated type of the relational pattern `!= null` is the
                // matched value type promoted to non-nullable.
                let mut h = set_up();
                let x = Var::new("x");
                h.run(vec![
                    declare(x).with_initializer(expr("(Object?,)")),
                    if_case(
                        x,
                        record_pattern(vec![
                            relational_pattern("!=", null_literal()).record_field(None),
                        ]),
                        vec![check_promoted(x, "(Object,)")],
                        None,
                    ),
                ]);
            }
        }
    }

    mod other {
        use super::*;

        #[test]
        fn does_not_assume_anything_even_though_eq_or_not_eq_would() {
            // This is a bit of a contrived test case, since it exercises
            // `null < null`.  But such a thing is possible with extension
            // methods.
            let mut h = set_up();
            h.add_member("Null", "<", Some("bool Function(Object?)"), false, None);
            h.run(vec![if_case(
                expr("Null"),
                relational_pattern("<", null_literal()),
                vec![check_reachable(true)],
                Some(vec![check_reachable(true)]),
            )]);
        }

        #[test]
        fn demonstrated_type() {
            // The demonstrated type of a relational pattern using a
            // non-equality operator is the matched value type.
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("(int,)")),
                if_case(
                    x,
                    record_pattern(vec![
                        relational_pattern(">", int_literal(0)).record_field(None),
                    ]),
                    vec![check_not_promoted(x)],
                    None,
                ),
            ]);
        }
    }
}
