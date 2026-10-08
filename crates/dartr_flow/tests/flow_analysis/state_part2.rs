// Dart source: pkg/_fe_analyzer_shared/test/flow_analysis/flow_analysis_test.dart
// (lines 3884-4900: group 'State', groups 'write', 'demotion, to NonNull',
// 'declare', 'markNonNullable', 'conservativeJoin', 'rebaseForward')

//! Dart group `State` (second part): tests that call `FlowModel` methods
//! directly, with [`FlowAnalysisTestHarness`] as the `FlowModelHelper`.

use super::common::*;

/// The variables of the Dart `setUp` of group `State`.
struct StateVars {
    int_var: Var,
    int_q_var: Var,
    object_q_var: Var,
    null_var: Var,
}

/// Dart `setUp` of group `State`.
fn state_vars() -> StateVars {
    StateVars {
        int_var: Var::new("x").with_type("int"),
        int_q_var: Var::new("x").with_type("int?"),
        object_q_var: Var::new("x").with_type("Object?"),
        null_var: Var::new("x").with_type("Null"),
    }
}

/// Dart `same(...)` for `PromotionInfo?`.
fn same_promotion_info(a: &PromotionInfo, b: &PromotionInfo) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => std::rc::Rc::ptr_eq(a, b),
        _ => false,
    }
}

/// Dart `SharedTypeView(Type(typeStr))`.
fn view(type_str: &str) -> SharedTypeView<Type> {
    SharedTypeView::new(ty(type_str))
}

/// Dart `s._tryPromoteForTypeCheck(h, variable, type).ifTrue`.
fn promote_if_true(
    h: &FlowAnalysisTestHarness,
    s: &FlowModel,
    variable: Var,
    type_: &str,
) -> FlowModel {
    s.try_promote_for_type_check_var(h, variable, type_)
        .if_true
        .clone()
}

/// Dart `s._tryPromoteForTypeCheck(h, variable, type).ifFalse`.
fn promote_if_false(
    h: &FlowAnalysisTestHarness,
    s: &FlowModel,
    variable: Var,
    type_: &str,
) -> FlowModel {
    s.try_promote_for_type_check_var(h, variable, type_)
        .if_false
        .clone()
}

/// Dart `expect(s.promotionInfo.unwrap(h), {key(variable): matcher})`.
#[track_caller]
fn expect_info(
    h: &FlowAnalysisTestHarness,
    s: &FlowModel,
    variable: Var,
    matcher: VariableModelMatcher,
) {
    expect_promotion_models(
        &unwrap_promotion_info(&s.promotion_info, h),
        &[(h.key_for_variable(variable), matcher)],
    );
}

mod write {
    use super::*;

    /// Dart `setUp` of group `write` (a new `objectQVar`).
    fn object_q_var() -> Var {
        Var::new("x").with_type("Object?")
    }

    #[test]
    fn without_declaration() {
        // This should not happen in valid code, but test that we don't crash.
        let h = set_up();
        let _v = state_vars();
        let object_q_var = object_q_var();

        let s = FlowModel::new(Reachability::initial()).write_var(
            &h,
            None,
            object_q_var,
            view("Object?"),
            SsaNode::new(),
        );
        assert!(
            promotion_info_get(&s.promotion_info, &h, h.key_for_variable(object_q_var)).is_none()
        );
    }

    #[test]
    fn unchanged() {
        let h = set_up();
        let _v = state_vars();
        let object_q_var = object_q_var();
        let s1 = FlowModel::new(Reachability::initial()).declare_var(&h, object_q_var, true);
        let s2 = s1.write_var(&h, None, object_q_var, view("Object?"), SsaNode::new());
        assert!(!s2.ptr_eq(&s1));
        assert!(s2.reachable.ptr_eq(&s1.reachable));
        match_variable_model()
            .chain(&[])
            .of_interest(&[])
            .assigned(true)
            .unassigned(false)
            .check(&s2.info_for_var(&h, object_q_var));
    }

    #[test]
    fn marks_as_assigned() {
        let h = set_up();
        let _v = state_vars();
        let object_q_var = object_q_var();
        let s1 = FlowModel::new(Reachability::initial()).declare_var(&h, object_q_var, false);
        let s2 = s1.write_var(&h, None, object_q_var, view("int?"), SsaNode::new());
        assert!(s2.reachable.overall_reachable);
        match_variable_model()
            .chain(&[])
            .of_interest(&[])
            .assigned(true)
            .unassigned(false)
            .check(&s2.info_for_var(&h, object_q_var));
    }

