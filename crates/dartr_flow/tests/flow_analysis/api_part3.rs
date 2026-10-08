// Dart source: pkg/_fe_analyzer_shared/test/flow_analysis/flow_analysis_test.dart
// (lines 2300-3592: group 'API', from 'suspension: yield: does not demote
// variables that are not written anywhere' to 'issue 47991')

//! Dart group `API` (third part).

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

/// Dart `expect(nodes[x], isNot(expected))` (`SsaNode` has identity
/// equality).
#[track_caller]
fn expect_not_same(nodes: &SsaNodeHarness, x: Var, expected: &SsaNode) {
    assert!(
        !same_ssa(nodes.get(x).as_ref(), Some(expected)),
        "expected a different SSA node"
    );
}

mod suspension {
    use super::*;

    mod yield_ {
        use super::*;

        #[test]
        fn does_not_demote_variables_that_are_not_written_anywhere() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("Object?")),
                local_function(vec![
                    x.as_("int"),
                    check_promoted(x, "int"),
                    yield_(expr("Future<void>"), false),
                    check_promoted(x, "int"),
                ]),
            ]);
        }
    }
}

#[test]
fn switch_expression_throw_in_scrutinee_makes_all_cases_unreachable() {
    let mut h = set_up();
    h.run(vec![
        switch_expr(
            throw_(expr("C")),
            vec![
                int_literal(0)
                    .pattern()
                    .then_expr(second(check_reachable(false), int_literal(1))),
                default_().then_expr(second(check_reachable(false), int_literal(2))),
            ],
        ),
        check_reachable(false),
    ]);
}

#[test]
fn switch_expression_throw_in_case_body_has_isolated_effect() {
    let mut h = set_up();
    h.run(vec![
        switch_expr(
            expr("int"),
            vec![
                int_literal(0).pattern().then_expr(throw_(expr("C"))),
                default_().then_expr(second(check_reachable(true), int_literal(2))),
            ],
        ),
        check_reachable(true),
    ]);
}

#[test]
fn switch_expression_throw_in_all_case_bodies_affects_flow_after() {
    let mut h = set_up();
    h.run(vec![
        switch_expr(
            expr("int"),
            vec![
                int_literal(0).pattern().then_expr(throw_(expr("C"))),
                default_().then_expr(throw_(expr("C"))),
            ],
        ),
        check_reachable(false),
    ]);
}

#[test]
fn switch_expression_var_promotes() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![switch_expr(
        expr("int"),
        vec![
            x.pattern()
                .with_declared_type("int?")
                .then_expr(second(check_promoted(x, "int"), null_literal())),
        ],
    )]);
}

#[test]
fn switch_statement_throw_in_scrutinee_makes_all_cases_unreachable() {
    let mut h = set_up();
    h.run(vec![
        switch_(
            throw_(expr("int")),
            vec![
                int_literal(0).pattern().then(vec![check_reachable(false)]),
                int_literal(1).pattern().then(vec![check_reachable(false)]),
            ],
        ),
        check_reachable(false),
    ]);
}

#[test]
fn switch_statement_var_promotes() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![switch_(
        expr("int"),
        vec![
            x.pattern()
                .with_declared_type("int?")
                .then(vec![check_promoted(x, "int")]),
        ],
    )]);
}

#[test]
fn switch_statement_after_when_promotes() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![switch_(
        expr("num"),
        vec![
            x.pattern()
                .when(x.is_("int"))
                .then(vec![check_promoted(x, "int")]),
        ],
    )]);
}

#[test]
fn switch_statement_after_when_called_for_switch_expressions() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![switch_expr(
        expr("num"),
        vec![
            x.pattern()
                .when(x.is_("int"))
                .then_expr(second(check_promoted(x, "int"), expr("String"))),
        ],
    )]);
}

#[test]
fn switch_statement_begin_case_false_restores_previous_promotions() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        x.as_("int"),
        switch_(
            expr("int"),
            vec![
                int_literal(0).pattern().then(vec![
                    check_promoted(x, "int"),
                    x.write(expr("int?")),
                    check_not_promoted(x),
                ]),
                int_literal(1).pattern().then(vec![
                    check_promoted(x, "int"),
                    x.write(expr("int?")),
                    check_not_promoted(x),
                ]),
            ],
        ),
    ]);
}

#[test]
fn switch_statement_begin_case_false_does_not_un_promote() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        x.as_("int"),
        switch_(
            expr("int"),
            vec![int_literal(0).pattern().then(vec![
                check_promoted(x, "int"),
                x.write(expr("int?")),
                check_not_promoted(x),
            ])],
        ),
    ]);
}

#[test]
fn switch_statement_begin_case_false_handles_write_captures_in_cases() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        x.as_("int"),
        switch_(
            expr("int"),
            vec![int_literal(0).pattern().then(vec![
                check_promoted(x, "int"),
                local_function(vec![x.write(expr("int?"))]),
                check_not_promoted(x),
            ])],
        ),
    ]);
}

