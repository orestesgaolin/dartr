// Dart source: pkg/_fe_analyzer_shared/test/flow_analysis/flow_analysis_test.dart
// (lines 5826-7037: group 'Field promotion')

//! Dart group `Field promotion`.

use super::common::*;
use super::why_not_promoted::{expect_non_inherent_reason, expect_reason_keys, single_reason};

/// Dart `declare(x, type: type, initializer: expr(type))`.
#[track_caller]
fn declare_init(x: Var, type_: &str) -> Node {
    declare(x)
        .with_declared_type(type_)
        .with_initializer(expr(type_))
}

/// A cascade section (Dart `(v) => ...`).
fn section(f: impl Fn(Node) -> Node + 'static) -> Box<dyn Fn(Node) -> Node> {
    Box::new(f)
}

#[test]
fn promotable_field() {
    let mut h = set_up();
    h.add_member("C", "_field", Some("Object?"), true, None);
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "C"),
        if_(
            x.property("_field", false).eq(null_literal()),
            vec![return_()],
        ),
        check_promoted(x.property("_field", false), "Object"),
        x.property("_field", false).check_type("Object"),
    ]);
}

#[test]
fn promotable_field_this() {
    let mut h = set_up();
    h.set_this_type("C");
    h.add_member("C", "_field", Some("Object?"), true, None);
    h.run(vec![
        if_(this_property("_field").eq(null_literal()), vec![return_()]),
        check_promoted(this_property("_field"), "Object"),
        this_property("_field").check_type("Object"),
    ]);
}

#[test]
fn non_promotable_field() {
    let mut h = set_up();
    h.add_member("C", "_field", Some("Object?"), false, None);
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "C"),
        if_(
            x.property("_field", false).eq(null_literal()),
            vec![return_()],
        ),
        check_not_promoted(x.property("_field", false)),
        x.property("_field", false).check_type("Object?"),
    ]);
}

#[test]
fn non_promotable_field_this() {
    let mut h = set_up();
    h.set_this_type("C");
    h.add_member("C", "_field", Some("Object?"), false, None);
    h.run(vec![
        if_(this_property("_field").eq(null_literal()), vec![return_()]),
        check_not_promoted(this_property("_field")),
        this_property("_field").check_type("Object?"),
    ]);
}

#[test]
fn multiply_promoted() {
    let mut h = set_up();
    h.add_member("C", "_field", Some("Object?"), true, None);
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "C"),
        if_(
            x.property("_field", false).eq(null_literal()),
            vec![return_()],
        ),
        if_(x.property("_field", false).is_not("int"), vec![return_()]),
        check_promoted(x.property("_field", false), "int"),
        x.property("_field", false).check_type("int"),
    ]);
}

#[test]
fn multiply_promoted_this() {
    let mut h = set_up();
    h.set_this_type("C");
    h.add_member("C", "_field", Some("Object?"), true, None);
    h.run(vec![
        if_(this_property("_field").eq(null_literal()), vec![return_()]),
        if_(this_property("_field").is_not("int"), vec![return_()]),
        check_promoted(this_property("_field"), "int"),
        this_property("_field").check_type("int"),
    ]);
}

#[test]
fn promotion_of_target_breaks_field_promotion() {
    let mut h = set_up();
    h.add_member("B", "_field", Some("Object?"), true, None);
    h.add_member("C", "_field", Some("num?"), true, None);
    h.add_super_interfaces("C", |_| vec![ty("B"), ty("Object")]);
    h.add_super_interfaces("B", |_| vec![ty("Object")]);
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "B"),
        if_(
            x.property("_field", false).eq(null_literal()),
            vec![return_()],
        ),
        check_promoted(x.property("_field", false), "Object"),
        x.property("_field", false).check_type("Object"),
        if_(x.is_not("C"), vec![return_()]),
        check_not_promoted(x.property("_field", false)),
        x.property("_field", false).check_type("num?"),
    ]);
}

#[test]
fn promotion_of_target_does_not_break_field_promotion() {
    let mut h = set_up();
    h.add_member("B", "_field", Some("Object?"), true, None);
    h.add_member("C", "_field", Some("num?"), true, None);
    h.add_super_interfaces("C", |_| vec![ty("B"), ty("Object")]);
    h.add_super_interfaces("B", |_| vec![ty("Object")]);
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "B"),
        if_(x.property("_field", false).is_not("int"), vec![return_()]),
        check_promoted(x.property("_field", false), "int"),
        x.property("_field", false).check_type("int"),
        if_(x.is_not("C"), vec![return_()]),
        check_promoted(x.property("_field", false), "int"),
        x.property("_field", false).check_type("int"),
    ]);
}

#[test]
fn field_not_promotable_after_outer_variable_demoted() {
    let mut h = set_up();
    h.add_member("B", "_field", Some("Object?"), false, None);
    h.add_member("C", "_field", Some("Object?"), true, None);
    h.add_super_interfaces("C", |_| vec![ty("B"), ty("Object")]);
    h.add_super_interfaces("B", |_| vec![ty("Object")]);
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "B"),
        if_(
            x.is_("C"),
            vec![if_(
                x.property("_field", false).not_eq(null_literal()),
                vec![
                    check_promoted(x.property("_field", false), "Object"),
                    x.property("_field", false).check_type("Object"),
                ],
            )],
        ),
        if_(
            x.property("_field", false).not_eq(null_literal()),
            vec![
                check_not_promoted(x.property("_field", false)),
                x.property("_field", false).check_type("Object?"),
            ],
        ),
    ]);
}

#[test]
fn field_promotable_after_outer_variable_promoted() {
    let mut h = set_up();
    h.add_member("B", "_field", Some("Object?"), false, None);
    h.add_member("C", "_field", Some("Object?"), true, None);
    h.add_super_interfaces("C", |_| vec![ty("B"), ty("Object")]);
    h.add_super_interfaces("B", |_| vec![ty("Object")]);
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "B"),
        if_(
            x.property("_field", false).not_eq(null_literal()),
            vec![
                check_not_promoted(x.property("_field", false)),
                x.property("_field", false).check_type("Object?"),
            ],
        ),
        if_(
            x.is_("C"),
            vec![if_(
                x.property("_field", false).not_eq(null_literal()),
                vec![
                    check_promoted(x.property("_field", false), "Object"),
                    x.property("_field", false).check_type("Object"),
                ],
            )],
        ),
    ]);
}

