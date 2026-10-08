// Dart source: pkg/_fe_analyzer_shared/test/flow_analysis/flow_analysis_test.dart
// (lines 3721-3884: group 'State', groups 'setUnreachable', 'split',
// 'unsplit', 'unsplitTo', 'tryPromoteForTypeCheck')

//! Dart group `State` (first part): tests that call `FlowModel` methods
//! directly, with [`FlowAnalysisTestHarness`] as the `FlowModelHelper`.

use super::common::*;

/// The variables of the Dart `setUp` of group `State`.
struct StateVars {
    int_var: Var,
    int_q_var: Var,
    object_q_var: Var,
    #[allow(dead_code)]
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

mod set_unreachable {
    use super::*;

    #[test]
    fn unchanged() {
        let _h = set_up();
        let unreachable = FlowModel::new(Reachability::initial().set_unreachable());
        assert!(unreachable.set_unreachable().ptr_eq(&unreachable));
    }

    #[test]
    fn changed() {
        let _h = set_up();
        let reachable = FlowModel::new(Reachability::initial());
        let check = |initial: &FlowModel| {
            let s = initial.set_unreachable();
            assert!(!s.ptr_eq(initial));
            assert!(!s.reachable.overall_reachable);
            assert!(same_promotion_info(
                &s.promotion_info,
                &initial.promotion_info
            ));
        };
        check(&reachable);
    }
}

#[test]
fn split() {
    let _h = set_up();
    let s1 = FlowModel::new(Reachability::initial());
    let s2 = s1.split();
    assert!(Reachability::opt_ptr_eq(
        s2.reachable.parent.as_ref(),
        Some(&s1.reachable)
    ));
}

#[test]
fn unsplit() {
    let _h = set_up();
    let s1 = FlowModel::new(Reachability::initial().split());
    let s2 = s1.unsplit();
    assert!(s2.reachable.ptr_eq(&Reachability::initial()));
}

mod unsplit_to {
    use super::*;

    #[test]
    fn no_change() {
        let _h = set_up();
        let s1 = FlowModel::new(Reachability::initial().split());
        let result = s1.unsplit_to(s1.reachable.parent.as_ref().unwrap());
        assert!(result.ptr_eq(&s1));
    }

    #[test]
    fn unsplit_once_reachable() {
        let _h = set_up();
        let s1 = FlowModel::new(Reachability::initial().split());
        let s2 = s1.split();
        let result = s2.unsplit_to(s1.reachable.parent.as_ref().unwrap());
        assert!(result.reachable.ptr_eq(&s1.reachable));
    }

    #[test]
    fn unsplit_once_unreachable() {
        let _h = set_up();
        let s1 = FlowModel::new(Reachability::initial().split());
        let s2 = s1.split().set_unreachable();
        let result = s2.unsplit_to(s1.reachable.parent.as_ref().unwrap());
        assert!(!result.reachable.locally_reachable);
        assert!(Reachability::opt_ptr_eq(
            result.reachable.parent.as_ref(),
            s1.reachable.parent.as_ref()
        ));
    }

    #[test]
    fn unsplit_twice_reachable() {
        let _h = set_up();
        let s1 = FlowModel::new(Reachability::initial().split());
        let s2 = s1.split();
        let s3 = s2.split();
        let result = s3.unsplit_to(s1.reachable.parent.as_ref().unwrap());
        assert!(result.reachable.ptr_eq(&s1.reachable));
    }

    #[test]
    fn unsplit_twice_top_unreachable() {
        let _h = set_up();
        let s1 = FlowModel::new(Reachability::initial().split());
        let s2 = s1.split();
        let s3 = s2.split().set_unreachable();
        let result = s3.unsplit_to(s1.reachable.parent.as_ref().unwrap());
        assert!(!result.reachable.locally_reachable);
        assert!(Reachability::opt_ptr_eq(
            result.reachable.parent.as_ref(),
            s1.reachable.parent.as_ref()
        ));
    }

