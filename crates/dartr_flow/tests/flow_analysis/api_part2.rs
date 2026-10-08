// Dart source: pkg/_fe_analyzer_shared/test/flow_analysis/flow_analysis_test.dart
// (lines 1005-2299: group 'API', from 'functionExpression_begin() cancels
// promotions of non-final vars' to 'suspension: yield: does not demote
// variables declared in the current function')

//! Dart group `API` (second part).

use super::common::*;

/// Dart `declare(x, type: type, initializer: expr(type))`.
#[track_caller]
fn declare_init(x: Var, type_: &str) -> Node {
    declare(x)
        .with_declared_type(type_)
        .with_initializer(expr(type_))
}

/// Dart `declare(x, type: type)`.
#[track_caller]
fn declare_typed(x: Var, type_: &str) -> Node {
    declare(x).with_declared_type(type_)
}

/// Dart `expect(nodes[x], same(expected))`.
#[track_caller]
fn expect_same(nodes: &SsaNodeHarness, x: Var, expected: &SsaNode) {
    assert!(
        same_ssa(nodes.get(x).as_ref(), Some(expected)),
        "expected the same SSA node"
    );
}

#[test]
fn function_expression_begin_cancels_promotions_of_non_final_vars() {
    // num x;
    // if (<bool>) {
    //   x = <int>;
    // } else {
    //   x = <double>;
    // }
    // if (x is int) {
    //   () => x is not promoted
    // } else {
    //   () => x is not promoted
    // }
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_typed(x, "num"),
        if_else(
            expr("bool"),
            vec![x.write(expr("int"))],
            vec![x.write(expr("double"))],
        ),
        if_else(
            x.is_("int"),
            vec![local_function(vec![check_not_promoted(x)])],
            vec![local_function(vec![check_not_promoted(x)])],
        ),
    ]);
}

#[test]
fn function_expression_begin_preserves_promotions_of_final_variables() {
    // final num x;
    // if (<bool>) {
    //   x = <int>;
    // } else {
    //   x = <double>;
    // }
    // if (x is int) {
    //   () => x is promoted to int
    // } else {
    //   () => x is not promoted
    // }
    let mut h = set_up();
    let x = Var::new("x").with_final(true);
    h.run(vec![
        declare_typed(x, "num"),
        if_else(
            expr("bool"),
            vec![x.write(expr("int"))],
            vec![x.write(expr("double"))],
        ),
        if_else(
            x.is_("int"),
            vec![local_function(vec![check_promoted(x, "int")])],
            vec![local_function(vec![check_not_promoted(x)])],
        ),
    ]);
}

#[test]
fn function_expression_begin_preserves_promotions_of_initialized_vars() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?").with_late(),
        x.as_("int"),
        y.as_("int"),
        check_promoted(x, "int"),
        check_promoted(y, "int"),
        local_function(vec![
            // x and y remain promoted within the local function, because the
            // assignment that happens implicitly as part of the initialization
            // definitely happens before anything else, and hence the promotions
            // are still valid whenever the local function executes.
            check_promoted(x, "int"),
            check_promoted(y, "int"),
        ]),
        // x and y remain promoted after the local function too.
        check_promoted(x, "int"),
        check_promoted(y, "int"),
    ]);
}

#[test]
fn function_expression_begin_handles_not_yet_seen_variables() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        local_function(vec![]),
        // x is declared after the local function, so the local function
        // cannot possibly write to x.
        declare_init(x, "int?"),
        x.as_("int"),
        check_promoted(x, "int"),
        x.write(expr("Null")),
    ]);
}

#[test]
fn function_expression_begin_handles_not_yet_seen_write_captured_vars() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        y.as_("int"),
        get_ssa_nodes(move |nodes| assert!(nodes.get(x).is_some())),
        local_function(vec![
            get_ssa_nodes(move |nodes| {
                assert!(!same_ssa(nodes.get(x).as_ref(), nodes.get(y).as_ref()))
            }),
            x.as_("int"),
            // Promotion should not occur, because x might be write-captured by
            // the time this code is reached.
            check_not_promoted(x),
        ]),
        local_function(vec![x.write(expr("Null"))]),
    ]);
}

#[test]
fn function_expression_end_does_not_propagate_definitely_unassigned_data() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_typed(x, "int"),
        check_unassigned(x, true),
        local_function(vec![
            // The function expression could be called at any time, so x might
            // be assigned now.
            check_unassigned(x, false),
        ]),
        // But now that we are back outside the function expression, we once
        // again know that x is unassigned.
        check_unassigned(x, true),
        x.write(expr("int")),
        check_unassigned(x, false),
    ]);
}

#[test]
fn handle_break_handles_deep_nesting() {
    let mut h = set_up();
    h.run(vec![
        while_(
            boolean_literal(true),
            vec![
                if_(expr("bool"), vec![if_(expr("bool"), vec![break_(None)])]),
                return_(),
                check_reachable(false),
            ],
        ),
        check_reachable(true),
    ]);
}

