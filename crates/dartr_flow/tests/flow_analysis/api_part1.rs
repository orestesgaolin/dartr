// Dart source: pkg/_fe_analyzer_shared/test/flow_analysis/flow_analysis_test.dart
// (lines 37-1005: group 'API', from 'asExpression_end promotes variables'
// to 'functionExpression_begin() cancels promotions of final vars with
// inference-update-4 disabled')

//! Dart group `API` (first part).

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

/// Dart `expect(nodes[x], isNot(expected))`.
#[track_caller]
fn expect_not_same(nodes: &SsaNodeHarness, x: Var, expected: &SsaNode) {
    assert!(
        !same_ssa(nodes.get(x).as_ref(), Some(expected)),
        "expected a different SSA node"
    );
}

#[test]
fn as_expression_end_promotes_variables() {
    let mut h = set_up();
    let x = Var::new("x");
    let ssa_before_promotion = Late::<SsaNode>::new();
    h.run(vec![
        declare_init(x, "int?"),
        get_ssa_nodes(move |nodes| ssa_before_promotion.set(nodes.get(x).unwrap())),
        x.as_("int"),
        check_promoted(x, "int"),
        get_ssa_nodes(move |nodes| expect_same(nodes, x, &ssa_before_promotion.get())),
    ]);
}

#[test]
fn as_expression_end_handles_other_expressions() {
    let mut h = set_up();
    h.run(vec![expr("Object").as_("int")]);
}

#[test]
fn as_expression_end_sets_reachability_for_never() {
    // Note: this is handled by the general mechanism that marks control
    // flow as reachable after any expression with static type `Never`. This
    // is implemented in the flow analysis client, but we test it here anyway
    // as a validation of the "mini AST" logic.
    let mut h = set_up();
    h.run(vec![
        check_reachable(true),
        expr("int").as_("Never"),
        check_reachable(false),
    ]);
}

#[test]
fn assert_after_condition_promotes() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        assert_(
            x.eq(null_literal()),
            second(check_promoted(x, "int"), expr("String")),
        ),
    ]);
}

#[test]
fn assert_end_joins_previous_and_if_true_states() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    let z = Var::new("z");
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        declare_init(z, "int?"),
        x.as_("int"),
        z.as_("int"),
        assert_(
            second(
                list_literal(
                    vec![x.write(expr("int?")), z.write(expr("int?"))],
                    "dynamic",
                ),
                expr("bool"),
            )
            .and(x.not_eq(null_literal()).and(y.not_eq(null_literal()))),
            None,
        ),
        // x should be promoted because it was promoted before the assert,
        // and it is re-promoted within the assert (if it passes)
        check_promoted(x, "int"),
        // y should not be promoted because it was not promoted before the
        // assert.
        check_not_promoted(y),
        // z should not be promoted because it is demoted in the assert
        // condition.
        check_not_promoted(z),
    ]);
}

#[test]
fn conditional_then_begin_promotes_true_branch() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        x.not_eq(null_literal()).conditional(
            second(check_promoted(x, "int"), expr("int")),
            second(check_not_promoted(x), expr("int")),
        ),
        check_not_promoted(x),
    ]);
}

#[test]
fn conditional_else_begin_promotes_false_branch() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        x.eq(null_literal()).conditional(
            second(check_not_promoted(x), expr("Null")),
            second(check_promoted(x, "int"), expr("Null")),
        ),
        check_not_promoted(x),
    ]);
}

#[test]
fn conditional_end_keeps_promotions_common_to_true_and_false_branches() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    let z = Var::new("z");
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        declare_init(z, "int?"),
        expr("bool").conditional(
            second(
                list_literal(vec![x.as_("int"), y.as_("int")], "dynamic"),
                expr("Null"),
            ),
            second(
                list_literal(vec![x.as_("int"), z.as_("int")], "dynamic"),
                expr("Null"),
            ),
        ),
        check_promoted(x, "int"),
        check_not_promoted(y),
        check_not_promoted(z),
    ]);
}