#[test]
fn promotion_targets_properly_distinguished() {
    let mut h = set_up();
    h.set_this_type("C");
    h.add_member("C", "_field1", Some("Object?"), true, None);
    h.add_member("C", "_field2", Some("Object?"), true, None);
    let x = Var::new("x");
    let y = Var::new("y");
    h.run(vec![
        declare_init(x, "C"),
        declare_init(y, "C"),
        if_(this_property("_field1").is_not("String"), vec![return_()]),
        if_(
            this_().property("_field2", false).is_not("String?"),
            vec![return_()],
        ),
        if_(x.property("_field1", false).is_not("int"), vec![return_()]),
        if_(
            y.property("_field1", false).is_not("double"),
            vec![return_()],
        ),
        check_promoted(this_property("_field1"), "String"),
        this_property("_field1").check_type("String"),
        check_promoted(this_().property("_field1", false), "String"),
        this_().property("_field1", false).check_type("String"),
        check_promoted(this_property("_field2"), "String?"),
        this_property("_field2").check_type("String?"),
        check_promoted(this_().property("_field2", false), "String?"),
        this_().property("_field2", false).check_type("String?"),
        check_promoted(x.property("_field1", false), "int"),
        x.property("_field1", false).check_type("int"),
        check_not_promoted(x.property("_field2", false)),
        x.property("_field2", false).check_type("Object?"),
        check_promoted(y.property("_field1", false), "double"),
        y.property("_field1", false).check_type("double"),
        check_not_promoted(y.property("_field2", false)),
        y.property("_field2", false).check_type("Object?"),
    ]);
}

#[test]
fn cancelled_by_write_to_local_var() {
    let mut h = set_up();
    h.set_this_type("C");
    h.add_member("C", "_field", Some("Object?"), true, None);
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "C"),
        if_(
            x.property("_field", false).is_not("String"),
            vec![return_()],
        ),
        check_promoted(x.property("_field", false), "String"),
        x.property("_field", false).check_type("String"),
        x.write(expr("C")),
        check_not_promoted(x.property("_field", false)),
        x.property("_field", false).check_type("Object?"),
    ]);
}

#[test]
fn cancelled_by_write_to_local_var_nested() {
    let mut h = set_up();
    h.set_this_type("C");
    h.add_member("C", "_field1", Some("D"), true, None);
    h.add_member("D", "_field2", Some("Object?"), true, None);
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "C"),
        if_(
            x.property("_field1", false)
                .property("_field2", false)
                .is_not("String"),
            vec![return_()],
        ),
        check_promoted(
            x.property("_field1", false).property("_field2", false),
            "String",
        ),
        x.property("_field1", false)
            .property("_field2", false)
            .check_type("String"),
        x.write(expr("C")),
        check_not_promoted(x.property("_field1", false).property("_field2", false)),
        x.property("_field1", false)
            .property("_field2", false)
            .check_type("Object?"),
    ]);
}

#[test]
fn cancelled_by_write_to_local_var_later_in_loop() {
    let mut h = set_up();
    h.set_this_type("C");
    h.add_member("C", "_field", Some("Object?"), true, None);
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "C"),
        if_(
            x.property("_field", false).is_not("String"),
            vec![return_()],
        ),
        check_promoted(x.property("_field", false), "String"),
        x.property("_field", false).check_type("String"),
        while_(
            expr("bool"),
            vec![
                check_not_promoted(x.property("_field", false)),
                x.property("_field", false).check_type("Object?"),
                x.write(expr("C")),
            ],
        ),
    ]);
}

#[test]
fn cancelled_by_write_to_local_var_later_in_loop_nested() {
    let mut h = set_up();
    h.set_this_type("C");
    h.add_member("C", "_field1", Some("D"), true, None);
    h.add_member("D", "_field2", Some("Object?"), true, None);
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "C"),
        if_(
            x.property("_field1", false)
                .property("_field2", false)
                .is_not("String"),
            vec![return_()],
        ),
        check_promoted(
            x.property("_field1", false).property("_field2", false),
            "String",
        ),
        x.property("_field1", false)
            .property("_field2", false)
            .check_type("String"),
        while_(
            expr("bool"),
            vec![
                check_not_promoted(x.property("_field1", false).property("_field2", false)),
                x.property("_field1", false)
                    .property("_field2", false)
                    .check_type("Object?"),
                x.write(expr("C")),
            ],
        ),
    ]);
}

#[test]
fn cancelled_by_capture_of_local_var() {
    let mut h = set_up();
    h.set_this_type("C");
    h.add_member("C", "_field", Some("Object?"), true, None);
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "C"),
        if_(
            x.property("_field", false).is_not("String"),
            vec![return_()],
        ),
        check_promoted(x.property("_field", false), "String"),
        x.property("_field", false).check_type("String"),
        local_function(vec![x.write(expr("C"))]),
        check_not_promoted(x.property("_field", false)),
        x.property("_field", false).check_type("Object?"),
    ]);
}

#[test]
fn cancelled_by_capture_of_local_var_nested() {
    let mut h = set_up();
    h.set_this_type("C");
    h.add_member("C", "_field1", Some("D"), true, None);
    h.add_member("D", "_field2", Some("Object?"), true, None);
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "C"),
        if_(
            x.property("_field1", false)
                .property("_field2", false)
                .is_not("String"),
            vec![return_()],
        ),
        check_promoted(
            x.property("_field1", false).property("_field2", false),
            "String",
        ),
        x.property("_field1", false)
            .property("_field2", false)
            .check_type("String"),
        local_function(vec![x.write(expr("C"))]),
        check_not_promoted(x.property("_field1", false).property("_field2", false)),
        x.property("_field1", false)
            .property("_field2", false)
            .check_type("Object?"),
    ]);
}

#[test]
fn prevented_by_previous_capture_of_local_var() {
    let mut h = set_up();
    h.set_this_type("C");
    h.add_member("C", "_field", Some("Object?"), true, None);
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "C"),
        local_function(vec![x.write(expr("C"))]),
        if_(
            x.property("_field", false).is_not("String"),
            vec![return_()],
        ),
        check_not_promoted(x.property("_field", false)),
        x.property("_field", false).check_type("Object?"),
    ]);
}

