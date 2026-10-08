// Dart source: pkg/_fe_analyzer_shared/test/flow_analysis/flow_analysis_test.dart
// (lines 5451-5825: group 'why not promoted')

//! Dart group `why not promoted`.

use super::common::*;

/// Dart `expect(reasons.keys, unorderedEquals([Type(a), Type(b), ...]))`.
#[track_caller]
pub(crate) fn expect_reason_keys(reasons: &WhyNotPromotedMap, expected: &[&str]) {
    let actual: Vec<Type> = reasons.iter().map(|(t, _)| t.unwrap_type_view()).collect();
    assert_eq!(
        actual.len(),
        expected.len(),
        "expected keys {expected:?}, got {actual:?}"
    );
    for e in expected {
        assert!(
            actual.contains(&ty(e)),
            "expected keys {expected:?}, got {actual:?}"
        );
    }
}

/// Dart `reasons.values.single`.
#[track_caller]
pub(crate) fn single_reason(reasons: &WhyNotPromotedMap) -> &NonPromotionReason {
    assert_eq!(reasons.len(), 1, "expected a single reason");
    &reasons[0].1
}

/// Dart `reasons[SharedTypeView(Type(type))]`.
#[track_caller]
pub(crate) fn reason_for<'a>(
    reasons: &'a WhyNotPromotedMap,
    type_: &str,
) -> &'a NonPromotionReason {
    let key = SharedTypeView::new(ty(type_));
    &reasons
        .iter()
        .find(|(t, _)| *t == key)
        .unwrap_or_else(|| panic!("no reason for {type_}"))
        .1
}

/// Dart `reason as DemoteViaExplicitWrite<Var>` followed by
/// `expect(reason.node, same(node))` and the documentation link check.
#[track_caller]
fn expect_demote_via_explicit_write(reason: &NonPromotionReason, expected_node: Node) {
    match reason {
        NonPromotionReason::DemoteViaExplicitWrite { node, .. } => {
            assert_eq!(*node, expected_node, "expected the same node");
        }
        r => panic!("expected DemoteViaExplicitWrite, got {r:?}"),
    }
    assert_eq!(
        reason.documentation_link(),
        Some(NonPromotionDocumentationLink::Write)
    );
}

/// Dart `reason as DemoteViaSuspension<Var>` followed by
/// `expect(reason.node, same(node))` and the documentation link check.
#[track_caller]
fn expect_demote_via_suspension(reason: &NonPromotionReason, expected_node: Node) {
    match reason {
        NonPromotionReason::DemoteViaSuspension { node, .. } => {
            assert_eq!(*node, expected_node, "expected the same node");
        }
        r => panic!("expected DemoteViaSuspension, got {r:?}"),
    }
    assert_eq!(
        reason.documentation_link(),
        Some(NonPromotionDocumentationLink::Suspension)
    );
}

/// Dart `reason as PropertyNotPromotedForNonInherentReason` followed by
/// `expect(reason.fieldPromotionEnabled, expected)`.
#[track_caller]
pub(crate) fn expect_non_inherent_reason(reason: &NonPromotionReason, expected: bool) {
    match reason {
        NonPromotionReason::PropertyNotPromotedForNonInherentReason {
            field_promotion_enabled,
            ..
        } => assert_eq!(*field_promotion_enabled, expected),
        r => panic!("expected PropertyNotPromotedForNonInherentReason, got {r:?}"),
    }
}

/// Dart `reason as ThisNotPromoted` followed by the documentation link
/// check.
#[track_caller]
fn expect_this_not_promoted(reason: &NonPromotionReason) {
    assert!(
        matches!(reason, NonPromotionReason::ThisNotPromoted),
        "expected ThisNotPromoted, got {reason:?}"
    );
    assert_eq!(
        reason.documentation_link(),
        Some(NonPromotionDocumentationLink::This)
    );
}

