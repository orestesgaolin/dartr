// Dart source: pkg/_fe_analyzer_shared/test/flow_analysis/flow_analysis_test.dart
// (lines 4902-5450: groups 'joinPromotionChains', 'joinTypesOfInterest',
// 'join', 'inheritTested')

//! Dart groups `joinPromotionChains`, `joinTypesOfInterest`, `join` and
//! `inheritTested`: tests of the join operations of `PromotionModel` and
//! `FlowModel`.

use super::common::*;
use std::rc::Rc;

/// Dart `SharedTypeView(type)`.
fn view(type_: Type) -> SharedTypeView<Type> {
    SharedTypeView::new(type_)
}

/// Dart list literal `[a, b, ...]` of `SharedTypeView`s.
fn chain(types: &[SharedTypeView<Type>]) -> TypeList {
    Rc::from(types.to_vec())
}

mod join_promotion_chains {
    use super::*;

    /// The values of the Dart `setUp` of group `joinPromotionChains`.
    struct Types {
        double_type: SharedTypeView<Type>,
        int_type: SharedTypeView<Type>,
        num_type: SharedTypeView<Type>,
        object_type: SharedTypeView<Type>,
    }

    /// Dart `setUp` of group `joinPromotionChains`.
    fn types() -> Types {
        Types {
            double_type: view(ty("double")),
            int_type: view(ty("int")),
            num_type: view(ty("num")),
            object_type: view(ty("Object")),
        }
    }

    /// Dart `PromotionModel.joinPromotedTypes(chain1, chain2,
    /// h.typeOperations)`.
    fn join_promoted_types(
        h: &FlowAnalysisTestHarness,
        chain1: &TypeList,
        chain2: &TypeList,
    ) -> TypeList {
        PromotionModel::join_promoted_types(chain1, chain2, &h.type_operations())
    }

    #[test]
    fn should_handle_empty_promotion_chains() {
        let h = set_up();
        let t = types();
        assert!(join_promoted_types(&h, &chain(&[]), &chain(&[])).is_empty());
        assert!(join_promoted_types(&h, &chain(&[]), &chain(&[t.int_type])).is_empty());
        assert!(join_promoted_types(&h, &chain(&[t.int_type]), &chain(&[])).is_empty());
    }

    #[test]
    fn should_return_empty_list_if_there_are_no_common_types() {
        let h = set_up();
        let t = types();
        assert!(
            join_promoted_types(&h, &chain(&[t.int_type]), &chain(&[t.double_type])).is_empty()
        );
    }

    #[test]
    fn should_return_common_prefix_if_there_are_common_types() {
        let h = set_up();
        let t = types();
        expect_promotion_chain(
            &join_promoted_types(
                &h,
                &chain(&[t.object_type, t.int_type]),
                &chain(&[t.object_type, t.double_type]),
            ),
            &["Object"],
        );
        expect_promotion_chain(
            &join_promoted_types(
                &h,
                &chain(&[t.object_type, t.num_type, t.int_type]),
                &chain(&[t.object_type, t.num_type, t.double_type]),
            ),
            &["Object", "num"],
        );
    }

    #[test]
    fn should_return_an_input_if_it_is_a_prefix_of_the_other() {
        let h = set_up();
        let t = types();
        let prefix = chain(&[t.object_type, t.num_type]);
        let larger_chain = chain(&[t.object_type, t.num_type, t.int_type]);
        assert!(Rc::ptr_eq(
            &join_promoted_types(&h, &prefix, &larger_chain),
            &prefix
        ));
        assert!(Rc::ptr_eq(
            &join_promoted_types(&h, &larger_chain, &prefix),
            &prefix
        ));
        assert!(Rc::ptr_eq(
            &join_promoted_types(&h, &prefix, &prefix),
            &prefix
        ));
    }

    #[test]
    fn should_intersect() {
        let mut h = set_up();
        let _t = types();
        // F <: E <: D <: C <: B <: A
        let a = ty("A");
        let b = ty("B");
        let c = ty("C");
        let d = ty("D");
        let e = ty("E");
        let f = ty("F");
        h.add_super_interfaces("F", |_| {
            vec![ty("E"), ty("D"), ty("C"), ty("B"), ty("A"), ty("Object")]
        });
        h.add_super_interfaces("E", |_| {
            vec![ty("D"), ty("C"), ty("B"), ty("A"), ty("Object")]
        });
        h.add_super_interfaces("D", |_| vec![ty("C"), ty("B"), ty("A"), ty("Object")]);
        h.add_super_interfaces("C", |_| vec![ty("B"), ty("A"), ty("Object")]);
        h.add_super_interfaces("B", |_| vec![ty("A"), ty("Object")]);
        h.add_super_interfaces("A", |_| vec![ty("Object")]);

        /// The Dart `Matcher` argument of `check`.
        enum Matcher<'a> {
            /// `same(chain)`.
            Same(&'a TypeList),
            /// `_matchPromotionChain([...])`.
            Chain(&'a [&'a str]),
        }

