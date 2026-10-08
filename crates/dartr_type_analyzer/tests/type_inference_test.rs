// Dart source: pkg/_fe_analyzer_shared/test/type_inference/type_inference_test.dart

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
    type_registry_scope, PrimaryType, Type, TypeRegistry, TypeRegistryScope,
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

mod collection_elements {
    use super::*;

    mod if_ {
        use super::*;

        #[test]
        fn condition_schema() {
            let (_s, mut h) = set_up();
            h.run(vec![list_literal(
                vec![
                    if_element(expr("dynamic").check_schema("bool"), expr("Object"), None)
                        .check_ir("if(expr(dynamic), celt(expr(Object)), noop)"),
                ],
                "int",
            )]);
        }

        #[test]
        fn with_else() {
            let (_s, mut h) = set_up();
            h.run(vec![list_literal(
                vec![
                    if_element(expr("bool"), expr("Object"), Some(expr("Object")))
                        .check_ir("if(expr(bool), celt(expr(Object)), celt(expr(Object)))"),
                ],
                "int",
            )]);
        }

        mod schema {
            use super::*;

            #[test]
            fn element_type() {
                let (_s, mut h) = set_up();
                h.run(vec![list_literal(
                    vec![
                        if_element(
                            expr("bool"),
                            expr("Object").check_schema("int"),
                            Some(expr("Object").check_schema("int")),
                        )
                        .check_ir("if(expr(bool), celt(expr(Object)), celt(expr(Object)))"),
                    ],
                    "int",
                )]);
            }
        }
    }

    mod if_case {
        use super::*;

        #[test]
        fn expression_schema() {
            let (_s, mut h) = set_up();
            h.run(vec![list_literal(
                vec![
                    if_case_element(
                        expr("Object").check_schema("_"),
                        int_literal(0).pattern(),
                        int_literal(1).check_schema("int"),
                        None,
                    )
                    .check_ir(
                        "if(expression: expr(Object), pattern: \
                         const(0, matchedType: Object), guard: true, \
                         ifTrue: celt(1), ifFalse: noop)",
                    ),
                ],
                "int",
            )]);
        }

        #[test]
        fn with_else() {
            let (_s, mut h) = set_up();
            h.run(vec![list_literal(
                vec![
                    if_case_element(
                        expr("Object"),
                        int_literal(0).pattern(),
                        int_literal(1).check_schema("int"),
                        Some(int_literal(2).check_schema("int")),
                    )
                    .check_ir(
                        "if(expression: expr(Object), pattern: \
                         const(0, matchedType: Object), guard: true, \
                         ifTrue: celt(1), ifFalse: celt(2))",
                    ),
                ],
                "int",
            )]);
        }

        #[test]
        fn with_guard() {
            let (_s, mut h) = set_up();
            let x = Var::new("x");
            h.run(vec![list_literal(
                vec![
                    if_case_element(
                        expr("Object"),
                        x.pattern().when(Some(x.expr().eq(int_literal(0)))),
                        int_literal(1).check_schema("int"),
                        None,
                    )
                    .check_ir(
                        "if(expression: expr(Object), pattern: \
                         varPattern(x, matchedType: Object, staticType: Object), \
                         guard: ==(x, 0), ifTrue: celt(1), ifFalse: noop)",
                    ),
                ],
                "int",
            )]);
        }

        #[test]
        fn allows_refutable_patterns() {
            let (_s, mut h) = set_up();
            let x = Var::new("x");
            h.run(vec![list_literal(
                vec![
                    if_case_element(
                        expr("Object"),
                        x.pattern().with_declared_type("int"), // has type, refutable
                        int_literal(1).check_schema("int"),
                        None,
                    )
                    .check_ir(
                        "if(expression: expr(Object), pattern: varPattern(x, \
                         matchedType: Object, staticType: int), guard: true, \
                         ifTrue: celt(1), ifFalse: noop)",
                    ),
                ],
                "int",
            )]);
        }

        mod guard_not_assignable_to_bool {
            use super::*;

            #[test]
            fn int() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run_with(
                    vec![list_literal(
                        vec![if_case_element(
                            expr("Object"),
                            x.pattern().when(Some(expr("int").error_id("GUARD"))),
                            int_literal(0).check_schema("int"),
                            None,
                        )],
                        "int",
                    )],
                    errors(&["nonBooleanCondition(node: GUARD)"]),
                );
            }