#[test]
fn switch_statement_begin_case_true_un_promotes() {
    let mut h = set_up();
    let x = Var::new("x");
    let ssa_before_switch = Late::<SsaNode>::new();
    h.run(vec![
        declare_init(x, "int?"),
        x.as_("int"),
        switch_(
            expr("int").then_stmt(block(vec![
                check_promoted(x, "int"),
                get_ssa_nodes(move |nodes| ssa_before_switch.set(nodes.get(x).unwrap())),
            ])),
            vec![switch_statement_member(
                vec![int_literal(0).pattern()],
                vec![
                    check_not_promoted(x),
                    get_ssa_nodes(move |nodes| expect_not_same(nodes, x, &ssa_before_switch.get())),
                    x.write(expr("int?")),
                    check_not_promoted(x),
                ],
                true,
            )],
        ),
    ]);
}

#[test]
fn switch_statement_begin_case_true_handles_write_captures_in_cases() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        x.as_("int"),
        switch_(
            expr("int"),
            vec![switch_statement_member(
                vec![int_literal(0).pattern()],
                vec![
                    x.as_("int"),
                    check_not_promoted(x),
                    local_function(vec![x.write(expr("int?"))]),
                    check_not_promoted(x),
                ],
                true,
            )],
        ),
    ]);
}

#[test]
fn switch_statement_end_false_joins_break_and_default() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    let z = Var::new("z");
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        declare_init(z, "int?"),
        y.as_("int"),
        z.as_("int"),
        switch_(
            expr("int"),
            vec![int_literal(0).pattern().then(vec![
                x.as_("int"),
                y.write(expr("int?")),
                break_(None),
            ])],
        ),
        check_not_promoted(x),
        check_not_promoted(y),
        check_promoted(z, "int"),
    ]);
}

#[test]
fn switch_statement_end_true_joins_breaks() {
    let mut h = set_up();
    let w = Var::new("w");
    let x = Var::new("x");
    let y = Var::new("y");
    let z = Var::new("z");
    h.run(vec![
        declare_init(w, "int?"),
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        declare_init(z, "int?"),
        x.as_("int"),
        y.as_("int"),
        z.as_("int"),
        switch_(
            expr("int"),
            vec![
                int_literal(0).pattern().then(vec![
                    w.as_("int"),
                    y.as_("int"),
                    x.write(expr("int?")),
                    break_(None),
                ]),
                default_().then(vec![
                    w.as_("int"),
                    x.as_("int"),
                    y.write(expr("int?")),
                    break_(None),
                ]),
            ],
        ),
        check_promoted(w, "int"),
        check_not_promoted(x),
        check_not_promoted(y),
        check_promoted(z, "int"),
    ]);
}

#[test]
fn switch_statement_end_true_allows_fall_through_of_last_case() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        switch_(
            expr("int"),
            vec![
                int_literal(0)
                    .pattern()
                    .then(vec![x.as_("int"), break_(None)]),
                default_().then(vec![]),
            ],
        ),
        check_not_promoted(x),
    ]);
}

#[test]
fn switch_statement_end_alternative_joins_branches() {
    let mut h = set_up();
    let x1 = Var::new("x").with_identity("x1");
    let x2 = Var::new("x").with_identity("x2");
    let _ = Var::join("x", vec![x1, x2]);
    let y = Var::new("y");
    let z = Var::new("z");
    h.run(vec![
        declare_typed(y, "num"),
        declare_typed(z, "num"),
        switch_(
            expr("num"),
            vec![switch_statement_member(
                vec![
                    x1.pattern().when(x1.is_("int").and(y.is_("int"))),
                    x2.pattern().when(y.is_("int").and(z.is_("int"))),
                ],
                vec![
                    check_not_promoted(x2),
                    check_promoted(y, "int"),
                    check_not_promoted(z),
                ],
                false,
            )],
        ),
    ]);
}

#[test]
fn try_catch_statement_body_end_restores_pre_try_state() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        y.as_("int"),
        try_(vec![
            x.as_("int"),
            check_promoted(x, "int"),
            check_promoted(y, "int"),
        ])
        .catch_(
            Some("dynamic"),
            None,
            None,
            vec![check_not_promoted(x), check_promoted(y, "int")],
        ),
    ]);
}

#[test]
fn try_catch_statement_body_end_un_promotes_variables_assigned_in_body() {
    let mut h = set_up();
    let x = Var::new("x");
    let ssa_after_try = Late::<SsaNode>::new();
    h.run(vec![
        declare_init(x, "int?"),
        x.as_("int"),
        check_promoted(x, "int"),
        try_(vec![
            x.write(expr("int?")),
            x.as_("int"),
            check_promoted(x, "int"),
            get_ssa_nodes(move |nodes| ssa_after_try.set(nodes.get(x).unwrap())),
        ])
        .catch_(
            Some("dynamic"),
            None,
            None,
            vec![
                check_not_promoted(x),
                get_ssa_nodes(move |nodes| expect_not_same(nodes, x, &ssa_after_try.get())),
            ],
        ),
    ]);
}