#[test]
fn conditional_joins_true_states() {
    // if (... ? (x != null && y != null) : (x != null && z != null)) {
    //   promotes x, but not y or z
    // }
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    let z = Var::new("z");
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        declare_init(z, "int?"),
        if_(
            expr("bool").conditional(
                x.not_eq(null_literal()).and(y.not_eq(null_literal())),
                x.not_eq(null_literal()).and(z.not_eq(null_literal())),
            ),
            vec![
                check_promoted(x, "int"),
                check_not_promoted(y),
                check_not_promoted(z),
            ],
        ),
    ]);
}

#[test]
fn conditional_joins_false_states() {
    // if (... ? (x == null || y == null) : (x == null || z == null)) {
    // } else {
    //   promotes x, but not y or z
    // }
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    let z = Var::new("z");
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        declare_init(z, "int?"),
        if_else(
            expr("bool").conditional(
                x.eq(null_literal()).or(y.eq(null_literal())),
                x.eq(null_literal()).or(z.eq(null_literal())),
            ),
            vec![],
            vec![
                check_promoted(x, "int"),
                check_not_promoted(y),
                check_not_promoted(z),
            ],
        ),
    ]);
}

#[test]
fn declare_sets_ssa() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_typed(x, "Object"),
        get_ssa_nodes(move |nodes| assert!(nodes.get(x).is_some())),
    ]);
}

#[test]
fn equality_op_x_not_eq_null_promotes_true_branch() {
    let mut h = set_up();
    let x = Var::new("x");
    let ssa_before_promotion = Late::<SsaNode>::new();
    h.run(vec![
        declare_init(x, "int?"),
        get_ssa_nodes(move |nodes| ssa_before_promotion.set(nodes.get(x).unwrap())),
        if_else(
            x.not_eq(null_literal()),
            vec![
                check_reachable(true),
                check_promoted(x, "int"),
                get_ssa_nodes(move |nodes| expect_same(nodes, x, &ssa_before_promotion.get())),
            ],
            vec![
                check_reachable(true),
                check_not_promoted(x),
                get_ssa_nodes(move |nodes| expect_same(nodes, x, &ssa_before_promotion.get())),
            ],
        ),
    ]);
}

#[test]
fn equality_op_x_not_eq_null_when_x_is_non_nullable() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int"),
        if_else(
            x.not_eq(null_literal()),
            vec![check_reachable(true), check_not_promoted(x)],
            vec![check_reachable(false), check_not_promoted(x)],
        ),
    ]);
}

#[test]
fn equality_op_expr_eq_expr_has_no_special_effect() {
    let mut h = set_up();
    h.run(vec![if_else(
        expr("int?").eq(expr("int?")),
        vec![check_reachable(true)],
        vec![check_reachable(true)],
    )]);
}

#[test]
fn equality_op_expr_not_eq_expr_has_no_special_effect() {
    let mut h = set_up();
    h.run(vec![if_else(
        expr("int?").not_eq(expr("int?")),
        vec![check_reachable(true)],
        vec![check_reachable(true)],
    )]);
}

#[test]
fn equality_op_x_not_eq_null_expr_does_not_promote() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        if_else(
            x.not_eq(expr("Null")),
            vec![check_not_promoted(x)],
            vec![check_not_promoted(x)],
        ),
    ]);
}

#[test]
fn equality_op_x_eq_null_promotes_false_branch() {
    let mut h = set_up();
    let x = Var::new("x");
    let ssa_before_promotion = Late::<SsaNode>::new();
    h.run(vec![
        declare_init(x, "int?"),
        get_ssa_nodes(move |nodes| ssa_before_promotion.set(nodes.get(x).unwrap())),
        if_else(
            x.eq(null_literal()),
            vec![
                check_reachable(true),
                check_not_promoted(x),
                get_ssa_nodes(move |nodes| expect_same(nodes, x, &ssa_before_promotion.get())),
            ],
            vec![
                check_reachable(true),
                check_promoted(x, "int"),
                get_ssa_nodes(move |nodes| expect_same(nodes, x, &ssa_before_promotion.get())),
            ],
        ),
    ]);
}

#[test]
fn equality_op_x_eq_null_when_x_is_an_assignment_expression() {
    // int? x;
    // if ((x = <int?>) == null) {
    //   return;
    // }
    // x is promoted to `int`.
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_typed(x, "int?"),
        if_(x.write(expr("int?")).eq(null_literal()), vec![return_()]),
        check_promoted(x, "int"),
    ]);
}

