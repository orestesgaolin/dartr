// Dart source: pkg/_fe_analyzer_shared/test/type_inference/type_inference_test.dart (group "Patterns:", Cast to Logical-or)

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

    mod cast {
        use super::*;

        #[test]
        fn type_schema() {
            let (_s, mut h) = set_up();
            let x = Var::new("x");
            h.run(vec![
                pattern_variable_declaration(
                    x.pattern().as_("int"),
                    expr("num").check_schema("_"),
                    false,
                )
                .check_ir(
                    "match(expr(num), castPattern(varPattern(x, \
                 matchedType: int, staticType: int), int, matchedType: num))",
                ),
            ]);
        }

        mod refutable_context {
            use super::*;

            #[test]
            fn when_matched_type_is_a_subtype_of_required_type() {
                let (_s, mut h) = set_up();
                h.run_with(
                    vec![
                        if_case(
                            expr("int"),
                            wildcard().as_("num").error_id("PATTERN"),
                            vec![],
                            None,
                        )
                        .check_ir(
                            "ifCase(expr(int), castPattern(wildcardPattern(\
                         matchedType: num), num, matchedType: int), \
                         variables(), true, block(), noop)",
                        ),
                    ],
                    errors(&["matchedTypeIsSubtypeOfRequired(pattern: PATTERN, \
                         matchedType: int, requiredType: num)"]),
                );
            }
        }

        mod refutability {
            use super::*;

            #[test]
            fn when_matched_type_is_a_subtype_of_variable_type() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run_with(
                    vec![
                        pattern_variable_declaration(
                            x.pattern().as_("num").error_id("PATTERN"),
                            expr("int"),
                            false,
                        )
                        .check_ir(
                            "match(expr(int), \
                         castPattern(varPattern(x, matchedType: num, \
                         staticType: num), num, matchedType: int))",
                        ),
                    ],
                    errors(&["matchedTypeIsSubtypeOfRequired(pattern: PATTERN, \
                         matchedType: int, requiredType: num)"]),
                );
            }

            #[test]
            fn when_matched_type_is_dynamic() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run(vec![
                    pattern_variable_declaration(x.pattern().as_("num"), expr("dynamic"), false)
                        .check_ir(
                            "match(expr(dynamic), \
                     castPattern(varPattern(x, matchedType: num, \
                     staticType: num), num, matchedType: dynamic))",
                        ),
                ]);
            }

            #[test]
            fn when_matched_type_is_not_a_subtype_of_variable_type() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run(vec![
                    pattern_variable_declaration(x.pattern().as_("num"), expr("String"), false)
                        .check_ir(
                            "match(expr(String), \
                     castPattern(varPattern(x, matchedType: num, \
                     staticType: num), num, matchedType: String))",
                        ),
                ]);
            }
        }

        mod fully_covered_due_to_extension_type_erasure {
            use super::*;

            #[test]
            #[ignore = "needs flow analysis: if-case else branch unreachable when the cast pattern fully covers the matched type"]
            fn cast_to_representation_type() {
                // If an `as` pattern fully covers the matched value type due to
                // extension type erasure, the "matchedTypeIsSubtypeOfRequired"
                // warning should not be issued.
                let (_s, mut h) = set_up();
                h.add_super_interfaces("E", |_| vec![Type::parse("Object?")]);
                h.add_extension_type_erasure("E", "int");
                h.run(vec![if_case(
                    expr("E"),
                    wildcard().as_("int"),
                    vec![check_reachable(true)],
                    Some(vec![check_reachable(false)]),
                )]);
            }

            #[test]
            #[ignore = "needs flow analysis: if-case else branch unreachable when the cast pattern fully covers the matched type"]
            fn cast_to_extension_type() {
                // If an `as` pattern fully covers the matched value type due to
                // extension type erasure, the "matchedTypeIsSubtypeOfRequired"
                // warning should not be issued.
                let (_s, mut h) = set_up();
                h.add_super_interfaces("E", |_| vec![Type::parse("Object?")]);
                h.add_extension_type_erasure("E", "int");
                h.run(vec![if_case(
                    expr("int"),
                    wildcard().as_("E"),
                    vec![check_reachable(true)],
                    Some(vec![check_reachable(false)]),
                )]);
            }
        }
    }

    mod const_or_literal {
        use super::*;

        #[test]
        fn refutability() {
            let (_s, mut h) = set_up();
            h.run_with(
                vec![
                    pattern_variable_declaration(
                        int_literal(1).pattern().error_id("PATTERN"),
                        int_literal(0),
                        false,
                    )
                    .error_id("CONTEXT"),
                ],
                errors(&["refutablePatternInIrrefutableContext(pattern: PATTERN, \
                     context: CONTEXT)"]),
            );
        }
    }

    mod map {
        use super::*;

        mod type_schema {
            use super::*;

            #[test]
            fn explicit_type_arguments() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run(vec![pattern_variable_declaration(
                    map_pattern_with_type_arguments(
                        "bool",
                        "int",
                        vec![map_pattern_entry(expr("int"), x.pattern())],
                    ),
                    expr("dynamic").check_schema("Map<bool, int>"),
                    false,
                )]);
            }

            mod implicit_element_type {
                use super::*;

                #[test]
                fn no_elements() {
                    let (_s, mut h) = set_up();
                    h.run_with(
                        vec![pattern_variable_declaration(
                            map_pattern(vec![], None, None).error_id("PATTERN"),
                            expr("dynamic").check_schema("Map<_, _>"),
                            false,
                        )],
                        errors(&["emptyMapPattern(pattern: PATTERN)"]),
                    );
                }

                #[test]
                fn variable_patterns() {
                    let (_s, mut h) = set_up();
                    let x = Var::new("x");
                    let y = Var::new("y");
                    h.run(vec![pattern_variable_declaration(
                        map_pattern(
                            vec![
                                map_pattern_entry(
                                    expr("bool"),
                                    x.pattern().with_declared_type("int?"),
                                ),
                                map_pattern_entry(
                                    expr("bool"),
                                    y.pattern().with_declared_type("num"),
                                ),
                            ],
                            None,
                            None,
                        ),
                        expr("dynamic").check_schema("Map<_, int>"),
                        false,
                    )]);
                }
            }
        }

        mod static_type {
            use super::*;

            #[test]
            fn explicit_type_arguments() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run(vec![
                    if_case(
                        expr("dynamic"),
                        map_pattern_with_type_arguments(
                            "bool",
                            "int",
                            vec![map_pattern_entry(
                                expr("Object").check_schema("bool"),
                                x.pattern(),
                            )],
                        ),
                        vec![],
                        None,
                    )
                    .check_ir(
                        "ifCase(expr(dynamic), mapPattern(mapPatternEntry(\
                     expr(Object), varPattern(x, matchedType: int, staticType: \
                     int)), matchedType: dynamic, requiredType: Map<bool, int>), \
                     variables(x), true, block(), noop)",
                    ),
                ]);
            }

            #[test]
            fn matched_type_is_a_map() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run(vec![
                    if_case(
                        expr("Map<bool, int>"),
                        map_pattern(
                            vec![map_pattern_entry(
                                expr("Object").check_schema("bool"),
                                x.pattern(),
                            )],
                            None,
                            None,
                        ),
                        vec![],
                        None,
                    )
                    .check_ir(
                        "ifCase(expr(Map<bool, int>), mapPattern(mapPatternEntry(\
                     expr(Object), varPattern(x, matchedType: int, staticType: \
                     int)), matchedType: Map<bool, int>, requiredType: \
                     Map<bool, int>), variables(x), true, block(), noop)",
                    ),
                ]);
            }

            #[test]
            fn matched_type_is_dynamic() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run(vec![
                    if_case(
                        expr("dynamic"),
                        map_pattern(
                            vec![map_pattern_entry(
                                expr("Object").check_schema("_"),
                                x.pattern(),
                            )],
                            None,
                            None,
                        ),
                        vec![],
                        None,
                    )
                    .check_ir(
                        "ifCase(expr(dynamic), mapPattern(mapPatternEntry(\
                     expr(Object), varPattern(x, matchedType: dynamic, staticType: \
                     dynamic)), matchedType: dynamic, requiredType: \
                     Map<dynamic, dynamic>), variables(x), true, block(), noop)",
                    ),
                ]);
            }

            #[test]
            fn matched_type_is_error() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run(vec![
                    if_case(
                        expr("error"),
                        map_pattern(
                            vec![map_pattern_entry(
                                expr("Object").check_schema("_"),
                                x.pattern(),
                            )],
                            None,
                            None,
                        ),
                        vec![],
                        None,
                    )
                    .check_ir(
                        "ifCase(expr(error), mapPattern(mapPatternEntry(\
                     expr(Object), varPattern(x, matchedType: error, staticType: \
                     error)), matchedType: error, requiredType: \
                     Map<error, error>), variables(x), true, block(), noop)",
                    ),
                ]);
            }

            #[test]
            fn matched_type_is_other() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run(vec![
                    if_case(
                        expr("String"),
                        map_pattern(
                            vec![map_pattern_entry(
                                expr("Object").check_schema("_"),
                                x.pattern(),
                            )],
                            None,
                            None,
                        ),
                        vec![],
                        None,
                    )
                    .check_ir(
                        "ifCase(expr(String), mapPattern(mapPatternEntry(\
                     expr(Object), varPattern(x, matchedType: Object?, staticType: \
                     Object?)), matchedType: String, requiredType: \
                     Map<Object?, Object?>), variables(x), true, block(), noop)",
                    ),
                ]);
            }
        }

        mod refutable_context {
            use super::*;

            #[test]
            fn when_matched_type_is_a_subtype_of_required_type() {
                let (_s, mut h) = set_up();
                h.run(vec![
                    pattern_variable_declaration(
                        map_pattern_with_type_arguments(
                            "Object",
                            "num",
                            vec![map_pattern_entry(
                                expr("Object").check_schema("Object"),
                                wildcard(),
                            )],
                        ),
                        expr("Map<bool, int>"),
                        false,
                    )
                    .check_ir(
                        "match(expr(Map<bool, int>), mapPattern(mapPatternEntry(\
                     expr(Object), wildcardPattern(matchedType: num)), \
                     matchedType: Map<bool, int>, \
                     requiredType: Map<Object, num>))",
                    ),
                ]);
            }

            #[test]
            fn when_matched_type_is_dynamic() {
                let (_s, mut h) = set_up();
                h.run(vec![
                    pattern_variable_declaration(
                        map_pattern_with_type_arguments(
                            "Object",
                            "num",
                            vec![map_pattern_entry(
                                expr("Object").check_schema("Object"),
                                wildcard(),
                            )],
                        ),
                        expr("dynamic"),
                        false,
                    )
                    .check_ir(
                        "match(expr(dynamic), mapPattern(mapPatternEntry(\
                     expr(Object), wildcardPattern(matchedType: num)), \
                     matchedType: dynamic, requiredType: Map<Object, num>))",
                    ),
                ]);
            }

            #[test]
            fn when_matched_type_is_not_a_subtype_of_required_type() {
                let (_s, mut h) = set_up();
                h.run_with(
                    vec![
                        pattern_variable_declaration(
                            map_pattern_with_type_arguments(
                                "bool",
                                "int",
                                vec![map_pattern_entry(
                                    expr("Object").check_schema("bool"),
                                    wildcard(),
                                )],
                            )
                            .error_id("PATTERN"),
                            expr("String"),
                            false,
                        )
                        .error_id("CONTEXT"),
                    ],
                    errors(
                        &["patternTypeMismatchInIrrefutableContext(pattern: PATTERN, \
                         context: CONTEXT, matchedType: String, \
                         requiredType: Map<bool, int>)"],
                    ),
                );
            }
        }

        mod errors_ {
            use super::*;

            #[test]
            fn rest_pattern_first() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run_with(
                    vec![pattern_variable_declaration(
                        map_pattern(
                            vec![
                                rest_pattern(None).error_id("REST_ELEMENT"),
                                map_pattern_entry(
                                    expr("bool"),
                                    x.pattern().with_declared_type("int"),
                                ),
                            ],
                            None,
                            None,
                        )
                        .error_id("MAP_PATTERN"),
                        expr("dynamic"),
                        false,
                    )],
                    errors(&["restPatternInMap(node: MAP_PATTERN, element: REST_ELEMENT)"]),
                );
            }

            #[test]
            fn rest_pattern_last() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run_with(
                    vec![pattern_variable_declaration(
                        map_pattern(
                            vec![
                                map_pattern_entry(
                                    expr("bool"),
                                    x.pattern().with_declared_type("int"),
                                ),
                                rest_pattern(None).error_id("REST_ELEMENT"),
                            ],
                            None,
                            None,
                        )
                        .error_id("MAP_PATTERN"),
                        expr("dynamic"),
                        false,
                    )],
                    errors(&["restPatternInMap(node: MAP_PATTERN, element: REST_ELEMENT)"]),
                );
            }

            #[test]
            fn two_rest_elements_at_the_end() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run_with(
                    vec![pattern_variable_declaration(
                        map_pattern(
                            vec![
                                map_pattern_entry(
                                    expr("bool"),
                                    x.pattern().with_declared_type("int"),
                                ),
                                rest_pattern(None).error_id("REST_ELEMENT1"),
                                rest_pattern(None).error_id("REST_ELEMENT2"),
                            ],
                            None,
                            None,
                        )
                        .error_id("MAP_PATTERN"),
                        expr("dynamic"),
                        false,
                    )],
                    errors(&[
                        "restPatternInMap(node: MAP_PATTERN, element: REST_ELEMENT1)",
                        "restPatternInMap(node: MAP_PATTERN, element: REST_ELEMENT2)",
                    ]),
                );
            }

            #[test]
            fn two_rest_elements_not_at_the_end() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run_with(
                    vec![pattern_variable_declaration(
                        map_pattern(
                            vec![
                                rest_pattern(None).error_id("REST_ELEMENT1"),
                                rest_pattern(None).error_id("REST_ELEMENT2"),
                                map_pattern_entry(
                                    expr("bool"),
                                    x.pattern().with_declared_type("int"),
                                ),
                            ],
                            None,
                            None,
                        )
                        .error_id("MAP_PATTERN"),
                        expr("dynamic"),
                        false,
                    )],
                    errors(&[
                        "restPatternInMap(node: MAP_PATTERN, element: REST_ELEMENT1)",
                        "restPatternInMap(node: MAP_PATTERN, element: REST_ELEMENT2)",
                    ]),
                );
            }

            #[test]
            fn rest_pattern_with_subpattern() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run_with(
                    vec![pattern_variable_declaration(
                        map_pattern(
                            vec![
                                map_pattern_entry(
                                    expr("bool"),
                                    x.pattern().with_declared_type("int"),
                                ),
                                rest_pattern(Some(wildcard())).error_id("REST_ELEMENT"),
                            ],
                            None,
                            None,
                        )
                        .error_id("MAP_PATTERN"),
                        expr("dynamic"),
                        false,
                    )],
                    errors(&["restPatternInMap(node: MAP_PATTERN, element: REST_ELEMENT)"]),
                );
            }
        }
    }

    mod list {
        use super::*;

        mod type_schema {
            use super::*;

            #[test]
            fn explicit_element_type() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run(vec![pattern_variable_declaration(
                    list_pattern(vec![x.pattern()], Some("int")),
                    expr("dynamic").check_schema("List<int>"),
                    false,
                )]);
            }

            mod implicit_element_type {
                use super::*;

                #[test]
                fn no_elements() {
                    let (_s, mut h) = set_up();
                    h.run(vec![pattern_variable_declaration(
                        list_pattern(vec![], None),
                        expr("dynamic").check_schema("List<_>"),
                        false,
                    )]);
                }

                #[test]
                fn variable_patterns() {
                    let (_s, mut h) = set_up();
                    let x = Var::new("x");
                    let y = Var::new("y");
                    h.run(vec![pattern_variable_declaration(
                        list_pattern(
                            vec![
                                x.pattern().with_declared_type("int?"),
                                y.pattern().with_declared_type("num"),
                            ],
                            None,
                        ),
                        expr("dynamic").check_schema("List<int>"),
                        false,
                    )]);
                }

                mod rest_pattern {
                    use super::*;

                    mod with_pattern {
                        use super::*;

                        #[test]
                        fn iterable() {
                            let (_s, mut h) = set_up();
                            let x = Var::new("x");
                            h.run(vec![pattern_variable_declaration(
                                list_pattern(
                                    vec![rest_pattern(Some(
                                        x.pattern().with_declared_type("Iterable<int>"),
                                    ))],
                                    None,
                                ),
                                expr("List<int>").check_schema("List<int>"),
                                false,
                            )]);
                        }

                        #[test]
                        fn not_iterable() {
                            let (_s, mut h) = set_up();
                            let x = Var::new("x");
                            h.run_with(
                                vec![
                                    pattern_variable_declaration(
                                        list_pattern(
                                            vec![rest_pattern(Some(
                                                x.pattern()
                                                    .with_declared_type("String")
                                                    .error_id("VAR(x)"),
                                            ))],
                                            None,
                                        ),
                                        expr("List<int>").check_schema("List<_>"),
                                        false,
                                    )
                                    .error_id("CONTEXT"),
                                ],
                                errors(&["patternTypeMismatchInIrrefutableContext(\
                                     pattern: VAR(x), context: CONTEXT, matchedType: \
                                     List<int>, requiredType: String)"]),
                            );
                        }
                    }

                    mod without_pattern {
                        use super::*;

                        #[test]
                        fn no_other_elements() {
                            let (_s, mut h) = set_up();
                            h.run(vec![pattern_variable_declaration(
                                list_pattern(vec![rest_pattern(None)], None),
                                expr("dynamic").check_schema("List<_>"),
                                false,
                            )]);
                        }

                        #[test]
                        fn has_other_elements() {
                            let (_s, mut h) = set_up();
                            let x = Var::new("x");
                            h.run(vec![pattern_variable_declaration(
                                list_pattern(
                                    vec![x.pattern().with_declared_type("int"), rest_pattern(None)],
                                    None,
                                ),
                                expr("dynamic").check_schema("List<int>"),
                                false,
                            )]);
                        }
                    }
                }
            }
        }

        mod static_type {
            use super::*;

            #[test]
            fn explicit_type() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run(vec![
                    pattern_variable_declaration(
                        list_pattern(vec![x.pattern().with_declared_type("num")], Some("int")),
                        expr("dynamic"),
                        false,
                    )
                    .check_ir(
                        "match(expr(dynamic), \
                     listPattern(varPattern(x, matchedType: int, \
                     staticType: num), \
                     matchedType: dynamic, requiredType: List<int>))",
                    ),
                ]);
            }

            #[test]
            fn matched_type_is_a_list() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                let y = Var::new("y");
                h.run(vec![
                    pattern_variable_declaration(
                        list_pattern(
                            vec![
                                x.pattern().with_expect_inferred_type("int"),
                                rest_pattern(Some(
                                    y.pattern().with_expect_inferred_type("List<int>"),
                                )),
                            ],
                            None,
                        ),
                        expr("List<int>"),
                        false,
                    )
                    .check_ir(
                        "match(expr(List<int>), listPattern(varPattern(x, \
                     matchedType: int, staticType: int), ...(varPattern(y, \
                     matchedType: List<int>, staticType: List<int>)), \
                     matchedType: List<int>, requiredType: List<int>))",
                    ),
                ]);
            }

            #[test]
            fn matched_type_is_dynamic() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                let y = Var::new("y");
                h.run(vec![
                    pattern_variable_declaration(
                        list_pattern(
                            vec![
                                x.pattern().with_expect_inferred_type("dynamic"),
                                rest_pattern(Some(
                                    y.pattern().with_expect_inferred_type("List<dynamic>"),
                                )),
                            ],
                            None,
                        ),
                        expr("dynamic"),
                        false,
                    )
                    .check_ir(
                        "match(expr(dynamic), listPattern(varPattern(x, \
                     matchedType: dynamic, staticType: dynamic), ...(varPattern(y, \
                     matchedType: List<dynamic>, staticType: List<dynamic>)), \
                     matchedType: dynamic, requiredType: List<dynamic>))",
                    ),
                ]);
            }

            #[test]
            fn matched_type_is_error() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                let y = Var::new("y");
                h.run(vec![
                    pattern_variable_declaration(
                        list_pattern(
                            vec![
                                x.pattern().with_expect_inferred_type("error"),
                                rest_pattern(Some(
                                    y.pattern().with_expect_inferred_type("List<error>"),
                                )),
                            ],
                            None,
                        ),
                        expr("error"),
                        false,
                    )
                    .check_ir(
                        "match(expr(error), listPattern(varPattern(x, \
                     matchedType: error, staticType: error), ...(varPattern(y, \
                     matchedType: List<error>, staticType: List<error>)), \
                     matchedType: error, requiredType: List<error>))",
                    ),
                ]);
            }

            #[test]
            fn matched_type_is_other() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                let y = Var::new("y");
                h.run(vec![
                    if_case(
                        expr("Object"),
                        list_pattern(
                            vec![
                                x.pattern().with_expect_inferred_type("Object?"),
                                rest_pattern(Some(
                                    y.pattern().with_expect_inferred_type("List<Object?>"),
                                )),
                            ],
                            None,
                        ),
                        vec![],
                        None,
                    )
                    .check_ir(
                        "ifCase(expr(Object), listPattern(varPattern(x, \
                     matchedType: Object?, staticType: Object?), ...(varPattern(y, \
                     matchedType: List<Object?>, staticType: List<Object?>)), \
                     matchedType: Object, requiredType: List<Object?>), \
                     variables(x, y), true, block(), noop)",
                    ),
                ]);
            }

            mod rest_pattern {
                use super::*;

                #[test]
                fn with_pattern() {
                    let (_s, mut h) = set_up();
                    let x = Var::new("x");
                    h.run(vec![
                        pattern_variable_declaration(
                            list_pattern(vec![rest_pattern(Some(x.pattern()))], None),
                            expr("List<int>"),
                            false,
                        )
                        .check_ir(
                            "match(expr(List<int>), listPattern(...(varPattern(x, \
                         matchedType: List<int>, staticType: List<int>)), \
                         matchedType: List<int>, requiredType: List<int>))",
                        ),
                    ]);
                }

                #[test]
                fn without_pattern() {
                    let (_s, mut h) = set_up();
                    h.run(vec![
                        pattern_variable_declaration(
                            list_pattern(vec![rest_pattern(None)], None),
                            expr("List<int>"),
                            false,
                        )
                        .check_ir(
                            "match(expr(List<int>), listPattern(..., \
                         matchedType: List<int>, requiredType: List<int>))",
                        ),
                    ]);
                }
            }
        }

        mod refutability {
            use super::*;

            #[test]
            fn when_matched_type_is_a_subtype_of_pattern_type() {
                let (_s, mut h) = set_up();
                h.run(vec![
                    pattern_variable_declaration(
                        list_pattern(vec![wildcard()], Some("num")),
                        expr("List<int>"),
                        false,
                    )
                    .check_ir(
                        "match(expr(List<int>), listPattern(wildcardPattern\
                     (matchedType: num), matchedType: List<int>, \
                     requiredType: List<num>))",
                    ),
                ]);
            }

            #[test]
            fn when_matched_type_is_dynamic() {
                let (_s, mut h) = set_up();
                h.run(vec![
                    pattern_variable_declaration(
                        list_pattern(vec![wildcard()], Some("num")),
                        expr("dynamic"),
                        false,
                    )
                    .check_ir(
                        "match(expr(dynamic), listPattern(wildcardPattern(\
                     matchedType: num), matchedType: dynamic, \
                     requiredType: List<num>))",
                    ),
                ]);
            }

            #[test]
            fn when_matched_type_is_not_a_subtype_of_variable_type() {
                let (_s, mut h) = set_up();
                h.run_with(
                    vec![
                        pattern_variable_declaration(
                            list_pattern(vec![wildcard()], Some("num")).error_id("PATTERN"),
                            expr("String"),
                            false,
                        )
                        .error_id("CONTEXT"),
                    ],
                    errors(
                        &["patternTypeMismatchInIrrefutableContext(pattern: PATTERN, \
                         context: CONTEXT, matchedType: String, \
                         requiredType: List<num>)"],
                    ),
                );
            }

            #[test]
            fn sub_refutability() {
                let (_s, mut h) = set_up();
                h.run_with(
                    vec![
                        pattern_variable_declaration(
                            list_pattern(
                                vec![
                                    wildcard().with_declared_type("int").error_id("INT"),
                                    wildcard().with_declared_type("double").error_id("DOUBLE"),
                                ],
                                Some("num"),
                            ),
                            expr("List<num>"),
                            false,
                        )
                        .error_id("CONTEXT"),
                    ],
                    errors(&[
                        "patternTypeMismatchInIrrefutableContext(pattern: INT, \
                         context: CONTEXT, matchedType: num, requiredType: int)",
                        "patternTypeMismatchInIrrefutableContext(pattern: DOUBLE, \
                         context: CONTEXT, matchedType: num, requiredType: double)",
                    ]),
                );
            }
        }

        mod rest_pattern {
            use super::*;

            mod duplicate {
                use super::*;

                #[test]
                fn with_pattern() {
                    let (_s, mut h) = set_up();
                    let x = Var::new("x");
                    let y = Var::new("y");
                    h.run_with(
                        vec![
                            pattern_variable_declaration(
                                list_pattern(
                                    vec![
                                        rest_pattern(Some(x.pattern())).error_id("ORI"),
                                        rest_pattern(Some(y.pattern())).error_id("DUP"),
                                    ],
                                    None,
                                )
                                .error_id("LIST_PATTERN"),
                                expr("List<int>"),
                                false,
                            )
                            .check_ir(
                                "match(expr(List<int>), listPattern(...(varPattern(x, \
                             matchedType: List<int>, staticType: List<int>)), \
                             ...(varPattern(y, matchedType: List<int>, staticType: \
                             List<int>)), matchedType: List<int>, \
                             requiredType: List<int>))",
                            ),
                        ],
                        errors(&["duplicateRestPattern(mapOrListPattern: LIST_PATTERN, \
                             original: ORI, \
                             duplicate: DUP)"]),
                    );
                }

                #[test]
                fn without_pattern() {
                    let (_s, mut h) = set_up();
                    h.run_with(
                        vec![
                            pattern_variable_declaration(
                                list_pattern(
                                    vec![
                                        rest_pattern(None).error_id("ORI"),
                                        rest_pattern(None).error_id("DUP"),
                                    ],
                                    None,
                                )
                                .error_id("LIST_PATTERN"),
                                expr("List<int>"),
                                false,
                            )
                            .check_ir(
                                "match(expr(List<int>), listPattern(..., ..., \
                             matchedType: List<int>, requiredType: List<int>))",
                            ),
                        ],
                        errors(&["duplicateRestPattern(mapOrListPattern: LIST_PATTERN, \
                             original: ORI, \
                             duplicate: DUP)"]),
                    );
                }
            }

            #[test]
            fn first() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run(vec![
                    pattern_variable_declaration(
                        list_pattern(vec![rest_pattern(None), x.pattern()], None),
                        expr("List<int>"),
                        false,
                    )
                    .check_ir(
                        "match(expr(List<int>), listPattern(..., \
                     varPattern(x, matchedType: int, staticType: int), \
                     matchedType: List<int>, requiredType: List<int>))",
                    ),
                ]);
            }

            #[test]
            fn last() {
                let (_s, mut h) = set_up();
                let x = Var::new("x");
                h.run(vec![
                    pattern_variable_declaration(
                        list_pattern(vec![x.pattern(), rest_pattern(None)], None),
                        expr("List<int>"),
                        false,
                    )
                    .check_ir(
                        "match(expr(List<int>), listPattern(varPattern(x, \
                     matchedType: int, staticType: int), ..., \
                     matchedType: List<int>, requiredType: List<int>))",
                    ),
                ]);
            }
        }

        #[test]
        fn match_var_overlap() {
            let (_s, mut h) = set_up();
            let x1 = Var::new("x").with_identity("x1").error_id("x1");
            let x2 = Var::new("x").with_identity("x2").error_id("x2");
            h.run_with(
                vec![pattern_variable_declaration(
                    list_pattern(vec![x1.pattern(), x2.pattern()], None),
                    expr("List<int>"),
                    false,
                )],
                errors(&["duplicateVariablePattern(name: x, original: x1, duplicate: x2)"]),
            );
        }
    }

    mod logical_and {
        use super::*;

        #[test]
        fn type_schema() {
            let (_s, mut h) = set_up();
            h.run_with(
                vec![
                    pattern_variable_declaration(
                        wildcard()
                            .with_declared_type("int?")
                            .error_id("WILDCARD1")
                            .and(
                                wildcard()
                                    .with_declared_type("double?")
                                    .error_id("WILDCARD2"),
                            ),
                        null_literal().check_schema("Null"),
                        false,
                    )
                    .check_ir(
                        "match(null, logicalAndPattern(wildcardPattern(\
                     matchedType: Null), wildcardPattern(matchedType: Null), \
                     matchedType: Null))",
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
        fn refutability() {
            let (_s, mut h) = set_up();
            h.run_with(
                vec![
                    pattern_variable_declaration(
                        wildcard()
                            .with_declared_type("int")
                            .error_id("LHS")
                            .and(wildcard().with_declared_type("double").error_id("RHS")),
                        expr("num"),
                        false,
                    )
                    .error_id("CONTEXT"),
                ],
                errors(&[
                    "patternTypeMismatchInIrrefutableContext(pattern: LHS, \
                     context: CONTEXT, matchedType: num, requiredType: int)",
                    "patternTypeMismatchInIrrefutableContext(pattern: RHS, \
                     context: CONTEXT, matchedType: int, requiredType: double)",
                ]),
            );
        }

        #[test]
        fn duplicate_variable_pattern() {
            let (_s, mut h) = set_up();
            let x1 = Var::new("x").with_identity("x1").error_id("x1");
            let x2 = Var::new("x").with_identity("x2").error_id("x2");
            h.run_with(
                vec![pattern_variable_declaration(
                    x1.pattern().and(x2.pattern()),
                    expr("int"),
                    false,
                )],
                errors(&["duplicateVariablePattern(name: x, original: x1, duplicate: x2)"]),
            );
        }
    }

    mod logical_or {
        use super::*;

        #[test]
        fn type_schema() {
            let (_s, mut h) = set_up();
            h.run_with(
                vec![
                    pattern_variable_declaration(
                        wildcard()
                            .with_declared_type("int?")
                            .or(wildcard().with_declared_type("double?"))
                            .error_id("PATTERN"),
                        null_literal().check_schema("_"),
                        false,
                    )
                    .error_id("CONTEXT"),
                ],
                errors(&["refutablePatternInIrrefutableContext(pattern: PATTERN, \
                     context: CONTEXT)"]),
            );
        }

        #[test]
        fn refutability() {
            // Note: even though the logical-or contains refutable sub-patterns, we
            // don't issue errors for them because they would overlap with the error
            // we're issuing for the logical-or pattern as a whole.
            let (_s, mut h) = set_up();
            h.run_with(
                vec![
                    pattern_variable_declaration(
                        wildcard()
                            .with_declared_type("int")
                            .or(wildcard().with_declared_type("double"))
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

        mod variables {
            use super::*;

            mod should_have_same_types {
                use super::*;

                mod same {
                    use super::*;

                    #[test]
                    fn explicit_explicit() {
                        let (_s, mut h) = set_up();
                        let x1 = Var::new("x").with_identity("x1");
                        let x2 = Var::new("x").with_identity("x2");
                        Var::join("x", vec![x1, x2]);
                        h.run(vec![
                            if_case(
                                expr("Object"),
                                x1.pattern()
                                    .with_declared_type("int")
                                    .or(x2.pattern().with_declared_type("int")),
                                vec![],
                                None,
                            )
                            .check_ir(
                                "ifCase(expr(Object), logicalOrPattern(varPattern(x, \
                             matchedType: Object, staticType: int), varPattern(x, \
                             matchedType: Object, staticType: int), \
                             matchedType: Object), variables(int x = [x1, x2]), \
                             true, block(), noop)",
                            ),
                        ]);
                    }

                    #[test]
                    fn explicit_explicit_normalized() {
                        let (_s, mut h) = set_up();
                        let x1 = Var::new("x").with_identity("x1");
                        let x2 = Var::new("x").with_identity("x2");
                        Var::join("x", vec![x1, x2]);
                        h.run(vec![
                            if_case(
                                expr("Object"),
                                x1.pattern()
                                    .with_declared_type("Object")
                                    .or(x2.pattern().with_declared_type("FutureOr<Object>")),
                                vec![],
                                None,
                            )
                            .check_ir(
                                "ifCase(expr(Object), logicalOrPattern(varPattern(x, \
                             matchedType: Object, staticType: Object), varPattern(x, \
                             matchedType: Object, staticType: FutureOr<Object>), \
                             matchedType: Object), variables(Object x = [x1, x2]), \
                             true, block(), noop)",
                            ),
                        ]);
                    }

                    #[test]
                    fn explicit_implicit() {
                        let (_s, mut h) = set_up();
                        let x1 = Var::new("x").with_identity("x1");
                        let x2 = Var::new("x").with_identity("x2");
                        Var::join("x", vec![x1, x2]);
                        h.run(vec![
                            if_case(
                                expr("int"),
                                x1.pattern().with_declared_type("int").or(x2.pattern()),
                                vec![],
                                None,
                            )
                            .check_ir(
                                "ifCase(expr(int), logicalOrPattern(varPattern(x, \
                             matchedType: int, staticType: int), varPattern(x, \
                             matchedType: int, staticType: int), matchedType: int), \
                             variables(int x = [x1, x2]), true, block(), noop)",
                            ),
                        ]);
                    }

                    #[test]
                    fn implicit_explicit() {
                        let (_s, mut h) = set_up();
                        let x1 = Var::new("x").with_identity("x1");
                        let x2 = Var::new("x").with_identity("x2");
                        Var::join("x", vec![x1, x2]);
                        h.run(vec![
                            if_case(
                                expr("int"),
                                x1.pattern().or(x2.pattern().with_declared_type("int")),
                                vec![],
                                None,
                            )
                            .check_ir(
                                "ifCase(expr(int), logicalOrPattern(varPattern(x, \
                             matchedType: int, staticType: int), varPattern(x, \
                             matchedType: int, staticType: int), matchedType: int), \
                             variables(int x = [x1, x2]), true, block(), noop)",
                            ),
                        ]);
                    }

                    #[test]
                    fn implicit_implicit() {
                        let (_s, mut h) = set_up();
                        let x1 = Var::new("x").with_identity("x1");
                        let x2 = Var::new("x").with_identity("x2");
                        Var::join("x", vec![x1, x2]);
                        h.run(vec![
                            if_case(expr("int"), x1.pattern().or(x2.pattern()), vec![], None)
                                .check_ir(
                                    "ifCase(expr(int), logicalOrPattern(varPattern(x, \
                             matchedType: int, staticType: int), varPattern(x, \
                             matchedType: int, staticType: int), matchedType: int), \
                             variables(int x = [x1, x2]), true, block(), noop)",
                                ),
                        ]);
                    }
                }

                mod not_same {
                    use super::*;

                    #[test]
                    fn explicit_explicit() {
                        let (_s, mut h) = set_up();
                        let x1 = Var::new("x").with_identity("x1");
                        let x2 = Var::new("x").with_identity("x2").error_id("x2");
                        Var::join("x", vec![x1, x2]);
                        h.run_with(
                            vec![
                                if_case(
                                    expr("Object"),
                                    x1.pattern()
                                        .with_declared_type("int")
                                        .or(x2.pattern().with_declared_type("num")),
                                    vec![],
                                    None,
                                )
                                .check_ir(
                                    "ifCase(expr(Object), logicalOrPattern(varPattern(x, \
                                 matchedType: Object, staticType: int), varPattern(x, \
                                 matchedType: Object, staticType: num), matchedType: \
                                 Object), variables(notConsistent:differentFinalityOrType \
                                 error x = [x1, x2]), true, block(), noop)",
                                ),
                            ],
                            errors(
                                &["inconsistentJoinedPatternVariable(variable: x = [x1, x2], \
                                 component: x2)"],
                            ),
                        );
                    }

                    #[test]
                    fn explicit_implicit() {
                        let (_s, mut h) = set_up();
                        let x1 = Var::new("x").with_identity("x1");
                        let x2 = Var::new("x").with_identity("x2").error_id("x2");
                        Var::join("x", vec![x1, x2]);
                        h.run_with(
                            vec![
                                if_case(
                                    expr("num"),
                                    x1.pattern().with_declared_type("int").or(x2.pattern()),
                                    vec![],
                                    None,
                                )
                                .check_ir(
                                    "ifCase(expr(num), logicalOrPattern(varPattern(x, \
                                 matchedType: num, staticType: int), varPattern(x, \
                                 matchedType: num, staticType: num), matchedType: num), \
                                 variables(notConsistent:differentFinalityOrType error x = \
                                 [x1, x2]), true, block(), noop)",
                                ),
                            ],
                            errors(
                                &["inconsistentJoinedPatternVariable(variable: x = [x1, x2], \
                                 component: x2)"],
                            ),
                        );
                    }
                }
            }

            #[test]
            fn should_have_same_finality() {
                let (_s, mut h) = set_up();
                let x1 = Var::new("x").with_final(true).with_identity("x1");
                let x2 = Var::new("x").with_identity("x2").error_id("x2");
                Var::join("x", vec![x1, x2]);
                h.run_with(
                    vec![
                        if_case(expr("int"), x1.pattern().or(x2.pattern()), vec![], None).check_ir(
                            "ifCase(expr(int), logicalOrPattern(varPattern(x, \
                         matchedType: int, staticType: int), varPattern(x, \
                         matchedType: int, staticType: int), matchedType: int), \
                         variables(notConsistent:differentFinalityOrType int x = \
                         [x1, x2]), true, block(), noop)",
                        ),
                    ],
                    errors(
                        &["inconsistentJoinedPatternVariable(variable: x = [x1, x2], \
                         component: x2)"],
                    ),
                );
            }

            mod should_be_present_in_both_branches {
                use super::*;

                #[test]
                fn both_have() {
                    let (_s, mut h) = set_up();
                    let x1 = Var::new("x").with_identity("x1");
                    let x2 = Var::new("x").with_identity("x2");
                    Var::join("x", vec![x1, x2]);
                    h.run(vec![
                        if_case(expr("int"), x1.pattern().or(x2.pattern()), vec![], None).check_ir(
                            "ifCase(expr(int), logicalOrPattern(varPattern(x, \
                             matchedType: int, staticType: int), varPattern(x, \
                             matchedType: int, staticType: int), matchedType: int), \
                             variables(int x = [x1, x2]), true, block(), noop)",
                        ),
                    ]);
                }

                #[test]
                fn left_has() {
                    let (_s, mut h) = set_up();
                    let x1 = Var::new("x").with_identity("x1").error_id("x1");
                    Var::join("x", vec![x1]);
                    h.run_with(
                        vec![
                            if_case(
                                expr("int"),
                                x1.pattern().or(wildcard()).error_id("PATTERN"),
                                vec![],
                                None,
                            )
                            .check_ir(
                                "ifCase(expr(int), logicalOrPattern(varPattern(x, \
                             matchedType: int, staticType: int), wildcardPattern(\
                             matchedType: int), matchedType: int), variables(\
                             notConsistent:logicalOr int x = [x1]), true, block(), \
                             noop)",
                            ),
                        ],
                        errors(&["logicalOrPatternBranchMissingVariable(node: PATTERN, \
                             hasInLeft: true, name: x, variable: x1)"]),
                    );
                }

                #[test]
                fn right_has() {
                    let (_s, mut h) = set_up();
                    let x1 = Var::new("x").with_identity("x1").error_id("x1");
                    Var::join("x", vec![x1]);
                    h.run_with(
                        vec![
                            if_case(
                                expr("int"),
                                wildcard().or(x1.pattern()).error_id("PATTERN"),
                                vec![],
                                None,
                            )
                            .check_ir(
                                "ifCase(expr(int), logicalOrPattern(wildcardPattern(\
                             matchedType: int), varPattern(x, matchedType: int, \
                             staticType: int), matchedType: int), variables(\
                             notConsistent:logicalOr int x = [x1]), true, block(), \
                             noop)",
                            ),
                        ],
                        errors(&["logicalOrPatternBranchMissingVariable(node: PATTERN, \
                             hasInLeft: false, name: x, variable: x1)"]),
                    );
                }
            }
        }
    }
}