#[test]
fn try_catch_statement_body_end_preserves_write_captures_in_body() {
    // Note: it's not necessary for the write capture to survive to the end of
    // the try body, because an exception could occur at any time.  We check
    // this by putting an exit in the try body.

    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        x.as_("int"),
        check_promoted(x, "int"),
        try_(vec![local_function(vec![x.write(expr("int?"))]), return_()]).catch_(
            Some("dynamic"),
            None,
            None,
            vec![x.as_("int"), check_not_promoted(x)],
        ),
    ]);
}

#[test]
fn try_catch_statement_catch_begin_restores_previous_post_body_state() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        try_(vec![])
            .catch_(
                Some("dynamic"),
                None,
                None,
                vec![x.as_("int"), check_promoted(x, "int")],
            )
            .catch_(Some("dynamic"), None, None, vec![check_not_promoted(x)]),
    ]);
}

#[test]
fn try_catch_statement_catch_begin_initializes_vars() {
    let mut h = set_up();
    let e = Var::new("e");
    let st = Var::new("st");
    h.run(vec![try_(vec![]).catch_(
        None,
        Some(e),
        Some(st),
        vec![check_assigned(e, true), check_assigned(st, true)],
    )]);
}

#[test]
fn exception_variable_is_promotable() {
    let mut h = set_up();
    let e = Var::new("e");
    h.run(vec![try_(vec![]).catch_(
        None,
        Some(e),
        None,
        vec![
            check_not_promoted(e),
            e.as_("int"),
            check_promoted(e, "int"),
        ],
    )]);
}

/// Dart `test('Exception variable is promotable', ...)` (the second test of
/// that name, with `type: 'Object'`).
#[test]
fn exception_variable_is_promotable_2() {
    let mut h = set_up();
    let e = Var::new("e");
    h.run(vec![try_(vec![]).catch_(
        Some("Object"),
        Some(e),
        None,
        vec![
            e.expr().check_type("Object"),
            check_not_promoted(e),
            e.as_("String"),
            check_promoted(e, "String"),
        ],
    )]);
}

#[test]
fn stack_trace_variable_is_promotable() {
    let mut h = set_up();
    TypeRegistry::add_interface_type_name("StackTraceSubtype");
    h.add_super_interfaces("StackTraceSubtype", |_| {
        vec![ty("StackTrace"), ty("Object")]
    });
    let e = Var::new("e");
    let st = Var::new("st");
    h.run(vec![try_(vec![]).catch_(
        None,
        Some(e),
        Some(st),
        vec![
            st.expr().check_type("StackTrace"),
            check_not_promoted(st),
            st.as_("StackTraceSubtype"),
            check_promoted(st, "StackTraceSubtype"),
        ],
    )]);
}

#[test]
fn try_catch_statement_catch_end_joins_catch_state_with_after_try_state() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    let z = Var::new("z");
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        declare_init(z, "int?"),
        try_(vec![x.as_("int"), y.as_("int")]).catch_(
            Some("dynamic"),
            None,
            None,
            vec![x.as_("int"), z.as_("int")],
        ),
        // Only x should be promoted, because it's the only variable
        // promoted in both the try body and the catch handler.
        check_promoted(x, "int"),
        check_not_promoted(y),
        check_not_promoted(z),
    ]);
}

#[test]
fn try_catch_statement_catch_end_joins_catch_states() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    let z = Var::new("z");
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        declare_init(z, "int?"),
        try_(vec![return_()])
            .catch_(
                Some("dynamic"),
                None,
                None,
                vec![x.as_("int"), y.as_("int")],
            )
            .catch_(
                Some("dynamic"),
                None,
                None,
                vec![x.as_("int"), z.as_("int")],
            ),
        // Only x should be promoted, because it's the only variable promoted
        // in both catch handlers.
        check_promoted(x, "int"),
        check_not_promoted(y),
        check_not_promoted(z),
    ]);
}

#[test]
fn try_finally_statement_finally_begin_restores_pre_try_state() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        y.as_("int"),
        try_(vec![
            x.as_("int"),
            check_promoted(x, "int"),
            check_promoted(y, "int"),
        ])
        .finally_(vec![check_not_promoted(x), check_promoted(y, "int")]),
    ]);
}