#[test]
fn equality_op_x_eq_null_when_x_is_an_assignment_expression_and_inference_update_4_is_disabled() {
    let mut h = set_up();
    let x = Var::new("x");
    h.disable_inference_update4();
    h.run(vec![
        declare_typed(x, "int?"),
        if_(x.write(expr("int?")).eq(null_literal()), vec![return_()]),
        check_not_promoted(x),
    ]);
}

#[test]
fn equality_op_x_eq_null_when_x_is_non_nullable() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int"),
        if_else(
            x.eq(null_literal()),
            vec![check_reachable(false), check_not_promoted(x)],
            vec![check_reachable(true), check_not_promoted(x)],
        ),
    ]);
}

#[test]
fn equality_op_null_not_eq_x_promotes_true_branch() {
    let mut h = set_up();
    let x = Var::new("x");
    let ssa_before_promotion = Late::<SsaNode>::new();
    h.run(vec![
        declare_init(x, "int?"),
        get_ssa_nodes(move |nodes| ssa_before_promotion.set(nodes.get(x).unwrap())),
        if_else(
            null_literal().not_eq(x),
            vec![
                check_promoted(x, "int"),
                get_ssa_nodes(move |nodes| expect_same(nodes, x, &ssa_before_promotion.get())),
            ],
            vec![
                check_not_promoted(x),
                get_ssa_nodes(move |nodes| expect_same(nodes, x, &ssa_before_promotion.get())),
            ],
        ),
    ]);
}

#[test]
fn equality_op_null_expr_not_eq_x_does_not_promote() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        if_else(
            expr("Null").not_eq(x),
            vec![check_not_promoted(x)],
            vec![check_not_promoted(x)],
        ),
    ]);
}

#[test]
fn equality_op_null_eq_x_promotes_false_branch() {
    let mut h = set_up();
    let x = Var::new("x");
    let ssa_before_promotion = Late::<SsaNode>::new();
    h.run(vec![
        declare_init(x, "int?"),
        get_ssa_nodes(move |nodes| ssa_before_promotion.set(nodes.get(x).unwrap())),
        if_else(
            null_literal().eq(x),
            vec![
                check_not_promoted(x),
                get_ssa_nodes(move |nodes| expect_same(nodes, x, &ssa_before_promotion.get())),
            ],
            vec![
                check_promoted(x, "int"),
                get_ssa_nodes(move |nodes| expect_same(nodes, x, &ssa_before_promotion.get())),
            ],
        ),
    ]);
}

#[test]
fn equality_op_null_eq_null_equivalent_to_true() {
    let mut h = set_up();
    h.run(vec![if_else(
        expr("Null").eq(expr("Null")),
        vec![check_reachable(true)],
        vec![check_reachable(false)],
    )]);
}

#[test]
fn equality_op_null_not_eq_null_equivalent_to_false() {
    let mut h = set_up();
    h.run(vec![if_else(
        expr("Null").not_eq(expr("Null")),
        vec![check_reachable(false)],
        vec![check_reachable(true)],
    )]);
}

#[test]
fn condition_eq_null_does_not_promote_write_captured_vars() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        if_(x.not_eq(null_literal()), vec![check_promoted(x, "int")]),
        local_function(vec![x.write(expr("int?"))]),
        if_(x.not_eq(null_literal()), vec![check_not_promoted(x)]),
    ]);
}

#[test]
fn declare_initialized_false_assigns_new_ssa_ids() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    h.run(vec![
        declare_typed(x, "int?"),
        declare_typed(y, "int?"),
        get_ssa_nodes(move |nodes| {
            assert!(!same_ssa(nodes.get(y).as_ref(), nodes.get(x).as_ref()))
        }),
    ]);
}

#[test]
fn declare_initialized_true_assigns_new_ssa_ids() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        get_ssa_nodes(move |nodes| {
            assert!(!same_ssa(nodes.get(y).as_ref(), nodes.get(x).as_ref()))
        }),
    ]);
}

