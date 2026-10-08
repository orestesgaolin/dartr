// Dart source: pkg/_fe_analyzer_shared/test/type_inference/type_inference_test.dart (group "Patterns:", Null-assert to Wildcard)

//! The shared type analyzer tests, run through the mini-AST harness.
//!
//! Dart groups are nested modules, Dart tests are functions (snake case, in
//! Dart order). Tests that check results of flow analysis (promotion,
//! reachability) are `#[ignore = "needs flow analysis"]` until the real flow
//! analysis replaces the [`MiniFlow`](mini_ast::mini_flow::MiniFlow)
//! stand-in.

mod mini_ast;

#[allow(unused_imports)]
use mini_ast::harness::{BodyContext, Harness, RunOptions};
#[allow(unused_imports)]
use mini_ast::mini_types::{
    PrimaryType, Type, TypeRegistry, TypeRegistryScope, type_registry_scope,
};
#[allow(unused_imports)]
use mini_ast::node::*;

/// Dart `setUp` (and `tearDown` when the scope is dropped).
fn set_up() -> (TypeRegistryScope, Harness) {
    let scope = type_registry_scope();
    for name in ["A", "B", "B1", "B2", "C", "C1", "C2", "E", "X"] {
        TypeRegistry::add_interface_type_name(name);
    }
    (scope, Harness::new())
}

/// Dart `h.run(statements, expectedErrors: {...})`.
#[allow(dead_code)]
fn errors(expected: &[&str]) -> RunOptions {
    RunOptions {
        expected_errors: expected.iter().map(|s| s.to_string()).collect(),
        ..RunOptions::default()
    }
}

mod patterns {
    use super::*;

    mod null_assert {
        use super::*;

        #[test]
        fn type_schema() {
            let (_s, mut h) = set_up();
            let x = Var::new("x");
            h.run_with(
                vec![
                    pattern_variable_declaration(
                        x.pattern()
                            .with_declared_type("int")
                            .null_assert()
                            .error_id("PATTERN"),
                        expr("int").check_schema("int?"),
                        false,
                    )
                    .check_ir(
                        "match(expr(int), \
                     nullAssertPattern(varPattern(x, matchedType: int, \
                     staticType: int), matchedType: int))",
                    ),
                ],
                errors(&["matchedTypeIsStrictlyNonNullable(pattern: PATTERN, \
                          matchedType: int)"]),
            );
        }

        mod refutability {
            use super::*;

            #[test]
            fn when_matched_type_is_nullable() {
                let (_s, mut h) = set_up();
                h.run(vec![
                    pattern_variable_declaration(wildcard().null_assert(), expr("int?"), false)
                        .check_ir(
                            "match(expr(int?), nullAssertPattern(\
                     wildcardPattern(matchedType: int), matchedType: int?))",
                        ),
                ]);
            }

            #[test]
            fn when_matched_type_is_non_nullable() {
                let (_s, mut h) = set_up();
                h.run_with(
                    vec![
                        pattern_variable_declaration(
                            wildcard().null_assert().error_id("PATTERN"),
                            expr("int"),
                            false,
                        )
                        .check_ir(
                            "match(expr(int), nullAssertPattern(\
                         wildcardPattern(matchedType: int), matchedType: int))",
                        ),
                    ],
                    errors(&["matchedTypeIsStrictlyNonNullable(pattern: PATTERN, \
                              matchedType: int)"]),
                );
            }

            #[test]
            fn when_matched_type_is_dynamic() {
                let (_s, mut h) = set_up();
                h.run(vec![
                    pattern_variable_declaration(wildcard().null_assert(), expr("dynamic"), false)
                        .check_ir(
                            "match(expr(dynamic), nullAssertPattern(\
                     wildcardPattern(matchedType: dynamic), \
                     matchedType: dynamic))",
                        ),
                ]);
            }

            #[test]
            fn sub_refutability() {
                let (_s, mut h) = set_up();
                h.run_with(
                    vec![
                        pattern_variable_declaration(
                            wildcard()
                                .with_declared_type("int")
                                .error_id("INT")
                                .null_assert()
                                .error_id("PATTERN"),
                            expr("num"),
                            false,
                        )
                        .error_id("CONTEXT"),
                    ],
                    errors(&[
                        "matchedTypeIsStrictlyNonNullable(pattern: PATTERN, \
                         matchedType: num)",
                        "patternTypeMismatchInIrrefutableContext(pattern: INT, \
                         context: CONTEXT, matchedType: num, requiredType: int)",
                    ]),
                );
            }
        }

        mod refutable {
            use super::*;

            #[test]
            fn when_matched_type_is_nullable() {
                let (_s, mut h) = set_up();
                h.run(vec![
                    if_case(expr("int?"), wildcard().null_assert(), vec![], None).check_ir(
                        "ifCase(expr(int?), nullAssertPattern(wildcardPattern(\
                         matchedType: int), matchedType: int?), variables(), true, \
                         block(), noop)",
                    ),
                ]);
            }