    #[test]
    fn un_promotes_fully() {
        let h = set_up();
        let _v = state_vars();
        let object_q_var = object_q_var();
        let s1 = promote_if_true(
            &h,
            &FlowModel::new(Reachability::initial()).declare_var(&h, object_q_var, true),
            object_q_var,
            "int",
        );
        assert!(
            unwrap_promotion_info(&s1.promotion_info, &h)
                .contains_key(&h.key_for_variable(object_q_var))
        );
        let s2 = s1.write_var(
            &h,
            Some(&mock_non_promotion_reason()),
            object_q_var,
            view("int?"),
            SsaNode::new(),
        );
        assert!(s2.reachable.overall_reachable);
        expect_info(
            &h,
            &s2,
            object_q_var,
            match_variable_model()
                .chain(&[])
                .of_interest_types(&[ty("int")])
                .assigned(true)
                .unassigned(false),
        );
    }

    #[test]
    fn un_promotes_partially_when_no_exact_match() {
        let h = set_up();
        let _v = state_vars();
        let object_q_var = object_q_var();
        let s0 = FlowModel::new(Reachability::initial()).declare_var(&h, object_q_var, true);
        let s0 = promote_if_true(&h, &s0, object_q_var, "num?");
        let s1 = promote_if_true(&h, &s0, object_q_var, "int");
        expect_info(
            &h,
            &s1,
            object_q_var,
            match_variable_model()
                .chain(&["num?", "int"])
                .of_interest(&["num?", "int"])
                .assigned(true)
                .unassigned(false),
        );
        let s2 = s1.write_var(
            &h,
            Some(&mock_non_promotion_reason()),
            object_q_var,
            view("num"),
            SsaNode::new(),
        );
        assert!(s2.reachable.overall_reachable);
        expect_info(
            &h,
            &s2,
            object_q_var,
            match_variable_model()
                .chain(&["num?", "num"])
                .of_interest(&["num?", "int"])
                .assigned(true)
                .unassigned(false),
        );
    }

    #[test]
    fn un_promotes_partially_when_exact_match() {
        let h = set_up();
        let _v = state_vars();
        let object_q_var = object_q_var();
        let s0 = FlowModel::new(Reachability::initial()).declare_var(&h, object_q_var, true);
        let s0 = promote_if_true(&h, &s0, object_q_var, "num?");
        let s0 = promote_if_true(&h, &s0, object_q_var, "num");
        let s1 = promote_if_true(&h, &s0, object_q_var, "int");
        expect_info(
            &h,
            &s1,
            object_q_var,
            match_variable_model()
                .chain(&["num?", "num", "int"])
                .of_interest(&["num?", "num", "int"])
                .assigned(true)
                .unassigned(false),
        );
        let s2 = s1.write_var(
            &h,
            Some(&mock_non_promotion_reason()),
            object_q_var,
            view("num"),
            SsaNode::new(),
        );
        assert!(s2.reachable.overall_reachable);
        expect_info(
            &h,
            &s2,
            object_q_var,
            match_variable_model()
                .chain(&["num?", "num"])
                .of_interest(&["num?", "num", "int"])
                .assigned(true)
                .unassigned(false),
        );
    }

    #[test]
    fn leaves_promoted_when_exact_match() {
        let h = set_up();
        let _v = state_vars();
        let object_q_var = object_q_var();
        let s0 = FlowModel::new(Reachability::initial()).declare_var(&h, object_q_var, true);
        let s0 = promote_if_true(&h, &s0, object_q_var, "num?");
        let s1 = promote_if_true(&h, &s0, object_q_var, "num");
        expect_info(
            &h,
            &s1,
            object_q_var,
            match_variable_model()
                .chain(&["num?", "num"])
                .of_interest(&["num?", "num"])
                .assigned(true)
                .unassigned(false),
        );
        let s2 = s1.write_var(&h, None, object_q_var, view("num"), SsaNode::new());
        assert!(s2.reachable.overall_reachable);
        assert!(!same_promotion_info(&s2.promotion_info, &s1.promotion_info));
        expect_info(
            &h,
            &s2,
            object_q_var,
            match_variable_model()
                .chain(&["num?", "num"])
                .of_interest(&["num?", "num"])
                .assigned(true)
                .unassigned(false),
        );
    }

    #[test]
    fn leaves_promoted_when_writing_a_subtype() {
        let h = set_up();
        let _v = state_vars();
        let object_q_var = object_q_var();
        let s0 = FlowModel::new(Reachability::initial()).declare_var(&h, object_q_var, true);
        let s0 = promote_if_true(&h, &s0, object_q_var, "num?");
        let s1 = promote_if_true(&h, &s0, object_q_var, "num");
        expect_info(
            &h,
            &s1,
            object_q_var,
            match_variable_model()
                .chain(&["num?", "num"])
                .of_interest(&["num?", "num"])
                .assigned(true)
                .unassigned(false),
        );
        let s2 = s1.write_var(&h, None, object_q_var, view("int"), SsaNode::new());
        assert!(s2.reachable.overall_reachable);
        assert!(!same_promotion_info(&s2.promotion_info, &s1.promotion_info));
        expect_info(
            &h,
            &s2,
            object_q_var,
            match_variable_model()
                .chain(&["num?", "num"])
                .of_interest(&["num?", "num"])
                .assigned(true)
                .unassigned(false),
        );
    }