            #[test]
            fn bool() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run_with(
                    vec![list_literal(
                        vec![if_case_element(
                            expr("Object"),
                            x.pattern().when(Some(expr("bool"))),
                            int_literal(0).check_schema("int"),
                            None,
                        )],
                        "int",
                    )],
                    errors(&[]),
                );
            }

            #[test]
            fn dynamic() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run_with(
                    vec![list_literal(
                        vec![if_case_element(
                            expr("Object"),
                            x.pattern().when(Some(expr("dynamic"))),
                            int_literal(0).check_schema("int"),
                            None,
                        )],
                        "int",
                    )],
                    errors(&[]),
                );
            }
        }
    }

    mod pattern_for_in {
        use super::*;

        mod expression_type {
            use super::*;

            #[test]
            fn iterable() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run(vec![list_literal(
                    vec![
                        pattern_for_in_element(x.pattern(), expr("Iterable<int>"), expr("Object"), false)
                            .check_ir(
                                "forEach(expr(Iterable<int>), varPattern(x, \
                                 matchedType: int, staticType: int), celt(expr(Object)))",
                            ),
                    ],
                    "Object",
                )]);
            }

            #[test]
            fn dynamic() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run(vec![list_literal(
                    vec![
                        pattern_for_in_element(x.pattern(), expr("dynamic"), expr("Object"), false)
                            .check_ir(
                                "forEach(expr(dynamic), varPattern(x, \
                                 matchedType: dynamic, staticType: dynamic), \
                                 celt(expr(Object)))",
                            ),
                    ],
                    "Object",
                )]);
            }

            #[test]
            fn object() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run_with(
                    vec![list_literal(
                        vec![
                            pattern_for_in_element(
                                x.pattern(),
                                expr("Object").error_id("EXPRESSION"),
                                expr("Object"),
                                false,
                            )
                            .error_id("FOR")
                            .check_ir(
                                "forEach(expr(Object), varPattern(x, \
                                 matchedType: error, staticType: error), \
                                 celt(expr(Object)))",
                            ),
                        ],
                        "Object",
                    )],
                    errors(&[
                        "patternForInExpressionIsNotIterable(node: FOR, \
                         expression: EXPRESSION, expressionType: Object)",
                    ]),
                );
            }
        }

        mod refutability {
            use super::*;

            #[test]
            fn when_a_refutable_pattern() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run_with(
                    vec![list_literal(
                        vec![
                            pattern_for_in_element(
                                x.pattern().null_check().error_id("PATTERN"),
                                expr("Iterable<int?>"),
                                expr("Object"),
                                false,
                            )
                            .error_id("FOR")
                            .check_ir(
                                "forEach(expr(Iterable<int?>), nullCheckPattern(\
                                 varPattern(x, matchedType: int, staticType: int), \
                                 matchedType: int?), celt(expr(Object)))",
                            ),
                        ],
                        "Object",
                    )],
                    errors(&["refutablePatternInIrrefutableContext(pattern: PATTERN, context: FOR)"]),
                );
            }

            #[test]
            fn when_the_variable_type_is_not_a_subtype_of_the_matched_type() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run_with(
                    vec![list_literal(
                        vec![
                            pattern_for_in_element(
                                x.pattern().with_declared_type("String").error_id("PATTERN"),
                                expr("Iterable<int>"),
                                expr("Object"),
                                false,
                            )
                            .error_id("FOR")
                            .check_ir(
                                "forEach(expr(Iterable<int>), varPattern(x, \
                                 matchedType: int, staticType: String), \
                                 celt(expr(Object)))",
                            ),
                        ],
                        "Object",
                    )],
                    errors(&[
                        "patternTypeMismatchInIrrefutableContext(pattern: PATTERN, \
                         context: FOR, matchedType: int, requiredType: String)",
                    ]),
                );
            }
        }
    }
}

mod expressions {
    use super::*;

    mod await_ {
        use super::*;

        mod await_expression_result {
            use super::*;

            #[test]
            fn operand_type() {
                let (_s, mut h) = set_up();
                h.run(vec![await_(expr("int")).check_expression_type_analysis_result(
                    |result| {
                        assert_eq!(result.operand_type.unwrap().to_string(), "int");
                    },
                )]);
            }
        }

        mod downward_inference {
            use super::*;