#[test]
fn handle_break_handles_mixed_nesting() {
    let mut h = set_up();
    h.run(vec![
        while_(
            boolean_literal(true),
            vec![
                if_(
                    expr("bool"),
                    vec![if_(expr("bool"), vec![break_(None)]), break_(None)],
                ),
                break_(None),
                check_reachable(false),
            ],
        ),
        check_reachable(true),
    ]);
}

#[test]
fn handle_break_handles_null_target() {
    let mut h = set_up();
    h.run(vec![
        while_(
            boolean_literal(true),
            vec![
                check_reachable(true),
                break_(Some(Label::unbound())),
                check_reachable(false),
            ],
        ),
        check_reachable(false),
    ]);
}

#[test]
fn handle_continue_handles_deep_nesting() {
    let mut h = set_up();
    h.run(vec![
        do_(
            vec![
                if_(expr("bool"), vec![if_(expr("bool"), vec![continue_(None)])]),
                return_(),
                check_reachable(false),
            ],
            second(check_reachable(true), expr("bool")).or(boolean_literal(true)),
        ),
        check_reachable(false),
    ]);
}

#[test]
fn handle_continue_handles_mixed_nesting() {
    let mut h = set_up();
    h.run(vec![
        do_(
            vec![
                if_(
                    expr("bool"),
                    vec![if_(expr("bool"), vec![continue_(None)]), continue_(None)],
                ),
                continue_(None),
                check_reachable(false),
            ],
            second(check_reachable(true), expr("bool")).or(boolean_literal(true)),
        ),
        check_reachable(false),
    ]);
}

#[test]
fn handle_continue_handles_null_target() {
    let mut h = set_up();
    h.run(vec![
        for_(
            None,
            boolean_literal(true),
            second(check_reachable(false), expr("Object?")),
            vec![
                check_reachable(true),
                continue_(Some(Label::unbound())),
                check_reachable(false),
            ],
            false,
        ),
        check_reachable(false),
    ]);
}

#[test]
fn if_null_expression_allows_ensure_guarding() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        x.if_null(second(
            list_literal(
                vec![
                    check_reachable(true),
                    x.write(expr("int")),
                    check_promoted(x, "int"),
                ],
                "dynamic",
            ),
            expr("int?"),
        ))
        .then_stmt(block(vec![check_reachable(true), check_promoted(x, "int")])),
    ]);
}

#[test]
fn if_null_expression_allows_promotion_of_tested_var() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        x.if_null(second(
            list_literal(
                vec![
                    check_reachable(true),
                    x.as_("int"),
                    check_promoted(x, "int"),
                ],
                "dynamic",
            ),
            expr("int?"),
        ))
        .then_stmt(block(vec![check_reachable(true), check_promoted(x, "int")])),
    ]);
}

#[test]
fn if_null_expression_discards_promotions_unrelated_to_tested_expr() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        expr("int?")
            .if_null(second(
                list_literal(
                    vec![
                        check_reachable(true),
                        x.as_("int"),
                        check_promoted(x, "int"),
                    ],
                    "dynamic",
                ),
                expr("int?"),
            ))
            .then_stmt(block(vec![check_reachable(true), check_not_promoted(x)])),
    ]);
}

#[test]
fn if_null_expression_does_not_detect_when_rhs_is_unreachable() {
    // Note: sound flow analysis changes this behavior.
    let mut h = set_up();
    h.disable_sound_flow_analysis();
    h.run(vec![
        expr("int")
            .if_null(second(check_reachable(true), expr("int")))
            .then_stmt(check_reachable(true)),
    ]);
}

#[test]
fn if_null_expression_determines_reachability_correctly_for_null_type() {
    let mut h = set_up();
    h.run(vec![
        expr("Null")
            .if_null(second(check_reachable(true), expr("Null")))
            .then_stmt(check_reachable(true)),
    ]);
}

#[test]
fn if_null_expression_sets_shortcut_reachability_correctly_for_null_type() {
    let mut h = set_up();
    h.run(vec![
        expr("Null")
            .if_null(second(check_reachable(true), throw_(expr("Object"))))
            .then_stmt(check_reachable(false)),
    ]);
}

#[test]
fn if_null_expression_sets_shortcut_reachability_correctly_for_non_null_type() {
    // Note: sound flow analysis changes this behavior.
    let mut h = set_up();
    h.disable_sound_flow_analysis();
    h.run(vec![
        expr("Object")
            .if_null(second(check_reachable(true), throw_(expr("Object"))))
            .then_stmt(check_reachable(true)),
    ]);
}

#[test]
fn if_statement_with_early_exit_promotes_in_unreachable_code() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        return_(),
        check_reachable(false),
        if_(x.eq(null_literal()), vec![return_()]),
        check_reachable(false),
        check_promoted(x, "int"),
    ]);
}