#[test]
fn prevented_by_previous_capture_of_local_var_nested() {
    let mut h = set_up();
    h.set_this_type("C");
    h.add_member("C", "_field1", Some("D"), true, None);
    h.add_member("D", "_field2", Some("Object?"), true, None);
    let x = Var::new("x");
    h.run(vec![
        declare_init(x, "C"),
        local_function(vec![x.write(expr("C"))]),
        if_(
            x.property("_field1", false)
                .property("_field2", false)
                .is_not("String"),
            vec![return_()],
        ),
        check_not_promoted(x.property("_field1", false).property("_field2", false)),
        x.property("_field1", false)
            .property("_field2", false)
            .check_type("Object?"),
    ]);
}

#[test]
fn prevented_by_non_promotability_of_target() {
    let mut h = set_up();
    h.set_this_type("C");
    h.add_member("C", "_field1", Some("D"), false, None);
    h.add_member("D", "_field2", Some("Object?"), true, None);
    h.run(vec![
        if_(
            this_property("_field1")
                .property("_field2", false)
                .is_not("String"),
            vec![return_()],
        ),
        check_not_promoted(this_property("_field1").property("_field2", false)),
        this_property("_field1")
            .property("_field2", false)
            .check_type("Object?"),
    ]);
}

#[test]
fn super_tracked_separately() {
    // This test verifies that promotion of `this._field` and promotion of
    // `super._field` are tracked separately. This is necessary in case
    // `this._field` overrides `super._field` (and hence the two accesses
    // refer to different underlying fields).
    let mut h = set_up();
    h.set_this_type("C");
    h.add_member("C", "_field", Some("int?"), true, None);
    h.run(vec![
        if_(
            this_property("_field").not_eq(null_literal()),
            vec![
                check_promoted(this_property("_field"), "int"),
                this_().property("_field", false).check_type("int"),
                check_not_promoted(super_property("_field")),
            ],
        ),
        if_(
            super_property("_field").not_eq(null_literal()),
            vec![
                check_promoted(super_property("_field"), "int"),
                check_not_promoted(this_property("_field")),
                this_().property("_field", false).check_type("int?"),
            ],
        ),
    ]);
}

mod cascades {
    use super::*;

    mod not_null_aware {
        use super::*;

        #[test]
        fn cascaded_access_receives_the_benefit_of_promotion() {
            let mut h = set_up();
            h.add_member("C", "_field", Some("Object?"), true, None);
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("C")),
                x.property("_field", false).as_("int"),
                check_promoted(x.property("_field", false), "int"),
                x.cascade(
                    vec![
                        section(|v| v.property("_field", false).check_type("int")),
                        section(|v| v.property("_field", false).check_type("int")),
                    ],
                    false,
                ),
            ]);
        }

        #[test]
        fn field_access_on_cascade_expression_retains_promotion() {
            let mut h = set_up();
            h.add_member("C", "_field", Some("Object?"), true, None);
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("C")),
                x.property("_field", false).as_("int"),
                check_promoted(x.property("_field", false), "int"),
                x.cascade(
                    vec![section(|v| v.property("_field", false).check_type("int"))],
                    false,
                )
                .property("_field", false)
                .check_type("int"),
            ]);
        }

        #[test]
        fn a_cascade_expression_is_not_promotable() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("int?")),
                x.cascade(
                    vec![section(|v| v.invoke_method("toString", vec![], false))],
                    false,
                )
                .non_null_assert(),
                check_not_promoted(x),
            ]);
        }

        #[test]
        fn even_a_field_of_an_ephemeral_object_can_be_promoted() {
            let mut h = set_up();
            h.add_member("C", "_field", Some("int?"), true, None);
            h.run(vec![
                expr("C")
                    .cascade(
                        vec![
                            section(|v| {
                                v.property("_field", false)
                                    .check_type("int?")
                                    .non_null_assert()
                            }),
                            section(|v| v.property("_field", false).check_type("int")),
                        ],
                        false,
                    )
                    .property("_field", false)
                    .check_type("int"),
            ]);
        }

        #[test]
        fn even_a_field_of_a_write_captured_variable_can_be_promoted() {
            let mut h = set_up();
            h.add_member("C", "_field", Some("int?"), true, None);
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("C")),
                local_function(vec![x.write(expr("C"))]),
                x.cascade(
                    vec![
                        section(|v| {
                            v.property("_field", false)
                                .check_type("int?")
                                .non_null_assert()
                        }),
                        section(|v| v.property("_field", false).check_type("int")),
                    ],
                    false,
                )
                .property("_field", false)
                .check_type("int"),
            ]);
        }
    }

    mod null_aware {
        use super::*;

        #[test]
        fn cascaded_access_receives_the_benefit_of_promotion() {
            let mut h = set_up();
            h.add_member("C", "_field", Some("Object?"), true, None);
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("C")),
                x.property("_field", false).as_("int"),
                check_promoted(x.property("_field", false), "int"),
                x.cascade(
                    vec![
                        section(|v| v.property("_field", false).check_type("int")),
                        section(|v| v.property("_field", false).check_type("int")),
                    ],
                    true,
                ),
            ]);
        }

        #[test]
        fn field_access_on_cascade_expression_retains_promotion() {
            let mut h = set_up();
            h.add_member("C", "_field", Some("Object?"), true, None);
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("C")),
                x.property("_field", false).as_("int"),
                check_promoted(x.property("_field", false), "int"),
                x.cascade(
                    vec![section(|v| v.property("_field", false).check_type("int"))],
                    true,
                )
                .property("_field", false)
                .check_type("int"),
            ]);
        }

        #[test]
        fn a_cascade_expression_is_not_promotable() {
            let mut h = set_up();
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("int?")),
                x.cascade(
                    vec![section(|v| v.invoke_method("toString", vec![], false))],
                    true,
                )
                .non_null_assert(),
                check_not_promoted(x),
            ]);
        }

        #[test]
        fn even_a_field_of_an_ephemeral_object_can_be_promoted() {
            let mut h = set_up();
            h.add_member("C", "_field", Some("int?"), true, None);
            h.add_super_interfaces("C", |_| vec![ty("Object")]);
            h.run(vec![
                expr("C?")
                    .cascade(
                        vec![
                            section(|v| {
                                v.property("_field", false)
                                    .check_type("int?")
                                    .non_null_assert()
                            }),
                            section(|v| v.property("_field", false).check_type("int")),
                        ],
                        true,
                    )
                    // But the promotion doesn't survive beyond the cascade
                    // expression, because of the implicit control flow join implied
                    // by the null-awareness of the cascade. (In principle it would
                    // be sound to preserve the promotion, but it's extra work to do
                    // so, and it's not clear that there would be enough user
                    // benefit to justify the work).
                    .non_null_assert()
                    .property("_field", false)
                    .check_type("int?"),
            ]);
        }

        #[test]
        fn even_a_field_of_a_write_captured_variable_can_be_promoted() {
            let mut h = set_up();
            h.add_super_interfaces("C", |_| vec![ty("Object")]);
            h.add_member("C", "_field", Some("int?"), true, None);
            let x = Var::new("x");
            h.run(vec![
                declare(x).with_initializer(expr("C?")),
                local_function(vec![x.write(expr("C?"))]),
                x.cascade(
                    vec![
                        section(|v| {
                            v.property("_field", false)
                                .check_type("int?")
                                .non_null_assert()
                        }),
                        section(|v| v.property("_field", false).check_type("int")),
                    ],
                    true,
                )
                // But the promotion doesn't survive beyond the cascade
                // expression, because of the implicit control flow join implied
                // by the null-awareness of the cascade. (In principle it would
                // be sound to preserve the promotion, but it's extra work to do
                // so, and it's not clear that there would be enough user
                // benefit to justify the work).
                .non_null_assert()
                .property("_field", false)
                .check_type("int?"),
            ]);
        }
    }

    #[test]
    fn unstable_target() {
        let mut h = set_up();
        h.add_member("C", "d", Some("D"), false, None);
        h.add_member("D", "_i", Some("int?"), true, None);
        let c = Var::new("c");
        h.run(vec![
            declare(c).with_initializer(expr("C")),
            // The value of `c.d` is cached in a temporary variable, call it `t0`.
            c.property("d", false).cascade(
                vec![
                    // `t0._i` could be null at this point.
                    section(|t0| t0.property("_i", false).check_type("int?")),
                    // But now we promote it to non-null
                    section(|t0| t0.property("_i", false).non_null_assert()),
                    // And the promotion sticks for the duration of the cascade.
                    section(|t0| t0.property("_i", false).check_type("int")),
                ],
                false,
            ),
            // Now, a new value of `c.d` is computed, and cached in a new
            // temporary variable, call it `t1`.
            c.property("d", false).cascade(
                vec![
                    // even though `t0._i` was promoted above, `t1._i` could still be
                    // null at this point.
                    section(|t1| t1.property("_i", false).check_type("int?")),
                    // But now we promote it to non-null
                    section(|t1| t1.property("_i", false).non_null_assert()),
                    // And the promotion sticks for the duration of the cascade.
                    section(|t1| t1.property("_i", false).check_type("int")),
                ],
                false,
            ),
        ]);
    }
}

