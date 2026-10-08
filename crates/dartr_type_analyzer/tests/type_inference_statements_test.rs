// Dart source: pkg/_fe_analyzer_shared/test/type_inference/type_inference_test.dart (group "Statements:")

//! The shared type analyzer tests of the group `Statements:`, run through
//! the mini-AST harness.
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

/// Dart `h.run(statements, bodyContext: BodyContext(...))`.
fn body_context(is_async: bool, yield_context: &str) -> RunOptions {
    RunOptions {
        body_context: Some(BodyContext::new(is_async, yield_context)),
        ..RunOptions::default()
    }
}

mod statements {
    use super::*;

    mod if_ {
        use super::*;

        #[test]
        fn condition_schema() {
            let (_s, mut h) = set_up();
            h.run(vec![if_(expr("dynamic").check_schema("bool"), vec![expr("Object")])
                .check_ir("if(expr(dynamic), block(stmt(expr(Object))), noop)")]);
        }

        #[test]
        fn with_else() {
            let (_s, mut h) = set_up();
            h.run(vec![if_else(expr("bool"), vec![expr("Object")], vec![expr("String")])
                .check_ir(
                    "if(expr(bool), block(stmt(expr(Object))), \
                     block(stmt(expr(String))))",
                )]);
        }
    }

    mod if_case {
        use super::*;

        #[test]
        fn type_schema() {
            let (_s, mut h) = set_up();
            let x = Var::new("x");
            h.run(vec![if_case(
                expr("int").check_schema("_"),
                x.pattern().with_declared_type("num"),
                vec![],
                None,
            )
            .check_ir(
                "ifCase(expr(int), \
                 varPattern(x, matchedType: int, staticType: num), variables(x), \
                 true, block(), noop)",
            )]);
        }

        #[test]
        fn with_else() {
            let (_s, mut h) = set_up();
            let x = Var::new("x");
            h.run(vec![if_case(
                expr("num"),
                x.pattern().with_declared_type("int"),
                vec![expr("Object")],
                Some(vec![expr("String")]),
            )
            .check_ir(
                "ifCase(expr(num), \
                 varPattern(x, matchedType: num, staticType: int), variables(x), \
                 true, block(stmt(expr(Object))), block(stmt(expr(String))))",
            )]);
        }

        #[test]
        fn with_guard() {
            let (_s, mut h) = set_up();
            let x = Var::new("x");
            h.run(vec![if_case(
                expr("num"),
                x.pattern()
                    .with_declared_type("int")
                    .when(Some(x.expr().eq(int_literal(0)))),
                vec![],
                None,
            )
            .check_ir(
                "ifCase(expr(num), \
                 varPattern(x, matchedType: num, staticType: int), variables(x), \
                 ==(x, 0), block(), noop)",
            )]);
        }

        #[test]
        fn allows_refutable_patterns() {
            let (_s, mut h) = set_up();
            let x = Var::new("x");
            h.run(vec![if_case(
                expr("num").check_schema("_"),
                x.pattern().with_declared_type("int"),
                vec![],
                None,
            )
            .check_ir(
                "ifCase(expr(num), \
                 varPattern(x, matchedType: num, staticType: int), variables(x), \
                 true, block(), noop)",
            )]);
        }

        mod guard_not_assignable_to_bool {
            use super::*;

            #[test]
            fn int() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run_with(
                    vec![if_case(
                        expr("int"),
                        x.pattern().when(Some(expr("int").error_id("GUARD"))),
                        vec![],
                        None,
                    )],
                    errors(&["nonBooleanCondition(node: GUARD)"]),
                );
            }

            #[test]
            fn bool() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run_with(
                    vec![if_case(
                        expr("int"),
                        x.pattern().when(Some(expr("bool"))),
                        vec![],
                        None,
                    )],
                    errors(&[]),
                );
            }