    #[test]
    fn unsplit_twice_previous_unreachable() {
        let _h = set_up();
        let s1 = FlowModel::new(Reachability::initial().split());
        let s2 = s1.split().set_unreachable();
        let s3 = s2.split();
        let result = s3.unsplit_to(s1.reachable.parent.as_ref().unwrap());
        assert!(!result.reachable.locally_reachable);
        assert!(Reachability::opt_ptr_eq(
            result.reachable.parent.as_ref(),
            s1.reachable.parent.as_ref()
        ));
    }
}

mod try_promote_for_type_check {
    use super::*;

    /// Dart `s._tryPromoteForTypeCheck(h, variable, type).ifTrue`.
    fn if_true(
        h: &FlowAnalysisTestHarness,
        s: &FlowModel,
        variable: Var,
        type_: &str,
    ) -> FlowModel {
        s.try_promote_for_type_check_var(h, variable, type_)
            .if_true
            .clone()
    }

    #[test]
    fn unpromoted_unchanged_same() {
        let h = set_up();
        let v = state_vars();
        let s1 = FlowModel::new(Reachability::initial());
        let s2 = if_true(&h, &s1, v.int_var, "int");
        assert!(s2.ptr_eq(&s1));
    }

    #[test]
    fn unpromoted_unchanged_supertype() {
        let h = set_up();
        let v = state_vars();
        let s1 = FlowModel::new(Reachability::initial());
        let s2 = if_true(&h, &s1, v.int_var, "Object");
        assert!(s2.ptr_eq(&s1));
    }

    #[test]
    fn unpromoted_unchanged_unrelated() {
        let h = set_up();
        let v = state_vars();
        let s1 = FlowModel::new(Reachability::initial());
        let s2 = if_true(&h, &s1, v.int_var, "String");
        assert!(s2.ptr_eq(&s1));
    }

    #[test]
    fn unpromoted_subtype() {
        let h = set_up();
        let v = state_vars();
        let s1 = FlowModel::new(Reachability::initial());
        let s2 = if_true(&h, &s1, v.int_q_var, "int");
        assert!(s2.reachable.overall_reachable);
        expect_promotion_models(
            &unwrap_promotion_info(&s2.promotion_info, &h),
            &[(
                h.key_for_variable(v.int_q_var),
                match_variable_model().chain(&["int"]).of_interest(&["int"]),
            )],
        );
    }

    #[test]
    fn promoted_unchanged_same() {
        let h = set_up();
        let v = state_vars();
        let s0 = FlowModel::new(Reachability::initial());
        let s1 = if_true(&h, &s0, v.object_q_var, "int");
        let s2 = if_true(&h, &s1, v.object_q_var, "int");
        assert!(s2.ptr_eq(&s1));
    }

    #[test]
    fn promoted_unchanged_supertype() {
        let h = set_up();
        let v = state_vars();
        let s0 = FlowModel::new(Reachability::initial());
        let s1 = if_true(&h, &s0, v.object_q_var, "int");
        let s2 = if_true(&h, &s1, v.object_q_var, "Object");
        assert!(s2.ptr_eq(&s1));
    }

    #[test]
    fn promoted_unchanged_unrelated() {
        let h = set_up();
        let v = state_vars();
        let s0 = FlowModel::new(Reachability::initial());
        let s1 = if_true(&h, &s0, v.object_q_var, "int");
        let s2 = if_true(&h, &s1, v.object_q_var, "String");
        assert!(s2.ptr_eq(&s1));
    }

    #[test]
    fn promoted_subtype() {
        let h = set_up();
        let v = state_vars();
        let s0 = FlowModel::new(Reachability::initial());
        let s1 = if_true(&h, &s0, v.object_q_var, "int?");
        let s2 = if_true(&h, &s1, v.object_q_var, "int");
        assert!(s2.reachable.overall_reachable);
        expect_promotion_models(
            &unwrap_promotion_info(&s2.promotion_info, &h),
            &[(
                h.key_for_variable(v.object_q_var),
                match_variable_model()
                    .chain(&["int?", "int"])
                    .of_interest(&["int?", "int"]),
            )],
        );
    }
}