#[test]
fn try_finally_statement_finally_begin_un_promotes_variables_assigned_in_body() {
    let mut h = set_up();
    let x = Var::new("x");
    let ssa_at_start_of_try = Late::<SsaNode>::new();
    let ssa_after_try = Late::<SsaNode>::new();
    h.run(vec![
        declare_init(x, "int?"),
        x.as_("int"),
        check_promoted(x, "int"),
        try_(vec![
            get_ssa_nodes(move |nodes| ssa_at_start_of_try.set(nodes.get(x).unwrap())),
            x.write(expr("int?")),
            x.as_("int"),
            check_promoted(x, "int"),
            get_ssa_nodes(move |nodes| ssa_after_try.set(nodes.get(x).unwrap())),
        ])
        .finally_(vec![
            check_not_promoted(x),
            // The SSA node for X should be different from what it was at any time
            // during the try block, because there is no telling at what point an
            // exception might have occurred.
            get_ssa_nodes(move |nodes| {
                expect_not_same(nodes, x, &ssa_at_start_of_try.get());
                expect_not_same(nodes, x, &ssa_after_try.get());
            }),
        ]),
    ]);
}

#[test]
fn try_finally_statement_finally_begin_preserves_write_captures_in_body() {
    // Note: it's not necessary for the write capture to survive to the end
    // of the try body, because an exception could occur at any time.  We
    // check this by putting an exit in the try body.

    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        try_(vec![local_function(vec![x.write(expr("int?"))]), return_()])
            .finally_(vec![x.as_("int"), check_not_promoted(x)]),
    ]);
}

#[test]
fn try_finally_statement_end_restores_promotions_from_try_body() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        try_(vec![x.as_("int"), check_promoted(x, "int")]).finally_(vec![
            check_not_promoted(x),
            y.as_("int"),
            check_promoted(y, "int"),
        ]),
        // Both x and y should now be promoted.
        check_promoted(x, "int"),
        check_promoted(y, "int"),
    ]);
}

#[test]
fn try_finally_statement_end_does_not_restore_try_body_promotions_for_variables_assigned_in_finally()
 {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    let x_ssa_at_end_of_finally = Late::<SsaNode>::new();
    let y_ssa_at_end_of_finally = Late::<SsaNode>::new();
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        try_(vec![x.as_("int"), check_promoted(x, "int")]).finally_(vec![
            check_not_promoted(x),
            x.write(expr("int?")),
            y.write(expr("int?")),
            y.as_("int"),
            check_promoted(y, "int"),
            get_ssa_nodes(move |nodes| {
                x_ssa_at_end_of_finally.set(nodes.get(x).unwrap());
                y_ssa_at_end_of_finally.set(nodes.get(y).unwrap());
            }),
        ]),
        // x should not be re-promoted, because it might have been assigned a
        // non-promoted value in the "finally" block.  But y's promotion still
        // stands, because y was promoted in the finally block.
        check_not_promoted(x),
        check_promoted(y, "int"),
        // Both x and y should have the same SSA nodes they had at the end of
        // the finally block, since the finally block is guaranteed to have
        // executed.
        get_ssa_nodes(move |nodes| {
            expect_same(nodes, x, &x_ssa_at_end_of_finally.get());
            expect_same(nodes, y, &y_ssa_at_end_of_finally.get());
        }),
    ]);
}

mod allow_local_boolean_vars_to_promote {
    use super::*;

    /// Dart `test('tryFinallyStatement_end() restores SSA nodes from try
    /// block when it' 'is sound to do so', ...)` (the Dart name has no space
    /// between "it" and "is").
    #[test]
    fn try_finally_statement_end_restores_ssa_nodes_from_try_block_when_it_is_sound_to_do_so() {
        let mut h = set_up();
        let x = Var::new("x");
        let y = Var::new("y");
        let x_ssa_at_end_of_try = Late::<SsaNode>::new();
        let y_ssa_at_end_of_try = Late::<SsaNode>::new();
        let x_ssa_at_end_of_finally = Late::<SsaNode>::new();
        let y_ssa_at_end_of_finally = Late::<SsaNode>::new();
        h.run(vec![
            declare_init(x, "int?"),
            declare_init(y, "int?"),
            try_(vec![
                x.write(expr("int?")),
                y.write(expr("int?")),
                get_ssa_nodes(move |nodes| {
                    x_ssa_at_end_of_try.set(nodes.get(x).unwrap());
                    y_ssa_at_end_of_try.set(nodes.get(y).unwrap());
                }),
            ])
            .finally_(vec![
                if_(expr("bool"), vec![x.write(expr("int?"))]),
                if_(expr("bool"), vec![y.write(expr("int?")), return_()]),
                get_ssa_nodes(move |nodes| {
                    x_ssa_at_end_of_finally.set(nodes.get(x).unwrap());
                    y_ssa_at_end_of_finally.set(nodes.get(y).unwrap());
                    assert!(
                        !x_ssa_at_end_of_finally
                            .get()
                            .ptr_eq(&x_ssa_at_end_of_try.get())
                    );
                    assert!(
                        !y_ssa_at_end_of_finally
                            .get()
                            .ptr_eq(&y_ssa_at_end_of_try.get())
                    );
                }),
            ]),
            // x's SSA node should still match what it was at the end of the
            // finally block, because it might have been written to.  But y
            // can't have been written to, because once we reach here, we know
            // that the finally block completed normally, and the write to y
            // always leads to the explicit return.  So y's SSA node should be
            // restored back to match that from the end of the try block.
            get_ssa_nodes(move |nodes| {
                expect_same(nodes, x, &x_ssa_at_end_of_finally.get());
                expect_same(nodes, y, &y_ssa_at_end_of_try.get());
            }),
        ]);
    }