#[test]
fn due_to_assignment() {
    let mut h = set_up();
    let x = Var::new("x");
    let write_expression = x.write(expr("int?"));
    h.run(vec![
        declare(x)
            .with_declared_type("int?")
            .with_initializer(expr("int?")),
        if_(x.eq(null_literal()), vec![return_()]),
        check_promoted(x, "int"),
        write_expression,
        check_not_promoted(x),
        x.why_not_promoted(move |reasons| {
            expect_reason_keys(&reasons, &["int"]);
            expect_demote_via_explicit_write(single_reason(&reasons), write_expression);
        }),
    ]);
}

#[test]
fn due_to_assignment_multiple_demotions() {
    let mut h = set_up();
    let x = Var::new("x");
    let write_expression = x.write(expr("Object?"));
    h.run(vec![
        declare(x)
            .with_declared_type("Object?")
            .with_initializer(expr("Object?")),
        if_(x.is_not("int?"), vec![return_()]),
        if_(x.eq(null_literal()), vec![return_()]),
        check_promoted(x, "int"),
        write_expression,
        check_not_promoted(x),
        x.why_not_promoted(move |reasons| {
            expect_reason_keys(&reasons, &["int", "int?"]);
            for type_ in ["int", "int?"] {
                expect_demote_via_explicit_write(reason_for(&reasons, type_), write_expression);
            }
        }),
    ]);
}

#[test]
fn due_to_await() {
    let mut h = set_up();
    let x = Var::new("x");
    let await_expression = await_(expr("Object?"));
    h.run(vec![
        declare(x)
            .with_declared_type("int?")
            .with_initializer(expr("int?")),
        local_function(vec![
            if_(x.eq(null_literal()), vec![return_()]),
            check_promoted(x, "int"),
            await_expression,
            check_not_promoted(x),
            x.why_not_promoted(move |reasons| {
                expect_reason_keys(&reasons, &["int"]);
                expect_demote_via_suspension(single_reason(&reasons), await_expression);
            }),
        ]),
        x.write(expr("int?")),
    ]);
}

#[test]
fn due_to_yield() {
    let mut h = set_up();
    let x = Var::new("x");
    let yield_statement = yield_(expr("Object?"), false);
    h.run(vec![
        declare(x)
            .with_declared_type("int?")
            .with_initializer(expr("int?")),
        local_function(vec![
            if_(x.eq(null_literal()), vec![return_()]),
            check_promoted(x, "int"),
            yield_statement,
            check_not_promoted(x),
            x.why_not_promoted(move |reasons| {
                expect_reason_keys(&reasons, &["int"]);
                expect_demote_via_suspension(single_reason(&reasons), yield_statement);
            }),
        ]),
        x.write(expr("int?")),
    ]);
}

#[test]
fn due_to_pattern_assignment() {
    let mut h = set_up();
    let x = Var::new("x");
    let write_pattern = x.pattern();
    h.run(vec![
        declare(x)
            .with_declared_type("int?")
            .with_initializer(expr("int?")),
        if_(x.eq(null_literal()), vec![return_()]),
        check_promoted(x, "int"),
        write_pattern.assign(expr("int?")),
        check_not_promoted(x),
        x.why_not_promoted(move |reasons| {
            expect_reason_keys(&reasons, &["int"]);
            expect_demote_via_explicit_write(single_reason(&reasons), write_pattern);
        }),
    ]);
}

#[test]
fn preserved_in_join_when_one_branch_unreachable() {
    let mut h = set_up();
    let x = Var::new("x");
    let write_expression = x.write(expr("int?"));
    h.run(vec![
        declare(x)
            .with_declared_type("int?")
            .with_initializer(expr("int?")),
        if_(x.eq(null_literal()), vec![return_()]),
        check_promoted(x, "int"),
        write_expression,
        check_not_promoted(x),
        if_(expr("bool"), vec![return_()]),
        x.why_not_promoted(move |reasons| {
            expect_reason_keys(&reasons, &["int"]);
            expect_demote_via_explicit_write(single_reason(&reasons), write_expression);
        }),
    ]);
}