#[test]
fn do_statement_body_begin_un_promotes() {
    let mut h = set_up();
    let x = Var::new("x");
    let ssa_before_loop = Late::<SsaNode>::new();
    h.run(vec![
        declare_init(x, "int?"),
        x.as_("int"),
        check_promoted(x, "int"),
        get_ssa_nodes(move |nodes| ssa_before_loop.set(nodes.get(x).unwrap())),
        do_(
            vec![
                get_ssa_nodes(move |nodes| expect_not_same(nodes, x, &ssa_before_loop.get())),
                check_not_promoted(x),
                x.write(expr("Null")),
            ],
            expr("bool"),
        ),
    ]);
}

#[test]
fn do_statement_body_begin_handles_write_captures_in_the_loop() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        do_(
            vec![
                x.as_("int"),
                // The promotion should have no effect, because the second
                // time through the loop, x has been write-captured.
                check_not_promoted(x),
                local_function(vec![x.write(expr("int?"))]),
            ],
            expr("bool"),
        ),
    ]);
}

#[test]
fn do_statement_condition_begin_joins_continue_state() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        do_(
            vec![
                if_(x.not_eq(null_literal()), vec![continue_(None)]),
                return_(),
                check_reachable(false),
                check_not_promoted(x),
            ],
            second(
                list_literal(
                    vec![check_reachable(true), check_promoted(x, "int")],
                    "dynamic",
                ),
                expr("bool"),
            ),
        ),
    ]);
}

#[test]
fn do_statement_end_promotes() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        do_(
            vec![],
            second(check_not_promoted(x), expr("bool")).or(x.eq(null_literal())),
        ),
        check_promoted(x, "int"),
    ]);
}

#[test]
fn equality_op_end_on_property_get_preserves_target_variable() {
    // This is a regression test for a mistake made during the
    // implementation of "why not promoted" functionality: when storing
    // information about an attempt to promote a field (e.g. `x.y != null`)
    // we need to make sure we don't wipe out information about the target
    // variable (`x`).
    let mut h = set_up();
    h.add_member("C", "y", Some("Object?"), false, None);
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "C"),
        check_assigned(x, true),
        if_else(
            x.property("y", false).not_eq(null_literal()),
            vec![check_assigned(x, true)],
            vec![check_assigned(x, true)],
        ),
    ]);
}

#[test]
fn equality_op_end_does_not_set_reachability_for_this() {
    // Note: sound flow analysis changes this behavior.
    let mut h = set_up();
    h.disable_sound_flow_analysis();
    h.set_this_type("C");
    h.add_super_interfaces("C", |_| vec![ty("Object")]);
    h.run(vec![if_(
        this_().is_not("Null"),
        vec![if_else(
            this_().eq(null_literal()),
            vec![check_reachable(true)],
            vec![check_reachable(true)],
        )],
    )]);
}

mod equality_op_end_does_not_set_reachability_for_property_gets {
    use super::*;

    #[test]
    fn on_a_variable() {
        let mut h = set_up();
        h.add_member("C", "f", Some("Object?"), false, None);
        let x = Var::new("x");
        h.run(vec![
            declare_init(x, "C"),
            if_(
                x.property("f", false).is_("Null"),
                vec![if_else(
                    x.property("f", false).eq(null_literal()),
                    vec![check_reachable(true)],
                    vec![check_reachable(true)],
                )],
            ),
        ]);
    }

    #[test]
    fn on_an_arbitrary_expression() {
        let mut h = set_up();
        h.add_member("C", "f", Some("Object?"), false, None);
        h.run(vec![if_(
            expr("C").property("f", false).is_("Null"),
            vec![if_else(
                expr("C").property("f", false).eq(null_literal()),
                vec![check_reachable(true)],
                vec![check_reachable(true)],
            )],
        )]);
    }

    #[test]
    fn on_explicit_this() {
        let mut h = set_up();
        h.set_this_type("C");
        h.add_member("C", "f", Some("Object?"), false, None);
        h.run(vec![if_(
            this_().property("f", false).is_("Null"),
            vec![if_else(
                this_().property("f", false).eq(null_literal()),
                vec![check_reachable(true)],
                vec![check_reachable(true)],
            )],
        )]);
    }