#[test]
fn if_statement_end_false_keeps_else_branch_if_then_branch_exits() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        if_(x.eq(null_literal()), vec![return_()]),
        check_promoted(x, "int"),
    ]);
}

#[test]
fn if_statement_end_discards_non_matching_expression_info_from_joined_branches() {
    let mut h = set_up();
    let w = Var::new("w");
    let x = Var::new("x");
    let y = Var::new("y");
    let z = Var::new("z");
    let x_ssa_node_before_if = Late::<SsaNode>::new();
    h.run(vec![
        declare_init(w, "Object"),
        declare_init(x, "bool"),
        declare_init(y, "bool"),
        declare_init(z, "bool"),
        x.write(w.is_("int")),
        get_ssa_nodes(move |nodes| {
            x_ssa_node_before_if.set(nodes.get(x).unwrap());
            assert!(
                x_ssa_node_before_if
                    .get()
                    .condition_variable_state
                    .is_some()
            );
        }),
        if_else(
            expr("bool"),
            vec![y.write(w.is_("String"))],
            vec![z.write(w.is_("bool"))],
        ),
        get_ssa_nodes(move |nodes| {
            expect_same(nodes, x, &x_ssa_node_before_if.get());
            assert!(nodes.get(y).unwrap().condition_variable_state.is_none());
            assert!(nodes.get(z).unwrap().condition_variable_state.is_none());
        }),
    ]);
}

#[test]
fn if_statement_end_ignores_non_matching_ssa_info_from_then_path_if_unreachable() {
    let mut h = set_up();
    let x = Var::new("x");
    let x_ssa_node_before_if = Late::<SsaNode>::new();
    h.run(vec![
        declare_init(x, "Object"),
        get_ssa_nodes(move |nodes| {
            x_ssa_node_before_if.set(nodes.get(x).unwrap());
        }),
        if_(expr("bool"), vec![x.write(expr("Object")), return_()]),
        get_ssa_nodes(move |nodes| {
            expect_same(nodes, x, &x_ssa_node_before_if.get());
        }),
    ]);
}

#[test]
fn if_statement_end_ignores_non_matching_ssa_info_from_else_path_if_unreachable() {
    let mut h = set_up();
    let x = Var::new("x");
    let x_ssa_node_before_if = Late::<SsaNode>::new();
    h.run(vec![
        declare_init(x, "Object"),
        get_ssa_nodes(move |nodes| {
            x_ssa_node_before_if.set(nodes.get(x).unwrap());
        }),
        if_else(
            expr("bool"),
            vec![],
            vec![x.write(expr("Object")), return_()],
        ),
        get_ssa_nodes(move |nodes| {
            expect_same(nodes, x, &x_ssa_node_before_if.get());
        }),
    ]);
}

#[test]
fn initialize_promotes_when_not_final() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare(x)
            .with_declared_type("int?")
            .with_initializer(expr("int")),
        check_promoted(x, "int"),
    ]);
}

#[test]
fn initialize_does_not_promote_when_final() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare(x)
            .with_final()
            .with_declared_type("int?")
            .with_initializer(expr("int")),
        check_not_promoted(x),
    ]);
}

mod initialize_promotes_implicitly_typed_vars_to_type_parameter_types {
    use super::*;

    #[test]
    fn when_not_final() {
        let mut h = set_up();
        TypeRegistry::add_type_parameter("T");
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("T&int")),
            check_promoted(x, "T&int"),
        ]);
    }

    #[test]
    fn when_final() {
        let mut h = set_up();
        TypeRegistry::add_type_parameter("T");
        let x = Var::new("x");
        h.run(vec![
            declare(x)
                .with_final()
                .with_initializer(expr("T&int"))
                .with_expect_inferred_type("T"),
            check_promoted(x, "T&int"),
        ]);
    }
}

// Dart group: "initialize() doesn't promote explicitly typed vars to type
// parameter types".
mod initialize_doesnt_promote_explicitly_typed_vars_to_type_parameter_types {
    use super::*;

    #[test]
    fn when_not_final() {
        let mut h = set_up();
        let x = Var::new("x");
        TypeRegistry::add_type_parameter("T");
        h.run(vec![
            declare(x)
                .with_declared_type("T")
                .with_initializer(expr("T&int")),
            check_not_promoted(x),
        ]);
    }

    #[test]
    fn when_final() {
        let mut h = set_up();
        let x = Var::new("x");
        TypeRegistry::add_type_parameter("T");
        h.run(vec![
            declare(x)
                .with_final()
                .with_declared_type("T")
                .with_initializer(expr("T&int")),
            check_not_promoted(x),
        ]);
    }
}

// Dart group: "initialize() doesn't promote implicitly typed vars to
// ordinary types".
mod initialize_doesnt_promote_implicitly_typed_vars_to_ordinary_types {
    use super::*;