        let check = |chain1: &TypeList, chain2: &TypeList, matcher: Matcher| {
            for result in [
                join_promoted_types(&h, chain1, chain2),
                join_promoted_types(&h, chain2, chain1),
            ] {
                match &matcher {
                    Matcher::Same(expected) => assert!(Rc::ptr_eq(&result, expected)),
                    Matcher::Chain(expected) => expect_promotion_chain(&result, expected),
                }
            }
        };

        {
            let chain1 = chain(&[view(a), view(b), view(c)]);
            let chain2 = chain(&[view(a), view(c)]);
            check(&chain1, &chain2, Matcher::Same(&chain2));
        }

        check(
            &chain(&[view(a), view(b), view(c), view(f)]),
            &chain(&[view(a), view(d), view(e), view(f)]),
            Matcher::Chain(&["A", "F"]),
        );

        check(
            &chain(&[view(a), view(b), view(e), view(f)]),
            &chain(&[view(a), view(c), view(d), view(f)]),
            Matcher::Chain(&["A", "F"]),
        );

        check(
            &chain(&[view(a), view(c), view(e)]),
            &chain(&[view(b), view(c), view(d)]),
            Matcher::Chain(&["C"]),
        );

        check(
            &chain(&[view(a), view(c), view(e), view(f)]),
            &chain(&[view(b), view(c), view(d), view(f)]),
            Matcher::Chain(&["C", "F"]),
        );

        check(
            &chain(&[view(a), view(b), view(c)]),
            &chain(&[view(a), view(b), view(d)]),
            Matcher::Chain(&["A", "B"]),
        );
    }
}

mod join_types_of_interest {
    use super::*;

    /// Dart `_makeTypes(typeNames)`.
    fn make_types(type_names: &[&str]) -> Rc<[Type]> {
        type_names.iter().map(|t| ty(t)).collect()
    }

    /// Dart `expect(types, _matchOfInterestSet([...]))` for a `List<Type>`.
    #[track_caller]
    fn expect_set(types: &Rc<[Type]>, expected: &[&str]) {
        let views: Vec<SharedTypeView<Type>> = types.iter().map(|t| view(*t)).collect();
        expect_of_interest_set(&views, expected);
    }

    #[test]
    fn simple_prefix() {
        let _h = set_up();
        let s1 = make_types(&["double", "int"]);
        let s2 = make_types(&["double", "int", "bool"]);
        let expected = ["double", "int", "bool"];
        expect_set(&PromotionModel::join_tested(&s1, &s2), &expected);
        expect_set(&PromotionModel::join_tested(&s2, &s1), &expected);
    }

    #[test]
    fn common_prefix() {
        let _h = set_up();
        let s1 = make_types(&["double", "int", "String"]);
        let s2 = make_types(&["double", "int", "bool"]);
        let expected = ["double", "int", "String", "bool"];
        expect_set(&PromotionModel::join_tested(&s1, &s2), &expected);
        expect_set(&PromotionModel::join_tested(&s2, &s1), &expected);
    }

    #[test]
    fn order_mismatch() {
        let _h = set_up();
        let s1 = make_types(&["double", "int"]);
        let s2 = make_types(&["int", "double"]);
        let expected = ["double", "int"];
        expect_set(&PromotionModel::join_tested(&s1, &s2), &expected);
        expect_set(&PromotionModel::join_tested(&s2, &s1), &expected);
    }

    #[test]
    fn small_common_prefix() {
        let _h = set_up();
        let s1 = make_types(&["int", "double", "String", "bool"]);
        let s2 = make_types(&["int", "List", "bool", "Future"]);
        let expected = ["int", "double", "String", "bool", "List", "Future"];
        expect_set(&PromotionModel::join_tested(&s1, &s2), &expected);
        expect_set(&PromotionModel::join_tested(&s2, &s1), &expected);
    }
}

mod join {
    use super::*;

    /// The values of the Dart `setUp` of group `join`.
    struct JoinVars {
        x: PromotionKey,
        y: PromotionKey,
        z: PromotionKey,
        w: PromotionKey,
        int_type: Type,
        int_q_type: Type,
        string_type: Type,
    }