    #[test]
    fn try_finally_statement_end_sets_unreachable_if_end_of_try_block_unreachable() {
        let mut h = set_up();
        h.run(vec![
            try_(vec![return_(), check_reachable(false)]).finally_(vec![check_reachable(true)]),
            check_reachable(false),
        ]);
    }

    #[test]
    fn try_finally_statement_end_sets_unreachable_if_end_of_finally_block_unreachable() {
        let mut h = set_up();
        h.run(vec![
            try_(vec![check_reachable(true)]).finally_(vec![return_(), check_reachable(false)]),
            check_reachable(false),
        ]);
    }

    #[test]
    fn try_finally_statement_end_handles_a_variable_declared_only_in_the_try_block() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![try_(vec![declare_init(x, "int?")]).finally_(vec![])]);
    }

    #[test]
    fn try_finally_statement_end_handles_a_variable_declared_only_in_the_finally_block() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![try_(vec![]).finally_(vec![declare_init(x, "int?")])]);
    }

    #[test]
    fn try_finally_statement_end_handles_a_variable_that_was_write_captured_in_the_try_block() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare_init(x, "int?"),
            try_(vec![local_function(vec![x.write(expr("int?"))])]).finally_(vec![]),
            if_(x.not_eq(null_literal()), vec![check_not_promoted(x)]),
        ]);
    }

    #[test]
    fn try_finally_statement_end_handles_a_variable_that_was_write_captured_in_the_finally_block() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare_init(x, "int?"),
            try_(vec![]).finally_(vec![local_function(vec![x.write(expr("int?"))])]),
            if_(x.not_eq(null_literal()), vec![check_not_promoted(x)]),
        ]);
    }

    #[test]
    fn try_finally_statement_end_handles_a_variable_that_was_promoted_in_the_try_block_and_write_captured_in_the_finally_block()
     {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare_init(x, "int?"),
            try_(vec![
                if_(x.eq(null_literal()), vec![return_()]),
                check_promoted(x, "int"),
            ])
            .finally_(vec![local_function(vec![x.write(expr("int?"))])]),
            // The capture in the `finally` cancels old promotions and prevents
            // future promotions.
            check_not_promoted(x),
            if_(x.not_eq(null_literal()), vec![check_not_promoted(x)]),
        ]);
    }

    #[test]
    fn try_finally_statement_end_keeps_promotions_from_both_try_and_finally_blocks_when_there_is_no_write_in_the_finally_block()
     {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare_init(x, "Object"),
            try_(vec![
                if_(x.is_not("num"), vec![return_()]),
                check_promoted(x, "num"),
            ])
            .finally_(vec![if_(x.is_not("int"), vec![return_()])]),
            // The promotion chain now contains both `num` and `int`.
            check_promoted(x, "int"),
            x.write(expr("num")),
            check_promoted(x, "num"),
        ]);
    }

    #[test]
    fn try_finally_statement_end_keeps_promotions_from_the_finally_block_when_there_is_a_write_in_the_finally_block()
     {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare_init(x, "Object"),
            try_(vec![
                if_(x.is_not("String"), vec![return_()]),
                check_promoted(x, "String"),
            ])
            .finally_(vec![
                x.write(expr("Object")),
                if_(x.is_not("int"), vec![return_()]),
            ]),
            check_promoted(x, "int"),
        ]);
    }

    #[test]
    fn try_finally_statement_end_keeps_tests_from_both_the_try_and_finally_blocks() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare_init(x, "Object"),
            try_(vec![if_(x.is_not("String"), vec![]), check_not_promoted(x)])
                .finally_(vec![if_(x.is_not("int"), vec![]), check_not_promoted(x)]),
            check_not_promoted(x),
            if_else(
                expr("bool"),
                vec![x.write(expr("String")), check_promoted(x, "String")],
                vec![x.write(expr("int")), check_promoted(x, "int")],
            ),
        ]);
    }

    #[test]
    fn try_finally_statement_end_handles_variables_not_definitely_assigned_in_either_the_try_or_finally_block()
     {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare_typed(x, "Object"),
            check_assigned(x, false),
            try_(vec![
                if_(expr("bool"), vec![x.write(expr("Object"))]),
                check_assigned(x, false),
            ])
            .finally_(vec![
                if_(expr("bool"), vec![x.write(expr("Object"))]),
                check_assigned(x, false),
            ]),
            check_assigned(x, false),
        ]);
    }

    #[test]
    fn try_finally_statement_end_handles_variables_definitely_assigned_in_the_try_block() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare_typed(x, "Object"),
            check_assigned(x, false),
            try_(vec![x.write(expr("Object")), check_assigned(x, true)]).finally_(vec![
                if_(expr("bool"), vec![x.write(expr("Object"))]),
                check_assigned(x, false),
            ]),
            check_assigned(x, true),
        ]);
    }

    #[test]
    fn try_finally_statement_end_handles_variables_definitely_assigned_in_the_finally_block() {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare_typed(x, "Object"),
            check_assigned(x, false),
            try_(vec![
                if_(expr("bool"), vec![x.write(expr("Object"))]),
                check_assigned(x, false),
            ])
            .finally_(vec![x.write(expr("Object")), check_assigned(x, true)]),
            check_assigned(x, true),
        ]);
    }

    #[test]
    fn try_finally_statement_end_handles_variables_definitely_unassigned_in_both_the_try_and_finally_blocks()
     {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare_typed(x, "Object"),
            check_unassigned(x, true),
            try_(vec![check_unassigned(x, true)]).finally_(vec![check_unassigned(x, true)]),
            check_unassigned(x, true),
        ]);
    }

    #[test]
    fn try_finally_statement_end_handles_variables_definitely_unassigned_in_the_try_but_not_the_finally_block()
     {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare_typed(x, "Object"),
            check_unassigned(x, true),
            try_(vec![check_unassigned(x, true)]).finally_(vec![
                if_(expr("bool"), vec![x.write(expr("Object"))]),
                check_unassigned(x, false),
            ]),
            check_unassigned(x, false),
        ]);
    }

    #[test]
    fn try_finally_statement_end_handles_variables_definitely_unassigned_in_the_finally_but_not_the_try_block()
     {
        let mut h = set_up();
        let x = Var::new("x");
        h.run(vec![
            declare_typed(x, "Object"),
            check_unassigned(x, true),
            try_(vec![
                if_(expr("bool"), vec![x.write(expr("Object"))]),
                check_unassigned(x, false),
            ])
            .finally_(vec![check_unassigned(x, false)]),
            check_unassigned(x, false),
        ]);
    }
}