    #[test]
    fn when_not_final() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x)
                .with_initializer(expr("Null"))
                .with_expect_inferred_type("dynamic"),
            check_not_promoted(x),
        ]);
    }

    #[test]
    fn when_final() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare(x)
                .with_final()
                .with_initializer(expr("Null"))
                .with_expect_inferred_type("dynamic"),
            check_not_promoted(x),
        ]);
    }
}

#[test]
fn initialize_stores_expression_info_when_not_late() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    h.run(vec![
        declare_init(y, "int?"),
        declare(x)
            .with_declared_type("Object")
            .with_initializer(y.eq(null_literal())),
        get_ssa_nodes(move |nodes| {
            let info = nodes
                .get(x)
                .unwrap()
                .condition_variable_state
                .clone()
                .unwrap();
            // Dart reads the promotion info through `h`; the harness is
            // borrowed by `h.run` here, so the flow analysis being run (a
            // `FlowModelHelper` too) is used instead.
            let key = nodes.key_for_variable(y);
            assert!(
                promotion_info_get(&info.if_true.promotion_info, nodes.flow(), key)
                    .unwrap()
                    .promoted_types
                    .is_empty()
            );
            let promoted_types =
                promotion_info_get(&info.if_false.promotion_info, nodes.flow(), key)
                    .unwrap()
                    .promoted_types
                    .clone();
            assert_eq!(promoted_types.len(), 1);
            assert_eq!(promoted_types[0].unwrap_type_view().type_string(), "int");
        }),
    ]);
}

#[test]
fn initialize_does_not_store_expression_info_when_late() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    h.run(vec![
        declare_init(y, "int?"),
        declare(x)
            .with_late()
            .with_declared_type("Object")
            .with_initializer(y.eq(null_literal())),
        get_ssa_nodes(move |nodes| {
            assert!(nodes.get(x).unwrap().condition_variable_state.is_none());
        }),
    ]);
}

#[test]
fn initialize_does_not_store_expression_info_for_implicitly_typed_vars_pre_bug_fix() {
    let mut h = set_up();
    h.disable_respect_implicitly_typed_var_initializers();
    let x = Var::new("x");
    let y = Var::new("y");
    h.run(vec![
        declare_init(y, "int?"),
        declare(x)
            .with_initializer(y.eq(null_literal()))
            .with_expect_inferred_type("bool"),
        get_ssa_nodes(move |nodes| {
            assert!(nodes.get(x).unwrap().condition_variable_state.is_none());
        }),
    ]);
}

#[test]
fn initialize_stores_expression_info_for_implicitly_typed_vars_post_bug_fix() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    h.run(vec![
        declare_init(y, "int?"),
        declare(x)
            .with_initializer(y.eq(null_literal()))
            .with_expect_inferred_type("bool"),
        get_ssa_nodes(move |nodes| {
            assert!(nodes.get(x).unwrap().condition_variable_state.is_some());
        }),
    ]);
}

#[test]
fn initialize_stores_expression_info_for_explicitly_typed_vars_pre_bug_fix() {
    let mut h = set_up();
    h.disable_respect_implicitly_typed_var_initializers();
    let x = Var::new("x");
    let y = Var::new("y");
    h.run(vec![
        declare_init(y, "int?"),
        declare(x)
            .with_declared_type("Object")
            .with_initializer(y.eq(null_literal())),
        get_ssa_nodes(move |nodes| {
            assert!(nodes.get(x).unwrap().condition_variable_state.is_some());
        }),
    ]);
}

#[test]
fn initialize_does_not_store_expression_info_for_trivial_expressions() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    h.run(vec![
        declare_init(y, "int?"),
        local_function(vec![y.write(expr("int?"))]),
        declare(x)
            .with_declared_type("Object")
            // `y == null` is a trivial expression because y has been write
            // captured.
            .with_initializer(
                y.eq(null_literal())
                    .get_expression_info(|info| assert!(info.is_some())),
            ),
        get_ssa_nodes(move |nodes| {
            assert!(nodes.get(x).unwrap().condition_variable_state.is_none());
        }),
    ]);
}

/// Dart `_checkIs(declaredType, tryPromoteType, expectedPromotedTypeThen,
/// expectedPromotedTypeElse, {inverted, expectedReachableThen,
/// expectedReachableElse})`.
#[allow(clippy::too_many_arguments)]
fn check_is(
    h: &mut FlowAnalysisTestHarness,
    declared_type: &str,
    try_promote_type: &str,
    expected_promoted_type_then: Option<&str>,
    expected_promoted_type_else: Option<&str>,
    inverted: bool,
    expected_reachable_then: bool,
    expected_reachable_else: bool,
) {
    let x = Var::new("x");
    let ssa_before_promotion = Late::<SsaNode>::new();
    h.run(vec![
        declare_init(x, declared_type),
        get_ssa_nodes(move |nodes| ssa_before_promotion.set(nodes.get(x).unwrap())),
        if_else(
            if inverted {
                x.is_not(try_promote_type)
            } else {
                x.is_(try_promote_type)
            },
            vec![
                check_reachable(expected_reachable_then),
                check_promoted(x, expected_promoted_type_then),
                get_ssa_nodes(move |nodes| expect_same(nodes, x, &ssa_before_promotion.get())),
            ],
            vec![
                check_reachable(expected_reachable_else),
                check_promoted(x, expected_promoted_type_else),
                get_ssa_nodes(move |nodes| expect_same(nodes, x, &ssa_before_promotion.get())),
            ],
        ),
    ]);
}