#[test]
fn preserved_in_later_promotions() {
    let mut h = set_up();
    let x = Var::new("x");
    let write_expression = x.write(expr("Object"));
    h.run(vec![
        declare(x)
            .with_declared_type("Object")
            .with_initializer(expr("Object")),
        if_(x.is_not("int"), vec![return_()]),
        check_promoted(x, "int"),
        write_expression,
        check_not_promoted(x),
        if_(x.is_not("num"), vec![return_()]),
        check_promoted(x, "num"),
        x.why_not_promoted(move |reasons| {
            expect_demote_via_explicit_write(reason_for(&reasons, "int"), write_expression);
        }),
    ]);
}

#[test]
fn re_promotion() {
    let mut h = set_up();
    let x = Var::new("x");
    h.run(vec![
        declare(x)
            .with_declared_type("int?")
            .with_initializer(expr("int?")),
        if_(x.eq(null_literal()), vec![return_()]),
        check_promoted(x, "int"),
        x.write(expr("int?")),
        check_not_promoted(x),
        if_(x.eq(null_literal()), vec![return_()]),
        check_promoted(x, "int"),
        x.why_not_promoted(|reasons| {
            assert!(reasons.is_empty());
        }),
    ]);
}

mod field_promotion_disabled {
    use super::*;

    #[test]
    fn via_explicit_this() {
        let mut h = set_up();
        h.disable_field_promotion();
        h.set_this_type("C");
        h.add_member("C", "_field", Some("Object?"), true, None);
        h.run(vec![
            if_(
                this_().property("_field", false).eq(null_literal()),
                vec![return_()],
            ),
            this_()
                .property("_field", false)
                .why_not_promoted(|reasons| {
                    expect_reason_keys(&reasons, &["Object"]);
                    expect_non_inherent_reason(single_reason(&reasons), false);
                }),
        ]);
    }

    #[test]
    fn via_implicit_this_super() {
        // Dart name: 'via implicit this/super'.
        let mut h = set_up();
        h.disable_field_promotion();
        h.set_this_type("C");
        h.add_member("C", "_field", Some("Object?"), true, None);
        h.run(vec![
            if_(this_property("_field").eq(null_literal()), vec![return_()]),
            this_property("_field").why_not_promoted(|reasons| {
                expect_reason_keys(&reasons, &["Object"]);
                expect_non_inherent_reason(single_reason(&reasons), false);
            }),
        ]);
    }

    #[test]
    fn via_variable() {
        let mut h = set_up();
        h.disable_field_promotion();
        h.add_member("C", "_field", Some("Object?"), true, None);
        let x = Var::new("x");
        h.run(vec![
            declare(x)
                .with_declared_type("C")
                .with_initializer(expr("C")),
            if_(
                x.property("_field", false).eq(null_literal()),
                vec![return_()],
            ),
            x.property("_field", false).why_not_promoted(|reasons| {
                expect_reason_keys(&reasons, &["Object"]);
                expect_non_inherent_reason(single_reason(&reasons), false);
            }),
        ]);
    }
}

mod because_this {
    use super::*;

    #[test]
    fn explicit() {
        let mut h = set_up();
        h.disable_this_promotion();
        h.set_this_type("C");
        h.add_super_interfaces("D", |_| vec![ty("C"), ty("Object")]);
        h.add_super_interfaces("C", |_| vec![ty("Object")]);
        h.run(vec![
            if_(this_().is_not("D"), vec![return_()]),
            this_().why_not_promoted(|reasons| {
                expect_reason_keys(&reasons, &["D"]);
                expect_this_not_promoted(single_reason(&reasons));
            }),
        ]);
    }

    #[test]
    fn implicit() {
        let mut h = set_up();
        h.disable_this_promotion();
        h.set_this_type("C");
        h.add_super_interfaces("D", |_| vec![ty("C"), ty("Object")]);
        h.add_super_interfaces("C", |_| vec![ty("Object")]);
        h.run(vec![
            if_(this_().is_not("D"), vec![return_()]),
            implicit_this_why_not_promoted("C", |reasons| {
                expect_reason_keys(&reasons, &["D"]);
                expect_this_not_promoted(single_reason(&reasons));
            }),
        ]);
    }
}

mod this_promotion {
    use super::*;