/// Dart `x.notEq(nullLiteral).conditional(booleanLiteral(true),
/// y.notEq(nullLiteral).conditional(booleanLiteral(false),
/// throw_(expr('Object'))))`.
fn x_or_y_not_null(x: Var, y: Var) -> Node {
    x.not_eq(null_literal()).conditional(
        boolean_literal(true),
        y.not_eq(null_literal())
            .conditional(boolean_literal(false), throw_(expr("Object"))),
    )
}

#[test]
fn variable_read_restores_promotions_from_previous_write() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    let z = Var::new("z");
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        declare_init(z, "bool"),
        // Create a variable that promotes x if its value is true, and y if its
        // value is false.
        z.write(x_or_y_not_null(x, y)),
        check_not_promoted(x),
        check_not_promoted(y),
        // Simply reading the variable shouldn't promote anything.
        z.expr(),
        check_not_promoted(x),
        check_not_promoted(y),
        // But reading it in an "if" condition should promote.
        if_else(
            z,
            vec![check_promoted(x, "int"), check_not_promoted(y)],
            vec![check_not_promoted(x), check_promoted(y, "int")],
        ),
    ]);
}

#[test]
fn variable_read_restores_promotions_from_previous_initialization() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    let z = Var::new("z");
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        // Create a variable that promotes x if its value is true, and y if its
        // value is false.
        declare(z).with_initializer(x_or_y_not_null(x, y)),
        check_not_promoted(x),
        check_not_promoted(y),
        // Simply reading the variable shouldn't promote anything.
        z.expr(),
        check_not_promoted(x),
        check_not_promoted(y),
        // But reading it in an "if" condition should promote.
        if_else(
            z,
            vec![check_promoted(x, "int"), check_not_promoted(y)],
            vec![check_not_promoted(x), check_promoted(y, "int")],
        ),
    ]);
}

#[test]
fn variable_read_rebases_old_promotions() {
    let mut h = set_up();
    let w = Var::new("w");
    let x = Var::new("x");
    let y = Var::new("y");
    let z = Var::new("z");
    h.run(vec![
        declare_init(w, "int?"),
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        declare_init(z, "bool"),
        // Create a variable that promotes x if its value is true, and y if its
        // value is false.
        z.write(x_or_y_not_null(x, y)),
        check_not_promoted(w),
        check_not_promoted(x),
        check_not_promoted(y),
        w.non_null_assert(),
        check_promoted(w, "int"),
        // Reading the value of z in an "if" condition should promote x or y,
        // and keep the promotion of w.
        if_else(
            z,
            vec![
                check_promoted(w, "int"),
                check_promoted(x, "int"),
                check_not_promoted(y),
            ],
            vec![
                check_promoted(w, "int"),
                check_not_promoted(x),
                check_promoted(y, "int"),
            ],
        ),
    ]);
}