#[test]
fn is_expression_end_promotes_to_a_subtype() {
    let mut h = set_up();
    check_is(
        &mut h,
        "int?",
        "int",
        Some("int"),
        Some("Never?"),
        false,
        true,
        true,
    );
}

#[test]
fn is_expression_end_promotes_to_a_subtype_inverted() {
    let mut h = set_up();
    check_is(
        &mut h,
        "int?",
        "int",
        Some("Never?"),
        Some("int"),
        true,
        true,
        true,
    );
}

#[test]
fn is_expression_end_does_not_promote_to_a_supertype() {
    let mut h = set_up();
    check_is(&mut h, "int", "int?", None, None, false, true, false);
}

#[test]
fn is_expression_end_does_not_promote_to_a_supertype_inverted() {
    let mut h = set_up();
    check_is(&mut h, "int", "int?", None, None, true, false, true);
}

#[test]
fn is_expression_end_does_not_promote_to_an_unrelated_type() {
    let mut h = set_up();
    check_is(&mut h, "int", "String", None, None, false, true, true);
}

#[test]
fn is_expression_end_does_not_promote_to_an_unrelated_type_inverted() {
    let mut h = set_up();
    check_is(&mut h, "int", "String", None, None, true, true, true);
}

#[test]
fn is_expression_end_does_not_promote_write_captured_vars() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        if_(x.is_("int"), vec![check_promoted(x, "int")]),
        local_function(vec![x.write(expr("int?"))]),
        if_(x.is_("int"), vec![check_not_promoted(x)]),
    ]);
}

#[test]
fn is_expression_end_sets_reachability_for_this() {
    let mut h = set_up();
    h.set_this_type("C");
    h.run(vec![if_else(
        this_().is_("Never"),
        vec![check_reachable(false)],
        vec![check_reachable(true)],
    )]);
}

mod is_expression_end_sets_reachability_for_property_gets {
    use super::*;

    #[test]
    fn on_a_variable() {
        let mut h = set_up();
        h.add_member("C", "f", Some("Object?"), false, None);
        let x = Var::new("x");
        h.run(vec![
            declare_init(x, "C"),
            if_else(
                x.property("f", false).is_("Never"),
                vec![check_reachable(false)],
                vec![check_reachable(true)],
            ),
        ]);
    }

    #[test]
    fn is_expression_end_variables_in_assignment_expressions_are_promoted() {
        // num x;
        // if ((x = <int>) is int) {
        //   x is promoted to `int`.
        // }
        // x is not promoted
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare_typed(x, "num"),
            if_(
                x.write(expr("int")).is_("int"),
                vec![check_promoted(x, "int")],
            ),
            check_not_promoted(x),
        ]);
    }

    #[test]
    fn is_expression_end_variables_in_assignment_expressions_are_not_promoted_when_inference_update_4_is_disabled()
     {
        let mut h = set_up();
        let x = Var::new("x");
        h.disable_inference_update4();
        h.run(vec![
            declare_typed(x, "num"),
            if_(x.write(expr("int")).is_("int"), vec![check_not_promoted(x)]),
            check_not_promoted(x),
        ]);
    }

    #[test]
    fn on_an_arbitrary_expression() {
        let mut h = set_up();
        h.add_member("C", "f", Some("Object?"), false, None);
        h.run(vec![if_else(
            expr("C").property("f", false).is_("Never"),
            vec![check_reachable(false)],
            vec![check_reachable(true)],
        )]);
    }

    #[test]
    fn on_explicit_this() {
        let mut h = set_up();
        h.set_this_type("C");
        h.add_member("C", "f", Some("Object?"), false, None);
        h.run(vec![if_else(
            this_().property("f", false).is_("Never"),
            vec![check_reachable(false)],
            vec![check_reachable(true)],
        )]);
    }

    #[test]
    fn on_implicit_this_super() {
        let mut h = set_up();
        h.set_this_type("C");
        h.add_member("C", "f", Some("Object?"), false, None);
        h.run(vec![if_else(
            this_property("f").is_("Never"),
            vec![check_reachable(false)],
            vec![check_reachable(true)],
        )]);
    }
}

#[test]
fn is_expression_end_sets_reachability_for_arbitrary_exprs() {
    let mut h = set_up();
    h.run(vec![if_else(
        expr("int").is_("Never"),
        vec![check_reachable(false)],
        vec![check_reachable(true)],
    )]);
}

