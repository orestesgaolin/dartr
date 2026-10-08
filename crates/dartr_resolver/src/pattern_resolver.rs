// Dart source: pkg/analyzer/lib/src/dart/ast/ast.dart (resolvePattern /
// computePatternSchema of the DartPatternImpl subclasses),
// pkg/analyzer/lib/src/generated/resolver.dart (resolveAssignedVariablePattern,
// resolveMapPattern, resolveRelationalPatternOperator, the switch members of
// the TypeAnalyzer mixin, SwitchExhaustiveness, visitSwitchStatement,
// visitSwitchExpression, visitPatternAssignment,
// visitPatternVariableDeclaration, visitIfStatement with a case clause)

//! STUB: patterns, switch statements and expressions, if-case, pattern
//! declarations and assignments. Unit: patterns (with C7
//! `list_pattern_resolver`).
//!
//! The `ResolverVisitor` routes these node kinds here; until they are
//! ported, the visitors use the fallback ([`ResolverVisitor::fallback_expression`]
//! for expressions, nothing for statements) and the `TypeAnalyzer`
//! callbacks below are `todo!()` (they are only reached from the pattern
//! analysis).

use dartr_ast::{DartPattern, Expression, Id, NodeId, Statement};
use dartr_element::{EId, ElemRef, Name, PromotableElement, TypeId};
use dartr_flow::type_analyzer::{
    JoinedPatternVariableInconsistency, JoinedPatternVariableLocation, RecordPatternFieldOf,
    RelationalOperatorResolution, SwitchExpressionMemberInfo, SwitchStatementMemberInfo,
};

use crate::resolver::{ResolverVisitor, TypeViewOf};

/// Dart `downwardInferObjectPatternRequiredType`.
pub fn downward_infer_object_pattern_required_type(
    rv: &mut ResolverVisitor<'_>,
    matched_type: TypeViewOf,
    pattern: Id<DartPattern>,
) -> TypeViewOf {
    let _ = (rv, matched_type, pattern);
    todo!("downwardInferObjectPatternRequiredType (patterns)")
}

/// Dart `finishExpressionCase`.
pub fn finish_expression_case(rv: &mut ResolverVisitor<'_>, node: Id<Expression>, case_index: usize) {
    let _ = (rv, node, case_index);
    todo!("finishExpressionCase (patterns)")
}

/// Dart `finishJoinedPatternVariable`.
pub fn finish_joined_pattern_variable(
    rv: &mut ResolverVisitor<'_>,
    variable: EId<PromotableElement>,
    location: JoinedPatternVariableLocation,
    inconsistency: JoinedPatternVariableInconsistency,
    is_final: bool,
    type_: TypeViewOf,
) {
    let _ = (rv, variable, location, inconsistency, is_final, type_);
    todo!("finishJoinedPatternVariable (patterns)")
}

/// Dart `getSwitchExpressionMemberInfo`.
pub fn get_switch_expression_member_info(
    rv: &mut ResolverVisitor<'_>,
    node: Id<Expression>,
    index: usize,
) -> SwitchExpressionMemberInfo<NodeId, Id<Expression>, EId<PromotableElement>, Name> {
    let _ = (rv, node, index);
    todo!("getSwitchExpressionMemberInfo (patterns)")
}

/// Dart `getSwitchStatementMemberInfo`.
pub fn get_switch_statement_member_info(
    rv: &mut ResolverVisitor<'_>,
    node: Id<Statement>,
    case_index: usize,
) -> SwitchStatementMemberInfo<NodeId, Id<Statement>, Id<Expression>, EId<PromotableElement>, Name> {
    let _ = (rv, node, case_index);
    todo!("getSwitchStatementMemberInfo (patterns)")
}

/// Dart `handleCaseHead` (after the guard is popped).
pub fn handle_case_head(rv: &mut ResolverVisitor<'_>, node: NodeId, case_index: usize, sub_index: usize) {
    let _ = (rv, node, case_index, sub_index);
    todo!("handleCaseHead (patterns)")
}

/// Dart `handleDefault`.
pub fn handle_default(rv: &mut ResolverVisitor<'_>, node: NodeId, case_index: usize, sub_index: usize) {
    let _ = (rv, node, case_index, sub_index);
    todo!("handleDefault (patterns)")
}

/// Dart `handleSwitchBeforeAlternative`.
pub fn handle_switch_before_alternative(
    rv: &mut ResolverVisitor<'_>,
    node: NodeId,
    case_index: usize,
    sub_index: usize,
) {
    let _ = (rv, node, case_index, sub_index);
}

/// Dart `handleSwitchScrutinee`.
pub fn handle_switch_scrutinee(rv: &mut ResolverVisitor<'_>, type_: TypeViewOf) {
    let _ = (rv, type_);
    todo!("handleSwitchScrutinee (patterns)")
}

/// Dart `isLegacySwitchExhaustive`.
pub fn is_legacy_switch_exhaustive(
    rv: &mut ResolverVisitor<'_>,
    node: NodeId,
    expression_type: TypeViewOf,
) -> bool {
    let _ = (rv, node, expression_type);
    todo!("isLegacySwitchExhaustive (patterns)")
}

/// Dart `resolveObjectPatternPropertyGet`.
pub fn resolve_object_pattern_property_get(
    rv: &mut ResolverVisitor<'_>,
    object_pattern: Id<DartPattern>,
    receiver_type: TypeViewOf,
    field: &RecordPatternFieldOf<ResolverVisitor<'_>>,
) -> (Option<ElemRef>, TypeViewOf) {
    let _ = (rv, object_pattern, receiver_type, field);
    todo!("resolveObjectPatternPropertyGet (patterns)")
}

/// Dart `resolveRelationalPatternOperator`.
pub fn resolve_relational_pattern_operator(
    rv: &mut ResolverVisitor<'_>,
    node: Id<DartPattern>,
    matched_value_type: TypeViewOf,
) -> Option<RelationalOperatorResolution<TypeId>> {
    let _ = (rv, node, matched_value_type);
    todo!("resolveRelationalPatternOperator (patterns)")
}