            #[test]
            fn when_matched_type_is_non_nullable() {
                let (_s, mut h) = set_up();
                h.run_with(
                    vec![
                        if_case(
                            expr("int"),
                            wildcard().null_assert().error_id("PATTERN"),
                            vec![],
                            None,
                        )
                        .check_ir(
                            "ifCase(expr(int), nullAssertPattern(wildcardPattern(\
                         matchedType: int), matchedType: int), variables(), true, \
                         block(), noop)",
                        ),
                    ],
                    errors(&["matchedTypeIsStrictlyNonNullable(pattern: PATTERN, \
                              matchedType: int)"]),
                );
            }
        }
    }

    mod null_check {
        use super::*;

        #[test]
        fn type_schema() {
            let (_s, mut h) = set_up();
            let x = Var::new("x");
            h.run_with(
                vec![
                    pattern_variable_declaration(
                        x.pattern()
                            .with_declared_type("int")
                            .null_check()
                            .error_id("PATTERN"),
                        expr("int").check_schema("_"),
                        false,
                    )
                    .error_id("CONTEXT"),
                ],
                errors(&["refutablePatternInIrrefutableContext(pattern: PATTERN, \
                          context: CONTEXT)"]),
            );
        }

        mod refutability {
            use super::*;

            #[test]
            fn when_matched_type_is_nullable() {
                let (_s, mut h) = set_up();
                h.run_with(
                    vec![
                        pattern_variable_declaration(
                            wildcard().null_check().error_id("PATTERN"),
                            expr("int?"),
                            false,
                        )
                        .error_id("CONTEXT"),
                    ],
                    errors(&["refutablePatternInIrrefutableContext(pattern: PATTERN, \
                              context: CONTEXT)"]),
                );
            }

            #[test]
            fn when_matched_type_is_non_nullable() {
                let (_s, mut h) = set_up();
                h.run_with(
                    vec![
                        pattern_variable_declaration(
                            wildcard().null_check().error_id("PATTERN"),
                            expr("int"),
                            false,
                        )
                        .error_id("CONTEXT"),
                    ],
                    errors(&["refutablePatternInIrrefutableContext(pattern: PATTERN, \
                              context: CONTEXT)"]),
                );
            }

            #[test]
            fn when_matched_type_is_dynamic() {
                let (_s, mut h) = set_up();
                h.run_with(
                    vec![
                        pattern_variable_declaration(
                            wildcard().null_check().error_id("PATTERN"),
                            expr("dynamic"),
                            false,
                        )
                        .error_id("CONTEXT"),
                    ],
                    errors(&["refutablePatternInIrrefutableContext(pattern: PATTERN, \
                              context: CONTEXT)"]),
                );
            }

            #[test]
            fn sub_refutability() {
                let (_s, mut h) = set_up();
                h.run_with(
                    vec![
                        pattern_variable_declaration(
                            wildcard()
                                .with_declared_type("int")
                                .null_check()
                                .error_id("PATTERN"),
                            expr("num"),
                            false,
                        )
                        .error_id("CONTEXT"),
                    ],
                    errors(&["refutablePatternInIrrefutableContext(pattern: PATTERN, \
                              context: CONTEXT)"]),
                );
            }
        }

        mod refutable {
            use super::*;

            #[test]
            fn when_matched_type_is_nullable() {
                let (_s, mut h) = set_up();
                h.run(vec![
                    if_case(expr("int?"), wildcard().null_check(), vec![], None).check_ir(
                        "ifCase(expr(int?), nullCheckPattern(wildcardPattern(\
                         matchedType: int), matchedType: int?), variables(), true, \
                         block(), noop)",
                    ),
                ]);
            }

            #[test]
            fn when_matched_type_is_non_nullable() {
                let (_s, mut h) = set_up();
                h.run_with(
                    vec![
                        if_case(
                            expr("int"),
                            wildcard().null_check().error_id("PATTERN"),
                            vec![],
                            None,
                        )
                        .check_ir(
                            "ifCase(expr(int), nullCheckPattern(wildcardPattern(\
                         matchedType: int), matchedType: int), variables(), true, \
                         block(), noop)",
                        ),
                    ],
                    errors(&["matchedTypeIsStrictlyNonNullable(pattern: PATTERN, \
                              matchedType: int)"]),
                );
            }
        }
    }

    mod object {
        use super::*;

        mod refutable {
            use super::*;

            #[test]
            fn inferred() {
                let (_s, mut h) = set_up();
                let a = TypeRegistry::lookup("A");
                h.add_downward_infer("B", "A<int>", "B<int>");
                h.add_member("B<int>", "foo", Some("int"), false, None);
                h.add_super_interfaces("B", move |args| {
                    vec![PrimaryType::new(a, args.to_vec()), Type::parse("Object")]
                });
                h.add_super_interfaces("A", |_| vec![Type::parse("Object")]);
                h.run(vec![
                    if_case(
                        expr("A<int>").check_schema("_"),
                        object_pattern(
                            "B",
                            vec![Var::new("foo").pattern().record_field(Some("foo"))],
                        ),
                        vec![],
                        None,
                    )
                    .check_ir(
                        "ifCase(expr(A<int>), objectPattern(varPattern(foo, \
                     matchedType: int, staticType: int), matchedType: A<int>, \
                     requiredType: B<int>), variables(foo), true, block(), noop)",
                    ),
                ]);
            }