#[test]
fn labeled_block_without_break() {
    let mut h = set_up();
    let x = Var::new("x");
    let l = Label::new("l");
    h.run(vec![
        declare_init(x, "int?"),
        if_(x.is_not("int"), vec![l.then_stmt(return_())]),
        check_promoted(x, "int"),
    ]);
}

#[test]
fn labeled_block_with_break_joins() {
    let mut h = set_up();
    let x = Var::new("x");
    let l = Label::new("l");
    h.run(vec![
        declare_init(x, "int?"),
        if_(
            x.is_not("int"),
            vec![l.then_stmt(block(vec![
                if_(expr("bool"), vec![break_(Some(l))]),
                return_(),
            ]))],
        ),
        check_not_promoted(x),
    ]);
}

#[test]
fn logical_binary_op_right_begin_is_and_true_promotes_in_rhs() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        x.not_eq(null_literal())
            .and(second(check_promoted(x, "int"), expr("bool"))),
    ]);
}

#[test]
fn logical_binary_op_right_end_is_and_true_keeps_promotions_from_rhs() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        if_(
            expr("bool").and(x.not_eq(null_literal())),
            vec![check_promoted(x, "int")],
        ),
    ]);
}

#[test]
fn logical_binary_op_right_end_is_and_false_keeps_promotions_from_rhs() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        if_else(
            expr("bool").or(x.eq(null_literal())),
            vec![],
            vec![check_promoted(x, "int")],
        ),
    ]);
}

#[test]
fn logical_binary_op_right_begin_is_and_false_promotes_in_rhs() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        x.eq(null_literal())
            .or(second(check_promoted(x, "int"), expr("bool"))),
    ]);
}

#[test]
fn logical_binary_op_is_and_true_joins_promotions() {
    // if (x != null && y != null) {
    //   promotes x and y
    // }
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        if_(
            x.not_eq(null_literal()).and(y.not_eq(null_literal())),
            vec![check_promoted(x, "int"), check_promoted(y, "int")],
        ),
    ]);
}

#[test]
fn logical_binary_op_is_and_false_joins_promotions() {
    // if (x == null || y == null) {} else {
    //   promotes x and y
    // }
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        if_else(
            x.eq(null_literal()).or(y.eq(null_literal())),
            vec![],
            vec![check_promoted(x, "int"), check_promoted(y, "int")],
        ),
    ]);
}

#[test]
fn logical_not_end_inverts_a_condition() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        if_else(
            x.eq(null_literal()).not(),
            vec![check_promoted(x, "int")],
            vec![check_not_promoted(x)],
        ),
    ]);
}

#[test]
fn logical_not_end_handles_null_literals() {
    let mut h = set_up();
    h.run(vec![
        // `!null` would be a compile error, but we need to make sure we don't
        // crash.
        if_else(null_literal().not(), vec![], vec![]),
    ]);
}

#[test]
fn non_null_assert_end_x_promotes() {
    let mut h = set_up();
    let x = Var::new("x");
    let ssa_before_promotion = Late::<SsaNode>::new();
    h.run(vec![
        declare_init(x, "int?"),
        get_ssa_nodes(move |nodes| ssa_before_promotion.set(nodes.get(x).unwrap())),
        x.non_null_assert(),
        check_promoted(x, "int"),
        get_ssa_nodes(move |nodes| expect_same(nodes, x, &ssa_before_promotion.get())),
    ]);
}

#[test]
fn non_null_assert_end_sets_reachability_if_type_is_null() {
    // Note: this is handled by the general mechanism that marks control flow
    // as reachable after any expression with static type `Never`.  This is
    // implemented in the flow analysis client, but we test it here anyway as
    // a validation of the "mini AST" logic.
    let mut h = set_up();
    h.run(vec![
        expr("Null")
            .non_null_assert()
            .then_stmt(check_reachable(false)),
    ]);
}

#[test]
fn null_aware_access_temporarily_promotes() {
    let mut h = set_up();
    let x = Var::new("x");
    let ssa_before_promotion = Late::<SsaNode>::new();
    h.add_member("int", "f", Some("Null Function(Object?)"), false, None);
    h.run(vec![
        declare_init(x, "int?"),
        get_ssa_nodes(move |nodes| ssa_before_promotion.set(nodes.get(x).unwrap())),
        x.invoke_method(
            "f",
            vec![list_literal(
                vec![
                    check_reachable(true),
                    check_promoted(x, "int"),
                    get_ssa_nodes(move |nodes| expect_same(nodes, x, &ssa_before_promotion.get())),
                ],
                "dynamic",
            )],
            true,
        ),
        check_not_promoted(x),
        get_ssa_nodes(move |nodes| expect_same(nodes, x, &ssa_before_promotion.get())),
    ]);
}

