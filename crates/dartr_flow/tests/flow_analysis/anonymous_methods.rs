// Dart source: pkg/_fe_analyzer_shared/test/flow_analysis/flow_analysis_test.dart
// (lines 13066-13303: group 'Anonymous methods:')

//! Dart group `Anonymous methods:`.

use super::common::*;

#[test]
fn nested_return_targets() {
    let mut h = set_up();
    let branch1 = Var::new("branch1");
    let branch2 = Var::new("branch2");
    let branch3 = Var::new("branch3");
    h.run(vec![
        declare(branch1),
        declare(branch2),
        declare(branch3),
        expr("int").invoke_anonymous_method(
            vec![
                if_(expr("bool"), vec![branch1.write(expr("int")), return_()]), // (1)
                expr("int").invoke_anonymous_method(
                    vec![
                        if_(expr("bool"), vec![branch2.write(expr("int")), return_()]), // (2)
                    ],
                    "void",
                    false,
                    true,
                    None,
                ),
                // (2) jumps to here, but not (1) or (3)
                check_unassigned(branch1, true),
                check_unassigned(branch2, false),
                check_unassigned(branch3, true),
                if_(expr("bool"), vec![branch3.write(expr("int")), return_()]), // (3)
            ],
            "void",
            false,
            true,
            None,
        ),
        // (1) and (3) jump to here
        check_unassigned(branch1, false),
        check_unassigned(branch2, false),
        check_unassigned(branch3, false),
    ]);
}

#[test]
fn function_expression_inside_an_anonymous_method() {
    let mut h = set_up();
    let branch1 = Var::new("branch1");
    let branch2 = Var::new("branch2");
    let branch3 = Var::new("branch3");
    h.run(vec![
        declare(branch1),
        declare(branch2),
        declare(branch3),
        expr("int").invoke_anonymous_method(
            vec![
                if_(expr("bool"), vec![branch1.write(expr("int")), return_()]), // (1)
                local_function(vec![]),
                if_(expr("bool"), vec![branch2.write(expr("int")), return_()]), // (2)
            ],
            "void",
            false,
            true,
            None,
        ),
        // (1) and (2) jump to here
        check_unassigned(branch1, false),
        check_unassigned(branch2, false),
        expr("int").invoke_anonymous_method(
            vec![
                local_function(vec![if_(
                    expr("bool"),
                    vec![branch3.write(expr("int")), check_reachable(true), return_()],
                )]), // (3)
                throw_(expr("int")),
            ],
            "void",
            false,
            true,
            None,
        ),
        // (3) does not jump to here
        check_reachable(false),
    ]);
}

#[test]
fn null_aware_anonymous_method_invocation() {
    let mut h = set_up();
    let branch1 = Var::new("branch1");
    h.run(vec![
        declare(branch1),
        expr("int?")
            .invoke_anonymous_method(
                vec![
                    branch1.write(expr("int")),
                    check_unassigned(branch1, false),
                    check_assigned(branch1, true),
                    return_(),
                ],
                "int",
                true,
                true,
                None,
            )
            .invoke_anonymous_method(
                vec![
                    // Null shorting has not terminated yet, so `branch1` is
                    // still known to be assigned.
                    check_unassigned(branch1, false),
                    check_assigned(branch1, true),
                ],
                "int",
                false,
                true,
                None,
            ),
        // Null shorting has now terminated, so `branch1` is now neither
        // definitely assigned nor definitely unassigned.
        check_unassigned(branch1, false),
        check_assigned(branch1, false),
    ]);
}

#[test]
fn anonymous_method_with_target_having_no_expression_info() {
    let mut h = set_up();
    h.run(vec![expr("A").invoke_anonymous_method(
        vec![check_reachable(true)],
        "void",
        false,
        true,
        None,
    )]);
}

#[test]
fn anonymous_method_has_a_different_notion_of_this() {
    let mut h = set_up();
    h.add_member("C", "_field", Some("Object"), true, None);
    h.set_this_type("C");
    h.run(vec![
        this_().as_("C"),
        this_().property("_field", false).as_("num"),
        check_promoted(this_().property("_field", false), "num"),
        expr("C").invoke_anonymous_method(
            vec![
                check_not_promoted(this_().property("_field", false)),
                this_().property("_field", false).as_("int"),
                check_promoted(this_().property("_field", false), "int"),
            ],
            "void",
            false,
            true,
            None,
        ),
        check_promoted(this_().property("_field", false), "num"),
    ]);
}

#[test]
fn anonymous_method_with_target_having_expression_info_but_not_a_reference() {
    let mut h = set_up();
    h.add_member("C", "_field", Some("Object"), true, None);
    h.set_this_type("C");
    h.run(vec![
        this_().as_("C"),
        this_().property("_field", false).as_("num"),
        check_promoted(this_().property("_field", false), "num"),
        expr("bool")
            .conditional(expr("C"), expr("C"))
            .invoke_anonymous_method(
                vec![
                    check_not_promoted(this_().property("_field", false)),
                    this_().property("_field", false).as_("int"),
                    check_promoted(this_().property("_field", false), "int"),
                ],
                "void",
                false,
                true,
                None,
            ),
        check_promoted(this_().property("_field", false), "num"),
    ]);
}