/// Dart `test("variableRead() doesn't restore the notion of whether a value
/// is null", ...)`.
#[test]
fn variable_read_doesnt_restore_the_notion_of_whether_a_value_is_null() {
    // Note: we have the available infrastructure to do this if we want, but
    // we think it will give an inconsistent feel because comparisons like
    // `if (i == null)` *don't* promote.

    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        y.write(null_literal()),
        check_not_promoted(x),
        check_not_promoted(y),
        if_else(
            x.eq(y),
            vec![check_not_promoted(x), check_not_promoted(y)],
            vec![
                // Even though x != y and y is known to contain the value `null`,
                // we don't promote x.
                check_not_promoted(x),
                check_not_promoted(y),
            ],
        ),
    ]);
}

#[test]
fn while_statement_condition_begin_un_promotes() {
    let mut h = set_up();
    let x = Var::new("x");
    let ssa_before_loop = Late::<SsaNode>::new();
    h.run(vec![
        declare_init(x, "int?"),
        x.as_("int"),
        check_promoted(x, "int"),
        get_ssa_nodes(move |nodes| ssa_before_loop.set(nodes.get(x).unwrap())),
        while_(
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
            vec![x.write(expr("Null"))],
        ),
    ]);
}

#[test]
fn while_statement_condition_begin_handles_write_captures_in_the_loop() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        x.as_("int"),
        check_promoted(x, "int"),
        while_(
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
            vec![],
        ),
    ]);
}

#[test]
fn while_statement_body_begin_promotes() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "int?"),
        while_(x.not_eq(null_literal()), vec![check_promoted(x, "int")]),
    ]);
}

#[test]
fn while_statement_end_joins_break_and_condition_false_states() {
    // To test that the states are properly joined, we have three variables:
    // x, y, and z.  We promote x and y in the break path, and x and z in the
    // condition-false path.  After the loop, only x should be promoted.

    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    let z = Var::new("z");
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        declare_init(z, "int?"),
        while_(
            x.eq(null_literal()).or(z.eq(null_literal())),
            vec![if_(
                expr("bool"),
                vec![x.as_("int"), y.as_("int"), break_(None)],
            )],
        ),
        check_promoted(x, "int"),
        check_not_promoted(y),
        check_not_promoted(z),
    ]);
}

#[test]
fn while_statement_end_with_break_updates_ssa_of_modified_vars() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    let x_ssa_inside_loop = Late::<SsaNode>::new();
    let y_ssa_inside_loop = Late::<SsaNode>::new();
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        while_(
            expr("bool"),
            vec![
                x.write(expr("int?")),
                if_(expr("bool"), vec![break_(None)]),
                get_ssa_nodes(move |nodes| {
                    x_ssa_inside_loop.set(nodes.get(x).unwrap());
                    y_ssa_inside_loop.set(nodes.get(y).unwrap());
                }),
            ],
        ),
        get_ssa_nodes(move |nodes| {
            // x's Ssa should have been changed because of the join at the end of
            // the loop.  y's should not, since it retains the value it had prior
            // to the loop.
            expect_not_same(nodes, x, &x_ssa_inside_loop.get());
            expect_same(nodes, y, &y_ssa_inside_loop.get());
        }),
    ]);
}

#[test]
fn while_statement_end_with_break_updates_ssa_of_modified_vars_when_types_were_tested() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    let x_ssa_inside_loop = Late::<SsaNode>::new();
    let y_ssa_inside_loop = Late::<SsaNode>::new();
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        while_(
            expr("bool"),
            vec![
                x.write(expr("int?")),
                if_(expr("bool"), vec![break_(None)]),
                if_(x.is_("int"), vec![]),
                get_ssa_nodes(move |nodes| {
                    x_ssa_inside_loop.set(nodes.get(x).unwrap());
                    y_ssa_inside_loop.set(nodes.get(y).unwrap());
                }),
            ],
        ),
        get_ssa_nodes(move |nodes| {
            // x's Ssa should have been changed because of the join at the end of
            // the loop.  y's should not, since it retains the value it had prior
            // to the loop.
            expect_not_same(nodes, x, &x_ssa_inside_loop.get());
            expect_same(nodes, y, &y_ssa_inside_loop.get());
        }),
    ]);
}

#[test]
fn write_de_promotes_and_updates_ssa_of_a_promoted_variable() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    let ssa_before_write = Late::<SsaNode>::new();
    let written_value_info = Late::<ExpressionInfo>::new();
    h.run(vec![
        declare_init(x, "Object"),
        declare_init(y, "int?"),
        x.as_("int"),
        check_promoted(x, "int"),
        get_ssa_nodes(move |nodes| ssa_before_write.set(nodes.get(x).unwrap())),
        x.write(y.eq(null_literal()).get_expression_info(move |info| {
            assert!(info.is_some());
            written_value_info.set(info.unwrap());
        })),
        check_not_promoted(x),
        get_ssa_nodes(move |nodes| {
            expect_not_same(nodes, x, &ssa_before_write.get());
            let state = nodes.get(x).unwrap().condition_variable_state.clone();
            assert!(
                state
                    .as_ref()
                    .is_some_and(|s| s.ptr_eq(&written_value_info.get()))
            );
        }),
    ]);
}