    mod promotes_to_non_null_of_a_type_of_interest {
        use super::*;

        #[test]
        fn when_declared_type() {
            let h = set_up();
            let _v = state_vars();
            let _object_q_var = object_q_var();
            let x = Var::new("x").with_type("int?");

            let s1 = FlowModel::new(Reachability::initial()).declare_var(&h, x, true);
            expect_info(&h, &s1, x, match_variable_model().chain(&[]));

            let s2 = s1.write_var(&h, None, x, view("int"), SsaNode::new());
            expect_info(&h, &s2, x, match_variable_model().chain(&["int"]));
        }

        #[test]
        fn when_declared_type_if_write_captured() {
            let h = set_up();
            let _v = state_vars();
            let _object_q_var = object_q_var();
            let x = Var::new("x").with_type("int?");

            let s1 = FlowModel::new(Reachability::initial()).declare_var(&h, x, true);
            expect_info(&h, &s1, x, match_variable_model().chain(&[]));

            let s2 = s1.conservative_join_vars(&h, &[], &[x]);
            expect_info(
                &h,
                &s2,
                x,
                match_variable_model().chain(&[]).write_captured(true),
            );

            // 'x' is write-captured, so not promoted
            let s3 = s2.write_var(&h, None, x, view("int"), SsaNode::new());
            expect_info(
                &h,
                &s3,
                x,
                match_variable_model().chain(&[]).write_captured(true),
            );
        }

        #[test]
        fn when_promoted() {
            let h = set_up();
            let _v = state_vars();
            let object_q_var = object_q_var();
            let s0 = FlowModel::new(Reachability::initial()).declare_var(&h, object_q_var, true);
            let s1 = promote_if_true(&h, &s0, object_q_var, "int?");
            expect_info(
                &h,
                &s1,
                object_q_var,
                match_variable_model()
                    .chain(&["int?"])
                    .of_interest(&["int?"]),
            );
            let s2 = s1.write_var(&h, None, object_q_var, view("int"), SsaNode::new());
            expect_info(
                &h,
                &s2,
                object_q_var,
                match_variable_model()
                    .chain(&["int?", "int"])
                    .of_interest(&["int?"]),
            );
        }

        #[test]
        fn when_not_promoted() {
            let h = set_up();
            let _v = state_vars();
            let object_q_var = object_q_var();
            let s0 = FlowModel::new(Reachability::initial()).declare_var(&h, object_q_var, true);
            let s1 = promote_if_false(&h, &s0, object_q_var, "int?");
            expect_info(
                &h,
                &s1,
                object_q_var,
                match_variable_model()
                    .chain(&["Object"])
                    .of_interest(&["int?"]),
            );
            let s2 = s1.write_var(&h, None, object_q_var, view("int"), SsaNode::new());
            expect_info(
                &h,
                &s2,
                object_q_var,
                match_variable_model()
                    .chain(&["Object", "int"])
                    .of_interest(&["int?"]),
            );
        }
    }

    #[test]
    fn promotes_to_type_of_interest_when_not_previously_promoted() {
        let h = set_up();
        let _v = state_vars();
        let object_q_var = object_q_var();
        let s0 = FlowModel::new(Reachability::initial()).declare_var(&h, object_q_var, true);
        let s1 = promote_if_false(&h, &s0, object_q_var, "num?");
        expect_info(
            &h,
            &s1,
            object_q_var,
            match_variable_model()
                .chain(&["Object"])
                .of_interest(&["num?"]),
        );
        let s2 = s1.write_var(
            &h,
            Some(&mock_non_promotion_reason()),
            object_q_var,
            view("num?"),
            SsaNode::new(),
        );
        expect_info(
            &h,
            &s2,
            object_q_var,
            match_variable_model()
                .chain(&["num?"])
                .of_interest(&["num?"]),
        );
    }

    #[test]
    fn promotes_to_type_of_interest_when_previously_promoted() {
        let h = set_up();
        let _v = state_vars();
        let object_q_var = object_q_var();
        let s0 = FlowModel::new(Reachability::initial()).declare_var(&h, object_q_var, true);
        let s0 = promote_if_true(&h, &s0, object_q_var, "num?");
        let s1 = promote_if_false(&h, &s0, object_q_var, "int?");
        expect_info(
            &h,
            &s1,
            object_q_var,
            match_variable_model()
                .chain(&["num?", "num"])
                .of_interest(&["num?", "int?"]),
        );
        let s2 = s1.write_var(
            &h,
            Some(&mock_non_promotion_reason()),
            object_q_var,
            view("int?"),
            SsaNode::new(),
        );
        expect_info(
            &h,
            &s2,
            object_q_var,
            match_variable_model()
                .chain(&["num?", "int?"])
                .of_interest(&["num?", "int?"]),
        );
    }