#[test]
fn field_becomes_promotable_after_type_test() {
    // In this test, `C._property` is not promotable, but `D` extends `C`, and
    // `D._property` is promotable. (This could happen if, for example,
    // `C._property` is an abstract getter, and `D._property` is a final
    // field). If `_property` is type-tested while the type of the target is
    // `C`, but then `_property` is accessed while the type of the target is
    // `D`, no promotion occurs, because the thing that is type tested is
    // non-promotable.
    let mut h = set_up();
    h.add_member("C", "_property", Some("int?"), false, None);
    h.add_member("D", "_property", Some("int?"), true, None);
    h.add_super_interfaces("C", |_| vec![ty("Object")]);
    h.add_super_interfaces("D", |_| vec![ty("C"), ty("Object")]);
    let x = Var::new("x");
    h.run(vec![
        declare(x).with_initializer(expr("C")),
        x.property("_property", false).non_null_assert(),
        x.as_("D"),
        check_not_promoted(x.property("_property", false)),
        x.property("_property", false).non_null_assert(),
        check_promoted(x.property("_property", false), "int"),
    ]);
}

mod preserved_by_join {
    use super::*;

    #[test]
    fn property() {
        let mut h = set_up();
        h.add_member("C", "_field", Some("int?"), true, None);
        let x = Var::new("x");
        // Even though the two branches of the "if" assign different values to
        // `x` (and hence the SSA nodes associated with `x._field` in the two
        // branches are different), the promotion is still preserved by the
        // join.
        h.run(vec![
            declare(x).with_declared_type("C"),
            if_else(
                expr("bool"),
                vec![
                    x.write(expr("C")),
                    x.property("_field", false).non_null_assert(),
                ],
                vec![
                    x.write(expr("C")),
                    x.property("_field", false).non_null_assert(),
                ],
            ),
            check_promoted(x.property("_field", false), "int"),
        ]);
    }

    #[test]
    fn property_of_property() {
        let mut h = set_up();
        h.add_member("C", "_i", Some("int?"), true, None);
        h.add_member("D", "_c", Some("C"), true, None);
        let x = Var::new("x");
        // Even though the two branches of the "if" assign different values to
        // `x` (and hence the SSA nodes associated with `x._c._i` in the two
        // branches are different), the promotion is still preserved by the
        // join.
        h.run(vec![
            declare(x).with_declared_type("D"),
            if_else(
                expr("bool"),
                vec![
                    x.write(expr("D")),
                    x.property("_c", false)
                        .property("_i", false)
                        .non_null_assert(),
                ],
                vec![
                    x.write(expr("D")),
                    x.property("_c", false)
                        .property("_i", false)
                        .non_null_assert(),
                ],
            ),
            check_promoted(x.property("_c", false).property("_i", false), "int"),
        ]);
    }