    #[test]
    fn on_implicit_this_super() {
        let mut h = set_up();
        h.set_this_type("C");
        h.add_member("C", "f", Some("Object?"), false, None);
        h.run(vec![if_(
            this_property("f").is_("Null"),
            vec![if_else(
                this_property("f").eq(null_literal()),
                vec![check_reachable(true)],
                vec![check_reachable(true)],
            )],
        )]);
    }
}

#[test]
fn finish_checks_proper_nesting() {
    let h = set_up();
    let e = expr("Null");
    let s = if_(e, vec![]);
    let mut flow = FlowAnalysisImpl::<MiniAstTypes>::new(
        h.type_operations(),
        AssignedVariablesImpl::new(),
        h.compute_type_analyzer_options(),
    );
    flow.if_statement_condition_begin();
    flow.if_statement_then_begin(None, s);
    expect_asserts(move || flow.finish());
}

#[test]
fn for_condition_begin_un_promotes() {
    let mut h = set_up();
    let x = Var::new("x");
    let ssa_before_loop = Late::<SsaNode>::new();
    h.run(vec![
        declare_init(x, "int?"),
        x.as_("int"),
        check_promoted(x, "int"),
        get_ssa_nodes(move |nodes| ssa_before_loop.set(nodes.get(x).unwrap())),
        for_(
            None,
            second(
                list_literal(
                    vec![
                        check_not_promoted(x),
                        get_ssa_nodes(move |nodes| {
                            expect_not_same(nodes, x, &ssa_before_loop.get())
                        }),
                    ],
                    "dynamic",
                ),
                expr("bool"),
            ),
            None,
            vec![x.write(expr("int?"))],
            false,
        ),
    ]);
}

#[test]
fn for_condition_begin_handles_write_captures_in_the_loop() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        x.as_("int"),
        check_promoted(x, "int"),
        for_(
            None,
            second(
                list_literal(
                    vec![
                        x.as_("int"),
                        check_not_promoted(x),
                        local_function(vec![x.write(expr("int?"))]),
                    ],
                    "dynamic",
                ),
                expr("bool"),
            ),
            None,
            vec![],
            false,
        ),
    ]);
}

#[test]
fn for_body_begin_handles_empty_condition() {
    let mut h = set_up();
    h.run(vec![
        for_(
            None,
            None,
            second(check_reachable(true), expr("Null")),
            vec![],
            false,
        ),
        check_reachable(false),
    ]);
}

#[test]
fn for_body_begin_promotes() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![for_(
        declare_init(x, "int?"),
        x.not_eq(null_literal()),
        None,
        vec![check_promoted(x, "int")],
        false,
    )]);
}

#[test]
fn for_body_begin_can_be_used_with_a_null_statement() {
    // This is needed for collection elements that are for-loops.
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![for_(
        declare_init(x, "int?"),
        x.not_eq(null_literal()),
        None,
        vec![],
        true,
    )]);
}

#[test]
fn for_updater_begin_joins_current_and_continue_states() {
    // To test that the states are properly joined, we have three variables:
    // x, y, and z. We promote x and y in the continue path, and x and z in
    // the current path. Inside the updater, only x should be promoted.
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    let z = Var::new("z");
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        declare_init(z, "int?"),
        for_(
            None,
            expr("bool"),
            second(
                list_literal(
                    vec![
                        check_promoted(x, "int"),
                        check_not_promoted(y),
                        check_not_promoted(z),
                    ],
                    "dynamic",
                ),
                expr("Null"),
            ),
            vec![
                if_(
                    expr("bool"),
                    vec![x.as_("int"), y.as_("int"), continue_(None)],
                ),
                x.as_("int"),
                z.as_("int"),
            ],
            false,
        ),
    ]);
}

#[test]
fn for_end_joins_break_and_condition_false_states() {
    // To test that the states are properly joined, we have three variables:
    // x, y, and z. We promote x and y in the break path, and x and z in the
    // condition-false path. After the loop, only x should be promoted.
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    let z = Var::new("z");
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        declare_init(z, "int?"),
        for_(
            None,
            x.eq(null_literal()).or(z.eq(null_literal())),
            None,
            vec![if_(
                expr("bool"),
                vec![x.as_("int"), y.as_("int"), break_(None)],
            )],
            false,
        ),
        check_promoted(x, "int"),
        check_not_promoted(y),
        check_not_promoted(z),
    ]);
}