    mod multiple_candidate_types_of_interest {
        use super::*;

        mod choose_most_specific {
            use super::*;

            /// Dart `setUp` of group `; choose most specific`.
            fn set_up_classes(h: &mut FlowAnalysisTestHarness) {
                // class A {}
                // class B extends A {}
                // class C extends B {}
                h.add_super_interfaces("C", |_| vec![ty("B"), ty("A"), ty("Object")]);
                h.add_super_interfaces("B", |_| vec![ty("A"), ty("Object")]);
                h.add_super_interfaces("A", |_| vec![ty("Object")]);
            }

            #[test]
            fn first() {
                let mut h = set_up();
                let _v = state_vars();
                let _object_q_var = object_q_var();
                set_up_classes(&mut h);
                let x = Var::new("x").with_type("Object?");

                let s0 = FlowModel::new(Reachability::initial()).declare_var(&h, x, true);
                let s0 = promote_if_false(&h, &s0, x, "B?");
                let s1 = promote_if_false(&h, &s0, x, "A?");
                expect_info(
                    &h,
                    &s1,
                    x,
                    match_variable_model()
                        .chain(&["Object"])
                        .of_interest(&["A?", "B?"]),
                );

                let s2 = s1.write_var(&h, None, x, view("C"), SsaNode::new());
                expect_info(
                    &h,
                    &s2,
                    x,
                    match_variable_model()
                        .chain(&["Object", "B"])
                        .of_interest(&["A?", "B?"]),
                );
            }

            #[test]
            fn second() {
                let mut h = set_up();
                let _v = state_vars();
                let _object_q_var = object_q_var();
                set_up_classes(&mut h);
                let x = Var::new("x").with_type("Object?");

                let s0 = FlowModel::new(Reachability::initial()).declare_var(&h, x, true);
                let s0 = promote_if_false(&h, &s0, x, "A?");
                let s1 = promote_if_false(&h, &s0, x, "B?");
                expect_info(
                    &h,
                    &s1,
                    x,
                    match_variable_model()
                        .chain(&["Object"])
                        .of_interest(&["A?", "B?"]),
                );

                let s2 = s1.write_var(&h, None, x, view("C"), SsaNode::new());
                expect_info(
                    &h,
                    &s2,
                    x,
                    match_variable_model()
                        .chain(&["Object", "B"])
                        .of_interest(&["A?", "B?"]),
                );
            }

            #[test]
            fn nullable_and_non_nullable() {
                let mut h = set_up();
                let _v = state_vars();
                let _object_q_var = object_q_var();
                set_up_classes(&mut h);
                let x = Var::new("x").with_type("Object?");

                let s0 = FlowModel::new(Reachability::initial()).declare_var(&h, x, true);
                let s0 = promote_if_false(&h, &s0, x, "A");
                let s1 = promote_if_false(&h, &s0, x, "A?");
                expect_info(
                    &h,
                    &s1,
                    x,
                    match_variable_model()
                        .chain(&["Object"])
                        .of_interest(&["A", "A?"]),
                );

                let s2 = s1.write_var(&h, None, x, view("B"), SsaNode::new());
                expect_info(
                    &h,
                    &s2,
                    x,
                    match_variable_model()
                        .chain(&["Object", "A"])
                        .of_interest(&["A", "A?"]),
                );
            }
        }

        mod ambiguous {
            use super::*;

            #[test]
            fn no_promotion() {
                let h = set_up();
                let _v = state_vars();
                let object_q_var = object_q_var();
                let s0 =
                    FlowModel::new(Reachability::initial()).declare_var(&h, object_q_var, true);
                let s0 = promote_if_false(&h, &s0, object_q_var, "List<Object?>");
                let s1 = promote_if_false(&h, &s0, object_q_var, "List<dynamic>");
                expect_info(
                    &h,
                    &s1,
                    object_q_var,
                    match_variable_model().of_interest(&["List<Object?>", "List<dynamic>"]),
                );
                let s2 = s1.write_var(&h, None, object_q_var, view("List<int>"), SsaNode::new());
                // It's ambiguous whether to promote to List<Object?> or
                // List<dynamic>, so we don't promote.
                assert!(!s2.ptr_eq(&s1));
                expect_info(
                    &h,
                    &s2,
                    object_q_var,
                    match_variable_model().of_interest(&["List<Object?>", "List<dynamic>"]),
                );
            }
        }