    #[test]
    fn property_promoted_only_in_first_joined_control_flow_path() {
        let mut h = set_up();
        h.add_member("C", "_field", Some("int?"), true, None);
        let x = Var::new("x");
        // No promotion because the property is only promoted in one control
        // flow path.
        h.run(vec![
            declare(x).with_declared_type("C"),
            if_else(
                expr("bool"),
                vec![
                    x.write(expr("C")),
                    x.property("_field", false).non_null_assert(),
                ],
                vec![x.write(expr("C")), x.property("_field", false)],
            ),
            check_not_promoted(x.property("_field", false)),
        ]);
    }

    #[test]
    fn property_promoted_only_in_second_joined_control_flow_path() {
        let mut h = set_up();
        h.add_member("C", "_field", Some("int?"), true, None);
        let x = Var::new("x");
        // No promotion because the property is only promoted in one control
        // flow path.
        h.run(vec![
            declare(x).with_declared_type("C"),
            if_else(
                expr("bool"),
                vec![x.write(expr("C")), x.property("_field", false)],
                vec![
                    x.write(expr("C")),
                    x.property("_field", false).non_null_assert(),
                ],
            ),
            check_not_promoted(x.property("_field", false)),
        ]);
    }

    #[test]
    fn property_accessed_only_in_first_joined_control_flow_path() {
        let mut h = set_up();
        h.add_member("C", "_field", Some("int?"), true, None);
        let x = Var::new("x");
        // No promotion because the property is only promoted in one control
        // flow path.
        h.run(vec![
            declare(x).with_declared_type("C"),
            if_else(
                expr("bool"),
                vec![
                    x.write(expr("C")),
                    x.property("_field", false).non_null_assert(),
                ],
                vec![x.write(expr("C"))],
            ),
            check_not_promoted(x.property("_field", false)),
        ]);
    }

    #[test]
    fn property_accessed_only_in_second_joined_control_flow_path() {
        let mut h = set_up();
        h.add_member("C", "_field", Some("int?"), true, None);
        let x = Var::new("x");
        // No promotion because the property is only promoted in one control
        // flow path.
        h.run(vec![
            declare(x).with_declared_type("C"),
            if_else(
                expr("bool"),
                vec![x.write(expr("C"))],
                vec![
                    x.write(expr("C")),
                    x.property("_field", false).non_null_assert(),
                ],
            ),
            check_not_promoted(x.property("_field", false)),
        ]);
    }
}

// In a try/finally statement, the `finally` clause is analyzed as though
// the `try` block hasn't executed yet (and any variables written inside
// the `try` block have been de-promoted), to account for the fact that
// an exception might occur at any time during the `try` block. However,
// after the `finally` block is finished, any flow model changes that
// occurred during the `finally` block are rewound and re-applied to the
// flow model state after the `try` block, to account for the fact that
// if the try/finally statement completes normally, it is known that the
// `try` block executed fully.
//
// We need to verify that this rebasing logic handles all the possible
// ways that field promotion can occur relative to a try/finally
// statement.
mod in_try_finally {
    use super::*;

    #[test]
    fn promoted_in_try() {
        let mut h = set_up();
        h.add_member("C", "_property", Some("int?"), true, None);
        let c = Var::new("c");
        h.run(vec![
            declare(c).with_initializer(expr("C")),
            try_(vec![
                check_not_promoted(c.property("_property", false)),
                c.property("_property", false).non_null_assert(),
                check_promoted(c.property("_property", false), "int"),
            ])
            .finally_(vec![check_not_promoted(c.property("_property", false))]),
            check_promoted(c.property("_property", false), "int"),
        ]);
    }

    #[test]
    fn promoted_in_try_nested() {
        let mut h = set_up();
        h.add_member("C", "_i", Some("int?"), true, None);
        h.add_member("D", "_c", Some("C"), true, None);
        let d = Var::new("d");
        h.run(vec![
            declare(d).with_initializer(expr("D")),
            try_(vec![
                check_not_promoted(d.property("_c", false).property("_i", false)),
                d.property("_c", false)
                    .property("_i", false)
                    .non_null_assert(),
                check_promoted(d.property("_c", false).property("_i", false), "int"),
            ])
            .finally_(vec![check_not_promoted(
                d.property("_c", false).property("_i", false),
            )]),
            check_promoted(d.property("_c", false).property("_i", false), "int"),
        ]);
    }

    #[test]
    fn promoted_before_try_finally() {
        let mut h = set_up();
        h.add_member("C", "_property", Some("int?"), true, None);
        let c = Var::new("c");
        h.run(vec![
            declare(c).with_initializer(expr("C")),
            c.property("_property", false).non_null_assert(),
            check_promoted(c.property("_property", false), "int"),
            try_(vec![check_promoted(c.property("_property", false), "int")])
                .finally_(vec![check_promoted(c.property("_property", false), "int")]),
            check_promoted(c.property("_property", false), "int"),
        ]);
    }

    #[test]
    fn promoted_before_try_finally_and_in_try() {
        let mut h = set_up();
        h.add_member("C", "_property", Some("num?"), true, None);
        let c = Var::new("c");
        h.run(vec![
            declare(c).with_initializer(expr("C")),
            c.property("_property", false).non_null_assert(),
            check_promoted(c.property("_property", false), "num"),
            try_(vec![
                check_promoted(c.property("_property", false), "num"),
                c.property("_property", false).as_("int"),
                check_promoted(c.property("_property", false), "int"),
            ])
            .finally_(vec![check_promoted(c.property("_property", false), "num")]),
            check_promoted(c.property("_property", false), "int"),
        ]);
    }

    mod promoted_in_both_try_and_finally {
        use super::*;

        #[test]
        fn same_type() {
            let mut h = set_up();
            h.add_member("C", "_property", Some("int?"), true, None);
            let c = Var::new("c");
            h.run(vec![
                declare(c).with_initializer(expr("C")),
                try_(vec![
                    check_not_promoted(c.property("_property", false)),
                    c.property("_property", false).non_null_assert(),
                    check_promoted(c.property("_property", false), "int"),
                ])
                .finally_(vec![
                    check_not_promoted(c.property("_property", false)),
                    c.property("_property", false).non_null_assert(),
                    check_promoted(c.property("_property", false), "int"),
                ]),
                check_promoted(c.property("_property", false), "int"),
            ]);
        }