    /// Dart `setUp` of group `join`.
    fn join_vars(h: &FlowAnalysisTestHarness) -> JoinVars {
        JoinVars {
            x: h.key_for_variable(Var::new("x").with_type("Object?")),
            y: h.key_for_variable(Var::new("y").with_type("Object?")),
            z: h.key_for_variable(Var::new("z").with_type("Object?")),
            w: h.key_for_variable(Var::new("w").with_type("Object?")),
            int_type: ty("int"),
            int_q_type: ty("int?"),
            string_type: ty("String"),
        }
    }

    /// Dart `model(promotionChain, typesOfInterest: ..., assigned: ...)`.
    fn model_with(
        promotion_chain: TypeList,
        types_of_interest: Option<TypeList>,
        assigned: bool,
    ) -> PromotionModel {
        PromotionModel::new(
            promotion_chain.clone(),
            types_of_interest.unwrap_or(promotion_chain),
            assigned,
            !assigned,
            Some(SsaNode::new()),
            None,
        )
    }

    /// Dart `model(promotionChain)`.
    fn model(promotion_chain: &[SharedTypeView<Type>]) -> PromotionModel {
        model_with(chain(promotion_chain), None, false)
    }

    /// Dart `FlowModel.joinPromotionInfo(h, first, second)`.
    fn join_promotion_info(
        h: &FlowAnalysisTestHarness,
        first: &FlowModel,
        second: &FlowModel,
    ) -> FlowModel {
        FlowModel::join_promotion_info(h, first, second)
    }

    mod without_input_reuse {
        use super::*;

        #[test]
        fn promoted_with_unpromoted() {
            let h = set_up();
            let v = join_vars(&h);
            let s0 = FlowModel::new(Reachability::initial());
            let s1 = s0.set_info(&h, &[(v.x, model(&[view(v.int_type)])), (v.y, model(&[]))]);
            let s2 = s0.set_info(&h, &[(v.x, model(&[])), (v.y, model(&[view(v.int_type)]))]);
            expect_promotion_models(
                &unwrap_promotion_info(&join_promotion_info(&h, &s1, &s2).promotion_info, &h),
                &[
                    (v.x, match_variable_model().chain(&[]).of_interest(&["int"])),
                    (v.y, match_variable_model().chain(&[]).of_interest(&["int"])),
                ],
            );
        }
    }

    mod should_re_use_an_input_if_possible {
        use super::*;

        #[test]
        fn identical_inputs() {
            let h = set_up();
            let v = join_vars(&h);
            let s0 = FlowModel::new(Reachability::initial());
            let s1 = s0.set_info(
                &h,
                &[
                    (v.x, model(&[view(v.int_type)])),
                    (v.y, model(&[view(v.string_type)])),
                ],
            );
            assert!(join_promotion_info(&h, &s1, &s1).ptr_eq(&s1));
        }

        #[test]
        fn one_input_empty() {
            let h = set_up();
            let v = join_vars(&h);
            let s0 = FlowModel::new(Reachability::initial());
            let s1 = s0.set_info(
                &h,
                &[
                    (v.x, model(&[view(v.int_type)])),
                    (v.y, model(&[view(v.string_type)])),
                ],
            );
            let s2 = s0.clone();
            // Dart `const Null expected = null;` and `same(expected)`.
            assert!(join_promotion_info(&h, &s1, &s2).promotion_info.is_none());
            assert!(join_promotion_info(&h, &s2, &s1).promotion_info.is_none());
        }

        #[test]
        fn promoted_with_unpromoted() {
            let h = set_up();
            let v = join_vars(&h);
            let s0 = FlowModel::new(Reachability::initial());
            let s1 = s0.set_info(&h, &[(v.x, model(&[view(v.int_type)]))]);
            let s2 = s0.set_info(&h, &[(v.x, model(&[]))]);
            let expected = [(v.x, match_variable_model().chain(&[]).of_interest(&["int"]))];
            expect_promotion_models(
                &unwrap_promotion_info(&join_promotion_info(&h, &s1, &s2).promotion_info, &h),
                &expected,
            );
            expect_promotion_models(
                &unwrap_promotion_info(&join_promotion_info(&h, &s2, &s1).promotion_info, &h),
                &expected,
            );
        }