            #[test]
            fn dynamic_type() {
                let (_s, mut h) = set_up();
                h.run(vec![
                    if_case(
                        expr("int").check_schema("_"),
                        object_pattern(
                            "dynamic",
                            vec![Var::new("foo").pattern().record_field(Some("foo"))],
                        ),
                        vec![],
                        None,
                    )
                    .check_ir(
                        "ifCase(expr(int), objectPattern(varPattern(foo, \
                     matchedType: dynamic, staticType: dynamic), matchedType: int, \
                     requiredType: dynamic), variables(foo), true, block(), noop)",
                    ),
                ]);
            }

            #[test]
            fn error_type() {
                let (_s, mut h) = set_up();
                h.run(vec![
                    if_case(
                        expr("int").check_schema("_"),
                        object_pattern(
                            "error",
                            vec![Var::new("foo").pattern().record_field(Some("foo"))],
                        ),
                        vec![],
                        None,
                    )
                    .check_ir(
                        "ifCase(expr(int), objectPattern(varPattern(foo, \
                     matchedType: error, staticType: error), matchedType: int, \
                     requiredType: error), variables(foo), true, block(), noop)",
                    ),
                ]);
            }

            #[test]
            fn never_type() {
                let (_s, mut h) = set_up();
                h.run(vec![
                    if_case(
                        expr("int").check_schema("_"),
                        object_pattern(
                            "Never",
                            vec![Var::new("foo").pattern().record_field(Some("foo"))],
                        ),
                        vec![],
                        None,
                    )
                    .check_ir(
                        "ifCase(expr(int), objectPattern(varPattern(foo, \
                     matchedType: Never, staticType: Never), matchedType: int, \
                     requiredType: Never), variables(foo), true, block(), noop)",
                    ),
                ]);
            }

            #[test]
            fn duplicate_field_name() {
                let (_s, mut h) = set_up();
                h.add_member("A<int>", "foo", Some("int"), false, None);
                h.run_with(
                    vec![if_case(
                        expr("A<int>"),
                        object_pattern(
                            "A<int>",
                            vec![
                                Var::new("a")
                                    .pattern()
                                    .record_field(Some("foo"))
                                    .error_id("ORIGINAL"),
                                Var::new("b")
                                    .pattern()
                                    .record_field(Some("foo"))
                                    .error_id("DUPLICATE"),
                            ],
                        )
                        .error_id("PATTERN"),
                        vec![],
                        None,
                    )],
                    errors(&["duplicateRecordPatternField(\
                              objectOrRecordPattern: PATTERN, \
                              name: foo, original: ORIGINAL, \
                              duplicate: DUPLICATE)"]),
                );
            }
        }

        mod irrefutable {
            use super::*;

            #[test]
            fn assignable() {
                let (_s, mut h) = set_up();
                h.add_member("num", "foo", Some("bool"), false, None);
                h.run(vec![
                    pattern_variable_declaration(
                        object_pattern(
                            "num",
                            vec![Var::new("foo").pattern().record_field(Some("foo"))],
                        ),
                        expr("int").check_schema("num"),
                        false,
                    )
                    .check_ir(
                        "match(expr(int), objectPattern(varPattern(foo, \
                     matchedType: bool, staticType: bool), \
                     matchedType: int, requiredType: num))",
                    ),
                ]);
            }

            #[test]
            fn not_assignable() {
                let (_s, mut h) = set_up();
                h.add_member("int", "foo", Some("bool"), false, None);
                h.run_with(
                    vec![
                        pattern_variable_declaration(
                            object_pattern(
                                "int",
                                vec![Var::new("foo").pattern().record_field(Some("foo"))],
                            )
                            .error_id("PATTERN"),
                            expr("num").check_schema("int"),
                            false,
                        )
                        .error_id("CONTEXT")
                        .check_ir(
                            "match(expr(num), objectPattern(varPattern(foo, \
                         matchedType: bool, staticType: bool), \
                         matchedType: num, requiredType: int))",
                        ),
                    ],
                    errors(
                        &["patternTypeMismatchInIrrefutableContext(pattern: PATTERN, \
                              context: CONTEXT, matchedType: num, requiredType: int)"],
                    ),
                );
            }
        }
    }

    mod pattern_assignment {
        use super::*;

        mod static_type {
            use super::*;

            #[test]
            fn matched_type_is_int() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run(vec![
                    declare(x).with_declared_type("int"),
                    x.pattern().assign(expr("int")).check_type("int"),
                ]);
            }