        #[test]
        fn finally_type_is_subtype_of_try_type() {
            let mut h = set_up();
            h.add_member("C", "_property", Some("num?"), true, None);
            let c = Var::new("c");
            h.run(vec![
                declare(c).with_initializer(expr("C")),
                try_(vec![
                    check_not_promoted(c.property("_property", false)),
                    c.property("_property", false).non_null_assert(),
                    check_promoted(c.property("_property", false), "num"),
                ])
                .finally_(vec![
                    check_not_promoted(c.property("_property", false)),
                    c.property("_property", false).as_("int"),
                    check_promoted(c.property("_property", false), "int"),
                ]),
                check_promoted(c.property("_property", false), "int"),
            ]);
        }

        #[test]
        fn finally_type_is_supertype_of_try_type() {
            let mut h = set_up();
            h.add_member("C", "_property", Some("num?"), true, None);
            let c = Var::new("c");
            h.run(vec![
                declare(c).with_initializer(expr("C")),
                try_(vec![
                    check_not_promoted(c.property("_property", false)),
                    c.property("_property", false).as_("int"),
                    check_promoted(c.property("_property", false), "int"),
                ])
                .finally_(vec![
                    check_not_promoted(c.property("_property", false)),
                    c.property("_property", false).non_null_assert(),
                    check_promoted(c.property("_property", false), "num"),
                ]),
                check_promoted(c.property("_property", false), "int"),
            ]);
        }
    }

    #[test]
    fn promoted_in_finally() {
        let mut h = set_up();
        h.add_member("C", "_property", Some("int?"), true, None);
        let c = Var::new("c");
        h.run(vec![
            declare(c).with_initializer(expr("C")),
            try_(vec![check_not_promoted(c.property("_property", false))]).finally_(vec![
                check_not_promoted(c.property("_property", false)),
                c.property("_property", false).non_null_assert(),
                check_promoted(c.property("_property", false), "int"),
            ]),
            check_promoted(c.property("_property", false), "int"),
        ]);
    }

    #[test]
    fn promoted_in_finally_nested() {
        let mut h = set_up();
        h.add_member("C", "_i", Some("int?"), true, None);
        h.add_member("D", "_c", Some("C"), true, None);
        let d = Var::new("d");
        h.run(vec![
            declare(d).with_initializer(expr("D")),
            try_(vec![check_not_promoted(
                d.property("_c", false).property("_i", false),
            )])
            .finally_(vec![
                check_not_promoted(d.property("_c", false).property("_i", false)),
                d.property("_c", false)
                    .property("_i", false)
                    .non_null_assert(),
                check_promoted(d.property("_c", false).property("_i", false), "int"),
            ]),
            check_promoted(d.property("_c", false).property("_i", false), "int"),
        ]);
    }

    #[test]
    fn promoted_before_try_finally_assigned_in_try() {
        let mut h = set_up();
        h.add_member("C", "_property", Some("int?"), true, None);
        let c = Var::new("c");
        h.run(vec![
            declare(c).with_initializer(expr("C")),
            c.property("_property", false).non_null_assert(),
            check_promoted(c.property("_property", false), "int"),
            try_(vec![
                check_promoted(c.property("_property", false), "int"),
                c.write(expr("C")),
                check_not_promoted(c.property("_property", false)),
            ])
            .finally_(vec![check_not_promoted(c.property("_property", false))]),
            check_not_promoted(c.property("_property", false)),
        ]);
    }

    #[test]
    fn promoted_before_try_finally_assigned_and_re_promoted_in_try() {
        let mut h = set_up();
        h.add_member("C", "_property", Some("int?"), true, None);
        let c = Var::new("c");
        h.run(vec![
            declare(c).with_initializer(expr("C")),
            c.property("_property", false).non_null_assert(),
            check_promoted(c.property("_property", false), "int"),
            try_(vec![
                check_promoted(c.property("_property", false), "int"),
                c.write(expr("C")),
                c.property("_property", false).non_null_assert(),
                check_promoted(c.property("_property", false), "int"),
            ])
            .finally_(vec![check_not_promoted(c.property("_property", false))]),
            check_promoted(c.property("_property", false), "int"),
        ]);
    }

    #[test]
    fn assigned_in_try_promoted_in_finally() {
        let mut h = set_up();
        h.add_member("C", "_property", Some("int?"), true, None);
        let c = Var::new("c");
        h.run(vec![
            declare(c).with_initializer(expr("C")),
            try_(vec![
                // Note: no calls to `checkNotPromoted` here, because we want to
                // trigger the code path where flow analysis doesn't even know about
                // the property until the finally block
                c.write(expr("C")),
            ])
            .finally_(vec![
                c.property("_property", false).non_null_assert(),
                check_promoted(c.property("_property", false), "int"),
            ]),
            check_promoted(c.property("_property", false), "int"),
        ]);
    }

    #[test]
    fn assigned_in_try_promoted_in_finally_nested() {
        let mut h = set_up();
        h.add_member("C", "_i", Some("int?"), true, None);
        h.add_member("D", "_c", Some("C"), true, None);
        let d = Var::new("d");
        h.run(vec![
            declare(d).with_initializer(expr("D")),
            try_(vec![
                // Note: no calls to `checkNotPromoted` here, because we want to
                // trigger the code path where flow analysis doesn't even know about
                // the property until the finally block
                d.write(expr("D")),
            ])
            .finally_(vec![
                d.property("_c", false)
                    .property("_i", false)
                    .non_null_assert(),
                check_promoted(d.property("_c", false).property("_i", false), "int"),
            ]),
            check_promoted(d.property("_c", false).property("_i", false), "int"),
        ]);
    }

    #[test]
    fn assigned_but_not_promotable_in_try_promoted_in_finally() {
        let mut h = set_up();
        h.add_member("C", "_property", Some("int?"), false, None);
        h.add_member("D", "_property", Some("int?"), true, None);
        h.add_super_interfaces("C", |_| vec![ty("Object")]);
        h.add_super_interfaces("D", |_| vec![ty("C"), ty("Object")]);
        let c = Var::new("c");
        h.run(vec![
            declare(c).with_initializer(expr("C")),
            try_(vec![
                c.write(expr("C")),
                c.property("_property", false).non_null_assert(),
                check_not_promoted(c.property("_property", false)),
            ])
            .finally_(vec![
                c.as_("D"),
                c.property("_property", false).non_null_assert(),
                check_promoted(c.property("_property", false), "int"),
            ]),
            check_promoted(c.property("_property", false), "int"),
        ]);
    }