        #[test]
        fn related_type_chains() {
            let h = set_up();
            let v = join_vars(&h);
            let s0 = FlowModel::new(Reachability::initial());
            let s1 = s0.set_info(&h, &[(v.x, model(&[view(v.int_q_type), view(v.int_type)]))]);
            let s2 = s0.set_info(&h, &[(v.x, model(&[view(v.int_q_type)]))]);
            let expected = [(
                v.x,
                match_variable_model()
                    .chain(&["int?"])
                    .of_interest(&["int?", "int"]),
            )];
            expect_promotion_models(
                &unwrap_promotion_info(&join_promotion_info(&h, &s1, &s2).promotion_info, &h),
                &expected,
            );
            expect_promotion_models(
                &unwrap_promotion_info(&join_promotion_info(&h, &s2, &s1).promotion_info, &h),
                &expected,
            );
        }

        #[test]
        fn unrelated_type_chains() {
            let h = set_up();
            let v = join_vars(&h);
            let s0 = FlowModel::new(Reachability::initial());
            let s1 = s0.set_info(&h, &[(v.x, model(&[view(v.int_type)]))]);
            let s2 = s0.set_info(&h, &[(v.x, model(&[view(v.string_type)]))]);
            let expected = [(
                v.x,
                match_variable_model()
                    .chain(&[])
                    .of_interest(&["String", "int"]),
            )];
            expect_promotion_models(
                &unwrap_promotion_info(&join_promotion_info(&h, &s1, &s2).promotion_info, &h),
                &expected,
            );
            expect_promotion_models(
                &unwrap_promotion_info(&join_promotion_info(&h, &s2, &s1).promotion_info, &h),
                &expected,
            );
        }

        #[test]
        fn sub_map() {
            let h = set_up();
            let v = join_vars(&h);
            let s0 = FlowModel::new(Reachability::initial());
            let x_model = model(&[view(v.int_type)]);
            let s1 = s0.set_info(
                &h,
                &[(v.x, x_model.clone()), (v.y, model(&[view(v.string_type)]))],
            );
            let s2 = s0.set_info(&h, &[(v.x, x_model.clone())]);
            // Dart `{x: xModel}`: `PromotionModel` does not override `==`.
            let expected = [(v.x, same_model(&x_model))];
            expect_promotion_models(
                &unwrap_promotion_info(&join_promotion_info(&h, &s1, &s2).promotion_info, &h),
                &expected,
            );
            expect_promotion_models(
                &unwrap_promotion_info(&join_promotion_info(&h, &s2, &s1).promotion_info, &h),
                &expected,
            );
        }

        #[test]
        fn sub_map_with_matched_subtype() {
            let h = set_up();
            let v = join_vars(&h);
            let s0 = FlowModel::new(Reachability::initial());
            let s1 = s0.set_info(
                &h,
                &[
                    (v.x, model(&[view(v.int_q_type), view(v.int_type)])),
                    (v.y, model(&[view(v.string_type)])),
                ],
            );
            let s2 = s0.set_info(&h, &[(v.x, model(&[view(v.int_q_type)]))]);
            let expected = [(
                v.x,
                match_variable_model()
                    .chain(&["int?"])
                    .of_interest(&["int?", "int"]),
            )];
            expect_promotion_models(
                &unwrap_promotion_info(&join_promotion_info(&h, &s1, &s2).promotion_info, &h),
                &expected,
            );
            expect_promotion_models(
                &unwrap_promotion_info(&join_promotion_info(&h, &s2, &s1).promotion_info, &h),
                &expected,
            );
        }

        #[test]
        fn sub_map_with_mismatched_subtype() {
            let h = set_up();
            let v = join_vars(&h);
            let s0 = FlowModel::new(Reachability::initial());
            let s1 = s0.set_info(
                &h,
                &[
                    (v.x, model(&[view(v.int_q_type)])),
                    (v.y, model(&[view(v.string_type)])),
                ],
            );
            let s2 = s0.set_info(&h, &[(v.x, model(&[view(v.int_q_type), view(v.int_type)]))]);
            let expected = [(
                v.x,
                match_variable_model()
                    .chain(&["int?"])
                    .of_interest(&["int?", "int"]),
            )];
            expect_promotion_models(
                &unwrap_promotion_info(&join_promotion_info(&h, &s1, &s2).promotion_info, &h),
                &expected,
            );
            expect_promotion_models(
                &unwrap_promotion_info(&join_promotion_info(&h, &s2, &s1).promotion_info, &h),
                &expected,
            );
        }