        #[test]
        fn exact_match() {
            let h = set_up();
            let _v = state_vars();
            let object_q_var = object_q_var();
            let s0 = FlowModel::new(Reachability::initial()).declare_var(&h, object_q_var, true);
            let s0 = promote_if_false(&h, &s0, object_q_var, "List<Object?>");
            let s1 = promote_if_false(&h, &s0, object_q_var, "List<dynamic>");
            expect_info(
                &h,
                &s1,
                object_q_var,
                match_variable_model().of_interest(&["List<Object?>", "List<dynamic>"]),
            );
            let s2 = s1.write_var(
                &h,
                Some(&mock_non_promotion_reason()),
                object_q_var,
                view("List<Object?>"),
                SsaNode::new(),
            );
            // It's ambiguous whether to promote to List<Object?> or
            // List<dynamic>, but since the written type is exactly List<Object?>,
            // we use that.
            expect_info(
                &h,
                &s2,
                object_q_var,
                match_variable_model()
                    .chain(&["List<Object?>"])
                    .of_interest(&["List<Object?>", "List<dynamic>"]),
            );
        }
    }
}

mod demotion_to_non_null {
    use super::*;

    #[test]
    fn when_promoted_via_test() {
        let h = set_up();
        let _v = state_vars();
        let x = Var::new("x").with_type("Object?");

        let s0 = FlowModel::new(Reachability::initial()).declare_var(&h, x, true);
        let s0 = promote_if_true(&h, &s0, x, "num?");
        let s1 = promote_if_true(&h, &s0, x, "int?");
        expect_info(
            &h,
            &s1,
            x,
            match_variable_model()
                .chain(&["num?", "int?"])
                .of_interest(&["num?", "int?"]),
        );

        let s2 = s1.write_var(
            &h,
            Some(&mock_non_promotion_reason()),
            x,
            view("double"),
            SsaNode::new(),
        );
        expect_info(
            &h,
            &s2,
            x,
            match_variable_model()
                .chain(&["num?", "num"])
                .of_interest(&["num?", "int?"]),
        );
    }
}

mod declare {
    use super::*;

    /// Dart `setUp` of group `declare` (a new `objectQVar`).
    fn object_q_var() -> Var {
        Var::new("x").with_type("Object?")
    }

    #[test]
    fn initialized() {
        let h = set_up();
        let _v = state_vars();
        let object_q_var = object_q_var();
        let s = FlowModel::new(Reachability::initial()).declare_var(&h, object_q_var, true);
        expect_info(
            &h,
            &s,
            object_q_var,
            match_variable_model().assigned(true).unassigned(false),
        );
    }

    #[test]
    fn not_initialized() {
        let h = set_up();
        let _v = state_vars();
        let object_q_var = object_q_var();
        let s = FlowModel::new(Reachability::initial()).declare_var(&h, object_q_var, false);
        expect_info(
            &h,
            &s,
            object_q_var,
            match_variable_model().assigned(false).unassigned(true),
        );
    }
}

mod mark_non_nullable {
    use super::*;

    #[test]
    fn unpromoted_unchanged() {
        let h = set_up();
        let v = state_vars();
        let s1 = FlowModel::new(Reachability::initial());
        let s2 = s1.try_mark_non_nullable_var(&h, v.int_var).if_true.clone();
        assert!(s2.ptr_eq(&s1));
    }

    #[test]
    fn unpromoted_promoted() {
        let h = set_up();
        let v = state_vars();
        let s1 = FlowModel::new(Reachability::initial());
        let s2 = s1
            .try_mark_non_nullable_var(&h, v.int_q_var)
            .if_true
            .clone();
        assert!(s2.reachable.overall_reachable);
        match_variable_model()
            .chain(&["int"])
            .of_interest(&[])
            .check(&s2.info_for_var(&h, v.int_q_var));
    }

    #[test]
    fn promoted_unchanged() {
        let h = set_up();
        let v = state_vars();
        let s1 = promote_if_true(
            &h,
            &FlowModel::new(Reachability::initial()),
            v.object_q_var,
            "int",
        );
        let s2 = s1
            .try_mark_non_nullable_var(&h, v.object_q_var)
            .if_true
            .clone();
        assert!(s2.ptr_eq(&s1));
    }

    #[test]
    fn promoted_re_promoted() {
        let h = set_up();
        let v = state_vars();
        let s1 = promote_if_true(
            &h,
            &FlowModel::new(Reachability::initial()),
            v.object_q_var,
            "int?",
        );
        let s2 = s1
            .try_mark_non_nullable_var(&h, v.object_q_var)
            .if_true
            .clone();
        assert!(s2.reachable.overall_reachable);
        expect_info(
            &h,
            &s2,
            v.object_q_var,
            match_variable_model()
                .chain(&["int?", "int"])
                .of_interest(&["int?"]),
        );
    }