            #[test]
            fn matched_type_is_error() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run(vec![
                    declare(x).with_declared_type("int"),
                    x.pattern().assign(expr("error")).check_type("error"),
                ]);
            }
        }

        #[test]
        fn rhs_schema() {
            let (_s, mut h) = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_declared_type("num"),
                x.pattern()
                    .assign(expr("int").check_schema("num"))
                    .check_expression_type_analysis_result(|result| {
                        assert_eq!(result.pattern_schema.unwrap().to_string(), "num");
                    })
                    .in_type_schema("Object"),
            ]);
        }

        #[test]
        fn duplicate_assignment_to_same_variable() {
            let (_s, mut h) = set_up();
            let x = Var::new("x").error_id("x");
            h.run_with(
                vec![
                    declare(x).with_declared_type("num"),
                    record_pattern(vec![
                        x.pattern().error_id("x1").record_field(None),
                        x.pattern().error_id("x2").record_field(None),
                    ])
                    .assign(expr("(int, int)")),
                ],
                errors(&[
                    "duplicateAssignmentPatternVariable(variable: x, original: x1, \
                          duplicate: x2)",
                ]),
            );
        }

        mod refutability {
            use super::*;

            #[test]
            fn when_matched_type_is_a_subtype_of_variable_type() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run(vec![
                    declare(x).with_declared_type("num"),
                    x.pattern()
                        .assign(expr("int"))
                        .check_ir("patternAssignment(expr(int), assignedVarPattern(x))"),
                ]);
            }

            #[test]
            fn when_matched_type_is_dynamic() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run(vec![
                    declare(x).with_declared_type("num"),
                    x.pattern()
                        .assign(expr("dynamic"))
                        .check_ir("patternAssignment(expr(dynamic), assignedVarPattern(x))"),
                ]);
            }

            #[test]
            fn when_matched_type_is_not_a_subtype_of_variable_type() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run_with(
                    vec![
                        declare(x).with_declared_type("num"),
                        x.pattern()
                            .error_id("PATTERN")
                            .assign(expr("String"))
                            .error_id("CONTEXT"),
                    ],
                    errors(
                        &["patternTypeMismatchInIrrefutableContext(pattern: PATTERN, \
                              context: CONTEXT, matchedType: String, requiredType: num)"],
                    ),
                );
            }
        }
    }

    mod record {
        use super::*;

        mod positional {
            use super::*;

            mod match_dynamic {
                use super::*;

                #[test]
                fn refutable() {
                    let (_s, mut h) = set_up();
                    h.run(vec![
                        if_case(
                            expr("dynamic").check_schema("_"),
                            record_pattern(vec![
                                Var::new("a")
                                    .pattern()
                                    .with_declared_type("int")
                                    .record_field(None),
                                Var::new("b").pattern().record_field(None),
                            ]),
                            vec![],
                            None,
                        )
                        .check_ir(
                            "ifCase(expr(dynamic), recordPattern(varPattern(a, \
                         matchedType: dynamic, staticType: int), varPattern(b, \
                         matchedType: dynamic, staticType: dynamic), matchedType: \
                         dynamic, requiredType: (Object?, Object?)), \
                         variables(a, b), true, block(), noop)",
                        ),
                    ]);
                }
            }

            mod match_error {
                use super::*;

                #[test]
                fn refutable() {
                    let (_s, mut h) = set_up();
                    h.run(vec![
                        if_case(
                            expr("error").check_schema("_"),
                            record_pattern(vec![
                                Var::new("a")
                                    .pattern()
                                    .with_declared_type("int")
                                    .record_field(None),
                                Var::new("b").pattern().record_field(None),
                            ]),
                            vec![],
                            None,
                        )
                        .check_ir(
                            "ifCase(expr(error), recordPattern(varPattern(a, \
                         matchedType: error, staticType: int), varPattern(b, \
                         matchedType: error, staticType: error), matchedType: \
                         error, requiredType: (Object?, Object?)), \
                         variables(a, b), true, block(), noop)",
                        ),
                    ]);
                }
            }

            mod match_record_type {
                use super::*;

                mod same_shape {
                    use super::*;

                    #[test]
                    fn irrefutable() {
                        let (_s, mut h) = set_up();
                        h.run(vec![
                            pattern_variable_declaration(
                                record_pattern(vec![
                                    Var::new("a")
                                        .pattern()
                                        .with_declared_type("int")
                                        .record_field(None),
                                    Var::new("b").pattern().record_field(None),
                                ]),
                                expr("(int, String)").check_schema("(int, _)"),
                                false,
                            )
                            .check_ir(
                                "match(expr((int, String)), recordPattern(varPattern(a, \
                             matchedType: int, staticType: int), varPattern(b, \
                             matchedType: String, staticType: String), \
                             matchedType: (int, String), \
                             requiredType: (Object?, Object?)))",
                            ),
                        ]);
                    }
                }

                mod different_shape {
                    use super::*;

                    #[test]
                    fn irrefutable() {
                        let (_s, mut h) = set_up();
                        h.run_with(
                            vec![
                                pattern_variable_declaration(
                                    record_pattern(vec![
                                        Var::new("a")
                                            .pattern()
                                            .with_declared_type("int")
                                            .error_id("VAR(a)")
                                            .record_field(None),
                                        Var::new("b").pattern().record_field(None),
                                    ])
                                    .error_id("PATTERN"),
                                    expr("(int,)").check_schema("(int, _)"),
                                    false,
                                )
                                .error_id("CONTEXT")
                                .check_ir(
                                    "match(expr((int,)), recordPattern(varPattern(a, \
                                 matchedType: Object?, staticType: int), \
                                 varPattern(b, matchedType: Object?, staticType: \
                                 Object?), matchedType: (int,), requiredType: \
                                 (Object?, Object?)))",
                                ),
                            ],
                            errors(&[
                                "patternTypeMismatchInIrrefutableContext(pattern: VAR(a), \
                                 context: CONTEXT, matchedType: Object?, \
                                 requiredType: int)",
                                "patternTypeMismatchInIrrefutableContext(pattern: PATTERN, \
                                 context: CONTEXT, matchedType: (int,), \
                                 requiredType: (Object?, Object?))",
                            ]),
                        );
                    }

                    mod refutable {
                        use super::*;

                        #[test]
                        fn too_few() {
                            let (_s, mut h) = set_up();
                            h.run(vec![
                                if_case(
                                    expr("(int,)").check_schema("_"),
                                    record_pattern(vec![
                                        Var::new("a").pattern().record_field(None),
                                        Var::new("b").pattern().record_field(None),
                                    ]),
                                    vec![],
                                    None,
                                )
                                .check_ir(
                                    "ifCase(expr((int,)), recordPattern(varPattern(a, \
                                 matchedType: Object?, staticType: Object?), \
                                 varPattern(b, matchedType: Object?, staticType: \
                                 Object?), matchedType: (int,), requiredType: \
                                 (Object?, Object?)), variables(a, b), true, \
                                 block(), noop)",
                                ),
                            ]);
                        }

                        #[test]
                        fn too_many() {
                            let (_s, mut h) = set_up();
                            h.run(vec![
                                if_case(
                                    expr("(int, String)").check_schema("_"),
                                    record_pattern(vec![
                                        Var::new("a").pattern().record_field(None),
                                    ]),
                                    vec![],
                                    None,
                                )
                                .check_ir(
                                    "ifCase(expr((int, String)), \
                                 recordPattern(varPattern(a, matchedType: Object?, \
                                 staticType: Object?), matchedType: (int, String), \
                                 requiredType: (Object?,)), variables(a), true, \
                                 block(), noop)",
                                ),
                            ]);
                        }
                    }
                }
            }

            mod match_other_type {
                use super::*;

                #[test]
                fn refutable() {
                    let (_s, mut h) = set_up();
                    h.add_super_interfaces("X", |_| vec![Type::parse("Object")]);
                    h.run(vec![
                        if_case(
                            expr("X").check_schema("_"),
                            record_pattern(vec![
                                Var::new("a")
                                    .pattern()
                                    .with_declared_type("int")
                                    .record_field(None),
                                Var::new("b").pattern().record_field(None),
                            ]),
                            vec![],
                            None,
                        )
                        .check_ir(
                            "ifCase(expr(X), recordPattern(varPattern(a, \
                         matchedType: Object?, staticType: int), varPattern(b, \
                         matchedType: Object?, staticType: Object?), matchedType: X, \
                         requiredType: (Object?, Object?)), variables(a, b), \
                         true, block(), noop)",
                        ),
                    ]);
                }
            }
        }

        mod named {
            use super::*;

            mod match_dynamic {
                use super::*;

                #[test]
                fn refutable() {
                    let (_s, mut h) = set_up();
                    h.run(vec![
                        if_case(
                            expr("dynamic").check_schema("_"),
                            record_pattern(vec![
                                Var::new("a")
                                    .pattern()
                                    .with_declared_type("int")
                                    .record_field(Some("a")),
                                Var::new("b").pattern().record_field(Some("b")),
                            ]),
                            vec![],
                            None,
                        )
                        .check_ir(
                            "ifCase(expr(dynamic), recordPattern(varPattern(a, \
                         matchedType: dynamic, staticType: int), varPattern(b, \
                         matchedType: dynamic, staticType: dynamic), matchedType: \
                         dynamic, requiredType: ({Object? a, Object? b})), \
                         variables(a, b), true, block(), noop)",
                        ),
                    ]);
                }
            }

            mod match_record_type {
                use super::*;

                mod same_shape {
                    use super::*;

                    #[test]
                    fn irrefutable() {
                        let (_s, mut h) = set_up();
                        h.run(vec![
                            pattern_variable_declaration(
                                record_pattern(vec![
                                    Var::new("a")
                                        .pattern()
                                        .with_declared_type("int")
                                        .record_field(Some("a")),
                                    Var::new("b").pattern().record_field(Some("b")),
                                ]),
                                expr("({int a, String b})").check_schema("({int a, _ b})"),
                                false,
                            )
                            .check_ir(
                                "match(expr(({int a, String b})), \
                             recordPattern(varPattern(a, matchedType: int, \
                             staticType: int), varPattern(b, matchedType: String, \
                             staticType: String), matchedType: ({int a, String b}), \
                             requiredType: ({Object? a, Object? b})))",
                            ),
                        ]);
                    }
                }

                mod different_shape {
                    use super::*;

                    #[test]
                    fn irrefutable() {
                        let (_s, mut h) = set_up();
                        h.run_with(
                            vec![
                                pattern_variable_declaration(
                                    record_pattern(vec![
                                        Var::new("a")
                                            .pattern()
                                            .with_declared_type("int")
                                            .error_id("VAR(a)")
                                            .record_field(Some("a")),
                                        Var::new("b").pattern().record_field(Some("b")),
                                    ])
                                    .error_id("PATTERN"),
                                    expr("({int a})").check_schema("({int a, _ b})"),
                                    false,
                                )
                                .error_id("CONTEXT")
                                .check_ir(
                                    "match(expr(({int a})), \
                                 recordPattern(varPattern(a, matchedType: Object?, \
                                 staticType: int), varPattern(b, matchedType: Object?, \
                                 staticType: Object?), matchedType: ({int a}), \
                                 requiredType: ({Object? a, Object? b})))",
                                ),
                            ],
                            errors(&[
                                "patternTypeMismatchInIrrefutableContext(pattern: VAR(a), \
                                 context: CONTEXT, matchedType: Object?, \
                                 requiredType: int)",
                                "patternTypeMismatchInIrrefutableContext(pattern: PATTERN, \
                                 context: CONTEXT, matchedType: ({int a}), \
                                 requiredType: ({Object? a, Object? b}))",
                            ]),
                        );
                    }

                    mod refutable {
                        use super::*;

                        #[test]
                        fn too_few() {
                            let (_s, mut h) = set_up();
                            h.run(vec![
                                if_case(
                                    expr("({int a})").check_schema("_"),
                                    record_pattern(vec![
                                        Var::new("a").pattern().record_field(Some("a")),
                                        Var::new("b").pattern().record_field(Some("b")),
                                    ]),
                                    vec![],
                                    None,
                                )
                                .check_ir(
                                    "ifCase(expr(({int a})), recordPattern(\
                                 varPattern(a, matchedType: Object?, staticType: \
                                 Object?), varPattern(b, matchedType: Object?, \
                                 staticType: Object?), matchedType: ({int a}), \
                                 requiredType: ({Object? a, Object? b})), \
                                 variables(a, b), true, block(), noop)",
                                ),
                            ]);
                        }

                        #[test]
                        fn too_many() {
                            let (_s, mut h) = set_up();
                            h.run(vec![
                                if_case(
                                    expr("({int a, String b})").check_schema("_"),
                                    record_pattern(vec![
                                        Var::new("a").pattern().record_field(Some("a")),
                                    ]),
                                    vec![],
                                    None,
                                )
                                .check_ir(
                                    "ifCase(expr(({int a, String b})), \
                                 recordPattern(varPattern(a, matchedType: Object?, \
                                 staticType: Object?), matchedType: ({int a, String b}), \
                                 requiredType: ({Object? a})), variables(a), true, \
                                 block(), noop)",
                                ),
                            ]);
                        }
                    }
                }
            }

            mod match_other_type {
                use super::*;

                #[test]
                fn refutable() {
                    let (_s, mut h) = set_up();
                    h.add_super_interfaces("X", |_| vec![Type::parse("Object")]);
                    h.run(vec![
                        if_case(
                            expr("X").check_schema("_"),
                            record_pattern(vec![
                                Var::new("a")
                                    .pattern()
                                    .with_declared_type("int")
                                    .record_field(Some("a")),
                                Var::new("b").pattern().record_field(Some("b")),
                            ]),
                            vec![],
                            None,
                        )
                        .check_ir(
                            "ifCase(expr(X), recordPattern(varPattern(a, \
                         matchedType: Object?, staticType: int), varPattern(b, \
                         matchedType: Object?, staticType: Object?), matchedType: X, \
                         requiredType: ({Object? a, Object? b})), variables(a, b), \
                         true, block(), noop)",
                        ),
                    ]);
                }
            }

            #[test]
            fn duplicate_field_name() {
                let (_s, mut h) = set_up();
                h.run_with(
                    vec![if_case(
                        expr("({int a})"),
                        record_pattern(vec![
                            Var::new("a")
                                .pattern()
                                .record_field(Some("a"))
                                .error_id("ORIGINAL"),
                            Var::new("b")
                                .pattern()
                                .record_field(Some("a"))
                                .error_id("DUPLICATE"),
                        ])
                        .error_id("PATTERN"),
                        vec![],
                        None,
                    )],
                    errors(&["duplicateRecordPatternField(\
                              objectOrRecordPattern: PATTERN, \
                              name: a, original: ORIGINAL, \
                              duplicate: DUPLICATE)"]),
                );
            }
        }
    }

    mod relational {
        use super::*;

        #[test]
        fn refutability() {
            let (_s, mut h) = set_up();
            h.run_with(
                vec![
                    pattern_variable_declaration(
                        relational_pattern(">", int_literal(0).check_schema("num"))
                            .error_id("PATTERN"),
                        int_literal(1).check_schema("_"),
                        false,
                    )
                    .error_id("CONTEXT")
                    .check_ir("match(1, >(0, matchedType: int))"),
                ],
                errors(&["refutablePatternInIrrefutableContext(pattern: PATTERN, \
                          context: CONTEXT)"]),
            );
        }

        #[test]
        fn no_operator() {
            let (_s, mut h) = set_up();
            h.add_member("C", ">", None, false, None);
            h.run(vec![
                if_case(
                    expr("C").check_schema("_"),
                    relational_pattern(">", int_literal(0).check_schema("_")),
                    vec![],
                    None,
                )
                .check_ir(
                    "ifCase(expr(C), >(0, matchedType: C), \
                 variables(), true, block(), noop)",
                ),
            ]);
        }

        mod has_operator {
            use super::*;

            #[test]
            fn int_ge() {
                let (_s, mut h) = set_up();
                h.run(vec![
                    if_case(
                        expr("int").check_schema("_"),
                        relational_pattern(">=", int_literal(0).check_schema("num")),
                        vec![],
                        None,
                    )
                    .check_ir(
                        "ifCase(expr(int), >=(0, matchedType: \
                     int), variables(), true, block(), noop)",
                    ),
                ]);
            }

            #[test]
            fn object_eq_nullable() {
                let (_s, mut h) = set_up();
                h.run(vec![
                    if_case(
                        expr("Object").check_schema("_"),
                        relational_pattern("==", expr("int?").check_schema("Object?")),
                        vec![],
                        None,
                    )
                    .check_ir(
                        "ifCase(expr(Object), ==(expr(int?), \
                     matchedType: Object), variables(), true, block(), noop)",
                    ),
                ]);
            }

            #[test]
            fn object_not_eq_nullable() {
                let (_s, mut h) = set_up();
                h.run(vec![
                    if_case(
                        expr("Object").check_schema("_"),
                        relational_pattern("!=", expr("int?").check_schema("Object?")),
                        vec![],
                        None,
                    )
                    .check_ir(
                        "ifCase(expr(Object), !=(expr(int?), \
                     matchedType: Object), variables(), true, block(), noop)",
                    ),
                ]);
            }

            mod argument_type_not_assignable {
                use super::*;

                #[test]
                fn basic() {
                    let (_s, mut h) = set_up();
                    h.run_with(
                        vec![
                            if_case(
                                expr("int").check_schema("_"),
                                relational_pattern(">", expr("String")).error_id("PATTERN"),
                                vec![],
                                None,
                            )
                            .check_ir(
                                "ifCase(expr(int), >(expr(String), \
                             matchedType: int), variables(), true, block(), noop)",
                            ),
                        ],
                        errors(&[
                            "relationalPatternOperandTypeNotAssignable(pattern: PATTERN, \
                                  operandType: String, parameterType: num)",
                        ]),
                    );
                }

                #[test]
                fn gt_nullable() {
                    let (_s, mut h) = set_up();
                    h.run_with(
                        vec![if_case(
                            expr("int"),
                            relational_pattern(">", expr("int?")).error_id("PATTERN"),
                            vec![],
                            None,
                        )],
                        errors(&[
                            "relationalPatternOperandTypeNotAssignable(pattern: PATTERN, \
                                  operandType: int?, parameterType: num)",
                        ]),
                    );
                }

                #[test]
                fn lt_nullable() {
                    let (_s, mut h) = set_up();
                    h.run_with(
                        vec![if_case(
                            expr("int"),
                            relational_pattern("<", expr("int?")).error_id("PATTERN"),
                            vec![],
                            None,
                        )],
                        errors(&[
                            "relationalPatternOperandTypeNotAssignable(pattern: PATTERN, \
                                  operandType: int?, parameterType: num)",
                        ]),
                    );
                }

                #[test]
                fn ge_nullable() {
                    let (_s, mut h) = set_up();
                    h.run_with(
                        vec![if_case(
                            expr("int"),
                            relational_pattern(">=", expr("int?")).error_id("PATTERN"),
                            vec![],
                            None,
                        )],
                        errors(&[
                            "relationalPatternOperandTypeNotAssignable(pattern: PATTERN, \
                                  operandType: int?, parameterType: num)",
                        ]),
                    );
                }

                #[test]
                fn le_nullable() {
                    let (_s, mut h) = set_up();
                    h.run_with(
                        vec![if_case(
                            expr("int"),
                            relational_pattern("<=", expr("int?")).error_id("PATTERN"),
                            vec![],
                            None,
                        )],
                        errors(&[
                            "relationalPatternOperandTypeNotAssignable(pattern: PATTERN, \
                                  operandType: int?, parameterType: num)",
                        ]),
                    );
                }

                #[test]
                fn extension_type_to_representation() {
                    let (_s, mut h) = set_up();
                    h.add_super_interfaces("E", |_| vec![Type::parse("Object?")]);
                    h.add_extension_type_erasure("E", "int");
                    h.add_member("C", ">", Some("bool Function(int)"), false, None);
                    h.run_with(
                        vec![if_case(
                            expr("C"),
                            relational_pattern(">", expr("E")).error_id("PATTERN"),
                            vec![],
                            None,
                        )],
                        errors(&[
                            "relationalPatternOperandTypeNotAssignable(pattern: PATTERN, \
                                  operandType: E, parameterType: int)",
                        ]),
                    );
                }

                #[test]
                fn representation_to_extension_type() {
                    let (_s, mut h) = set_up();
                    h.add_super_interfaces("E", |_| vec![Type::parse("Object?")]);
                    h.add_extension_type_erasure("E", "int");
                    h.add_member("C", ">", Some("bool Function(E)"), false, None);
                    h.run_with(
                        vec![if_case(
                            expr("C"),
                            relational_pattern(">", expr("int")).error_id("PATTERN"),
                            vec![],
                            None,
                        )],
                        errors(&[
                            "relationalPatternOperandTypeNotAssignable(pattern: PATTERN, \
                                  operandType: int, parameterType: E)",
                        ]),
                    );
                }
            }

            mod argument_type_assignable {
                use super::*;

                #[test]
                fn eq_nullable() {
                    let (_s, mut h) = set_up();
                    h.run(vec![if_case(
                        expr("int"),
                        relational_pattern("==", expr("int?")),
                        vec![],
                        None,
                    )]);
                }

                #[test]
                fn not_eq_nullable() {
                    let (_s, mut h) = set_up();
                    h.run(vec![if_case(
                        expr("int"),
                        relational_pattern("!=", expr("int?")),
                        vec![],
                        None,
                    )]);
                }
            }

            #[test]
            fn return_type_is_not_assignable_to_bool() {
                let (_s, mut h) = set_up();
                h.add_member("A", ">", Some("int Function(Object)"), false, None);
                h.run_with(
                    vec![
                        if_case(
                            expr("A").check_schema("_"),
                            relational_pattern(">", expr("String").check_schema("Object"))
                                .error_id("PATTERN"),
                            vec![],
                            None,
                        )
                        .check_ir(
                            "ifCase(expr(A), >(expr(String), \
                         matchedType: A), variables(), true, block(), noop)",
                        ),
                    ],
                    errors(&["relationalPatternOperatorReturnTypeNotAssignableToBool(\
                              pattern: PATTERN, returnType: int)"]),
                );
            }
        }
    }

    mod variable {
        use super::*;

        mod refutability {
            use super::*;

            #[test]
            fn when_matched_type_is_a_subtype_of_variable_type() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run(vec![
                    pattern_variable_declaration(
                        x.pattern().with_declared_type("num"),
                        expr("int"),
                        false,
                    )
                    .check_ir(
                        "match(expr(int), \
                     varPattern(x, matchedType: int, staticType: num))",
                    ),
                ]);
            }

            #[test]
            fn when_matched_type_is_dynamic() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run(vec![
                    pattern_variable_declaration(
                        x.pattern().with_declared_type("num"),
                        expr("dynamic"),
                        false,
                    )
                    .check_ir(
                        "match(expr(dynamic), \
                     varPattern(x, matchedType: dynamic, staticType: num))",
                    ),
                ]);
            }

            #[test]
            fn when_matched_type_is_error() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run(vec![
                    pattern_variable_declaration(
                        x.pattern().with_declared_type("num"),
                        expr("error"),
                        false,
                    )
                    .check_ir(
                        "match(expr(error), \
                     varPattern(x, matchedType: error, staticType: num))",
                    ),
                ]);
            }

            #[test]
            fn when_matched_type_is_not_a_subtype_of_variable_type() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run_with(
                    vec![
                        pattern_variable_declaration(
                            x.pattern().with_declared_type("num").error_id("PATTERN"),
                            expr("String"),
                            false,
                        )
                        .error_id("CONTEXT"),
                    ],
                    errors(
                        &["patternTypeMismatchInIrrefutableContext(pattern: PATTERN, \
                              context: CONTEXT, matchedType: String, requiredType: num)"],
                    ),
                );
            }
        }
    }

    mod wildcard {
        use super::*;

        #[test]
        fn untyped() {
            let (_s, mut h) = set_up();
            h.run(vec![
                if_case(expr("int"), wildcard(), vec![], None).check_ir(
                    "ifCase(expr(int), wildcardPattern(matchedType: int), \
                 variables(), true, block(), noop)",
                ),
            ]);
        }

        #[test]
        fn typed() {
            let (_s, mut h) = set_up();
            h.run(vec![
                if_case(
                    expr("num"),
                    wildcard().with_declared_type("int"),
                    vec![],
                    None,
                )
                .check_ir(
                    "ifCase(expr(num), wildcardPattern(matchedType: num), \
                     variables(), true, block(), noop)",
                ),
            ]);
        }

        mod refutability {
            use super::*;

            #[test]
            fn when_matched_type_is_a_subtype_of_variable_type() {
                let (_s, mut h) = set_up();
                h.run(vec![
                    pattern_variable_declaration(
                        wildcard().with_declared_type("num"),
                        expr("int"),
                        false,
                    )
                    .check_ir("match(expr(int), wildcardPattern(matchedType: int))"),
                ]);
            }

            #[test]
            fn when_matched_type_is_dynamic() {
                let (_s, mut h) = set_up();
                h.run(vec![
                    pattern_variable_declaration(
                        wildcard().with_declared_type("num"),
                        expr("dynamic"),
                        false,
                    )
                    .check_ir(
                        "match(expr(dynamic), wildcardPattern(\
                     matchedType: dynamic))",
                    ),
                ]);
            }

            #[test]
            fn when_matched_type_is_not_a_subtype_of_variable_type() {
                let (_s, mut h) = set_up();
                h.run_with(
                    vec![
                        pattern_variable_declaration(
                            wildcard().with_declared_type("num").error_id("PATTERN"),
                            expr("String"),
                            false,
                        )
                        .error_id("CONTEXT"),
                    ],
                    errors(
                        &["patternTypeMismatchInIrrefutableContext(pattern: PATTERN, \
                              context: CONTEXT, matchedType: String, requiredType: num)"],
                    ),
                );
            }
        }
    }
}