    #[test]
    fn assigned_and_promoted_in_try_promoted_to_subtype_in_finally() {
        let mut h = set_up();
        h.add_member("C", "_property", Some("Object"), true, None);
        let c = Var::new("c");
        h.run(vec![
            declare(c).with_initializer(expr("C")),
            try_(vec![
                check_not_promoted(c.property("_property", false)),
                c.write(expr("C")),
                check_not_promoted(c.property("_property", false)),
                c.property("_property", false).as_("num"),
                check_promoted(c.property("_property", false), "num"),
            ])
            .finally_(vec![
                c.property("_property", false).as_("int"),
                check_promoted(c.property("_property", false), "int"),
            ]),
            check_promoted(c.property("_property", false), "int"),
        ]);
    }
}

mod via_local_condition_variable {
    use super::*;

    // These tests exercise the code path in `FlowModel.rebaseForward` where
    // `this` model (which represents the state captured at the time the
    // condition variable is written) contains a promotion key for the
    // field, but the `base` model (which represents state just prior to
    // reading from the condition variable) doesn't contain any promotion
    // key for the field. Furthermore, since no other promotions occur
    // between writing and reading the condition variable, `rebaseForward`
    // will not create a fresh `FlowModel`; it will simply return `this`
    // model.
    mod without_intervening_promotion {
        use super::*;

        #[test]
        fn using_null_check() {
            let mut h = set_up();
            h.add_member("C", "_field", Some("int?"), true, None);
            let c = Var::new("c");
            let b = Var::new("b");
            h.run(vec![
                declare(c).with_initializer(expr("C")),
                declare(b).with_initializer(c.property("_field", false).not_eq(null_literal())),
                if_(b, vec![check_promoted(c.property("_field", false), "int")]),
            ]);
        }

        #[test]
        fn using_is_test() {
            // Dart name: 'using `is` test'.
            let mut h = set_up();
            h.add_member("C", "_field", Some("Object"), true, None);
            h.add_super_interfaces("C", |_| vec![ty("Object")]);
            let c = Var::new("c");
            let b = Var::new("b");
            h.run(vec![
                declare(c).with_initializer(expr("C")),
                declare(b).with_initializer(c.property("_field", false).is_("int")),
                if_(b, vec![check_promoted(c.property("_field", false), "int")]),
            ]);
        }
    }

    // These tests exercise the code path in `FlowModel.rebaseForward` where
    // `this` model (which represents the state captured at the time the
    // condition variable is written) and the `base` model (which represents
    // state just prior to reading from the condition variable) both contain
    // a promotion key for the field.
    mod with_intervening_related_promotion {
        use super::*;

        #[test]
        fn using_null_check() {
            let mut h = set_up();
            h.add_member("C", "_field", Some("int?"), true, None);
            let c = Var::new("c");
            let b = Var::new("b");
            h.run(vec![
                declare(c).with_initializer(expr("C")),
                declare(b).with_initializer(c.property("_field", false).not_eq(null_literal())),
                if_(
                    c.property("_field", false).not_eq(null_literal()),
                    vec![check_promoted(c.property("_field", false), "int")],
                ),
                if_(b, vec![check_promoted(c.property("_field", false), "int")]),
            ]);
        }

        #[test]
        fn using_is_test() {
            // Dart name: 'using `is` test'.
            let mut h = set_up();
            h.add_member("C", "_field", Some("Object"), true, None);
            h.add_super_interfaces("C", |_| vec![ty("Object")]);
            let c = Var::new("c");
            let b = Var::new("b");
            h.run(vec![
                declare(c).with_initializer(expr("C")),
                declare(b).with_initializer(c.property("_field", false).is_("int")),
                if_(
                    c.property("_field", false).is_("int"),
                    vec![check_promoted(c.property("_field", false), "int")],
                ),
                if_(b, vec![check_promoted(c.property("_field", false), "int")]),
            ]);
        }
    }

    // These tests exercise the code path in `FlowModel.rebaseForward` where
    // `this` model (which represents the state captured at the time the
    // condition variable is written) contains a promotion key for the
    // field, but the `base` model (which represents state just prior to
    // reading from the condition variable) doesn't contain any promotion
    // key for the field. Since a different variable is promoted in between
    // writing and reading the condition variable, `rebaseForward` will be
    // forced to create a fresh `FlowModel`; it will not be able to simply
    // return `this` model.
    mod with_intervening_unrelated_promotion {
        use super::*;

        #[test]
        fn using_null_check() {
            let mut h = set_up();
            h.add_member("C", "_field", Some("int?"), true, None);
            let c = Var::new("c");
            let unrelated = Var::new("unrelated");
            let b = Var::new("b");
            h.run(vec![
                declare(c).with_initializer(expr("C")),
                declare(unrelated).with_initializer(expr("int?")),
                declare(b).with_initializer(c.property("_field", false).not_eq(null_literal())),
                unrelated.non_null_assert(),
                if_(b, vec![check_promoted(c.property("_field", false), "int")]),
            ]);
        }

        #[test]
        fn using_is_test() {
            // Dart name: 'using `is` test'.
            let mut h = set_up();
            h.add_member("C", "_field", Some("Object"), true, None);
            h.add_super_interfaces("C", |_| vec![ty("Object")]);
            let c = Var::new("c");
            let unrelated = Var::new("unrelated");
            let b = Var::new("b");
            h.run(vec![
                declare(c).with_initializer(expr("C")),
                declare(unrelated).with_initializer(expr("int?")),
                declare(b).with_initializer(c.property("_field", false).is_("int")),
                unrelated.non_null_assert(),
                if_(b, vec![check_promoted(c.property("_field", false), "int")]),
            ]);
        }
    }

    mod disabled_by_intervening_assignment {
        use super::*;