    #[test]
    fn promote_to_never() {
        let h = set_up();
        let v = state_vars();
        let s1 = FlowModel::new(Reachability::initial());
        let s2 = s1.try_mark_non_nullable_var(&h, v.null_var).if_true.clone();
        assert!(s2.reachable.overall_reachable);
        match_variable_model()
            .chain(&["Never"])
            .of_interest(&[])
            .check(&s2.info_for_var(&h, v.null_var));
    }
}

mod conservative_join {
    use super::*;

    #[test]
    fn unchanged() {
        let h = set_up();
        let v = state_vars();
        let s0 = FlowModel::new(Reachability::initial()).declare_var(&h, v.int_q_var, true);
        let s1 = promote_if_true(&h, &s0, v.object_q_var, "int");
        let s2 = s1.conservative_join_vars(&h, &[v.int_q_var], &[]);
        assert!(!s2.ptr_eq(&s1));
        assert!(s2.reachable.ptr_eq(&s1.reachable));
        expect_promotion_models(
            &unwrap_promotion_info(&s2.promotion_info, &h),
            &[
                (
                    h.key_for_variable(v.object_q_var),
                    match_variable_model().chain(&["int"]).of_interest(&["int"]),
                ),
                (
                    h.key_for_variable(v.int_q_var),
                    match_variable_model().chain(&[]).of_interest(&[]),
                ),
            ],
        );
    }

    #[test]
    fn written() {
        let h = set_up();
        let v = state_vars();
        let s0 = promote_if_true(
            &h,
            &FlowModel::new(Reachability::initial()),
            v.object_q_var,
            "int",
        );
        let s1 = promote_if_true(&h, &s0, v.int_q_var, "int");
        let s2 = s1.conservative_join_vars(&h, &[v.int_q_var], &[]);
        assert!(s2.reachable.overall_reachable);
        expect_promotion_models(
            &unwrap_promotion_info(&s2.promotion_info, &h),
            &[
                (
                    h.key_for_variable(v.object_q_var),
                    match_variable_model().chain(&["int"]).of_interest(&["int"]),
                ),
                (
                    h.key_for_variable(v.int_q_var),
                    match_variable_model().chain(&[]).of_interest(&["int"]),
                ),
            ],
        );
    }

    #[test]
    fn write_captured() {
        let h = set_up();
        let v = state_vars();
        let s0 = promote_if_true(
            &h,
            &FlowModel::new(Reachability::initial()),
            v.object_q_var,
            "int",
        );
        let s1 = promote_if_true(&h, &s0, v.int_q_var, "int");
        let s2 = s1.conservative_join_vars(&h, &[], &[v.int_q_var]);
        assert!(s2.reachable.overall_reachable);
        expect_promotion_models(
            &unwrap_promotion_info(&s2.promotion_info, &h),
            &[
                (
                    h.key_for_variable(v.object_q_var),
                    match_variable_model().chain(&["int"]).of_interest(&["int"]),
                ),
                (
                    h.key_for_variable(v.int_q_var),
                    match_variable_model()
                        .chain(&[])
                        .of_interest(&[])
                        .unassigned(false),
                ),
            ],
        );
    }
}

mod rebase_forward {
    use super::*;

    #[test]
    fn reachability() {
        let h = set_up();
        let _v = state_vars();
        let reachable = FlowModel::new(Reachability::initial());
        let unreachable = reachable.set_unreachable();
        assert!(reachable.rebase_forward(&h, &reachable).ptr_eq(&reachable));
        assert!(
            reachable
                .rebase_forward(&h, &unreachable)
                .ptr_eq(&unreachable)
        );
        assert!(
            !unreachable
                .rebase_forward(&h, &reachable)
                .reachable
                .overall_reachable
        );
        assert!(same_promotion_info(
            &unreachable.rebase_forward(&h, &reachable).promotion_info,
            &unreachable.promotion_info
        ));
        assert!(
            unreachable
                .rebase_forward(&h, &unreachable)
                .ptr_eq(&unreachable)
        );
    }

    #[test]
    fn assignments() {
        let h = set_up();
        let _v = state_vars();
        let a = Var::new("a").with_type("int");
        let b = Var::new("b").with_type("int");
        let c = Var::new("c").with_type("int");
        let d = Var::new("d").with_type("int");
        let s0 = FlowModel::new(Reachability::initial())
            .declare_var(&h, a, false)
            .declare_var(&h, b, false)
            .declare_var(&h, c, false)
            .declare_var(&h, d, false);
        let s1 = s0
            .write_var(&h, None, a, view("int"), SsaNode::new())
            .write_var(&h, None, b, view("int"), SsaNode::new());
        let s2 = s0
            .write_var(&h, None, a, view("int"), SsaNode::new())
            .write_var(&h, None, c, view("int"), SsaNode::new());
        let result = s1.rebase_forward(&h, &s2);
        assert!(result.info_for_var(&h, a).assigned);
        assert!(result.info_for_var(&h, b).assigned);
        assert!(result.info_for_var(&h, c).assigned);
        assert!(!result.info_for_var(&h, d).assigned);
    }