        #[test]
        fn assigned() {
            let h = set_up();
            let v = join_vars(&h);
            let s0 = FlowModel::new(Reachability::initial());
            let unassigned = model_with(chain(&[]), None, false);
            let assigned = model_with(chain(&[]), None, true);
            let s1 = s0.set_info(
                &h,
                &[
                    (v.x, assigned.clone()),
                    (v.y, assigned.clone()),
                    (v.z, unassigned.clone()),
                    (v.w, unassigned.clone()),
                ],
            );
            let s2 = s0.set_info(
                &h,
                &[
                    (v.x, assigned.clone()),
                    (v.y, unassigned.clone()),
                    (v.z, assigned.clone()),
                    (v.w, unassigned.clone()),
                ],
            );
            let joined = join_promotion_info(&h, &s1, &s2);
            expect_promotion_models(
                &unwrap_promotion_info(&joined.promotion_info, &h),
                &[
                    (v.x, same_model(&assigned)),
                    (
                        v.y,
                        match_variable_model()
                            .chain(&[])
                            .assigned(false)
                            .unassigned(false),
                    ),
                    (
                        v.z,
                        match_variable_model()
                            .chain(&[])
                            .assigned(false)
                            .unassigned(false),
                    ),
                    (v.w, same_model(&unassigned)),
                ],
            );
        }

        #[test]
        fn write_captured() {
            let h = set_up();
            let v = join_vars(&h);
            let s0 = FlowModel::new(Reachability::initial());
            let int_q_model = model(&[view(v.int_q_type)]);
            let write_captured_model = int_q_model.write_capture();
            let s1 = s0.set_info(
                &h,
                &[
                    (v.x, write_captured_model.clone()),
                    (v.y, write_captured_model.clone()),
                    (v.z, int_q_model.clone()),
                    (v.w, int_q_model.clone()),
                ],
            );
            let s2 = s0.set_info(
                &h,
                &[
                    (v.x, write_captured_model.clone()),
                    (v.y, int_q_model.clone()),
                    (v.z, write_captured_model.clone()),
                    (v.w, int_q_model.clone()),
                ],
            );
            let joined = join_promotion_info(&h, &s1, &s2);
            expect_promotion_models(
                &unwrap_promotion_info(&joined.promotion_info, &h),
                &[
                    (v.x, same_model(&write_captured_model)),
                    (v.y, same_model(&write_captured_model)),
                    (v.z, same_model(&write_captured_model)),
                    (v.w, same_model(&int_q_model)),
                ],
            );
        }
    }
}

mod inherit_tested {
    use super::*;

    /// The values of the Dart `setUp` of group `inheritTested`.
    struct InheritVars {
        x: PromotionKey,
        int_type: Type,
        string_type: Type,
    }

    /// Dart `setUp` of group `inheritTested`.
    fn inherit_vars(h: &FlowAnalysisTestHarness) -> InheritVars {
        InheritVars {
            x: h.key_for_variable(Var::new("x").with_type("Object?")),
            int_type: ty("int"),
            string_type: ty("String"),
        }
    }

    /// Dart `model(typesOfInterest)`.
    fn model(types_of_interest: &[SharedTypeView<Type>]) -> PromotionModel {
        PromotionModel::new(
            chain(&[]),
            chain(types_of_interest),
            true,
            false,
            Some(SsaNode::new()),
            None,
        )
    }

    #[test]
    fn inherits_types_of_interest_from_other() {
        let h = set_up();
        let v = inherit_vars(&h);
        let m0 = FlowModel::new(Reachability::initial());
        let m1 = m0.set_info(&h, &[(v.x, model(&[view(v.int_type)]))]);
        let m2 = m0.set_info(&h, &[(v.x, model(&[view(v.string_type)]))]);
        let inherited = m1.inherit_tested(&h, &m2);
        expect_of_interest_set(
            &promotion_info_get(&inherited.promotion_info, &h, v.x)
                .unwrap()
                .tested,
            &["int", "String"],
        );
    }

    #[test]
    fn handles_variable_missing_from_other() {
        let h = set_up();
        let v = inherit_vars(&h);
        let m0 = FlowModel::new(Reachability::initial());
        let m1 = m0.set_info(&h, &[(v.x, model(&[view(v.int_type)]))]);
        let m2 = m0.clone();
        assert!(m1.inherit_tested(&h, &m2).ptr_eq(&m1));
    }

    #[test]
    fn returns_identical_model_when_no_changes() {
        let h = set_up();
        let v = inherit_vars(&h);
        let m0 = FlowModel::new(Reachability::initial());
        let m1 = m0.set_info(&h, &[(v.x, model(&[view(v.int_type)]))]);
        let m2 = m0.set_info(&h, &[(v.x, model(&[view(v.int_type)]))]);
        assert!(m1.inherit_tested(&h, &m2).ptr_eq(&m1));
    }
}