            #[test]
            fn dynamic() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run_with(
                    vec![if_case(
                        expr("int"),
                        x.pattern().when(Some(expr("dynamic"))),
                        vec![],
                        None,
                    )],
                    errors(&[]),
                );
            }
        }
    }

    mod switch {
        use super::*;

        #[test]
        fn empty() {
            let (_s, mut h) = set_up();
            h.run(vec![switch_(expr("int"), vec![]).expect_last_case_terminates(true)]);
        }

        #[test]
        fn exhaustive() {
            let (_s, mut h) = set_up();
            h.add_exhaustiveness("E", true);
            h.run(vec![switch_(
                expr("E"),
                vec![expr("E").pattern().then(vec![break_(None)])],
            )
            .expect_is_exhaustive(true)]);
        }

        #[test]
        fn no_default() {
            let (_s, mut h) = set_up();
            h.run(vec![switch_(
                expr("int"),
                vec![int_literal(0).pattern().then(vec![break_(None)])],
            )
            .expect_has_default(false)
            .expect_is_exhaustive(false)]);
        }

        #[test]
        fn has_default() {
            let (_s, mut h) = set_up();
            h.run(vec![switch_(
                expr("int"),
                vec![
                    int_literal(0).pattern().then(vec![break_(None)]),
                    default_().then(vec![break_(None)]),
                ],
            )
            .expect_has_default(true)
            .expect_is_exhaustive(true)]);
        }

        #[test]
        fn last_case_terminates() {
            let (_s, mut h) = set_up();
            h.run(vec![switch_(
                expr("int"),
                vec![
                    int_literal(0).pattern().then(vec![expr("int")]),
                    int_literal(1).pattern().then(vec![break_(None)]),
                ],
            )
            .expect_last_case_terminates(true)]);
        }

        #[test]
        fn last_case_doesnt_terminate() {
            let (_s, mut h) = set_up();
            h.run(vec![switch_(
                expr("int"),
                vec![
                    int_literal(0).pattern().then(vec![break_(None)]),
                    int_literal(1).pattern().then(vec![expr("int")]),
                ],
            )
            .expect_last_case_terminates(false)]);
        }

        #[test]
        fn scrutinee_type() {
            let (_s, mut h) = set_up();
            h.run(vec![switch_(expr("int"), vec![]).expect_scrutinee_type("int")]);
        }

        #[test]
        fn const_pattern() {
            let (_s, mut h) = set_up();
            h.run(vec![switch_(
                expr("int").check_schema("_"),
                vec![int_literal(0).pattern().then(vec![break_(None)])],
            )
            .check_ir(
                "switch(expr(int), case(heads(head(const(0, \
                 matchedType: int), true, variables()), variables()), \
                 block(break())))",
            )]);
        }

        mod var_pattern {
            use super::*;

            #[test]
            fn untyped() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run(vec![switch_(
                    expr("int").check_schema("_"),
                    vec![x.pattern().then(vec![break_(None)])],
                )
                .check_ir(
                    "switch(expr(int), case(heads(head(varPattern(x, \
                     matchedType: int, staticType: int), true, variables(x)), \
                     variables(x)), block(break())))",
                )]);
            }

            #[test]
            fn typed() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run(vec![switch_(
                    expr("int").check_schema("_"),
                    vec![x
                        .pattern()
                        .with_declared_type("num")
                        .then(vec![break_(None)])],
                )
                .check_ir(
                    "switch(expr(int), case(heads(head(varPattern(x, \
                     matchedType: int, staticType: num), true, variables(x)), \
                     variables(x)), block(break())))",
                )]);
            }
        }

        #[test]
        fn scrutinee_expression_schema() {
            let (_s, mut h) = set_up();
            h.run(vec![switch_(
                expr("int").check_schema("_"),
                vec![int_literal(0).pattern().then(vec![break_(None)])],
            )]);
        }

        #[test]
        fn empty_final_case() {
            let (_s, mut h) = set_up();
            h.run(vec![switch_(
                expr("int"),
                vec![
                    int_literal(0).pattern().then(vec![break_(None)]),
                    int_literal(1).pattern().then(vec![]),
                ],
            )
            .check_ir(
                "switch(expr(int), case(heads(head(const(0, \
                 matchedType: int), true, variables()), variables()), \
                 block(break())), case(heads(head(const(1, matchedType: int), \
                 true, variables()), variables()), block(synthetic-break())))",
            )]);
        }

        #[test]
        fn guard() {
            let (_s, mut h) = set_up();
            let i = Var::new("i");
            h.run(vec![switch_(
                expr("int"),
                vec![i
                    .pattern()
                    .when(Some(
                        i.expr()
                            .check_type("int")
                            .eq(expr("num"))
                            .check_schema("bool"),
                    ))
                    .then(vec![break_(None)])],
            )
            .check_ir(
                "switch(expr(int), case(heads(head(varPattern(i, \
                 matchedType: int, staticType: int), ==(i, expr(num)), \
                 variables(i)), variables(i)), block(break())))",
            )]);
        }

        mod variables {
            use super::*;

            #[test]
            fn independent_cases() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                let y = Var::new("y");
                h.run(vec![switch_(
                    expr("int"),
                    vec![
                        x.pattern().then(vec![break_(None)]),
                        y.pattern().then(vec![break_(None)]),
                    ],
                )
                .check_ir(
                    "switch(expr(int), case(heads(head(varPattern(x, \
                     matchedType: int, staticType: int), true, variables(x)), \
                     variables(x)), block(break())), case(heads(head(varPattern(y, \
                     matchedType: int, staticType: int), true, variables(y)), \
                     variables(y)), block(break())))",
                )]);
            }

            mod shared_case_scope {
                use super::*;

                mod present_in_both_cases {
                    use super::*;

                    #[test]
                    fn with_the_same_type_and_finality() {
                        let (_s, mut h) = set_up();
                        let x1 = Var::new("x").with_identity("x1");
                        let x2 = Var::new("x").with_identity("x2");
                        Var::join("x", vec![x1, x2]);
                        h.run(vec![switch_(
                            expr("int"),
                            vec![switch_statement_member(
                                vec![x1.pattern(), x2.pattern()],
                                vec![break_(None)],
                                false,
                            )],
                        )
                        .check_ir(
                            "switch(expr(int), case(heads(head(varPattern(x, \
                             matchedType: int, staticType: int), true, variables(x1)), \
                             head(varPattern(x, matchedType: int, staticType: int), \
                             true, variables(x2)), variables(int x = [x1, x2])), \
                             block(break())))",
                        )]);
                    }

                    #[test]
                    fn with_the_same_type_and_finality_with_logical_or() {
                        let (_s, mut h) = set_up();
                        let x1 = Var::new("x").with_identity("x1");
                        let x2 = Var::new("x").with_identity("x2");
                        let x3 = Var::new("x").with_identity("x3");
                        let x4 = Var::join("x", vec![x1, x2]).with_identity("x4");
                        Var::join("x", vec![x4, x3]);
                        h.run(vec![switch_(
                            expr("int"),
                            vec![switch_statement_member(
                                vec![x1.pattern().or(x2.pattern()), x3.pattern()],
                                vec![break_(None)],
                                false,
                            )],
                        )
                        .check_ir(
                            "switch(expr(int), case(heads(head(logicalOrPattern(\
                             varPattern(x, matchedType: int, staticType: int), \
                             varPattern(x, matchedType: int, staticType: int), \
                             matchedType: int), true, variables(int x = [x1, x2])), \
                             head(varPattern(x, matchedType: int, staticType: int), \
                             true, variables(x3)), variables(int x = \
                             [int x = [x1, x2], x3])), block(break())))",
                        )]);
                    }

                    mod with_different_type {
                        use super::*;

                        #[test]
                        fn explicit_explicit() {
                            let (_s, mut h) = set_up();
                            let x1 = Var::new("x").with_identity("x1");
                            let x2 = Var::new("x").with_identity("x2");
                            Var::join("x", vec![x1, x2]);
                            h.run(vec![switch_(
                                expr("int"),
                                vec![switch_statement_member(
                                    vec![
                                        x1.pattern().with_declared_type("num"),
                                        x2.pattern().with_declared_type("int"),
                                    ],
                                    vec![break_(None)],
                                    false,
                                )],
                            )
                            .check_ir(
                                "switch(expr(int), case(heads(head(varPattern(x, \
                                 matchedType: int, staticType: num), true, \
                                 variables(x1)), head(varPattern(x, matchedType: int, \
                                 staticType: int), true, variables(x2)), \
                                 variables(notConsistent:differentFinalityOrType error \
                                 x = [x1, x2])), block(break())))",
                            )]);
                        }

                        #[test]
                        fn explicit_implicit() {
                            let (_s, mut h) = set_up();
                            let x1 = Var::new("x").with_identity("x1");
                            let x2 = Var::new("x").with_identity("x2");
                            Var::join("x", vec![x1, x2]);
                            h.run(vec![switch_(
                                expr("int"),
                                vec![switch_statement_member(
                                    vec![x1.pattern().with_declared_type("num"), x2.pattern()],
                                    vec![break_(None)],
                                    false,
                                )],
                            )
                            .check_ir(
                                "switch(expr(int), case(heads(head(varPattern(x, \
                                 matchedType: int, staticType: num), true, variables(\
                                 x1)), head(varPattern(x, matchedType: int, \
                                 staticType: int), true, variables(x2)), \
                                 variables(notConsistent:differentFinalityOrType error \
                                 x = [x1, x2])), block(break())))",
                            )]);
                        }

                        #[test]
                        fn implicit_implicit() {
                            let (_s, mut h) = set_up();
                            let x1 = Var::new("x").with_identity("x1");
                            let x2 = Var::new("x").with_identity("x2");
                            Var::join("x", vec![x1, x2]);
                            h.run(vec![switch_(
                                expr("List<int>"),
                                vec![switch_statement_member(
                                    vec![x1.pattern(), list_pattern(vec![x2.pattern()], None)],
                                    vec![break_(None)],
                                    false,
                                )],
                            )
                            .check_ir(
                                "switch(expr(List<int>), case(heads(head(varPattern(x, \
                                 matchedType: List<int>, staticType: List<int>), true, \
                                 variables(x1)), head(listPattern(varPattern(x, \
                                 matchedType: int, staticType: int), matchedType: \
                                 List<int>, requiredType: List<int>), true, \
                                 variables(x2)), variables(\
                                 notConsistent:differentFinalityOrType error \
                                 x = [x1, x2])), block(break())))",
                            )]);
                        }
                    }

                    #[test]
                    fn with_different_finality() {
                        let (_s, mut h) = set_up();
                        let x1 = Var::new("x").with_final(true).with_identity("x1");
                        let x2 = Var::new("x").with_identity("x2");
                        Var::join("x", vec![x1, x2]);
                        h.run(vec![switch_(
                            expr("int"),
                            vec![switch_statement_member(
                                vec![x1.pattern(), x2.pattern()],
                                vec![break_(None)],
                                false,
                            )],
                        )
                        .check_ir(
                            "switch(expr(int), case(heads(head(varPattern(x, \
                             matchedType: int, staticType: int), true, variables(x1)), \
                             head(varPattern(x, matchedType: int, staticType: int), \
                             true, variables(x2)), variables(\
                             notConsistent:differentFinalityOrType int x = [x1, x2])), \
                             block(break())))",
                        )]);
                    }
                }

                #[test]
                fn case_has_case_not() {
                    let (_s, mut h) = set_up();
                    let x1 = Var::new("x").with_identity("x1");
                    Var::join("x", vec![x1]);
                    h.run(vec![switch_(
                        expr("int"),
                        vec![switch_statement_member(
                            vec![x1.pattern(), int_literal(0).pattern()],
                            vec![break_(None)],
                            false,
                        )],
                    )
                    .check_ir(
                        "switch(expr(int), case(heads(head(varPattern(x, \
                         matchedType: int, staticType: int), true, variables(x1)), \
                         head(const(0, matchedType: int), true, variables()), \
                         variables(notConsistent:sharedCaseAbsent int x = [x1])), \
                         block(break())))",
                    )]);
                }

                #[test]
                fn case_not_case_has() {
                    let (_s, mut h) = set_up();
                    let x1 = Var::new("x").with_identity("x1");
                    Var::join("x", vec![x1]);
                    h.run(vec![switch_(
                        expr("int"),
                        vec![switch_statement_member(
                            vec![int_literal(0).pattern(), x1.pattern()],
                            vec![break_(None)],
                            false,
                        )],
                    )
                    .check_ir(
                        "switch(expr(int), case(heads(head(const(0, \
                         matchedType: int), true, variables()), head(varPattern(x, \
                         matchedType: int, staticType: int), true, variables(x1)), \
                         variables(notConsistent:sharedCaseAbsent int x = [x1])), \
                         block(break())))",
                    )]);
                }

                #[test]
                fn case_has_default() {
                    let (_s, mut h) = set_up();
                    let x1 = Var::new("x").with_identity("x1");
                    Var::join("x", vec![x1]);
                    h.run(vec![switch_(
                        expr("int"),
                        vec![switch_statement_member(
                            vec![x1.pattern(), default_()],
                            vec![break_(None)],
                            false,
                        )],
                    )
                    .check_ir(
                        "switch(expr(int), case(heads(head(varPattern(x, \
                         matchedType: int, staticType: int), true, variables(x1)), \
                         default, variables(notConsistent:sharedCaseHasLabel int x \
                         = [x1])), block(break())))",
                    )]);
                }

                #[test]
                fn case_has_with_label() {
                    let (_s, mut h) = set_up();
                    let x1 = Var::new("x").with_identity("x1");
                    Var::join("x", vec![x1]);
                    h.run(vec![switch_(
                        expr("int"),
                        vec![switch_statement_member(
                            vec![x1.pattern()],
                            vec![break_(None)],
                            true,
                        )],
                    )
                    .check_ir(
                        "switch(expr(int), case(heads(head(varPattern(x, \
                         matchedType: int, staticType: int), true, variables(x1)), \
                         variables(notConsistent:sharedCaseHasLabel int x = \
                         [x1])), block(break())))",
                    )]);
                }
            }
        }

        mod case_completes_normally {
            use super::*;

            #[test]
            fn reported_when_patterns_disabled() {
                let (_s, mut h) = set_up();
                h.disable_patterns();
                h.run_with(
                    vec![switch_(
                        expr("int"),
                        vec![
                            int_literal(0).pattern().then(vec![expr("int")]),
                            default_().then(vec![break_(None)]),
                        ],
                    )
                    .with_legacy_exhaustive(true)
                    .error_id("SWITCH")],
                    errors(&["switchCaseCompletesNormally(node: SWITCH, caseIndex: 0)"]),
                );
            }

            #[test]
            fn handles_cases_that_share_a_body() {
                let (_s, mut h) = set_up();
                h.disable_patterns();
                h.run_with(
                    vec![switch_(
                        expr("int"),
                        vec![
                            switch_statement_member(
                                vec![
                                    int_literal(0).pattern(),
                                    int_literal(1).pattern(),
                                    int_literal(2).pattern(),
                                ],
                                vec![expr("int")],
                                false,
                            ),
                            default_().then(vec![break_(None)]),
                        ],
                    )
                    .with_legacy_exhaustive(true)
                    .error_id("SWITCH")],
                    errors(&["switchCaseCompletesNormally(node: SWITCH, caseIndex: 0)"]),
                );
            }

            #[test]
            fn not_reported_when_unreachable() {
                let (_s, mut h) = set_up();
                h.disable_patterns();
                h.run_with(
                    vec![switch_(
                        expr("int"),
                        vec![
                            int_literal(0).pattern().then(vec![break_(None)]),
                            default_().then(vec![break_(None)]),
                        ],
                    )
                    .with_legacy_exhaustive(true)],
                    errors(&[]),
                );
            }

            #[test]
            fn not_reported_for_final_case() {
                let (_s, mut h) = set_up();
                h.disable_patterns();
                h.run_with(
                    vec![switch_(
                        expr("int"),
                        vec![int_literal(0).pattern().then(vec![expr("int")])],
                    )
                    .with_legacy_exhaustive(false)],
                    errors(&[]),
                );
            }

            #[test]
            fn not_reported_when_patterns_enabled() {
                let (_s, mut h) = set_up();
                // When patterns are enabled, there is an implicit `break` at the end
                // of every switch body.
                h.run_with(
                    vec![switch_(
                        expr("int"),
                        vec![
                            int_literal(0).pattern().then(vec![expr("int")]),
                            default_().then(vec![break_(None)]),
                        ],
                    )],
                    errors(&[]),
                );
            }
        }

        mod case_expression_type_mismatch {
            use super::*;

            mod null_safe_patterns_disabled {
                use super::*;

                #[test]
                fn subtype() {
                    let (_s, mut h) = set_up();
                    h.disable_patterns();
                    h.run(vec![switch_(
                        expr("num"),
                        vec![expr("int").pattern().then(vec![break_(None)])],
                    )
                    .with_legacy_exhaustive(false)]);
                }

                #[test]
                fn supertype() {
                    let (_s, mut h) = set_up();
                    h.disable_patterns();
                    h.run_with(
                        vec![switch_(
                            expr("int").error_id("SCRUTINEE"),
                            vec![expr("num")
                                .error_id("EXPRESSION")
                                .pattern()
                                .then(vec![break_(None)])],
                        )
                        .with_legacy_exhaustive(false)],
                        errors(&["caseExpressionTypeMismatch(scrutinee: SCRUTINEE, \
                                  caseExpression: EXPRESSION, scrutineeType: int, \
                                  caseExpressionType: num)"]),
                    );
                }

                #[test]
                fn unrelated_types() {
                    let (_s, mut h) = set_up();
                    h.disable_patterns();
                    h.run_with(
                        vec![switch_(
                            expr("int").error_id("SCRUTINEE"),
                            vec![expr("String")
                                .error_id("EXPRESSION")
                                .pattern()
                                .then(vec![break_(None)])],
                        )
                        .with_legacy_exhaustive(false)],
                        errors(&["caseExpressionTypeMismatch(scrutinee: SCRUTINEE, \
                                  caseExpression: EXPRESSION, scrutineeType: int, \
                                  caseExpressionType: String)"]),
                    );
                }

                #[test]
                fn dynamic_scrutinee() {
                    let (_s, mut h) = set_up();
                    h.disable_patterns();
                    h.run(vec![switch_(
                        expr("dynamic"),
                        vec![expr("int").pattern().then(vec![break_(None)])],
                    )
                    .with_legacy_exhaustive(false)]);
                }

                #[test]
                fn dynamic_case() {
                    let (_s, mut h) = set_up();
                    h.disable_patterns();
                    h.run_with(
                        vec![switch_(
                            expr("int").error_id("SCRUTINEE"),
                            vec![expr("dynamic")
                                .error_id("EXPRESSION")
                                .pattern()
                                .then(vec![break_(None)])],
                        )
                        .with_legacy_exhaustive(false)],
                        errors(&["caseExpressionTypeMismatch(scrutinee: SCRUTINEE, \
                                  caseExpression: EXPRESSION, scrutineeType: int, \
                                  caseExpressionType: dynamic)"]),
                    );
                }
            }

            mod patterns_enabled {
                use super::*;

                #[test]
                fn subtype() {
                    let (_s, mut h) = set_up();
                    h.run(vec![switch_(
                        expr("num"),
                        vec![expr("int").pattern().then(vec![break_(None)])],
                    )]);
                }

                #[test]
                fn supertype() {
                    let (_s, mut h) = set_up();
                    h.run(vec![switch_(
                        expr("int"),
                        vec![expr("num").pattern().then(vec![break_(None)])],
                    )]);
                }

                #[test]
                fn unrelated_types() {
                    let (_s, mut h) = set_up();
                    h.run(vec![switch_(
                        expr("int"),
                        vec![expr("String").pattern().then(vec![break_(None)])],
                    )]);
                }

                #[test]
                fn dynamic_scrutinee() {
                    let (_s, mut h) = set_up();
                    h.run(vec![switch_(
                        expr("dynamic"),
                        vec![expr("int").pattern().then(vec![break_(None)])],
                    )]);
                }

                #[test]
                fn dynamic_case() {
                    let (_s, mut h) = set_up();
                    h.run(vec![switch_(
                        expr("int"),
                        vec![expr("dynamic").pattern().then(vec![break_(None)])],
                    )]);
                }
            }
        }

        mod pre_merged {
            use super::*;

            // The CFE merges cases that share a body at parse time, so make sure we
            // we can handle merged cases
            #[test]
            fn empty() {
                let (_s, mut h) = set_up();
                // During CFE error recovery, there can be an empty case.
                h.run_with(
                    vec![switch_(
                        expr("int"),
                        vec![switch_statement_member(vec![], vec![break_(None)], false)],
                    )
                    .check_ir(
                        "switch(expr(int), case(heads(variables()), \
                         block(break())))",
                    )],
                    RunOptions {
                        error_recovery_ok: true,
                        ..RunOptions::default()
                    },
                );
            }

            #[test]
            fn multiple() {
                let (_s, mut h) = set_up();
                h.run(vec![switch_(
                    expr("int"),
                    vec![switch_statement_member(
                        vec![int_literal(0).pattern(), int_literal(1).pattern()],
                        vec![break_(None)],
                        false,
                    )],
                )
                .check_ir(
                    "switch(expr(int), case(heads(head(const(0, \
                     matchedType: int), true, variables()), head(const(1, \
                     matchedType: int), true, variables()), variables()), \
                     block(break())))",
                )]);
            }
        }

        mod guard_not_assignable_to_bool {
            use super::*;

            #[test]
            fn int() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run_with(
                    vec![switch_(
                        expr("int"),
                        vec![x
                            .pattern()
                            .when(Some(expr("int").error_id("GUARD")))
                            .then(vec![break_(None)])],
                    )],
                    errors(&["nonBooleanCondition(node: GUARD)"]),
                );
            }

            #[test]
            fn bool() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run_with(
                    vec![switch_(
                        expr("int"),
                        vec![x
                            .pattern()
                            .when(Some(expr("bool")))
                            .then(vec![break_(None)])],
                    )],
                    errors(&[]),
                );
            }

            #[test]
            fn dynamic() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run_with(
                    vec![switch_(
                        expr("int"),
                        vec![x
                            .pattern()
                            .when(Some(expr("dynamic")))
                            .then(vec![break_(None)])],
                    )],
                    errors(&[]),
                );
            }
        }

        mod requires_exhaustiveness_validation {
            use super::*;

            #[test]
            fn when_a_default_clause_is_present() {
                let (_s, mut h) = set_up();
                h.add_exhaustiveness("E", true);
                h.run(vec![switch_(expr("E"), vec![default_().then(vec![break_(None)])])
                    .expect_requires_exhaustiveness_validation(false)]);
            }

            #[test]
            fn when_the_scrutinee_is_an_always_exhaustive_type() {
                let (_s, mut h) = set_up();
                h.add_exhaustiveness("E", true);
                h.run(vec![switch_(
                    expr("E"),
                    vec![expr("E").pattern().then(vec![break_(None)])],
                )
                .expect_requires_exhaustiveness_validation(true)]);
            }

            #[test]
            fn when_the_scrutinee_is_not_an_always_exhaustive_type() {
                let (_s, mut h) = set_up();
                h.add_exhaustiveness("C", false);
                h.run(vec![switch_(
                    expr("C"),
                    vec![expr("C").pattern().then(vec![break_(None)])],
                )
                .expect_requires_exhaustiveness_validation(false)]);
            }

            #[test]
            fn when_pattern_support_is_disabled() {
                let (_s, mut h) = set_up();
                h.disable_patterns();
                h.add_exhaustiveness("E", true);
                h.run(vec![switch_(
                    expr("E"),
                    vec![expr("E").pattern().then(vec![break_(None)])],
                )
                .with_legacy_exhaustive(true)
                .expect_requires_exhaustiveness_validation(false)]);
            }
        }
    }

    mod variable_declaration {
        use super::*;

        #[test]
        fn initialized_typed() {
            let (_s, mut h) = set_up();
            let x = Var::new("x");
            h.run(vec![declare(x)
                .with_declared_type("num")
                .with_initializer(expr("int").check_schema("num"))
                .check_ir("declare(x, expr(int), initializerType: int, staticType: num)")]);
        }

        #[test]
        fn initialized_untyped() {
            let (_s, mut h) = set_up();
            let x = Var::new("x");
            h.run(vec![declare(x)
                .with_initializer(expr("int").check_schema("_"))
                .check_ir("declare(x, expr(int), initializerType: int, staticType: int)")]);
        }

        #[test]
        fn uninitialized_typed() {
            let (_s, mut h) = set_up();
            let x = Var::new("x");
            h.run(vec![declare(x)
                .with_declared_type("int")
                .check_ir("declare(x, staticType: int)")]);
        }

        #[test]
        fn uninitialized_untyped() {
            let (_s, mut h) = set_up();
            let x = Var::new("x");
            h.run(vec![declare(x).check_ir("declare(x, staticType: dynamic)")]);
        }

        #[test]
        fn promoted_initializer() {
            let (_s, mut h) = set_up();
            TypeRegistry::add_type_parameter("T");
            let x = Var::new("x");
            h.run(vec![declare(x)
                .with_initializer(expr("T&int"))
                .check_ir("declare(x, expr(T&int), initializerType: T&int, staticType: T)")]);
        }

        #[test]
        fn legal_late_pattern() {
            let (_s, mut h) = set_up();
            let x = Var::new("x");
            h.run(vec![declare(x)
                .with_initializer(int_literal(0))
                .with_late()
                .check_ir("declare_late(x, 0, initializerType: int, staticType: int)")]);
        }

        #[test]
        fn illegal_refutable_pattern() {
            let (_s, mut h) = set_up();
            h.run_with(
                vec![pattern_variable_declaration(
                    int_literal(1).pattern().error_id("PATTERN"),
                    int_literal(0),
                    false,
                )
                .error_id("CONTEXT")],
                errors(&["refutablePatternInIrrefutableContext(pattern: PATTERN, \
                          context: CONTEXT)"]),
            );
        }
    }

    mod pattern_for_in {
        use super::*;

        mod sync {
            use super::*;

            mod expression_context_type_schema {
                use super::*;

                #[test]
                fn pattern_has_type() {
                    let (_s, mut h) = set_up();
                    let x = Var::new("x");
                    h.run(vec![pattern_for_in(
                        x.pattern().with_declared_type("num"),
                        expr("List<int>").check_schema("Iterable<num>"),
                        vec![],
                        false,
                    )
                    .check_ir(
                        "forEach(expr(List<int>), varPattern(x, \
                         matchedType: int, staticType: num), block())",
                    )]);
                }

                #[test]
                fn pattern_does_not_have_type() {
                    let (_s, mut h) = set_up();
                    let x = Var::new("x");
                    h.run(vec![pattern_for_in(
                        x.pattern(),
                        expr("List<int>").check_schema("Iterable<_>"),
                        vec![],
                        false,
                    )
                    .check_ir(
                        "forEach(expr(List<int>), varPattern(x, \
                         matchedType: int, staticType: int), block())",
                    )]);
                }
            }

            mod expression_type {
                use super::*;

                #[test]
                fn iterable() {
                    let (_s, mut h) = set_up();
                    let x = Var::new("x");
                    h.run(vec![pattern_for_in(x.pattern(), expr("Iterable<int>"), vec![], false)
                        .check_ir(
                            "forEach(expr(Iterable<int>), varPattern(x, \
                             matchedType: int, staticType: int), block())",
                        )]);
                }

                #[test]
                fn dynamic() {
                    let (_s, mut h) = set_up();
                    let x = Var::new("x");
                    h.run(vec![pattern_for_in(x.pattern(), expr("dynamic"), vec![], false)
                        .check_ir(
                            "forEach(expr(dynamic), varPattern(x, \
                             matchedType: dynamic, staticType: dynamic), block())",
                        )]);
                }

                #[test]
                fn object() {
                    let (_s, mut h) = set_up();
                    let x = Var::new("x");
                    h.run_with(
                        vec![pattern_for_in(
                            x.pattern(),
                            expr("Object").error_id("EXPRESSION"),
                            vec![],
                            false,
                        )
                        .error_id("FOR")
                        .check_ir(
                            "forEach(expr(Object), varPattern(x, \
                             matchedType: error, staticType: error), block())",
                        )],
                        errors(&["patternForInExpressionIsNotIterable(node: FOR, \
                                  expression: EXPRESSION, expressionType: Object)"]),
                    );
                }

                #[test]
                fn error() {
                    let (_s, mut h) = set_up();
                    let x = Var::new("x");
                    h.run(vec![pattern_for_in(x.pattern(), expr("error"), vec![], false)
                        .check_ir(
                            "forEach(expr(error), varPattern(x, \
                             matchedType: error, staticType: error), block())",
                        )]);
                }
            }

            mod refutability {
                use super::*;

                #[test]
                fn when_a_refutable_pattern() {
                    let (_s, mut h) = set_up();
                    let x = Var::new("x");
                    h.run_with(
                        vec![pattern_for_in(
                            x.pattern().null_check().error_id("PATTERN"),
                            expr("Iterable<int?>"),
                            vec![],
                            false,
                        )
                        .error_id("FOR")
                        .check_ir(
                            "forEach(expr(Iterable<int?>), nullCheckPattern(\
                             varPattern(x, matchedType: int, staticType: int), \
                             matchedType: int?), block())",
                        )],
                        errors(&["refutablePatternInIrrefutableContext(pattern: PATTERN, \
                                  context: FOR)"]),
                    );
                }

                #[test]
                fn when_the_variable_type_is_not_a_subtype_of_the_matched_type() {
                    let (_s, mut h) = set_up();
                    let x = Var::new("x");
                    h.run_with(
                        vec![pattern_for_in(
                            x.pattern().with_declared_type("String").error_id("PATTERN"),
                            expr("Iterable<int>"),
                            vec![],
                            false,
                        )
                        .error_id("FOR")
                        .check_ir(
                            "forEach(expr(Iterable<int>), varPattern(x, \
                             matchedType: int, staticType: String), block())",
                        )],
                        errors(&["patternTypeMismatchInIrrefutableContext(pattern: PATTERN, \
                                  context: FOR, matchedType: int, requiredType: String)"]),
                    );
                }
            }
        }

        mod async_ {
            use super::*;

            mod expression_context_type_schema {
                use super::*;

                #[test]
                fn pattern_has_type() {
                    let (_s, mut h) = set_up();
                    let x = Var::new("x");
                    h.run(vec![pattern_for_in(
                        x.pattern().with_declared_type("num"),
                        expr("Stream<int>").check_schema("Stream<num>"),
                        vec![],
                        true,
                    )
                    .check_ir(
                        "forEach(expr(Stream<int>), varPattern(x, \
                         matchedType: int, staticType: num), block())",
                    )]);
                }

                #[test]
                fn pattern_does_not_have_type() {
                    let (_s, mut h) = set_up();
                    let x = Var::new("x");
                    h.run(vec![pattern_for_in(
                        x.pattern(),
                        expr("Stream<int>").check_schema("Stream<_>"),
                        vec![],
                        true,
                    )
                    .check_ir(
                        "forEach(expr(Stream<int>), varPattern(x, \
                         matchedType: int, staticType: int), block())",
                    )]);
                }
            }

            mod expression_type {
                use super::*;

                #[test]
                fn stream() {
                    let (_s, mut h) = set_up();
                    let x = Var::new("x");
                    h.run(vec![pattern_for_in(x.pattern(), expr("Stream<int>"), vec![], true)
                        .check_ir(
                            "forEach(expr(Stream<int>), varPattern(x, \
                             matchedType: int, staticType: int), block())",
                        )]);
                }

                #[test]
                fn dynamic() {
                    let (_s, mut h) = set_up();
                    let x = Var::new("x");
                    h.run(vec![pattern_for_in(x.pattern(), expr("dynamic"), vec![], true)
                        .check_ir(
                            "forEach(expr(dynamic), varPattern(x, \
                             matchedType: dynamic, staticType: dynamic), block())",
                        )]);
                }

                #[test]
                fn object() {
                    let (_s, mut h) = set_up();
                    let x = Var::new("x");
                    h.run_with(
                        vec![pattern_for_in(
                            x.pattern(),
                            expr("Object").error_id("EXPRESSION"),
                            vec![],
                            true,
                        )
                        .error_id("FOR")
                        .check_ir(
                            "forEach(expr(Object), varPattern(x, \
                             matchedType: error, staticType: error), block())",
                        )],
                        errors(&["patternForInExpressionIsNotIterable(node: FOR, \
                                  expression: EXPRESSION, expressionType: Object)"]),
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
                        vec![pattern_for_in(
                            x.pattern().null_check().error_id("PATTERN"),
                            expr("Stream<int?>"),
                            vec![],
                            true,
                        )
                        .error_id("FOR")
                        .check_ir(
                            "forEach(expr(Stream<int?>), nullCheckPattern(\
                             varPattern(x, matchedType: int, staticType: int), \
                             matchedType: int?), block())",
                        )],
                        errors(&["refutablePatternInIrrefutableContext(pattern: PATTERN, \
                                  context: FOR)"]),
                    );
                }

                #[test]
                fn when_the_variable_type_is_not_a_subtype_of_the_matched_type() {
                    let (_s, mut h) = set_up();
                    let x = Var::new("x");
                    h.run_with(
                        vec![pattern_for_in(
                            x.pattern().with_declared_type("String").error_id("PATTERN"),
                            expr("Stream<int>"),
                            vec![],
                            true,
                        )
                        .error_id("FOR")
                        .check_ir(
                            "forEach(expr(Stream<int>), varPattern(x, \
                             matchedType: int, staticType: String), block())",
                        )],
                        errors(&["patternTypeMismatchInIrrefutableContext(pattern: PATTERN, \
                                  context: FOR, matchedType: int, requiredType: String)"]),
                    );
                }
            }
        }
    }

    mod yield_ {
        use super::*;

        mod yield_expression_result {
            use super::*;

            mod operand_type {
                use super::*;

                // Dart checks `(result as YieldStatementResult).operandType`
                // here. The harness `check_statement_type_analysis_result`
                // checker gets no result, so these tests only run the
                // analysis.
                #[test]
                fn yield_() {
                    let (_s, mut h) = set_up();
                    h.run(vec![crate::mini_ast::node::yield_(expr("int"), false).check_statement_type_analysis_result(
                        |result| {
                            assert_eq!(result.operand_type.unwrap().to_string(), "int");
                        },
                    )]);
                }

                #[test]
                fn yield_star() {
                    let (_s, mut h) = set_up();
                    h.run(vec![crate::mini_ast::node::yield_(expr("List<int>"), true)
                        .check_statement_type_analysis_result(|result| {
                            assert_eq!(result.operand_type.unwrap().to_string(), "List<int>");
                        })]);
                }
            }
        }

        mod downward_inference {
            use super::*;

            mod with_unknown_context {
                use super::*;

                mod yield_ {
                    use super::*;

                    #[test]
                    fn sync() {
                        let (_s, mut h) = set_up();
                        h.run_with(
                            vec![yield_(expr("int").check_schema("_"), false)],
                            body_context(false, "_"),
                        );
                    }

                    #[test]
                    fn async_() {
                        let (_s, mut h) = set_up();
                        h.run_with(
                            vec![yield_(expr("int").check_schema("_"), false)],
                            body_context(true, "_"),
                        );
                    }
                }

                mod yield_star {
                    use super::*;

                    #[test]
                    fn sync() {
                        let (_s, mut h) = set_up();
                        h.run_with(
                            vec![yield_(expr("int").check_schema("_"), true)],
                            body_context(false, "_"),
                        );
                    }

                    #[test]
                    fn async_() {
                        let (_s, mut h) = set_up();
                        h.run_with(
                            vec![yield_(expr("int").check_schema("_"), true)],
                            body_context(true, "_"),
                        );
                    }
                }
            }

            mod with_known_context {
                use super::*;

                mod yield_ {
                    use super::*;

                    #[test]
                    fn sync() {
                        let (_s, mut h) = set_up();
                        h.run_with(
                            vec![yield_(expr("int").check_schema("int"), false)],
                            body_context(false, "int"),
                        );
                    }

                    #[test]
                    fn async_() {
                        let (_s, mut h) = set_up();
                        h.run_with(
                            vec![yield_(expr("int").check_schema("int"), false)],
                            body_context(true, "int"),
                        );
                    }
                }

                mod yield_star {
                    use super::*;

                    #[test]
                    fn sync() {
                        let (_s, mut h) = set_up();
                        h.run_with(
                            vec![yield_(expr("int").check_schema("Iterable<int>"), true)],
                            body_context(false, "int"),
                        );
                    }

                    #[test]
                    fn async_() {
                        let (_s, mut h) = set_up();
                        h.run_with(
                            vec![yield_(expr("int").check_schema("Stream<int>"), true)],
                            body_context(true, "int"),
                        );
                    }
                }
            }
        }
    }
}