#[test]
fn anonymous_method_promotes_this_field_from_local_variable_field_and_vice_versa() {
    let mut h = set_up();
    let x = Var::new("x");
    h.add_member("A", "_field", Some("Object"), true, None);
    h.run(vec![
        declare(x)
            .with_declared_type("A")
            .with_initializer(expr("A")),
        x.property("_field", false).as_("num"),
        x.invoke_anonymous_method(
            vec![
                check_promoted(this_().property("_field", false), "num"),
                this_().property("_field", false).as_("int"),
            ],
            "void",
            false,
            true,
            None,
        ),
        check_promoted(x.property("_field", false), "int"),
    ]);
}

#[test]
fn anonymous_method_promotes_this_field_from_instance_variable_field_and_vice_versa() {
    let mut h = set_up();
    h.add_member("A", "_field", Some("Object"), true, None);
    h.add_member("B", "_subField", Some("Object"), true, None);
    h.set_this_type("A");
    h.run(vec![
        this_().as_("A"),
        this_().property("_field", false).as_("B"),
        this_()
            .property("_field", false)
            .property("_subField", false)
            .as_("num"),
        this_().property("_field", false).invoke_anonymous_method(
            vec![
                check_promoted(this_().property("_subField", false), "num"),
                this_().property("_subField", false).as_("int"),
            ],
            "void",
            false,
            true,
            None,
        ),
        check_promoted(
            this_()
                .property("_field", false)
                .property("_subField", false),
            "int",
        ),
    ]);
}

#[test]
fn parameterized_anonymous_method_has_the_same_notion_of_this() {
    let mut h = set_up();
    h.add_member("C", "_field", Some("Object"), true, None);
    h.set_this_type("C");
    h.run(vec![
        this_().as_("C"),
        this_().property("_field", false).as_("num"),
        check_promoted(this_().property("_field", false), "num"),
        expr("C").invoke_anonymous_method(
            vec![
                check_promoted(this_().property("_field", false), "num"),
                this_().property("_field", false).as_("int"),
                check_promoted(this_().property("_field", false), "int"),
            ],
            "void",
            false,
            false,
            None,
        ),
        check_promoted(this_().property("_field", false), "int"),
    ]);
}

#[test]
fn parameterized_anonymous_method_with_target_having_expression_info_but_not_a_reference() {
    let mut h = set_up();
    h.add_member("C", "_field", Some("Object"), true, None);
    h.set_this_type("C");
    h.run(vec![
        this_().as_("C"),
        this_().property("_field", false).as_("num"),
        check_promoted(this_().property("_field", false), "num"),
        expr("bool")
            .conditional(expr("C"), expr("C"))
            .invoke_anonymous_method(
                vec![
                    check_promoted(this_().property("_field", false), "num"),
                    this_().property("_field", false).as_("int"),
                    check_promoted(this_().property("_field", false), "int"),
                ],
                "void",
                false,
                false,
                None,
            ),
        check_promoted(this_().property("_field", false), "int"),
    ]);
}

#[test]
fn parameterized_anonymous_method_parameter_does_not_inherit_promotion() {
    let mut h = set_up();
    let x = Var::new("x");
    let p = Var::new("p");
    h.add_member("A", "_field", Some("Object"), true, None);
    h.set_this_type("A");
    h.run(vec![
        declare(x)
            .with_declared_type("A")
            .with_initializer(expr("A")),
        x.property("_field", false).as_("num"),
        x.invoke_anonymous_method(
            vec![
                check_not_promoted(p.property("_field", false)),
                p.property("_field", false).as_("int"),
            ],
            "void",
            false,
            false,
            Some(p),
        ),
        check_promoted(x.property("_field", false), "num"),
    ]);
}

#[test]
fn parameterized_anonymous_method_does_not_promote_parameter_field_from_instance_variable_field_or_vice_versa()
 {
    let mut h = set_up();
    let p = Var::new("p");
    h.add_member("A", "_field", Some("B"), true, None);
    h.add_member("B", "_subField", Some("Object"), true, None);
    h.set_this_type("A");
    h.run(vec![
        this_().as_("A"),
        this_().property("_field", false).as_("B"),
        this_()
            .property("_field", false)
            .property("_subField", false)
            .as_("num"),
        this_().property("_field", false).invoke_anonymous_method(
            vec![
                check_not_promoted(p.property("_subField", false)),
                p.property("_subField", false).as_("int"),
            ],
            "void",
            false,
            false,
            Some(p),
        ),
        check_promoted(
            this_()
                .property("_field", false)
                .property("_subField", false),
            "num",
        ),
    ]);
}

#[test]
fn anonymous_method_this_serves_as_condition_variable() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare(x)
            .with_declared_type("int?")
            .with_initializer(expr("int?")),
        x.eq(null_literal()).not().invoke_anonymous_method(
            vec![this_().conditional(check_promoted(x, "int"), expr("bool"))],
            "bool",
            false,
            true,
            None,
        ),
    ]);
}