#[test]
fn for_end_with_break_updates_ssa_of_modified_vars() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    let x_ssa_inside_loop = Late::<SsaNode>::new();
    let y_ssa_inside_loop = Late::<SsaNode>::new();
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        for_(
            None,
            expr("bool"),
            None,
            vec![
                x.write(expr("int?")),
                if_(expr("bool"), vec![break_(None)]),
                get_ssa_nodes(move |nodes| {
                    x_ssa_inside_loop.set(nodes.get(x).unwrap());
                    y_ssa_inside_loop.set(nodes.get(y).unwrap());
                }),
            ],
            false,
        ),
        get_ssa_nodes(move |nodes| {
            // x's Ssa should have been changed because of the join at the
            // end of the loop. y's should not, since it retains the value it
            // had prior to the loop.
            expect_not_same(nodes, x, &x_ssa_inside_loop.get());
            expect_same(nodes, y, &y_ssa_inside_loop.get());
        }),
    ]);
}

#[test]
fn for_end_with_break_updates_ssa_of_modified_vars_when_types_were_tested() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    let x_ssa_inside_loop = Late::<SsaNode>::new();
    let y_ssa_inside_loop = Late::<SsaNode>::new();
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        for_(
            None,
            expr("bool"),
            None,
            vec![
                x.write(expr("int?")),
                if_(expr("bool"), vec![break_(None)]),
                if_(x.is_("int"), vec![]),
                get_ssa_nodes(move |nodes| {
                    x_ssa_inside_loop.set(nodes.get(x).unwrap());
                    y_ssa_inside_loop.set(nodes.get(y).unwrap());
                }),
            ],
            false,
        ),
        get_ssa_nodes(move |nodes| {
            // x's Ssa should have been changed because of the join at the
            // end of the loop. y's should not, since it retains the value it
            // had prior to the loop.
            expect_not_same(nodes, x, &x_ssa_inside_loop.get());
            expect_same(nodes, y, &y_ssa_inside_loop.get());
        }),
    ]);
}

#[test]
fn for_each_body_begin_un_promotes() {
    let mut h = set_up();
    let x = Var::new("x");
    let ssa_before_loop = Late::<SsaNode>::new();
    h.run(vec![
        declare_init(x, "int?"),
        x.as_("int"),
        check_promoted(x, "int"),
        get_ssa_nodes(move |nodes| ssa_before_loop.set(nodes.get(x).unwrap())),
        for_each_with_non_variable(
            expr("List<int?>"),
            vec![
                check_not_promoted(x),
                get_ssa_nodes(move |nodes| expect_not_same(nodes, x, &ssa_before_loop.get())),
                x.write(expr("int?")),
            ],
        ),
    ]);
}

#[test]
fn for_each_body_begin_handles_write_captures_in_the_loop() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        x.as_("int"),
        check_promoted(x, "int"),
        for_each_with_non_variable(
            expr("List<int?>"),
            vec![
                x.as_("int"),
                check_not_promoted(x),
                local_function(vec![x.write(expr("int?"))]),
            ],
        ),
    ]);
}

#[test]
fn for_each_body_begin_writes_to_loop_variable() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_typed(x, "int?"),
        check_assigned(x, false),
        for_each_with_variable_set(x, expr("List<int?>"), vec![check_assigned(x, true)]),
        check_assigned(x, false),
    ]);
}

#[test]
fn for_each_body_begin_does_not_write_capture_loop_variable() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_typed(x, "int?"),
        check_assigned(x, false),
        for_each_with_variable_set(
            x,
            expr("List<int?>"),
            vec![
                check_assigned(x, true),
                if_(x.not_eq(null_literal()), vec![check_promoted(x, "int")]),
            ],
        ),
        check_assigned(x, false),
    ]);
}