            #[test]
            fn schema_is_future_or_s() {
                let (_s, mut h) = set_up();
                h.run(vec![
                    await_(expr("int").check_schema("FutureOr<int>")).in_type_schema("FutureOr<int>"),
                ]);
            }

            #[test]
            fn schema_is_future_or_s_nullable() {
                let (_s, mut h) = set_up();
                h.run(vec![
                    await_(expr("int").check_schema("FutureOr<int>?")).in_type_schema("FutureOr<int>?"),
                ]);
            }

            #[test]
            fn schema_is_dynamic() {
                let (_s, mut h) = set_up();
                h.run(vec![
                    await_(expr("int").check_schema("FutureOr<_>")).in_type_schema("dynamic"),
                ]);
            }

            #[test]
            fn schema_is_other_type() {
                let (_s, mut h) = set_up();
                h.run(vec![
                    await_(expr("int").check_schema("FutureOr<int>")).in_type_schema("int"),
                ]);
            }
        }

        mod upward_inference {
            use super::*;

            #[test]
            fn operand_has_type_future_s() {
                let (_s, mut h) = set_up();
                h.run(vec![await_(expr("Future<int>")).check_type("int")]);
            }

            #[test]
            fn operand_has_type_future_or_s() {
                let (_s, mut h) = set_up();
                h.run(vec![await_(expr("FutureOr<int>")).check_type("int")]);
            }

            #[test]
            fn operand_has_type_future_s_nullable() {
                let (_s, mut h) = set_up();
                h.run(vec![await_(expr("Future<int>?")).check_type("int?")]);
            }

            #[test]
            fn operand_has_type_future_or_s_nullable() {
                let (_s, mut h) = set_up();
                h.run(vec![await_(expr("FutureOr<int>?")).check_type("int?")]);
            }

            #[test]
            fn operand_has_other_type() {
                let (_s, mut h) = set_up();
                h.run(vec![await_(expr("int")).check_type("int")]);
            }
        }
    }

    mod cascade {
        use super::*;

        mod ir {
            use super::*;

            #[test]
            fn not_null_aware() {
                let (_s, mut h) = set_up();
                h.run(vec![
                    expr("dynamic")
                        .cascade(
                            vec![
                                Box::new(|t: Node| t.invoke_method("f", vec![], false)),
                                Box::new(|t: Node| t.invoke_method("g", vec![], false)),
                            ],
                            false,
                        )
                        .check_ir("let(t0, expr(dynamic), let(t1, f(t0), let(t2, g(t0), t0)))"),
                ]);
            }

            #[test]
            fn null_aware() {
                let (_s, mut h) = set_up();
                h.run(vec![
                    expr("dynamic")
                        .cascade(
                            vec![
                                Box::new(|t: Node| t.invoke_method("f", vec![], false)),
                                Box::new(|t: Node| t.invoke_method("g", vec![], false)),
                            ],
                            true,
                        )
                        .check_ir(
                            "let(t0, expr(dynamic), \
                             if(==(t0, null), t0, let(t1, f(t0), let(t2, g(t0), t0))))",
                        ),
                ]);
            }
        }
    }

    mod integer_literal {
        use super::*;

        fn check(schema: &str, converted_to_double: bool, ty: &str, ir: &str) {
            let (_s, mut h) = set_up();
            h.run(vec![
                int_literal(1)
                    .check_expression_type_analysis_result(move |result| {
                        assert_eq!(result.converted_to_double, Some(converted_to_double));
                    })
                    .check_type(ty)
                    .check_ir(ir)
                    .in_type_schema(schema),
            ]);
        }

        #[test]
        fn double_type_schema() {
            check("double", true, "double", "1.0f");
        }

        #[test]
        fn int_type_schema() {
            check("int", false, "int", "1");
        }

        #[test]
        fn num_type_schema() {
            check("num", false, "int", "1");
        }

        #[test]
        fn double_nullable_type_schema() {
            check("double?", true, "double", "1.0f");
        }

        #[test]
        fn int_nullable_type_schema() {
            check("int?", false, "int", "1");
        }

        #[test]
        fn unknown_type_schema() {
            check("_", false, "int", "1");
        }

        #[test]
        fn unrelated_type_schema() {
            // Note: an unrelated type schema can arise in the case of
            // assigning to a promoted variable, e.g.:
            //
            //   Object x;
            //   if (x is String) {
            //     x = 1;
            //   }
            check("String", false, "int", "1");
        }
    }
}