        #[test]
        fn using_null_check() {
            let mut h = set_up();
            h.add_member("C", "_field", Some("int?"), true, None);
            let c = Var::new("c");
            let b = Var::new("b");
            h.run(vec![
                declare(c).with_initializer(expr("C")),
                declare(b).with_initializer(c.property("_field", false).not_eq(null_literal())),
                if_(
                    c.property("_field", false).not_eq(null_literal()),
                    vec![check_promoted(c.property("_field", false), "int")],
                ),
                c.write(expr("C")),
                if_(b, vec![check_not_promoted(c.property("_field", false))]),
            ]);
        }

        #[test]
        fn using_is_test() {
            // Dart name: 'using `is` test'.
            let mut h = set_up();
            h.add_member("C", "_field", Some("Object"), true, None);
            h.add_super_interfaces("C", |_| vec![ty("Object")]);
            let c = Var::new("c");
            let b = Var::new("b");
            h.run(vec![
                declare(c).with_initializer(expr("C")),
                declare(b).with_initializer(c.property("_field", false).is_("int")),
                if_(
                    c.property("_field", false).is_("int"),
                    vec![check_promoted(c.property("_field", false), "int")],
                ),
                c.write(expr("C")),
                if_(b, vec![check_not_promoted(c.property("_field", false))]),
            ]);
        }
    }
}

mod and_object_pattern {
    use super::*;

    #[test]
    fn promotion_via_object_promotion() {
        let mut h = set_up();
        h.add_member("C", "_property", Some("int?"), true, None);
        h.add_downward_infer("C", "C", "C");
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("C")),
            if_case(
                x,
                object_pattern(
                    "C",
                    vec![wildcard().null_check().record_field(Some("_property"))],
                ),
                vec![check_promoted(x.property("_property", false), "int")],
                Some(vec![check_not_promoted(x.property("_property", false))]),
            ),
        ]);
    }

    #[test]
    fn scrutinee_restored_after_object_pattern() {
        let mut h = set_up();
        h.add_member("C", "_property", Some("int?"), true, None);
        h.add_downward_infer("C", "C?", "C");
        h.add_super_interfaces("C", |_| vec![ty("Object")]);
        let x = Var::new("x");
        h.run(vec![
            declare(x).with_initializer(expr("C?")),
            if_case(
                x,
                object_pattern(
                    "C",
                    vec![wildcard().null_check().record_field(Some("_property"))],
                )
                .or(
                    // After visiting the object pattern, the scrutinee should now
                    // be restored to point to the `x`, so this null check should
                    // promote `x` to `C`.
                    wildcard().null_check(),
                ),
                vec![check_promoted(x, "C")],
                Some(vec![check_not_promoted(x)]),
            ),
        ]);
    }

    #[test]
    fn subpattern_matched_value_type_accounts_for_previous_promotion() {
        let mut h = set_up();
        h.add_member("C", "_property", Some("int?"), true, None);
        h.add_downward_infer("C", "C", "C");
        let x = Var::new("x");
        let y = Var::new("y");
        h.run(vec![
            declare(x).with_initializer(expr("C")),
            x.property("_property", false).non_null_assert(),
            check_promoted(x.property("_property", false), "int"),
            if_case(
                x,
                object_pattern(
                    "C",
                    vec![
                        y.pattern()
                            .with_expect_inferred_type("int")
                            .record_field(Some("_property")),
                    ],
                ),
                vec![],
                None,
            ),
        ]);
    }
}

mod non_promotion_reasons {
    use super::*;

    #[test]
    fn inherent_reason() {
        // It's only necessary to test one of the inherent reasons, because flow
        // analysis just passes it through.
        let mut h = set_up();
        h.set_this_type("C");
        h.add_member(
            "C",
            "_field",
            Some("Object?"),
            false,
            Some(PropertyNonPromotabilityReason::IsNotFinal),
        );
        h.run(vec![
            if_(this_property("_field").eq(null_literal()), vec![return_()]),
            this_property("_field").why_not_promoted(|reasons| {
                expect_reason_keys(&reasons, &["Object"]);
                match single_reason(&reasons) {
                    NonPromotionReason::PropertyNotPromotedForInherentReason {
                        why_not_promotable,
                        ..
                    } => assert_eq!(
                        *why_not_promotable,
                        PropertyNonPromotabilityReason::IsNotFinal
                    ),
                    r => panic!("expected PropertyNotPromotedForInherentReason, got {r:?}"),
                }
            }),
        ]);
    }

    #[test]
    fn due_to_conflict() {
        let mut h = set_up();
        h.set_this_type("C");
        h.add_member("C", "_field", Some("Object?"), false, None);
        h.run(vec![
            if_(this_property("_field").eq(null_literal()), vec![return_()]),
            this_property("_field").why_not_promoted(|reasons| {
                expect_reason_keys(&reasons, &["Object"]);
                expect_non_inherent_reason(single_reason(&reasons), true);
            }),
        ]);
    }
}

mod and_equality {
    use super::*;

    #[test]
    fn promoted_type_accounted_for_on_lhs() {
        // Flow analysis understands when an `if` test is guaranteed to succeed
        // (or fail) based on the static types of the LHS and RHS. Make sure
        // this works when the LHS or RHS is a property reference.
        let mut h = set_up();
        h.add_member("C", "f", Some("Object?"), true, None);
        h.set_this_type("C");
        h.run(vec![
            if_(this_property("f").is_not("Null"), vec![return_()]),
            check_promoted(this_property("f"), "Null"),
            if_else(
                this_property("f").eq(null_literal()),
                vec![check_reachable(true)],
                vec![check_reachable(false)],
            ),
        ]);
    }

    #[test]
    fn promoted_type_accounted_for_on_rhs() {
        // Flow analysis understands when an `if` test is guaranteed to succeed
        // (or fail) based on the static types of the LHS and RHS. Make sure
        // this works when the LHS or RHS is a property reference.
        let mut h = set_up();
        h.add_member("C", "f", Some("Object?"), true, None);
        h.set_this_type("C");
        h.run(vec![
            if_(this_property("f").is_not("Null"), vec![return_()]),
            check_promoted(this_property("f"), "Null"),
            if_else(
                null_literal().eq(this_property("f")),
                vec![check_reachable(true)],
                vec![check_reachable(false)],
            ),
        ]);
    }
}