#[test]
fn write_updates_ssa() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    let ssa_before_write = Late::<SsaNode>::new();
    let written_value_info = Late::<ExpressionInfo>::new();
    h.run(vec![
        declare_init(x, "Object"),
        declare_init(y, "int?"),
        get_ssa_nodes(move |nodes| ssa_before_write.set(nodes.get(x).unwrap())),
        x.write(y.eq(null_literal()).get_expression_info(move |info| {
            assert!(info.is_some());
            written_value_info.set(info.unwrap());
        })),
        get_ssa_nodes(move |nodes| {
            expect_not_same(nodes, x, &ssa_before_write.get());
            let state = nodes.get(x).unwrap().condition_variable_state.clone();
            assert!(
                state
                    .as_ref()
                    .is_some_and(|s| s.ptr_eq(&written_value_info.get()))
            );
        }),
    ]);
}

#[test]
fn write_does_not_copy_ssa_from_one_variable_to_another() {
    // We could do so, and it would enable us to promote in slightly more
    // situations, e.g.:
    //   bool b = x != null;
    //   if (b) { /* x promoted here */ }
    //   var tmp = x;
    //   x = ...;
    //   if (b) { /* x not promoted here */ }
    //   x = tmp;
    //   if (b) { /* x promoted again */ }
    // But there are a lot of corner cases to test and it's not clear how much
    // the benefit will be, so for now we're not doing it.

    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    let x_ssa_before_write = Late::<SsaNode>::new();
    let y_ssa = Late::<SsaNode>::new();
    h.run(vec![
        declare_init(x, "int?"),
        declare_init(y, "int?"),
        get_ssa_nodes(move |nodes| {
            x_ssa_before_write.set(nodes.get(x).unwrap());
            y_ssa.set(nodes.get(y).unwrap());
        }),
        x.write(y),
        get_ssa_nodes(move |nodes| {
            expect_not_same(nodes, x, &x_ssa_before_write.get());
            expect_not_same(nodes, x, &y_ssa.get());
        }),
    ]);
}

#[test]
fn write_does_not_store_expression_info_for_trivial_expressions() {
    let mut h = set_up();
    let x = Var::new("x");
    let y = Var::new("y");
    let ssa_before_write = Late::<SsaNode>::new();
    h.run(vec![
        declare_init(x, "Object"),
        declare_init(y, "int?"),
        local_function(vec![y.write(expr("int?"))]),
        get_ssa_nodes(move |nodes| ssa_before_write.set(nodes.get(x).unwrap())),
        // `y == null` is a trivial expression because y has been write
        // captured.
        x.write(
            y.eq(null_literal())
                .get_expression_info(|info| assert!(info.is_some())),
        ),
        get_ssa_nodes(move |nodes| {
            expect_not_same(nodes, x, &ssa_before_write.get());
            assert!(nodes.get(x).unwrap().condition_variable_state.is_none());
        }),
    ]);
}

#[test]
fn infinite_loop_does_not_implicitly_assign_variables() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_typed(x, "int"),
        while_(boolean_literal(true), vec![x.write(expr("Null"))]),
        check_assigned(x, false),
    ]);
}

#[test]
fn if_false_does_not_discard_promotions() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "Object"),
        x.as_("int"),
        check_promoted(x, "int"),
        if_(boolean_literal(false), vec![check_promoted(x, "int")]),
    ]);
}

#[test]
fn promotions_do_not_occur_when_a_variable_is_write_captured() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "Object"),
        local_function(vec![x.write(expr("Object"))]),
        get_ssa_nodes(move |nodes| assert!(nodes.get(x).is_none())),
        x.as_("int"),
        check_not_promoted(x),
        get_ssa_nodes(move |nodes| assert!(nodes.get(x).is_none())),
    ]);
}

#[test]
fn promotion_cancellation_of_write_captured_vars_survives_join() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "Object"),
        if_else(
            expr("bool"),
            vec![local_function(vec![x.write(expr("Object"))])],
            vec![
                // Promotion should work here because the write capture is in the
                // other branch.
                x.as_("int"),
                check_promoted(x, "int"),
            ],
        ),
        // But the promotion should be cancelled now, after the join.
        check_not_promoted(x),
        // And further attempts to promote should fail due to the write capture.
        x.as_("int"),
        check_not_promoted(x),
    ]);
}

#[test]
fn issue_47991() {
    let mut h = set_up();
    let b = Var::new("b");
    let i = Var::new("i");
    h.run(vec![local_function(vec![
        declare(b)
            .with_declared_type("bool")
            .with_initializer(expr("bool").or(expr("bool"))),
        declare(i).with_final().with_declared_type("int"),
        if_else(
            b,
            vec![check_unassigned(i, true), i.write(expr("int"))],
            vec![check_unassigned(i, true), i.write(expr("int"))],
        ),
    ])]);
}