    #[test]
    fn write_captured() {
        let h = set_up();
        let _v = state_vars();
        let a = Var::new("a").with_type("int");
        let b = Var::new("b").with_type("int");
        let c = Var::new("c").with_type("int");
        let d = Var::new("d").with_type("int");
        let s0 = FlowModel::new(Reachability::initial())
            .declare_var(&h, a, false)
            .declare_var(&h, b, false)
            .declare_var(&h, c, false)
            .declare_var(&h, d, false);
        // In s1, a and b are write captured.  In s2, a and c are.
        let s1 = s0.conservative_join_vars(&h, &[a, b], &[a, b]);
        let s2 = s1.conservative_join_vars(&h, &[a, c], &[a, c]);
        let result = s1.rebase_forward(&h, &s2);
        match_variable_model()
            .write_captured(true)
            .unassigned(false)
            .check(&result.info_for_var(&h, a));
        match_variable_model()
            .write_captured(true)
            .unassigned(false)
            .check(&result.info_for_var(&h, b));
        match_variable_model()
            .write_captured(true)
            .unassigned(false)
            .check(&result.info_for_var(&h, c));
        match_variable_model()
            .write_captured(false)
            .unassigned(true)
            .check(&result.info_for_var(&h, d));
    }

    #[test]
    fn write_captured_and_promoted() {
        let h = set_up();
        let _v = state_vars();
        let a = Var::new("a").with_type("num");
        let s0 = FlowModel::new(Reachability::initial()).declare_var(&h, a, false);
        // In s1, a is write captured.  In s2 it's promoted.
        let s1 = s0.conservative_join_vars(&h, &[a], &[a]);
        let s2 = promote_if_true(&h, &s0, a, "int");
        match_variable_model()
            .write_captured(true)
            .chain(&[])
            .check(&s1.rebase_forward(&h, &s2).info_for_var(&h, a));
        match_variable_model()
            .write_captured(true)
            .chain(&[])
            .check(&s2.rebase_forward(&h, &s1).info_for_var(&h, a));
    }

    #[test]
    fn promotion() {
        let h = set_up();
        let _v = state_vars();
        let check = |this_type: Option<&str>,
                     other_type: Option<&str>,
                     unsafe_: bool,
                     expected_chain: Option<&[&str]>| {
            let x = Var::new("x").with_type("Object?");
            let s0 = FlowModel::new(Reachability::initial()).declare_var(&h, x, true);
            let mut s1 = s0.clone();
            if unsafe_ {
                s1 = s1.write_var(&h, None, x, view("Object?"), SsaNode::new());
            }
            if let Some(this_type) = this_type {
                s1 = promote_if_true(&h, &s1, x, this_type);
            }
            let s2 = match other_type {
                None => s0.clone(),
                Some(other_type) => promote_if_true(&h, &s0, x, other_type),
            };
            let result = s2.rebase_forward(&h, &s1);
            match expected_chain {
                None => {
                    assert!(
                        unwrap_promotion_info(&result.promotion_info, &h)
                            .contains_key(&h.key_for_variable(x))
                    );
                    assert!(result.info_for_var(&h, x).promoted_types.is_empty());
                }
                Some(expected_chain) => {
                    assert_eq!(
                        type_strings(&result.info_for_var(&h, x).promoted_types),
                        expected_chain
                    );
                }
            }
        };

        check(None, None, false, None);
        check(None, None, true, None);
        check(Some("int"), None, false, Some(&["int"]));
        check(Some("int"), None, true, Some(&["int"]));
        check(None, Some("int"), false, Some(&["int"]));
        check(None, Some("int"), true, None);
        check(Some("int?"), Some("int"), false, Some(&["int?", "int"]));
        check(Some("int"), Some("int?"), false, Some(&["int"]));
        check(Some("int"), Some("String"), false, Some(&["int"]));
        check(Some("int?"), Some("int"), true, Some(&["int?"]));
        check(Some("int"), Some("int?"), true, Some(&["int"]));
        check(Some("int"), Some("String"), true, Some(&["int"]));
    }