#[test]
fn for_each_body_begin_pushes_conservative_join_state() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_typed(x, "int"),
        check_unassigned(x, true),
        for_each_with_non_variable(
            expr("List<int>"),
            vec![
                // Since a write to x occurs somewhere in the loop, x should
                // no longer be considered unassigned.
                check_unassigned(x, false),
                break_(None),
                x.write(expr("int")),
            ],
        ),
        // Even though the write to x is unreachable (since it occurs after a
        // break), x should still be considered "possibly assigned" because
        // of the conservative join done at the top of the loop.
        check_unassigned(x, false),
    ]);
}

#[test]
fn for_each_end_restores_state_before_loop() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        for_each_with_non_variable(
            expr("List<int?>"),
            vec![x.as_("int"), check_promoted(x, "int")],
        ),
        check_not_promoted(x),
    ]);
}

#[test]
fn function_expression_begin_cancels_promotions_of_self_captured_vars() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        x.as_("int"),
        y.as_("int"),
        check_promoted(x, "int"),
        check_promoted(y, "int"),
        get_ssa_nodes(move |nodes| {
            assert!(nodes.get(x).is_some());
            assert!(nodes.get(y).is_some());
        }),
        local_function(vec![
            // x is unpromoted within the local function
            check_not_promoted(x),
            check_promoted(y, "int"),
            get_ssa_nodes(move |nodes| {
                assert!(nodes.get(x).is_none());
                assert!(nodes.get(y).is_some());
            }),
            x.write(expr("int?")),
            x.as_("int"),
        ]),
        // x is unpromoted after the local function too
        check_not_promoted(x),
        check_promoted(y, "int"),
        get_ssa_nodes(move |nodes| {
            assert!(nodes.get(x).is_none());
            assert!(nodes.get(y).is_some());
        }),
    ]);
}

#[test]
fn function_expression_begin_cancels_promotions_of_other_captured_vars() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        x.as_("int"),
        y.as_("int"),
        check_promoted(x, "int"),
        check_promoted(y, "int"),
        local_function(vec![
            // x is unpromoted within the local function, because the write
            // might have been captured by the time the local function
            // executes.
            check_not_promoted(x),
            check_promoted(y, "int"),
            // And any effort to promote x fails, because there is no way of
            // knowing when the captured write might occur.
            x.as_("int"),
            check_not_promoted(x),
            check_promoted(y, "int"),
        ]),
        // x is still promoted after the local function, though, because the
        // write hasn't been captured yet.
        check_promoted(x, "int"),
        check_promoted(y, "int"),
        local_function(vec![
            // x is unpromoted inside this local function too.
            check_not_promoted(x),
            check_promoted(y, "int"),
            x.write(expr("int?")),
        ]),
        // And since the second local function captured x, it remains
        // unpromoted.
        check_not_promoted(x),
        check_promoted(y, "int"),
    ]);
}

#[test]
fn function_expression_begin_cancels_promotions_of_written_vars() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    let ssa_before_function = Late::<SsaNode>::new();
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        x.as_("int"),
        y.as_("int"),
        check_promoted(x, "int"),
        get_ssa_nodes(move |nodes| ssa_before_function.set(nodes.get(x).unwrap())),
        check_promoted(y, "int"),
        local_function(vec![
            // x is unpromoted within the local function, because the write
            // might have happened by the time the local function executes.
            check_not_promoted(x),
            get_ssa_nodes(move |nodes| expect_not_same(nodes, x, &ssa_before_function.get())),
            check_promoted(y, "int"),
            // But it can be re-promoted because the write isn't captured.
            x.as_("int"),
            check_promoted(x, "int"),
            check_promoted(y, "int"),
        ]),
        // x is still promoted after the local function, though, because the
        // write hasn't occurred yet.
        check_promoted(x, "int"),
        get_ssa_nodes(move |nodes| expect_same(nodes, x, &ssa_before_function.get())),
        check_promoted(y, "int"),
        x.write(expr("int?")),
        // x is unpromoted now.
        check_not_promoted(x),
        check_promoted(y, "int"),
    ]);
}

#[test]
fn function_expression_begin_cancels_promotions_of_final_vars_with_inference_update_4_disabled() {
    // See test for "functionExpression_begin() preserves promotions of
    // final variables" for enabled behavior.
    let mut h = set_up();
    let x = Var::new("x").with_final(true);
    h.disable_inference_update4();
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
