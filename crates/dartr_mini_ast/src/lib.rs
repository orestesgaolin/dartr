//! dartr_mini_ast: test support crate (dev-only). Port of the "mini AST"
//! test harness of `pkg/_fe_analyzer_shared/test/`, shared by the flow
//! analysis tests (`crates/dartr_flow/tests/flow_analysis/`) and the type
//! analyzer tests (`crates/dartr_type_analyzer/tests/`).
//!
//! | Dart file (`_fe_analyzer_shared/test/...`) | module |
//! |---|---|
//! | `mini_ast.dart` (AST classes, builder functions, `Proto*` mixins) | [`node`] |
//! | `mini_ast.dart` (`Harness`, `_MiniAstTypeAnalyzer`, `_MiniAstErrors`, `PreVisitor`, `_VariableBinder`) | [`harness`] |
//! | `mini_ast.dart` (`MiniAstOperations`), plus the `FlowTypes` of the mini AST ([`operations::MiniAstTypes`]) | [`operations`] |
//! | `mini_types.dart` | [`mini_types`] |
//! | `mini_ir.dart` | [`mini_ir`] |
//! | `mini_type_constraint_gatherer.dart` | [`mini_type_constraint_gatherer`] |
//! | `flow_analysis/flow_analysis_mini_ast.dart` | [`flow_analysis_mini_ast`] |
//! | (none: Rust replacements of Dart language features) | [`test_util`] |
//!
//! The harness runs the real shared type analyzer (`dartr_type_analyzer`)
//! and the real flow analysis (`dartr_flow::flow_analysis_impl`).
//!
//! # Translation guide: `flow_analysis_test.dart` to Rust
//!
//! The translated tests live in `crates/dartr_flow/tests/flow_analysis/`,
//! one submodule per range of lines of the Dart file (see `main.rs` there).
//! For your line range, add a file `<group>_partN.rs`, register it in
//! `main.rs` (module list and the table in its doc comment), start the file
//! with `// Dart source: ... (lines A-B: ...)` and `use super::common::*;`.
//! `common.rs` re-exports this crate and holds the shared helpers. Translate
//! every Dart test of the range, in Dart order, and keep the Dart comments.
//!
//! ## Test structure
//!
//! | Dart | Rust |
//! |---|---|
//! | `group('Foo bar:', () { ... })` | `mod foo_bar { use super::*; ... }` (snake case) |
//! | `test('equalityOp(x != null) promotes', () { ... })` | `#[test] fn equality_op_x_not_eq_null_promotes() { ... }` (snake case; operators spelled out: `!=` → `not_eq`, `==` → `eq`; names unique in the module) |
//! | `setUp` of `main` (`TypeRegistry.init()`, `addInterfaceTypeName('A')`..`('F')`, `h = FlowAnalysisTestHarness()`) and `tearDown` | `let mut h = set_up();` as the first line of each test (the registry is un-initialized when `h` is dropped) |
//! | `setUp` of a nested group (e.g. `State`: `intVar = Var('x')..type = Type('int')`) | a helper `fn` in the module that returns the values (see `state_vars()` in `state_part1.rs`), called in each test after `set_up()` |
//! | values created in a `group` body (e.g. `var unreachable = FlowModel(...)`) | create them inside each test |
//! | tests generated in a Dart `for` loop | one `#[test]` per value, each calling a shared helper `fn`; for many values, a local `macro_rules!` that generates the test functions |
//! | helper functions defined in a test or group (`void _check(...)`) | a nested `fn` or closure with the same snake case name (`check`) |
//! | `expect(a, b)` | `assert_eq!(a, b)` |
//! | `expect(a, same(b))` / `isNot(same(b))` | `assert!(a.ptr_eq(&b))` / `assert!(!a.ptr_eq(&b))` (`FlowModel`, `Reachability`, `SsaNode`, `PromotionModel`, `ExpressionInfo`); for `Option`s `same_ssa(a, b)`, `opt_ptr_eq(a, b)`, `Reachability::opt_ptr_eq(a, b)` |
//! | `expect(nodes[x], isNotNull)` / `isNull` | `assert!(nodes.get(x).is_some())` / `is_none()` |
//! | `expect(() => f(), _asserts)` | `expect_asserts(move \|\| f())` |
//! | `late SsaNode s;` assigned in a callback | `let s = Late::<SsaNode>::new();`, assign `s.set(v)`, read `s.get()` ([`test_util::Late`] is `Copy`, so `move` closures capture it) |
//! | `fail('...')` | `panic!("...")` |
//!
//! ## Harness
//!
//! | Dart | Rust |
//! |---|---|
//! | `h.run([...])` | `h.run(vec![...])` (use `nodes![...]` if an item is a bare `Var`) |
//! | `h.run([...], expectedErrors: {'a', 'b'})` | `h.run_with(vec![...], errors(&["a", "b"]))` (`errors` is in `common.rs`) |
//! | `h.run([...], errorRecoveryOK: true)` / `bodyContext: BodyContext(isAsync: b, yieldContext: Type('T'))` | `h.run_with(vec![...], RunOptions { error_recovery_ok: true, ..RunOptions::default() })` / `body_context: Some(BodyContext::new(b, "T"))` |
//! | `h.addMember('C', '_f', 'int?')` | `h.add_member("C", "_f", Some("int?"), false, None)` |
//! | `h.addMember('C', '_f', 'int?', promotable: true)` | `h.add_member("C", "_f", Some("int?"), true, None)` |
//! | `h.addMember('C', 'f', 'int?', whyNotPromotable: PropertyNonPromotabilityReason.isNotFinal)` | `h.add_member("C", "f", Some("int?"), false, Some(PropertyNonPromotabilityReason::IsNotFinal))` |
//! | `h.addMember('C', 'f', null)` | `h.add_member("C", "f", None, false, None)` |
//! | `h.thisType = 'C'` | `h.set_this_type("C")` |
//! | `h.addSuperInterfaces('C', (_) => [Type('Object')])` | `h.add_super_interfaces("C", \|_\| vec![ty("Object")])` |
//! | `h.addExhaustiveness('C', true)`, `h.addDownwardInfer(name: 'C', context: 'C', result: 'C')`, `h.addExtensionTypeErasure('E', 'int')`, `h.addLub(...)`, `h.addPromotionException(...)` | `h.add_exhaustiveness("C", true)`, `h.add_downward_infer("C", "C", "C")`, `h.add_extension_type_erasure("E", "int")`, `h.add_lub(...)`, `h.add_promotion_exception(...)` |
//! | `h.disableSoundFlowAnalysis()` (and the other `disable...`) | `h.disable_sound_flow_analysis()`, `disable_field_promotion`, `disable_inference_update3`, `disable_inference_update4`, `disable_patterns`, `disable_respect_implicitly_typed_var_initializers`, `disable_this_promotion` |
//! | `h.computeTypeAnalyzerOptions()` | `h.compute_type_analyzer_options()` |
//! | `h.typeOperations` | `h.type_operations()` |
//! | `h.promotionKeyStore.keyForVariable(x)` | `h.key_for_variable(x)` |
//! | `h.reader` | `h.reader` (a `RefCell`) |
//! | `h` passed as a `FlowModelHelper` (`s.declare(h, ...)`) | `&h` ([`flow_analysis_mini_ast::FlowAnalysisTestHarness`] implements `FlowModelHelper`) |
//! | `FlowAnalysis<Node, Statement, Expression, Var>(h.typeOperations, AssignedVariables<Node, Var>(), typeAnalyzerOptions: ...)` | `FlowAnalysisImpl::<MiniAstTypes>::new(h.type_operations(), AssignedVariablesImpl::new(), h.compute_type_analyzer_options())` |
//! | `TypeRegistry.addTypeParameter('T')` | `TypeRegistry::add_type_parameter("T")` |
//! | `Type('int?')` | `ty("int?")` (or `Type::parse("int?")`) |
//! | `SharedTypeView(Type('int'))` | `SharedTypeView::new(ty("int"))` |
//! | `PropertyNonPromotabilityReason.isNotFinal`, `NonPromotionDocumentationLink.write` | `PropertyNonPromotabilityReason::IsNotFinal`, `NonPromotionDocumentationLink::Write` |
//!
//! ## Variables
//!
//! | Dart | Rust |
//! |---|---|
//! | `var x = Var('x');` | `let x = Var::new("x");` |
//! | `Var('x', isFinal: true)` | `Var::new("x").with_final(true)` |
//! | `Var('x', identity: 'x1')` | `Var::new("x").with_identity("x1")` |
//! | `Var('x')..type = Type('int')` | `Var::new("x").with_type("int")` |
//! | `PatternVariableJoin('x', expectedComponents: [x1, x2])` | `Var::join("x", vec![x1, x2])` (`.with_identity(...)` for `identity:`) |
//! | `x` where an expression is expected | `x` (every `impl IntoNode` parameter accepts a `Var`); inside `vec![...]` use `x.expr()` or the `nodes![...]` macro |
//! | `x.write(e)`, `x.pattern()`, `x.postIncDec()`, `x.preIncDec()`, `x.readAndCheckPromotedType(cb)` | `x.write(e)`, `x.pattern()`, `x.post_inc_dec()`, `x.pre_inc_dec()`, `x.read_and_check_promoted_type(\|t\| ...)` |
//! | `x.pattern(type: 'int', expectInferredType: 'int')` | `x.pattern().with_declared_type("int").with_expect_inferred_type("int")` |
//! | `x.eq(...)`, `x.as_(...)`, `x.property(...)`, ... | the same as for expressions: `Var` has the `ProtoExpression` methods |
//!
//! ## Statements
//!
//! | Dart | Rust |
//! |---|---|
//! | `declare(x, type: 'int?', initializer: expr('int?'))` | `declare(x).with_declared_type("int?").with_initializer(expr("int?"))` |
//! | `declare(x, isLate: true, isFinal: true, expectInferredType: 'int')` | `declare(x).with_late().with_final().with_expect_inferred_type("int")` |
//! | `if_(c, [a])` / `if_(c, [a], [b])` | `if_(c, vec![a])` / `if_else(c, vec![a], vec![b])` |
//! | `ifCase(e, p, [a])` / `ifCase(e, p, [a], [b])` | `if_case(e, p, vec![a], None)` / `if_case(e, p, vec![a], Some(vec![b]))` |
//! | `assert_(c)` / `assert_(c, m)` | `assert_(c, None)` / `assert_(c, m)` |
//! | `block([...])` | `block(vec![...])` |
//! | `break_()` / `break_(l)` / `continue_()` / `continue_(l)` | `break_(None)` / `break_(Some(l))` / `continue_(None)` / `continue_(Some(l))` |
//! | `Label('l')`, `Label.unbound()`, `l.thenStmt(s)` | `Label::new("l")`, `Label::unbound()`, `l.then_stmt(s)` |
//! | `do_([...], c)` / `while_(c, [...])` | `do_(vec![...], c)` / `while_(c, vec![...])` |
//! | `for_(init, cond, upd, [...])` (any part may be `null`) | `for_(init, cond, upd, vec![...], false)` (`None` for an absent part) |
//! | `for_(..., forCollection: true)` | `for_(..., vec![...], true)` |
//! | `forEachWithNonVariable(e, [...])`, `forEachWithVariableSet(x, e, [...])`, `forEachWithVariableDecl(x, e, [...])` | `for_each_with_non_variable(e, vec![...])`, `for_each_with_variable_set(x, e, vec![...])`, `for_each_with_variable_decl(x, e, vec![...])` |
//! | `patternForIn(p, e, [...])` / `hasAwait: true` | `pattern_for_in(p, e, vec![...], false)` / `..., true)` |
//! | `patternVariableDeclaration(p, e)` / `isFinal: true` | `pattern_variable_declaration(p, e, false)` / `..., true)` |
//! | `return_()` | `return_()` |
//! | `switch_(e, [cases])` | `switch_(e, vec![cases])` |
//! | `switch_(..., isLegacyExhaustive: b, expectHasDefault: b, expectIsExhaustive: b, expectLastCaseTerminates: b, expectRequiresExhaustivenessValidation: b, expectScrutineeType: 't')` | `switch_(...).with_legacy_exhaustive(b).expect_has_default(b).expect_is_exhaustive(b).expect_last_case_terminates(b).expect_requires_exhaustiveness_validation(b).expect_scrutinee_type("t")` |
//! | `p.then([...])`, `default_.then([...])` | `p.then(vec![...])`, `default_().then(vec![...])` |
//! | `switchStatementMember([p1, p2], [...])` / `hasLabels: true` | `switch_statement_member(vec![p1, p2], vec![...], false)` / `..., true)` |
//! | `p.when(g)` | `p.when(g)` (`p.when(None)` for Dart `when(null)`) |
//! | `try_([...]).catch_(type: 'T', exception: e, stackTrace: st, body: [...]).finally_([...])` | `try_(vec![...]).catch_(Some("T"), Some(e), Some(st), vec![...]).finally_(vec![...])` (`None` for an absent named parameter) |
//! | `yield_(e)` / `yield_(e, isYieldStar: true)` | `yield_(e, false)` / `yield_(e, true)` |
//! | `e.thenStmt(s)` | `e.then_stmt(s)` |
//! | `s.checkIR('...')` | `s.check_ir("...")` |
//!
//! ## Expressions
//!
//! | Dart | Rust |
//! |---|---|
//! | `expr('int?')` | `expr("int?")` |
//! | `nullLiteral`, `this_`, `default_` (getters) | `null_literal()`, `this_()`, `default_()` |
//! | `intLiteral(1)`, `booleanLiteral(true)` | `int_literal(1)`, `boolean_literal(true)` |
//! | `e.eq(f)`, `e.notEq(f)`, `e.and(f)`, `e.or(f)`, `e.ifNull(f)` | `e.eq(f)`, `e.not_eq(f)`, `e.and(f)`, `e.or(f)`, `e.if_null(f)` |
//! | `e.as_('int')`, `e.is_('int')`, `e.is_('int', isInverted: true)`, `e.isNot('int')` | `e.as_("int")`, `e.is_("int")`, `e.is_not("int")`, `e.is_not("int")` |
//! | `e.not`, `e.nonNullAssert`, `e.parenthesized`, `e.dotShorthand` (getters) | `e.not()`, `e.non_null_assert()`, `e.parenthesized()`, `e.dot_shorthand()` |
//! | `c.conditional(a, b)` | `c.conditional(a, b)` |
//! | `second(a, b)` | `second(a, b)` |
//! | `e.property('f')` / `e.property('f', isNullAware: true)` | `e.property("f", false)` / `e.property("f", true)` |
//! | `thisProperty('f')`, `superProperty('f')` | `this_property("f")`, `super_property("f")` |
//! | `e.invokeMethod('m', [a])` / `isNullAware: true` | `e.invoke_method("m", nodes![a], false)` / `..., true)` |
//! | `e.invokeAnonymousMethod([...], returnType: 'T', isNullAware: b, isParameterless: b, parameter: v)` | `e.invoke_anonymous_method(vec![...], "T", is_null_aware, is_parameterless, Some(v))` (Dart defaults: `false`, `true`, `None`) |
//! | `e.cascade([(v) => v.f(), ...])` / `isNullAware: true` | `e.cascade(vec![Box::new(\|v: Node\| v.f()), ...], false)` (annotate `v: Node`) / `..., true)` |
//! | `x.write(e)`, `e.property('f').write(v)` | `x.write(e)`, `e.property("f", false).write(v)` |
//! | `localFunction([...])` | `local_function(vec![...])` |
//! | `listLiteral([a, b], elementType: 'int')` | `list_literal(nodes![a, b], "int")` |
//! | `mapLiteral([mapEntry(k, v)], keyType: 'K', valueType: 'V')` | `map_literal(vec![map_entry(k, v, false)], "K", "V")` |
//! | `mapEntry(k, v, isKeyNullAware: true)` | `map_entry(k, v, true)` |
//! | `ifElement(c, a)` / `ifElement(c, a, b)` | `if_element(c, a, None)` / `if_element(c, a, b)` |
//! | `ifCaseElement(e, p, a)` / `ifCaseElement(e, p, a, b)` | `if_case_element(e, p, a, None)` / `if_case_element(e, p, a, b)` |
//! | `patternForInElement(p, e, body)` | `pattern_for_in_element(p, e, body, false)` |
//! | `switchExpr(e, [p.thenExpr(a), default_.thenExpr(b)])` | `switch_expr(e, vec![p.then_expr(a), default_().then_expr(b)])` |
//! | `throw_(e)`, `await_(e)` | `throw_(e)`, `await_(e)` |
//! | `p.assign(e)` (pattern assignment) | `p.assign(e)` |
//! | `dotShorthandHead('f')` | `dot_shorthand_head("f")` |
//! | `e.inTypeSchema('int')` | `e.in_type_schema("int")` |
//! | `e.checkType('int')`, `e.checkSchema('int')`, `e.checkIR('...')` | `e.check_type("int")`, `e.check_schema("int")`, `e.check_ir("...")` |
//! | `Node.placeholder()` | `Node::placeholder()` |
//!
//! ## Flow analysis checks
//!
//! | Dart | Rust |
//! |---|---|
//! | `checkPromoted(x, 'int')` / `checkNotPromoted(x)` | `check_promoted(x, "int")` / `check_not_promoted(x)` (`x`: a `Var`, `this_()`, a property, or `this_property(...)`) |
//! | `checkPromotionChain(x, ['num', 'int'])` | `check_promotion_chain(x, &["num", "int"])` |
//! | `checkReachable(true)` | `check_reachable(true)` |
//! | `checkAssigned(x, true)`, `checkUnassigned(x, false)` | `check_assigned(x, true)`, `check_unassigned(x, false)` |
//! | `getSsaNodes((nodes) => s = nodes[x]!)` | `get_ssa_nodes(move \|nodes\| s.set(nodes.get(x).unwrap()))` |
//! | `getSsaNodes((nodes) => expect(nodes[x], same(s)))` | `get_ssa_nodes(move \|nodes\| assert!(same_ssa(nodes.get(x).as_ref(), Some(&s.get()))))` |
//! | `e.getExpressionInfo((info) => ...)` | `e.get_expression_info(move \|info\| ...)` (`info: Option<ExprInfo>`) |
//! | `e.whyNotPromoted((reasons) => ...)` | `e.why_not_promoted(move \|reasons\| ...)` (`reasons: Vec<(SharedTypeView<Type>, NonPromotionReason)>`, in Dart map order) |
//! | `implicitThis_whyNotPromoted('C', (reasons) => ...)` | `implicit_this_why_not_promoted("C", move \|reasons\| ...)` |
//! | `reasons.keys.single` / `reasons.values.single` | `reasons[0].0` / `reasons[0].1` (after `assert_eq!(reasons.len(), 1)`) |
//! | `(r as DemoteViaExplicitWrite<Var>).node` | `match r { NonPromotionReason::DemoteViaExplicitWrite { variable, node } => ..., _ => panic!() }` |
//! | `r.documentationLink`, `r.shortName` | `r.documentation_link()`, `r.short_name()` |
//!
//! ## Patterns
//!
//! | Dart | Rust |
//! |---|---|
//! | `wildcard()` / `wildcard(type: 'int', expectInferredType: 'int')` | `wildcard()` / `wildcard().with_declared_type("int").with_expect_inferred_type("int")` |
//! | `p..errorId = 'X'` (applies to the whole expression before `..`) | `p.error_id("X")` (also on expressions, statements and `Var`s) |
//! | `p.and(q)`, `p.or(q)`, `p.as_('T')`, `p.nullCheck`, `p.nullAssert`, `p.parenthesized` | `p.and(q)`, `p.or(q)`, `p.as_("T")`, `p.null_check()`, `p.null_assert()`, `p.parenthesized()` |
//! | `intLiteral(0).pattern`, `expr('int').pattern`, `nullLiteral.pattern` | `int_literal(0).pattern()`, `expr("int").pattern()`, `null_literal().pattern()` |
//! | `p.recordField()` / `p.recordField('n')` | `p.record_field(None)` / `p.record_field(Some("n"))` |
//! | `recordPattern([...])` | `record_pattern(vec![...])` |
//! | `objectPattern(requiredType: 'C', fields: [...])` | `object_pattern("C", vec![...])` |
//! | `listPattern([...])` / `listPattern([...], elementType: 'int')` | `list_pattern(vec![...], None)` / `list_pattern(vec![...], Some("int"))` |
//! | `restPattern()` / `restPattern(p)` | `rest_pattern(None)` / `rest_pattern(p)` |
//! | `mapPattern([...])` / `mapPattern([...], keyType: 'K', valueType: 'V')` | `map_pattern(vec![...], None, None)` / `map_pattern(vec![...], Some("K"), Some("V"))` |
//! | `mapPatternEntry(k, p)`, `mapPatternWithTypeArguments(keyType:, valueType:, elements:)` | `map_pattern_entry(k, p)`, `map_pattern_with_type_arguments("K", "V", vec![...])` |
//! | `relationalPattern('>', e)` / `relationalPattern('>', e, errorId: 'X')` | `relational_pattern(">", e)` / `relational_pattern(">", e).error_id("X")` |
//!
//! ## `FlowModel` tests (groups `Reachability`, `State`, `join...`, `inheritTested`)
//!
//! | Dart | Rust |
//! |---|---|
//! | `FlowModel(Reachability.initial)` | `FlowModel::new(Reachability::initial())` (`FlowModel`, `PromotionModel`, `SsaNode`, `ExpressionInfo`, `PromotionInfo` are aliases in `common.rs`) |
//! | `s.reachable.parent!`, `.locallyReachable`, `.overallReachable` | `s.reachable.parent.as_ref().unwrap()`, `.locally_reachable`, `.overall_reachable` |
//! | `Reachability.commonAncestor(a, b)` | `Reachability::common_ancestor(Some(&a), Some(&b))` |
//! | `s.setUnreachable()`, `s.split()`, `s.unsplit()`, `s.unsplitTo(r)`, `s.rebaseForward(h, t)`, `s.updatePromotionInfo(h, k, m)`, `s.inheritTested(h, t)` | `s.set_unreachable()`, `s.split()`, `s.unsplit()`, `s.unsplit_to(&r)`, `s.rebase_forward(&h, &t)`, `s.update_promotion_info(&h, k, m)`, `s.inherit_tested(&h, &t)` |
//! | `FlowModel.joinPromotionInfo(...)`, `PromotionModel.joinPromotedTypes(...)`, `PromotionModel.joinTested(...)` | the associated functions of the same names (snake case) in `dartr_flow::flow_analysis_impl::model` |
//! | `s._declare(h, x, true)`, `s._write(h, reason, x, type, ssa)`, `s._infoFor(h, x)`, `s._setInfo(h, {...})`, `s._tryPromoteForTypeCheck(h, x, 'int')`, `s._tryMarkNonNullable(h, x)`, `s._conservativeJoin(h, [x], [y])`, `s._varRef(h, x)`, `s._varRefWithType(h, x)` | `s.declare_var(&h, x, true)`, `s.write_var(&h, reason, x, type, ssa)`, `s.info_for_var(&h, x)`, `s.set_info(&h, &[...])`, `s.try_promote_for_type_check_var(&h, x, "int")`, `s.try_mark_non_nullable_var(&h, x)`, `s.conservative_join_vars(&h, &[x], &[y])`, `s.var_ref(&h, x)`, `s.var_ref_with_type(&h, x)` (trait `FlowModelTestExt` in `common.rs`) |
//! | `s.promotionInfo.unwrap(h)` | `unwrap_promotion_info(&s.promotion_info, &h)` |
//! | `_matchVariableModel(chain: ['int'], ofInterest: ['int'], assigned: true, unassigned: false, writeCaptured: false)` | `match_variable_model().chain(&["int"]).of_interest(&["int"]).assigned(true).unassigned(false).write_captured(false)`; `isEmpty` is `&[]`; a list of `Type`s is `.of_interest_types(&[ty("int")])` |
//! | `expect(model, _matchVariableModel(...))` | `match_variable_model()....check(&model)` |
//! | `expect(map, {key: _matchVariableModel(...)})` | `expect_promotion_models(&map, &[(key, match_variable_model()...)])` |
//! | `_MockNonPromotionReason()` | `mock_non_promotion_reason()` |
//! | `new SsaNode()` | `SsaNode::new()` |
//! | `info.ifTrue`, `info.ifFalse` of an `ExpressionInfo` | `info.if_true`, `info.if_false` |
//!
//! If a Dart construct is missing here, add it to the DSL in [`node`] or to
//! `common.rs` (in the same style), and add a row to this guide.

#![allow(
    clippy::new_ret_no_self,
    clippy::should_implement_trait,
    // Keep the Dart structure (index loops, one branch per Dart case).
    clippy::needless_range_loop,
    clippy::if_same_then_else,
    clippy::too_many_arguments,
    clippy::type_complexity
)]

pub mod flow_analysis_mini_ast;
pub mod harness;
pub mod mini_ir;
pub mod mini_type_constraint_gatherer;
pub mod mini_types;
pub mod node;
pub mod operations;
pub mod test_util;