    #[test]
    fn promotion_chains() {
        let h = set_up();
        let _v = state_vars();
        // Verify that the given promotion chain matches the expected list of
        // strings.
        #[track_caller]
        fn check_chain(chain: &[SharedTypeView<Type>], expected: &[&str]) {
            let strings = type_strings(chain);
            assert_eq!(strings, expected);
        }

        // Test the following scenario:
        // - Prior to the try/finally block, the sequence of promotions in
        //   [before] is done.
        // - During the try block, the sequence of promotions in [inTry] is
        //   done.
        // - During the finally block, the sequence of promotions in
        //   [inFinally] is done.
        // - After calling `restrict` to refine the state from the finally
        //   block, the expected promotion chain is [expectedResult].
        let check =
            |before: &[&str], in_try: &[&str], in_finally: &[&str], expected_result: &[&str]| {
                let x = Var::new("x").with_type("Object?");
                let mut initial_model =
                    FlowModel::new(Reachability::initial()).declare_var(&h, x, true);
                for t in before {
                    initial_model = promote_if_true(&h, &initial_model, x, t);
                }
                check_chain(&initial_model.info_for_var(&h, x).promoted_types, before);
                let mut try_model = initial_model.clone();
                for t in in_try {
                    try_model = promote_if_true(&h, &try_model, x, t);
                }
                let expected_try_chain: Vec<&str> = before.iter().chain(in_try).copied().collect();
                check_chain(
                    &try_model.info_for_var(&h, x).promoted_types,
                    &expected_try_chain,
                );
                let mut finally_model = initial_model.clone();
                for t in in_finally {
                    finally_model = promote_if_true(&h, &finally_model, x, t);
                }
                let expected_finally_chain: Vec<&str> =
                    before.iter().chain(in_finally).copied().collect();
                check_chain(
                    &finally_model.info_for_var(&h, x).promoted_types,
                    &expected_finally_chain,
                );
                let result = try_model.rebase_forward(&h, &finally_model);
                check_chain(&result.info_for_var(&h, x).promoted_types, expected_result);
                // And verify that the inputs are unchanged.
                check_chain(&initial_model.info_for_var(&h, x).promoted_types, before);
                check_chain(
                    &try_model.info_for_var(&h, x).promoted_types,
                    &expected_try_chain,
                );
                check_chain(
                    &finally_model.info_for_var(&h, x).promoted_types,
                    &expected_finally_chain,
                );
            };

        check(
            &["Object"],
            &["num", "int"],
            &["Iterable<dynamic>", "List<dynamic>"],
            &["Object", "Iterable<dynamic>", "List<dynamic>"],
        );
        check(
            &[],
            &["num", "int"],
            &["Iterable<dynamic>", "List<dynamic>"],
            &["Iterable<dynamic>", "List<dynamic>"],
        );
        check(
            &["Object"],
            &[],
            &["Iterable<dynamic>", "List<dynamic>"],
            &["Object", "Iterable<dynamic>", "List<dynamic>"],
        );
        check(
            &[],
            &[],
            &["Iterable<dynamic>", "List<dynamic>"],
            &["Iterable<dynamic>", "List<dynamic>"],
        );
        check(&["Object"], &["num", "int"], &[], &["Object", "num", "int"]);
        check(&[], &["num", "int"], &[], &["num", "int"]);
        check(&["Object"], &[], &[], &["Object"]);
        check(&[], &[], &[], &[]);
        check(
            &[],
            &["num", "int"],
            &["Object", "Iterable<dynamic>"],
            &["Object", "Iterable<dynamic>"],
        );
        check(&[], &["num", "int"], &["Object"], &["Object", "num", "int"]);
        check(
            &[],
            &["Object", "Iterable<dynamic>"],
            &["num", "int"],
            &["num", "int"],
        );
        check(&[], &["Object"], &["num", "int"], &["num", "int"]);
        check(&[], &["num"], &["Object", "int"], &["Object", "int"]);
        check(&[], &["int"], &["Object", "num"], &["Object", "num", "int"]);
        check(&[], &["Object", "int"], &["num"], &["num", "int"]);
        check(&[], &["Object", "num"], &["int"], &["int"]);
    }

    #[test]
    fn types_of_interest() {
        let h = set_up();
        let _v = state_vars();
        let a = Var::new("a").with_type("Object");
        let s0 = FlowModel::new(Reachability::initial()).declare_var(&h, a, false);
        let s1 = promote_if_false(&h, &s0, a, "int");
        let s2 = promote_if_false(&h, &s0, a, "String");
        match_variable_model()
            .of_interest(&["int", "String"])
            .check(&s1.rebase_forward(&h, &s2).info_for_var(&h, a));
        match_variable_model()
            .of_interest(&["int", "String"])
            .check(&s2.rebase_forward(&h, &s1).info_for_var(&h, a));
    }

    #[test]
    fn variable_present_in_one_state_but_not_the_other() {
        let h = set_up();
        let _v = state_vars();
        let x = Var::new("x").with_type("Object?");
        let s0 = FlowModel::new(Reachability::initial());
        let s1 = s0.declare_var(&h, x, true);
        assert!(s1.rebase_forward(&h, &s0).ptr_eq(&s1));
        assert!(s0.rebase_forward(&h, &s1).ptr_eq(&s1));
    }
}