    #[test]
    fn promotes_this() {
        let mut h = set_up();
        h.set_this_type("C");
        h.add_super_interfaces("D", |_| vec![ty("C"), ty("Object")]);
        h.add_super_interfaces("C", |_| vec![ty("Object")]);
        h.run(vec![
            if_(this_().is_not("D"), vec![return_()]),
            check_promoted(this_(), "D"),
            this_().check_type("D"),
            this_().why_not_promoted(|reasons| {
                assert!(reasons.is_empty());
            }),
        ]);
    }

    #[test]
    fn implicit_this_why_not_promoted_is_empty() {
        let mut h = set_up();
        h.set_this_type("C");
        h.add_super_interfaces("D", |_| vec![ty("C"), ty("Object")]);
        h.add_super_interfaces("C", |_| vec![ty("Object")]);
        h.run(vec![
            if_(this_().is_not("D"), vec![return_()]),
            implicit_this_why_not_promoted("C", |reasons| {
                assert!(reasons.is_empty());
            }),
        ]);
    }

    #[test]
    fn switch_statement_this_promotes() {
        let mut h = set_up();
        h.set_this_type("C");
        h.add_exhaustiveness("C", false);
        h.add_super_interfaces("D", |_| vec![ty("C"), ty("Object")]);
        h.add_super_interfaces("C", |_| vec![ty("Object")]);
        h.run(vec![switch_(
            this_(),
            vec![
                wildcard()
                    .with_declared_type("D")
                    .then(vec![check_promoted(this_(), "D")]),
            ],
        )]);
    }

    #[test]
    fn switch_statement_this_does_not_promote_when_disabled() {
        let mut h = set_up();
        h.disable_this_promotion();
        h.set_this_type("C");
        h.add_exhaustiveness("C", false);
        h.add_super_interfaces("D", |_| vec![ty("C"), ty("Object")]);
        h.add_super_interfaces("C", |_| vec![ty("Object")]);
        h.run(vec![switch_(
            this_(),
            vec![
                wildcard()
                    .with_declared_type("D")
                    .then(vec![check_not_promoted(this_())]),
            ],
        )]);
    }

    #[test]
    fn switch_expression_this_promotes() {
        let mut h = set_up();
        h.set_this_type("C");
        h.add_exhaustiveness("C", false);
        h.add_super_interfaces("D", |_| vec![ty("C"), ty("Object")]);
        h.add_super_interfaces("C", |_| vec![ty("Object")]);
        h.run(vec![switch_expr(
            this_(),
            vec![
                wildcard()
                    .with_declared_type("D")
                    .then_expr(check_promoted(this_(), "D")),
            ],
        )]);
    }

    #[test]
    fn switch_expression_this_does_not_promote_when_disabled() {
        let mut h = set_up();
        h.disable_this_promotion();
        h.set_this_type("C");
        h.add_exhaustiveness("C", false);
        h.add_super_interfaces("D", |_| vec![ty("C"), ty("Object")]);
        h.add_super_interfaces("C", |_| vec![ty("Object")]);
        h.run(vec![switch_expr(
            this_(),
            vec![
                wildcard()
                    .with_declared_type("D")
                    .then_expr(check_not_promoted(this_())),
            ],
        )]);
    }

    #[test]
    fn if_case_this_promotes() {
        let mut h = set_up();
        h.set_this_type("C");
        h.add_exhaustiveness("C", false);
        h.add_super_interfaces("D", |_| vec![ty("C"), ty("Object")]);
        h.add_super_interfaces("C", |_| vec![ty("Object")]);
        h.run(vec![if_case(
            this_(),
            wildcard().with_declared_type("D"),
            vec![check_promoted(this_(), "D")],
            None,
        )]);
    }

    #[test]
    fn if_case_this_does_not_promote_when_disabled() {
        let mut h = set_up();
        h.disable_this_promotion();
        h.set_this_type("C");
        h.add_exhaustiveness("C", false);
        h.add_super_interfaces("D", |_| vec![ty("C"), ty("Object")]);
        h.add_super_interfaces("C", |_| vec![ty("Object")]);
        h.run(vec![if_case(
            this_(),
            wildcard().with_declared_type("D"),
            vec![check_not_promoted(this_())],
            None,
        )]);
    }
}