#[test]
fn null_aware_access_promotes_the_target_of_a_cascade() {
    let mut h = set_up();
    let x = Var::new("x");
    h.add_member("int", "f", Some("Null Function(Object?)"), false, None);
    h.run(vec![
        declare_init(x, "int?"),
        x.cascade(
            vec![Box::new(move |placeholder: Node| {
                placeholder.invoke_method(
                    "f",
                    vec![list_literal(
                        vec![check_reachable(true), check_promoted(x, "int")],
                        "dynamic",
                    )],
                    false,
                )
            })],
            true,
        ),
    ]);
}

#[test]
fn null_aware_access_preserves_demotions() {
    let mut h = set_up();
    let x = Var::new("x");
    h.add_member("int", "f", Some("Null Function(Object?)"), false, None);
    h.run(vec![
        declare_init(x, "int?"),
        x.as_("int"),
        expr("int").invoke_method(
            "f",
            vec![list_literal(
                vec![
                    check_reachable(true),
                    check_promoted(x, "int"),
                    x.write(expr("int?")),
                ],
                "dynamic",
            )],
            true,
        ),
        check_not_promoted(x),
    ]);
}

#[test]
fn null_aware_access_sets_reachability_correctly_for_null_type() {
    let mut h = set_up();
    h.add_member("Never", "f", Some("Null Function(Object?)"), false, None);
    h.run(vec![
        expr("Null").invoke_method("f", vec![check_reachable(false)], true),
        check_reachable(true),
    ]);
}

#[test]
fn parenthesized_expression_preserves_promotion_behaviors() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        if_(
            x.parenthesized()
                .not_eq(null_literal().parenthesized())
                .parenthesized(),
            vec![check_promoted(x, "int")],
        ),
    ]);
}

#[test]
fn if_case_splits_control_flow() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    let z = Var::new("z");
    let w = Var::new("w");
    h.run(vec![
        declare_typed(x, "int"),
        declare_typed(y, "int"),
        declare_typed(z, "int"),
        if_case(
            expr("num"),
            w.pattern().with_declared_type("int"),
            vec![x.write(expr("int")), y.write(expr("int"))],
            Some(vec![y.write(expr("int")), z.write(expr("int"))]),
        ),
        check_assigned(x, false),
        check_assigned(y, true),
        check_assigned(z, false),
    ]);
}

#[test]
fn if_case_does_not_promote_when_expression_true() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        if_case(
            x.not_eq(null_literal()),
            int_literal(0).pattern(),
            vec![check_not_promoted(x)],
            None,
        ),
    ]);
}

#[test]
fn promote_promotes_to_a_subtype_and_sets_type_of_interest() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "num?"),
        check_not_promoted(x),
        x.as_("num"),
        check_promoted(x, "num"),
        // Check that it's a type of interest by promoting and de-promoting.
        if_(
            x.is_("int"),
            vec![
                check_promoted(x, "int"),
                x.write(expr("num")),
                check_promoted(x, "num"),
            ],
        ),
    ]);
}

#[test]
fn promote_does_not_promote_to_a_non_subtype() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "num?"),
        check_not_promoted(x),
        x.as_("String"),
        check_not_promoted(x),
    ]);
}

#[test]
fn promote_does_not_promote_if_variable_is_write_captured() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "num?"),
        check_not_promoted(x),
        local_function(vec![x.write(expr("num"))]),
        x.as_("num"),
        check_not_promoted(x),
    ]);
}

#[test]
fn promoted_type_handles_not_yet_seen_variables() {
    // Note: this is needed for error recovery in the analyzer.
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![check_not_promoted(x), declare_init(x, "int")]);
}

#[test]
fn post_inc_dec_does_not_store_expression_info_in_the_write() {
    // num x;
    // if (x++ is int) {
    //   x is not promoted.
    // }
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_typed(x, "num"),
        if_(
            x.post_inc_dec()
                .is_("int")
                .get_expression_info(|info| assert!(info.is_none())),
            vec![check_not_promoted(x)],
        ),
        check_not_promoted(x),
    ]);
}

#[test]
fn post_inc_dec_demotes_to_the_written_type() {
    // If `x` has type B, but is promoted to subtype C and then D, and D
    // has a `+` operator that returns C, then after `x++`, `x` should be
    // demoted to C.
    let mut h = set_up();
    let x = Var::new("x");
    h.add_super_interfaces("B", |_| vec![ty("Object")]);
    h.add_super_interfaces("C", |_| vec![ty("B"), ty("Object")]);
    h.add_super_interfaces("D", |_| vec![ty("C"), ty("B"), ty("Object")]);
    h.add_member("D", "+", Some("C Function(int)"), false, None);
    h.run(vec![
        declare(x).with_initializer(expr("B")),
        x.as_("C"),
        x.as_("D"),
        x.post_inc_dec(),
        check_promoted(x, "C"),
    ]);
}

#[test]
fn pre_inc_dec_stores_expression_info_in_the_write() {
    // num x;
    // if (++x is int) {
    //   x is promoted.
    // }
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_typed(x, "num"),
        if_(
            x.pre_inc_dec()
                .is_("int")
                .get_expression_info(|info| assert!(info.is_some())),
            vec![check_promoted(x, "int")],
        ),
        check_not_promoted(x),
    ]);
}

#[test]
fn pre_inc_dec_demotes_to_the_written_type() {
    // If `x` has type B, but is promoted to subtype C and then D, and D
    // has a `+` operator that returns C, then after `++x`, `x` should be
    // demoted to C.
    let mut h = set_up();
    let x = Var::new("x");
    h.add_super_interfaces("B", |_| vec![ty("Object")]);
    h.add_super_interfaces("C", |_| vec![ty("B"), ty("Object")]);
    h.add_super_interfaces("D", |_| vec![ty("C"), ty("B"), ty("Object")]);
    h.add_member("D", "+", Some("C Function(int)"), false, None);
    h.run(vec![
        declare(x).with_initializer(expr("B")),
        x.as_("C"),
        x.as_("D"),
        x.pre_inc_dec(),
        check_promoted(x, "C"),
    ]);
}

mod suspension {
    use super::*;

    mod await_ {
        use super::*;

        #[test]
        fn demotes_variables_written_to_in_an_outer_function() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("Object?")),
                local_function(vec![
                    x.as_("int"),
                    check_promoted(x, "int"),
                    await_(expr("Future<void>")),
                    check_not_promoted(x),
                ]),
                x.write(expr("Object?")),
            ]);
        }

        #[test]
        fn unnecessary_to_demote_variables_written_to_in_an_inner_function() {
            // No demotion is necessary in this case because it's not sound to
            // promote the variable in the first place.
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("Object?")),
                local_function(vec![
                    x.as_("int"),
                    // As far as flow analysis knows, evaluation of any nontrivial
                    // expression in this local function could potentially cause
                    // another instance of this local function to be invoked, which
                    // could in turn potentially write to `x`. So it's not sound to
                    // promote `x` to `int`.
                    check_not_promoted(x),
                    await_(expr("Future<void>")),
                    check_not_promoted(x),
                    x.write(expr("Object?")),
                    check_not_promoted(x),
                ]),
            ]);
        }

        #[test]
        fn demotes_after_the_await_operand() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("Object?")),
                local_function(vec![
                    x.as_("int"),
                    check_promoted(x, "int"),
                    await_(second(check_promoted(x, "int"), expr("Future<void>"))),
                    check_not_promoted(x),
                ]),
                x.write(expr("Object?")),
            ]);
        }

        #[test]
        fn does_not_demote_variables_declared_in_the_current_function() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![local_function(vec![
                declare(x).with_initializer(expr("Object?")),
                x.as_("int"),
                check_promoted(x, "int"),
                await_(expr("Future<void>")),
                check_promoted(x, "int"),
                x.write(expr("Object?")),
            ])]);
        }

        #[test]
        fn does_not_demote_variables_that_are_not_written_anywhere() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("Object?")),
                local_function(vec![
                    x.as_("int"),
                    check_promoted(x, "int"),
                    await_(expr("Future<void>")),
                    check_promoted(x, "int"),
                ]),
            ]);
        }
    }

    mod yield_ {
        use super::*;

        #[test]
        fn demotes_variables_written_to_in_an_outer_function() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("Object?")),
                local_function(vec![
                    x.as_("int"),
                    check_promoted(x, "int"),
                    yield_(expr("Future<void>"), false),
                    check_not_promoted(x),
                ]),
                x.write(expr("Object?")),
            ]);
        }

        #[test]
        fn unnecessary_to_demote_variables_written_to_in_an_inner_function() {
            // No demotion is necessary in this case because it's not sound to
            // promote the variable in the first place.
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("Object?")),
                local_function(vec![
                    x.as_("int"),
                    // As far as flow analysis knows, evaluation of any nontrivial
                    // expression in this local function could potentially cause
                    // another instance of this local function to be invoked, which
                    // could in turn potentially write to `x`. So it's not sound to
                    // promote `x` to `int`.
                    check_not_promoted(x),
                    yield_(expr("Future<void>"), false),
                    check_not_promoted(x),
                    x.write(expr("Object?")),
                    check_not_promoted(x),
                ]),
            ]);
        }

        #[test]
        fn demotes_after_the_yield_operand() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("Object?")),
                local_function(vec![
                    x.as_("int"),
                    check_promoted(x, "int"),
                    yield_(
                        second(check_promoted(x, "int"), expr("Future<void>")),
                        false,
                    ),
                    check_not_promoted(x),
                ]),
                x.write(expr("Object?")),
            ]);
        }

        #[test]
        fn does_not_demote_variables_declared_in_the_current_function() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![local_function(vec![
                declare(x).with_initializer(expr("Object?")),
                x.as_("int"),
                check_promoted(x, "int"),
                yield_(expr("Future<void>"), false),
                check_promoted(x, "int"),
                x.write(expr("Object?")),
            ])]);
        }
    }
}
