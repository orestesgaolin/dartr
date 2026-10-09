// Dart source: pkg/analyzer/lib/src/generated/resolver.dart
// (resolveAssignedVariablePattern, resolveMapPattern,
// resolveObjectPatternPropertyGet, resolveRelationalPatternOperator,
// checkPatternNeverMatchesValueType, buildSharedPatternFields, the switch
// members of the TypeAnalyzer mixin, SwitchExhaustiveness,
// visitSwitchStatement, visitSwitchExpression, visitPatternAssignment,
// visitPatternVariableDeclaration, visitIfStatement with a case clause),
// pkg/analyzer/lib/src/dart/ast/ast.dart (SwitchStatementImpl.memberGroups,
// SwitchStatementCaseGroup, GuardedPatternImpl.variables,
// DartPatternImpl.variablePattern),
// pkg/analyzer/lib/src/diagnostic/diagnostic_factory.dart
// (duplicateAssignmentPatternVariable, duplicatePatternField,
// duplicateRestElementInPattern),
// pkg/_fe_analyzer_shared/lib/src/type_inference/variable_bindings.dart
// (the shared case scope of VariableBinder, for the fallback of
// SwitchStatementCaseGroup.variables)

//! The pattern parts of the `ResolverVisitor`: switch statements and
//! expressions, if-case statements, pattern variable declarations and
//! pattern assignments (the entry points), and the `TypeAnalyzer` callbacks
//! about patterns and switch members. The per-kind `resolvePattern` /
//! `computePatternSchema` are in [`crate::resolver::patterns`].
//!
//! # Pattern variables
//!
//! The Dart resolution visitor computes `GuardedPatternImpl.variables` and
//! `SwitchStatementCaseGroup.variables` (with the shared `VariableBinder`)
//! before the `ResolverVisitor` runs. Here they are the tables
//! `ResolverTables::guarded_pattern_variables` and
//! `ResolverTables::switch_group_variables`. When the binding passes did
//! not write them, [`guarded_pattern_variables`] and
//! [`switch_group_variables`] compute them from the bound pattern variable
//! elements (`declared_fragment` of each `DeclaredVariablePattern`, the
//! `join` of their fragments).
//!
//! # Not ported (diagnostics)
//!
//! - `nullSafetyDeadCodeVerifier.flowEnd(...)` calls (dead code reporting
//!   is wave D).
//! - `checkPatternNeverMatchesValueType` needs `TypeSystem.canBeSubtypeOf`,
//!   which is not ported yet (`dartr_typesystem`); see
//!   [`can_be_subtype_of`].
//! - `VariablePatternImpl.fieldNameWithImplicitName` (only read by
//!   search and indexing, not by resolution).

use dartr_ast::{
    AssignedVariablePattern, Ast, CastPattern, ConstantPattern, DartPattern,
    DeclaredVariablePattern, Expression, GuardedPattern, Id, IfStatement, Label, LogicalOrPattern,
    MapPattern, NodeId, NodeList, NullAssertPattern, NullCheckPattern, NullLiteral, ObjectPattern,
    ParenthesizedExpression, ParenthesizedPattern, PatternAssignment, PatternField,
    PatternVariableDeclaration, PrefixedIdentifier, PropertyAccess, RelationalPattern,
    SimpleIdentifier, Statement, SwitchCase, SwitchDefault, SwitchExpression, SwitchExpressionCase,
    SwitchPatternCase, SwitchStatement, TypeArgumentList,
};
use dartr_diagnostics::{DiagnosticMessage, diag};
use dartr_element::diagnostics::type_arg;
use dartr_element::{
    EId, ElemRef, ElementId, InterfaceElement, Name, Nullability, PromotableElement,
    PropertyAccessorElement, TypeAliasElement, TypeId, TypeKind,
};
use dartr_flow::flow_analysis::FlowAnalysis;
use dartr_flow::shared_type::SharedTypeView;
use dartr_flow::type_analysis_result::PatternResult;
use dartr_flow::type_analyzer::{
    CaseHeadOrDefaultInfo, JoinedPatternVariableInconsistency, JoinedPatternVariableLocation,
    RecordPatternField, RecordPatternFieldOf, RelationalOperatorKind, RelationalOperatorResolution,
    SwitchExpressionMemberInfo, SwitchStatementMemberInfo, TypeAnalyzer,
};
use dartr_flow::type_analyzer_operations::KeyValueTypes;
use dartr_typesystem::{TypeExt, member};
use indexmap::{IndexMap, IndexSet};

use crate::element_ext;
use crate::resolver::{PatternResultOf, ResolverVisitor, SchemaOf, SharedMatchContext, TypeViewOf};
use crate::type_property_resolver::{self, PropertyQuery};

/// The pattern variables of a guarded pattern or a group of switch members
/// (Dart `Map<String, PatternVariableElementImpl>`, in map order).
pub type PatternVariables = Vec<(Name, EId<PromotableElement>)>;

// ------------------------------------------------------------ entry points

/// Dart `ResolverVisitor.visitSwitchStatement(node)` (after
/// `checkUnreachableNode`).
pub fn visit_switch_statement(rv: &mut ResolverVisitor<'_>, node: Id<SwitchStatement>) {
    // Stack: ()
    let previous_exhaustiveness = rv.legacy_switch_exhaustiveness.clone();
    let num_groups = member_groups(rv.ast, node).len();
    let expression = rv.ast[node].expression;
    rv.analyze_switch_statement(node.upcast(), expression, num_groups);
    // Stack: (Expression)
    rv.pop_rewrite();
    // Stack: ()
    rv.legacy_switch_exhaustiveness = previous_exhaustiveness;
}

/// Dart `ResolverVisitor.visitSwitchExpression(node, contextType:)`.
pub fn visit_switch_expression(
    rv: &mut ResolverVisitor<'_>,
    node: Id<SwitchExpression>,
    context_type: TypeId,
) {
    let previous_exhaustiveness = rv.legacy_switch_exhaustiveness.clone();
    let expression = rv.ast[node].expression;
    let num_cases = rv.ast.list(rv.ast[node].cases).len();
    let static_type = rv
        .analyze_switch_expression(
            node.upcast(),
            expression,
            num_cases,
            SchemaOf::new(context_type),
        )
        .type_
        .unwrap_type_view();
    rv.record_static_type(node, static_type);
    rv.pop_rewrite();
    rv.legacy_switch_exhaustiveness = previous_exhaustiveness;
}

/// Dart `ResolverVisitor.visitIfStatement(node)` with a case clause (after
/// `checkUnreachableNode`).
pub fn visit_if_case_statement(rv: &mut ResolverVisitor<'_>, node: Id<IfStatement>) {
    let Some(case_clause) = rv.ast[node].case_clause else {
        return;
    };
    let guarded_pattern = rv.ast[case_clause].guarded_pattern;
    let pattern = rv.ast[guarded_pattern].pattern;
    let guard = rv.ast[guarded_pattern]
        .when_clause
        .map(|w| rv.ast[w].expression);
    let variables = guarded_pattern_variables(rv, guarded_pattern);
    let expression = rv.ast[node].expression;
    let then_statement = rv.ast[node].then_statement;
    let else_statement = rv.ast[node].else_statement;
    rv.analyze_if_case_statement(
        node.upcast(),
        expression,
        pattern,
        guard,
        then_statement,
        else_statement,
        &variables,
    );
    // Stack: (Expression, Guard)
    rv.pop_rewrite(); // guard
    rv.pop_rewrite(); // expression
}

/// Dart `ResolverVisitor.visitPatternAssignment(node, contextType:)`.
pub fn visit_pattern_assignment(
    rv: &mut ResolverVisitor<'_>,
    node: Id<PatternAssignment>,
    context_type: TypeId,
) {
    let _ = context_type;
    rv.check_unreachable_node(node);
    let pattern = rv.ast[node].pattern;
    let expression = rv.ast[node].expression;
    let analysis_result = rv.analyze_pattern_assignment(node.upcast(), pattern, expression);
    set_pattern_type_schema(
        rv,
        node.raw(),
        analysis_result.pattern_schema.unwrap_type_schema_view(),
    );
    rv.record_static_type(node, analysis_result.type_.unwrap_type_view());
    rv.pop_rewrite(); // expression
}

/// Dart `ResolverVisitor.visitPatternVariableDeclaration(node)`.
pub fn visit_pattern_variable_declaration(
    rv: &mut ResolverVisitor<'_>,
    node: Id<PatternVariableDeclaration>,
) {
    let metadata = rv.ast[node].metadata;
    rv.visit_list(metadata);
    let pattern = rv.ast[node].pattern;
    let expression = rv.ast[node].expression;
    let is_final = rv.lexeme(rv.ast[node].keyword) == "final";
    let pattern_schema = rv
        .analyze_pattern_variable_declaration(node.raw(), pattern, expression, is_final)
        .pattern_schema;
    set_pattern_type_schema(rv, node.raw(), pattern_schema.unwrap_type_schema_view());
    rv.pop_rewrite(); // expression
}

/// Dart `node.patternTypeSchema = schema` of a `PatternAssignment` or a
/// `PatternVariableDeclaration`.
fn set_pattern_type_schema(rv: &mut ResolverVisitor<'_>, node: NodeId, schema: TypeId) {
    let mut info = rv
        .tables
        .pattern_info
        .get(node)
        .copied()
        .unwrap_or_default();
    info.pattern_type_schema = Some(schema);
    rv.tables.pattern_info.insert(node, info);
}

/// Dart `pattern.requiredType = type` of a `ListPattern` or a `MapPattern`.
pub(crate) fn set_required_type(rv: &mut ResolverVisitor<'_>, node: NodeId, ty: TypeId) {
    let mut info = rv
        .tables
        .pattern_info
        .get(node)
        .copied()
        .unwrap_or_default();
    info.required_type = Some(ty);
    rv.tables.pattern_info.insert(node, info);
}

// ------------------------------------------------------------ switch groups

/// Dart `SwitchStatementCaseGroup`: switch members that share a body.
#[derive(Clone, Debug)]
pub struct SwitchStatementCaseGroup {
    /// Dart `members`.
    pub members: Vec<NodeId>,
    /// Dart `hasLabels`.
    pub has_labels: bool,
}

impl SwitchStatementCaseGroup {
    /// Dart `statements`: the statements of the last member.
    pub fn statements(&self, ast: &Ast) -> Vec<Id<Statement>> {
        match self.members.last() {
            Some(&last) => match switch_member_parts(ast, last) {
                Some((_, statements)) => ast.list(statements).to_vec(),
                None => Vec::new(),
            },
            None => Vec::new(),
        }
    }
}

/// The labels and the statements of the switch member [member].
fn switch_member_parts(
    ast: &Ast,
    member: NodeId,
) -> Option<(NodeList<Label>, NodeList<Statement>)> {
    if let Some(m) = ast.cast::<SwitchCase>(member) {
        Some((ast[m].labels, ast[m].statements))
    } else if let Some(m) = ast.cast::<SwitchDefault>(member) {
        Some((ast[m].labels, ast[m].statements))
    } else {
        ast.cast::<SwitchPatternCase>(member)
            .map(|m| (ast[m].labels, ast[m].statements))
    }
}

/// Dart `SwitchStatementImpl.memberGroups` (`_computeMemberGroups`).
pub fn member_groups(ast: &Ast, node: Id<SwitchStatement>) -> Vec<SwitchStatementCaseGroup> {
    let mut groups = Vec::new();
    let mut group_members = Vec::new();
    let mut group_has_labels = false;
    for &member in ast.list_raw(ast[node].members) {
        group_members.push(member);
        let Some((labels, statements)) = switch_member_parts(ast, member) else {
            continue;
        };
        group_has_labels |= !ast.list_raw(labels).is_empty();
        if !ast.list_raw(statements).is_empty() {
            groups.push(SwitchStatementCaseGroup {
                members: std::mem::take(&mut group_members),
                has_labels: group_has_labels,
            });
            group_has_labels = false;
        }
    }
    if !group_members.is_empty() {
        groups.push(SwitchStatementCaseGroup {
            members: group_members,
            has_labels: group_has_labels,
        });
    }
    groups
}

/// The group [case_index] of the switch statement [node].
pub(crate) fn member_group(ast: &Ast, node: NodeId, case_index: usize) -> Option<SwitchStatementCaseGroup> {
    let node = ast.cast::<SwitchStatement>(node)?;
    member_groups(ast, node).into_iter().nth(case_index)
}

// ------------------------------------------------------------ pattern variables

/// The variables that the resolution visitor recorded, as promotable
/// elements.
fn to_pattern_variables(
    variables: &indexmap::IndexMap<dartr_element::Name, dartr_element::ElementId>,
) -> PatternVariables {
    variables
        .iter()
        .filter_map(|(&name, &e)| e.cast::<dartr_element::PromotableElement>().map(|e| (name, e)))
        .collect()
}

/// Dart `GuardedPatternImpl.variables` of [guarded_pattern].
pub fn guarded_pattern_variables(
    rv: &ResolverVisitor<'_>,
    guarded_pattern: Id<GuardedPattern>,
) -> PatternVariables {
    if let Some(variables) = rv.rt.guarded_pattern_variables.get(guarded_pattern) {
        return to_pattern_variables(variables);
    }
    let pattern = rv.ast[guarded_pattern].pattern;
    compute_pattern_variables(rv, pattern)
}

/// The pattern variables of [pattern] when the binding passes did not
/// record them: the bind variable of each `DeclaredVariablePattern` (in
/// source order), or the logical-or join variable that it is a component
/// of. The first variable with a name wins (Dart `VariableBinder.add`).
fn compute_pattern_variables(
    rv: &ResolverVisitor<'_>,
    pattern: Id<DartPattern>,
) -> PatternVariables {
    let ast = &*rv.ast;
    let mut declared = Vec::new();
    collect_declared_variable_patterns(ast, pattern.raw(), &mut declared);
    let mut result: IndexMap<Name, EId<PromotableElement>> = IndexMap::new();
    for node in declared {
        let Some(element) = rv.declared_element(node) else {
            continue;
        };
        // A bind variable below a logical-or pattern is a component of the
        // join of that logical-or pattern (the outermost one: Dart expands
        // the components of nested logical-or joins).
        let mut variable = element;
        if has_logical_or_ancestor(ast, node, pattern.raw())
            && let Some(join) = element_ext::pattern_variable_join(&rv.ctx, element)
        {
            variable = join;
        }
        let Some(variable) = variable.cast::<PromotableElement>() else {
            continue;
        };
        let name = match rv.ctx.element_data(variable.raw()).and_then(|d| d.name) {
            Some(name) => name,
            None => match ast.cast::<DeclaredVariablePattern>(node) {
                Some(p) => rv.ctx.name(ast.tokens.lexeme(ast[p].name)),
                None => continue,
            },
        };
        result.entry(name).or_insert(variable);
    }
    result.into_iter().collect()
}

/// The `DeclaredVariablePattern`s in [node], in source order. Does not
/// enter expressions (constant patterns, relational operands, map keys).
fn collect_declared_variable_patterns(ast: &Ast, node: NodeId, result: &mut Vec<NodeId>) {
    if ast.is::<DeclaredVariablePattern>(node) {
        result.push(node);
        return;
    }
    for child in ast.children(node) {
        if ast.is::<DartPattern>(child)
            || ast.is::<PatternField>(child)
            || ast.is::<dartr_ast::MapPatternEntry>(child)
            || ast.is::<dartr_ast::RestPatternElement>(child)
        {
            collect_declared_variable_patterns(ast, child, result);
        }
    }
}

/// Whether a `LogicalOrPattern` is between [node] and [root] (exclusive).
fn has_logical_or_ancestor(ast: &Ast, node: NodeId, root: NodeId) -> bool {
    let mut current = node;
    while current != root {
        let Some(parent) = ast.parent(current) else {
            return false;
        };
        if ast.is::<LogicalOrPattern>(parent) {
            return true;
        }
        current = parent;
    }
    false
}

/// Dart `SwitchStatementCaseGroup.variables` of [group].
pub fn switch_group_variables(
    rv: &ResolverVisitor<'_>,
    group: &SwitchStatementCaseGroup,
) -> PatternVariables {
    if let Some(&last) = group.members.last()
        && let Some(variables) = rv.rt.switch_case_group_variables.get(last)
    {
        return to_pattern_variables(variables);
    }
    compute_switch_group_variables(rv, group)
}

/// Dart `_SharedCaseScopeVariable`.
struct SharedCaseScopeVariable {
    all_cases: bool,
    variables: Vec<EId<PromotableElement>>,
}

/// Dart `_SharedCaseScope`.
struct SharedCaseScope {
    is_empty: bool,
    variables: IndexMap<Name, SharedCaseScopeVariable>,
}

impl SharedCaseScope {
    /// Dart `addAll`.
    fn add_all(&mut self, new_variables: &PatternVariables) {
        if self.is_empty {
            self.is_empty = false;
            for &(name, variable) in new_variables {
                self.variables
                    .entry(name)
                    .or_insert(SharedCaseScopeVariable {
                        all_cases: true,
                        variables: Vec::new(),
                    })
                    .variables
                    .push(variable);
            }
        } else {
            for (name, variable) in self.variables.iter_mut() {
                match new_variables.iter().find(|(n, _)| n == name) {
                    Some(&(_, new_variable)) => variable.variables.push(new_variable),
                    None => variable.all_cases = false,
                }
            }
            for &(name, new_variable) in new_variables {
                if !self.variables.contains_key(&name) {
                    self.variables.insert(
                        name,
                        SharedCaseScopeVariable {
                            all_cases: false,
                            variables: vec![new_variable],
                        },
                    );
                }
            }
        }
    }
}

/// `SwitchStatementCaseGroup.variables` when the binding passes did not
/// record them: the shared case scope of `VariableBinder`
/// (`switchStatementSharedCaseScopeStart` / `Empty` / `Finish`) over the
/// guarded pattern variables of the members. A variable that Dart joins is
/// the join variable of the components (their `join`), when the binding
/// pass created it, or else the first component.
fn compute_switch_group_variables(
    rv: &ResolverVisitor<'_>,
    group: &SwitchStatementCaseGroup,
) -> PatternVariables {
    let ast = &*rv.ast;
    let mut scope = SharedCaseScope {
        is_empty: true,
        variables: IndexMap::new(),
    };
    for &member in &group.members {
        if ast.is::<SwitchDefault>(member) {
            scope.add_all(&Vec::new());
        } else if let Some(m) = ast.cast::<SwitchPatternCase>(member) {
            let variables = guarded_pattern_variables(rv, ast[m].guarded_pattern);
            scope.add_all(&variables);
        }
    }
    if group.has_labels {
        scope.add_all(&Vec::new());
    }
    let mut result = Vec::new();
    for (name, shared) in scope.variables {
        let Some(&first) = shared.variables.first() else {
            continue;
        };
        if shared.all_cases && shared.variables.len() == 1 {
            result.push((name, first));
        } else {
            let join = element_ext::pattern_variable_join(&rv.ctx, first.raw())
                .and_then(|j| j.cast::<PromotableElement>())
                .unwrap_or(first);
            result.push((name, join));
        }
    }
    result
}

// ------------------------------------------------------------ TypeAnalyzer callbacks

/// Dart `downwardInferObjectPatternRequiredType`.
pub fn downward_infer_object_pattern_required_type(
    rv: &mut ResolverVisitor<'_>,
    matched_type: TypeViewOf,
    pattern: Id<DartPattern>,
) -> TypeViewOf {
    let Some(pattern) = rv.ast.cast::<ObjectPattern>(pattern) else {
        return SharedTypeView::new(TypeId::DYNAMIC);
    };
    let type_node = rv.ast[pattern].type_;
    if rv.ast[type_node].type_arguments.is_none() {
        let type_name_element = rv.base_element(type_node);
        if let Some(element) = type_name_element.and_then(|e| e.cast::<InterfaceElement>()) {
            let type_parameters = rv.ctx.interface_type_parameters(element).to_vec();
            if !type_parameters.is_empty() {
                let declared_type = rv.ctx.interface_this_type(element);
                let type_arguments = infer_type_arguments(
                    rv,
                    &type_parameters,
                    type_node.raw(),
                    declared_type,
                    matched_type.unwrap_type_view(),
                );
                let ty = rv
                    .ctx
                    .instantiate_interface(element, &type_arguments, Nullability::None);
                rv.tables.annotation_type.insert(type_node, ty);
                return SharedTypeView::new(ty);
            }
        } else if let Some(element) = type_name_element.and_then(|e| e.cast::<TypeAliasElement>()) {
            let type_parameters = rv.ctx.get(element).type_params.clone();
            if !type_parameters.is_empty() {
                let declared_type = rv
                    .ctx
                    .get(element)
                    .aliased_type
                    .get()
                    .unwrap_or(TypeId::DYNAMIC);
                let type_arguments = infer_type_arguments(
                    rv,
                    &type_parameters,
                    type_node.raw(),
                    declared_type,
                    matched_type.unwrap_type_view(),
                );
                let ty = rv
                    .ctx
                    .instantiate_type_alias(element, &type_arguments, Nullability::None);
                rv.tables.annotation_type.insert(type_node, ty);
                return SharedTypeView::new(ty);
            }
        }
    }
    SharedTypeView::new(annotation_type(rv, type_node.raw()))
}

/// Dart `_inferTypeArguments`.
fn infer_type_arguments(
    rv: &mut ResolverVisitor<'_>,
    type_parameters: &[EId<dartr_element::TypeParameterElement>],
    error_node: NodeId,
    declared_type: TypeId,
    context_type: TypeId,
) -> Vec<TypeId> {
    use dartr_typesystem::generic_inferrer::{
        GenericInferrer, InferenceErrorEntity, InferenceFlags,
    };
    let flags = InferenceFlags {
        generic_metadata_is_enabled: rv.generic_metadata_is_enabled(),
        inference_using_bounds_is_enabled: rv.inference_using_bounds_is_enabled(),
        strict_inference: rv.unit.options.strict_inference,
    };
    let entity = InferenceErrorEntity::other(
        rv.ast.offset(error_node) as usize,
        rv.ast.length(error_node) as usize,
    );
    let mut reported = Vec::new();
    let result = {
        let mut listener = |d: dartr_diagnostics::Diagnostic| reported.push(d);
        let mut reporter = dartr_diagnostics::DiagnosticReporter::new(&mut listener);
        let mut inferrer = GenericInferrer::new(
            rv.type_system,
            type_parameters,
            Some(&mut reporter),
            Some(entity),
            flags,
            rv.flow_analysis.type_operations,
            None,
        );
        inferrer.constrain_return_type(declared_type, context_type, None);
        inferrer.choose_final_types()
    };
    rv.flush_type_analyzer_errors();
    if rv.lock_level == 0 {
        rv.diagnostics.extend(reported);
    }
    result
}

/// Dart `finishExpressionCase`.
pub fn finish_expression_case(
    rv: &mut ResolverVisitor<'_>,
    node: Id<Expression>,
    case_index: usize,
) {
    // Dart `case_.expression = popRewrite()!`: the rewrite already replaced
    // the expression in the AST.
    rv.pop_rewrite();
    // Dart `nullSafetyDeadCodeVerifier.flowEnd(case_)`.
    if let Some(node) = rv.ast.cast::<SwitchExpression>(node) {
        let case_ = rv.ast.list(rv.ast[node].cases)[case_index];
        crate::error::dead_code_verifier::flow_end(rv, case_);
    }
}

/// Converts the flow analysis inconsistency to the element model one.
fn to_element_inconsistency(
    value: JoinedPatternVariableInconsistency,
) -> dartr_element::JoinedPatternVariableInconsistency {
    use dartr_element::JoinedPatternVariableInconsistency as E;
    match value {
        JoinedPatternVariableInconsistency::None => E::None,
        JoinedPatternVariableInconsistency::LogicalOr => E::LogicalOr,
        JoinedPatternVariableInconsistency::SharedCaseAbsent => E::SharedCaseAbsent,
        JoinedPatternVariableInconsistency::SharedCaseHasLabel => E::SharedCaseHasLabel,
        JoinedPatternVariableInconsistency::DifferentFinalityOrType => E::DifferentFinalityOrType,
    }
}

/// Converts the element model inconsistency to the flow analysis one.
fn to_flow_inconsistency(
    value: dartr_element::JoinedPatternVariableInconsistency,
) -> JoinedPatternVariableInconsistency {
    use dartr_element::JoinedPatternVariableInconsistency as E;
    match value {
        E::None => JoinedPatternVariableInconsistency::None,
        E::LogicalOr => JoinedPatternVariableInconsistency::LogicalOr,
        E::SharedCaseAbsent => JoinedPatternVariableInconsistency::SharedCaseAbsent,
        E::SharedCaseHasLabel => JoinedPatternVariableInconsistency::SharedCaseHasLabel,
        E::DifferentFinalityOrType => JoinedPatternVariableInconsistency::DifferentFinalityOrType,
    }
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
    let e = variable.raw();
    let ty = type_.unwrap_type_view();
    if !element_ext::is_join_pattern_variable(e) {
        // Recovery: the binding pass did not create the join variable, so
        // a component stands for it. Only its type is updated.
        if let Some(local) = e.cast::<dartr_element::LocalVariableElement>() {
            element_ext::set_local_variable_type(&rv.ctx, local, ty);
        }
        return;
    }
    let ctx = rv.ctx;
    let current = to_flow_inconsistency(element_ext::join_pattern_variable_inconsistency(&ctx, e));
    let inconsistency = current.max_with(inconsistency);
    element_ext::set_join_pattern_variable_inconsistency(
        &ctx,
        e,
        to_element_inconsistency(inconsistency),
    );
    element_ext::set_is_final(&ctx, e, is_final);
    if let Some(local) = e.cast::<dartr_element::LocalVariableElement>() {
        element_ext::set_local_variable_type(&ctx, local, ty);
    }

    if location == JoinedPatternVariableLocation::SharedCaseScope {
        let name = ctx
            .element_data(e)
            .and_then(|d| d.name)
            .map(|n| ctx.name_str(n).to_string())
            .unwrap_or_default();
        for reference in element_ext::join_pattern_variable_references(&ctx, e) {
            let d = match inconsistency {
                JoinedPatternVariableInconsistency::SharedCaseAbsent => {
                    diag::pattern_variable_shared_case_scope_not_all_cases(&name)
                }
                JoinedPatternVariableInconsistency::SharedCaseHasLabel => {
                    diag::pattern_variable_shared_case_scope_has_label(&name)
                }
                JoinedPatternVariableInconsistency::DifferentFinalityOrType => {
                    diag::pattern_variable_shared_case_scope_different_finality_or_type(&name)
                }
                _ => continue,
            };
            let d = rv.at(d, reference);
            rv.report(d);
        }
    }
}

/// The case head of the guarded pattern [guarded_pattern].
fn guarded_case_head(
    rv: &ResolverVisitor<'_>,
    guarded_pattern: Id<GuardedPattern>,
) -> CaseHeadOrDefaultInfo<NodeId, Id<Expression>, EId<PromotableElement>, Name> {
    CaseHeadOrDefaultInfo {
        pattern: Some(rv.ast[guarded_pattern].pattern.raw()),
        variables: guarded_pattern_variables(rv, guarded_pattern),
        guard: rv.ast[guarded_pattern]
            .when_clause
            .map(|w| rv.ast[w].expression),
    }
}

/// Dart `getSwitchExpressionMemberInfo`.
pub fn get_switch_expression_member_info(
    rv: &mut ResolverVisitor<'_>,
    node: Id<Expression>,
    index: usize,
) -> SwitchExpressionMemberInfo<NodeId, Id<Expression>, EId<PromotableElement>, Name> {
    let node: Id<SwitchExpression> = rv.ast.cast(node).expect("SwitchExpression");
    let case_ = rv.ast.list(rv.ast[node].cases)[index];
    let guarded_pattern = rv.ast[case_].guarded_pattern;
    SwitchExpressionMemberInfo {
        head: guarded_case_head(rv, guarded_pattern),
        expression: rv.ast[case_].expression,
    }
}

/// Dart `getSwitchStatementMemberInfo`.
pub fn get_switch_statement_member_info(
    rv: &mut ResolverVisitor<'_>,
    node: Id<Statement>,
    case_index: usize,
) -> SwitchStatementMemberInfo<NodeId, Id<Statement>, Id<Expression>, EId<PromotableElement>, Name>
{
    let group = member_group(rv.ast, node.raw(), case_index).expect("switch statement case group");
    let heads = group
        .members
        .iter()
        .map(|&member| {
            if let Some(m) = rv.ast.cast::<SwitchCase>(member) {
                CaseHeadOrDefaultInfo {
                    pattern: Some(rv.ast[m].expression.raw()),
                    variables: Vec::new(),
                    guard: None,
                }
            } else if let Some(m) = rv.ast.cast::<SwitchPatternCase>(member) {
                guarded_case_head(rv, rv.ast[m].guarded_pattern)
            } else {
                CaseHeadOrDefaultInfo {
                    pattern: None,
                    variables: Vec::new(),
                    guard: None,
                }
            }
        })
        .collect();
    SwitchStatementMemberInfo {
        heads,
        has_labels: group.has_labels,
        body: group.statements(rv.ast),
        variables: switch_group_variables(rv, &group),
    }
}

/// Dart `handleCaseHead` (after the guard is popped).
pub fn handle_case_head(
    rv: &mut ResolverVisitor<'_>,
    node: NodeId,
    case_index: usize,
    sub_index: usize,
) {
    if rv.ast.is::<SwitchStatement>(node) {
        if let Some(group) = member_group(rv.ast, node, case_index)
            && let Some(mut exhaustiveness) = rv.legacy_switch_exhaustiveness.take()
        {
            exhaustiveness.visit_switch_member(rv, &group);
            rv.legacy_switch_exhaustiveness = Some(exhaustiveness);
        }
        // Dart `nullSafetyDeadCodeVerifier.flowEnd(group.members[subIndex])`.
        if let Some(group) = member_group(rv.ast, node, case_index)
            && let Some(&member) = group.members.get(sub_index)
        {
            crate::error::dead_code_verifier::flow_end(rv, member);
        }
    } else if let Some(node) = rv.ast.cast::<SwitchExpression>(node) {
        let case_ = rv.ast.list(rv.ast[node].cases)[case_index];
        if let Some(mut exhaustiveness) = rv.legacy_switch_exhaustiveness.take() {
            exhaustiveness.visit_switch_expression_case(rv, case_);
            rv.legacy_switch_exhaustiveness = Some(exhaustiveness);
        }
    }
}

/// Dart `handleDefault`.
pub fn handle_default(
    rv: &mut ResolverVisitor<'_>,
    node: NodeId,
    case_index: usize,
    sub_index: usize,
) {
    if let Some(group) = member_group(rv.ast, node, case_index)
        && let Some(mut exhaustiveness) = rv.legacy_switch_exhaustiveness.take()
    {
        exhaustiveness.visit_switch_member(rv, &group);
        rv.legacy_switch_exhaustiveness = Some(exhaustiveness);
    }
    // Dart `nullSafetyDeadCodeVerifier.flowEnd(group.members[subIndex])`.
    if let Some(group) = member_group(rv.ast, node, case_index)
        && let Some(&member) = group.members.get(sub_index)
    {
        crate::error::dead_code_verifier::flow_end(rv, member);
    }
}

/// Dart `handleSwitchBeforeAlternative`.
pub fn handle_switch_before_alternative(
    rv: &mut ResolverVisitor<'_>,
    node: NodeId,
    case_index: usize,
    sub_index: usize,
) {
    if let Some(node) = rv.ast.cast::<SwitchExpression>(node) {
        let case_ = rv.ast.list(rv.ast[node].cases)[case_index];
        rv.check_unreachable_node(case_);
    } else if let Some(group) = member_group(rv.ast, node, case_index)
        && let Some(&member) = group.members.get(sub_index)
    {
        rv.check_unreachable_node(member);
    }
}

/// Dart `handleSwitchScrutinee`.
pub fn handle_switch_scrutinee(rv: &mut ResolverVisitor<'_>, type_: TypeViewOf) {
    if !rv.type_analyzer_options.patterns_enabled {
        rv.legacy_switch_exhaustiveness =
            Some(SwitchExhaustiveness::new(rv, type_.unwrap_type_view()));
    }
}

/// Dart `isLegacySwitchExhaustive`.
pub fn is_legacy_switch_exhaustive(
    rv: &mut ResolverVisitor<'_>,
    node: NodeId,
    expression_type: TypeViewOf,
) -> bool {
    let _ = (node, expression_type);
    rv.legacy_switch_exhaustiveness
        .as_ref()
        .is_some_and(|e| e.is_exhaustive)
}

/// The name of the variable pattern of [pattern] (Dart
/// `DartPatternImpl.variablePattern?.name`): the pattern itself, or the
/// inner pattern of a cast, null-check, null-assert or parenthesized
/// pattern.
pub fn variable_pattern_name(ast: &Ast, pattern: Id<DartPattern>) -> Option<dartr_syntax::TokenId> {
    if let Some(p) = ast.cast::<DeclaredVariablePattern>(pattern) {
        Some(ast[p].name)
    } else if let Some(p) = ast.cast::<AssignedVariablePattern>(pattern) {
        Some(ast[p].name)
    } else if let Some(p) = ast.cast::<CastPattern>(pattern) {
        variable_pattern_name(ast, ast[p].pattern)
    } else if let Some(p) = ast.cast::<NullAssertPattern>(pattern) {
        variable_pattern_name(ast, ast[p].pattern)
    } else if let Some(p) = ast.cast::<NullCheckPattern>(pattern) {
        variable_pattern_name(ast, ast[p].pattern)
    } else if let Some(p) = ast.cast::<ParenthesizedPattern>(pattern) {
        variable_pattern_name(ast, ast[p].pattern)
    } else {
        None
    }
}

/// Moves the diagnostics reported since [start] at the range of [node] to
/// the range of [token]. The type property resolver reports at nodes; Dart
/// passes tokens there for patterns (the relational operator, the field
/// name).
fn relocate_diagnostics(
    rv: &mut ResolverVisitor<'_>,
    start: usize,
    node: NodeId,
    token: dartr_syntax::TokenId,
) {
    let offset = rv.ast.offset(node) as usize;
    let length = rv.ast.length(node) as usize;
    let t = rv.ast.tokens.get(token);
    let (token_offset, token_length) = (t.offset as usize, (t.end() - t.offset) as usize);
    for d in &mut rv.diagnostics[start..] {
        if d.offset == offset && d.length == length {
            d.offset = token_offset;
            d.length = token_length;
        }
    }
}

/// Dart `resolveObjectPatternPropertyGet`.
pub fn resolve_object_pattern_property_get(
    rv: &mut ResolverVisitor<'_>,
    object_pattern: Id<DartPattern>,
    receiver_type: TypeViewOf,
    field: &RecordPatternFieldOf<ResolverVisitor<'_>>,
) -> (Option<ElemRef>, TypeViewOf) {
    let dynamic = SharedTypeView::new(TypeId::DYNAMIC);
    let Some(field_node) = rv.ast.cast::<PatternField>(field.node) else {
        return (None, dynamic);
    };
    let name_token = rv.ast[field_node]
        .name
        .and_then(|n| rv.ast[n].name)
        .or_else(|| variable_pattern_name(rv.ast, field.pattern));
    let Some(name_token) = name_token else {
        return (None, dynamic);
    };
    let Some(object_pattern) = rv.ast.cast::<ObjectPattern>(object_pattern) else {
        return (None, dynamic);
    };
    let receiver_type = receiver_type.unwrap_type_view();
    let name = rv.lexeme(name_token).to_string();
    let type_node = rv.ast[object_pattern].type_;

    rv.flush_type_analyzer_errors();
    let start = rv.diagnostics.len();
    let result = type_property_resolver::resolve(
        rv,
        PropertyQuery {
            receiver: None,
            receiver_type,
            name: &name,
            has_read: true,
            has_write: false,
            property_error_entity: type_node.raw(),
            // Dart `nameErrorEntity: nameToken`: reported at the field, then
            // moved to the name token.
            name_error_entity: field_node.raw(),
            parent_node: None,
        },
    );
    relocate_diagnostics(rv, start, field_node.raw(), name_token);

    if result.needs_getter_error {
        let d = diag::undefined_getter(&name, type_arg(&rv.ctx, receiver_type));
        let d = rv.at_token(d, name_token);
        rv.report(d);
    }

    if let Some(getter) = result.getter {
        rv.set_element(field_node, Some(getter));
        let base = member::base_element(&rv.ctx, getter);
        let ty = if base.is::<PropertyAccessorElement>() {
            member::return_type(&rv.ctx, getter)
        } else {
            member::type_(&rv.ctx, getter)
        };
        return (Some(getter), SharedTypeView::new(ty));
    }

    if let Some(record_field) = result.record_field {
        return (None, SharedTypeView::new(record_field.ty));
    }

    (None, dynamic)
}

/// Dart `resolveRelationalPatternOperator`.
pub fn resolve_relational_pattern_operator(
    rv: &mut ResolverVisitor<'_>,
    node: Id<DartPattern>,
    matched_value_type: TypeViewOf,
) -> Option<RelationalOperatorResolution<TypeId>> {
    let node = rv.ast.cast::<RelationalPattern>(node)?;
    let operator = rv.ast[node].operator;
    let operator_lexeme = rv.lexeme(operator).to_string();
    let (kind, method_name) = match operator_lexeme.as_str() {
        "==" => (RelationalOperatorKind::Equals, "==".to_string()),
        "!=" => (RelationalOperatorKind::NotEquals, "==".to_string()),
        _ => (RelationalOperatorKind::Other, operator_lexeme.clone()),
    };
    let matched_type = matched_value_type.unwrap_type_view();

    rv.flush_type_analyzer_errors();
    let start = rv.diagnostics.len();
    let result = type_property_resolver::resolve(
        rv,
        PropertyQuery {
            receiver: None,
            receiver_type: matched_type,
            name: &method_name,
            has_read: true,
            has_write: false,
            // Dart `propertyErrorEntity: node.operator`: reported at the
            // pattern, then moved to the operator.
            property_error_entity: node.raw(),
            name_error_entity: node.raw(),
            parent_node: Some(node.raw()),
        },
    );
    relocate_diagnostics(rv, start, node.raw(), operator);

    if result.needs_getter_error {
        let d = diag::undefined_operator(&method_name, type_arg(&rv.ctx, matched_type));
        let d = rv.at_token(d, operator);
        rv.report(d);
    }

    // Dart `result.getter2 as InternalMethodElement?`.
    let element = result
        .getter
        .filter(|&g| member::base_element(&rv.ctx, g).is::<dartr_element::MethodElement>());
    rv.set_element(node, element);
    let element = element?;

    // Dart `element.firstParameterType`.
    let parameter = *member::formal_parameters(&rv.ctx, element).first()?;
    let parameter_type = member::type_(&rv.ctx, parameter);

    Some(RelationalOperatorResolution {
        kind,
        parameter_type: SharedTypeView::new(parameter_type),
        return_type: SharedTypeView::new(member::return_type(&rv.ctx, element)),
    })
}

// ------------------------------------------------------------ ResolverVisitor helpers

/// Dart `typeNode.typeOrThrow` of a type annotation; `dynamic` when the
/// resolution visitor did not resolve it (recovery, instead of the Dart
/// exception).
pub(crate) fn annotation_type(rv: &ResolverVisitor<'_>, node: NodeId) -> TypeId {
    rv.tables
        .annotation_type
        .get(node)
        .copied()
        .unwrap_or(TypeId::DYNAMIC)
}

/// Dart `ResolverVisitor.resolveAssignedVariablePattern`.
pub fn resolve_assigned_variable_pattern(
    rv: &mut ResolverVisitor<'_>,
    node: Id<AssignedVariablePattern>,
    context: &SharedMatchContext,
) -> PatternResultOf {
    let element = rv
        .base_element(node)
        .and_then(|e| e.cast::<PromotableElement>());
    let Some(element) = element else {
        return PatternResult {
            matched_value_type: SharedTypeView::new(TypeId::INVALID),
        };
    };

    if element_ext::is_final(&rv.ctx, element.raw()) {
        let name = rv.ast[node].name;
        if element_ext::is_late(&rv.ctx, element.raw()) {
            if rv.flow().is_assigned(element) {
                let d = rv.at_token(diag::late_final_local_already_assigned(), name);
                rv.report(d);
            }
        } else if !rv.flow().is_unassigned(element) {
            let lexeme = rv.lexeme(name).to_string();
            let d = rv.at_token(diag::assignment_to_final_local(&lexeme), name);
            rv.report(d);
        }
    }

    rv.analyze_assigned_variable_pattern(context, node.upcast(), element)
        .into()
}

/// The types of the type arguments of [type_arguments] (Dart
/// `typeArguments.arguments[i].typeOrThrow`).
pub(crate) fn type_argument_types(
    rv: &ResolverVisitor<'_>,
    type_arguments: Id<TypeArgumentList>,
) -> Vec<TypeId> {
    rv.ast
        .list_raw(rv.ast[type_arguments].arguments)
        .iter()
        .map(|&a| annotation_type(rv, a))
        .collect()
}

/// The key and value types of the type arguments of a map pattern, if
/// there are exactly two.
pub(crate) fn map_pattern_type_arguments(
    rv: &ResolverVisitor<'_>,
    node: Id<MapPattern>,
) -> Option<KeyValueTypes<TypeViewOf>> {
    let type_arguments = rv.ast[node].type_arguments?;
    match type_argument_types(rv, type_arguments)[..] {
        [key_type, value_type] => Some(KeyValueTypes {
            key_type: SharedTypeView::new(key_type),
            value_type: SharedTypeView::new(value_type),
        }),
        _ => None,
    }
}

/// Dart `ResolverVisitor.resolveMapPattern`.
pub fn resolve_map_pattern(
    rv: &mut ResolverVisitor<'_>,
    node: Id<MapPattern>,
    context: &SharedMatchContext,
) -> PatternResultOf {
    let mut type_arguments = None;
    if let Some(type_arguments_list) = rv.ast[node].type_arguments {
        rv.visit_node(type_arguments_list.raw());
        // Check that we have exactly two type arguments.
        let length = rv.ast.list_raw(rv.ast[type_arguments_list].arguments).len();
        if length == 2 {
            type_arguments = map_pattern_type_arguments(rv, node);
        } else {
            let d = rv.at(
                diag::expected_two_map_pattern_type_arguments(length as i64),
                type_arguments_list,
            );
            rv.report(d);
        }
    }

    let elements = rv.ast.list_raw(rv.ast[node].elements).to_vec();
    let result = rv.analyze_map_pattern(context, node.upcast(), type_arguments, &elements);
    let required_type = result.required_type.unwrap_type_view();
    set_required_type(rv, node.raw(), required_type);

    check_pattern_never_matches_value_type(
        rv,
        context,
        node.upcast(),
        required_type,
        result.matched_value_type.unwrap_type_view(),
    );

    result.into()
}

/// Dart `ResolverVisitor.buildSharedPatternFields`.
pub fn build_shared_pattern_fields(
    rv: &mut ResolverVisitor<'_>,
    fields: NodeList<PatternField>,
    must_be_named: bool,
) -> Vec<RecordPatternField<NodeId, Id<DartPattern>, Name>> {
    let fields = rv.ast.list(fields).to_vec();
    let mut result = Vec::with_capacity(fields.len());
    for field in fields {
        let pattern = rv.ast[field].pattern;
        let mut name_token = None;
        if let Some(field_name) = rv.ast[field].name {
            name_token = rv.ast[field_name].name;
            if name_token.is_none() {
                match variable_pattern_name(rv.ast, pattern) {
                    // Dart also records `variablePattern
                    // .fieldNameWithImplicitName = fieldName` (not read by
                    // resolution).
                    Some(token) => name_token = Some(token),
                    None => {
                        let d = rv.at(diag::missing_named_pattern_field_name(), field);
                        rv.report(d);
                    }
                }
            }
        } else if must_be_named {
            let d = rv.at(diag::positional_field_in_object_pattern(), field);
            rv.report(d);
        }
        let name = name_token.map(|t| {
            let lexeme = rv.ast.tokens.lexeme(t);
            rv.ctx.name(lexeme)
        });
        result.push(RecordPatternField {
            node: field.raw(),
            name,
            pattern,
        });
    }
    result
}

/// Dart `ResolverVisitor.checkPatternNeverMatchesValueType`.
pub fn check_pattern_never_matches_value_type(
    rv: &mut ResolverVisitor<'_>,
    context: &SharedMatchContext,
    pattern: Id<DartPattern>,
    required_type: TypeId,
    matched_value_type: TypeId,
) {
    if context.irrefutable_context.is_some() {
        return;
    }
    if can_be_subtype_of(rv, matched_value_type, required_type) {
        return;
    }
    let ast = &*rv.ast;
    let error_node: NodeId = if let Some(p) = ast.cast::<CastPattern>(pattern) {
        ast[p].type_.raw()
    } else if let Some(p) = ast.cast::<DeclaredVariablePattern>(pattern) {
        ast[p].type_.map_or(pattern.raw(), |t| t.raw())
    } else if let Some(p) = ast.cast::<ObjectPattern>(pattern) {
        ast[p].type_.raw()
    } else if let Some(p) = ast.cast::<dartr_ast::WildcardPattern>(pattern) {
        ast[p].type_.map_or(pattern.raw(), |t| t.raw())
    } else {
        pattern.raw()
    };
    let d = diag::pattern_never_matches_value_type(
        type_arg(&rv.ctx, matched_value_type),
        type_arg(&rv.ctx, required_type),
    );
    let d = rv.at(d, error_node);
    rv.report(d);
}

/// Dart `TypeSystemImpl.canBeSubtypeOf(left, right)`.
///
/// TODO(patterns): `canBeSubtypeOf` is not ported in `dartr_typesystem`
/// (it needs `ClassElementImpl.allSubtypes`). Until it is, every pair can
/// be a subtype, so `patternNeverMatchesValueType` is not reported.
fn can_be_subtype_of(rv: &ResolverVisitor<'_>, left: TypeId, right: TypeId) -> bool {
    let _ = (rv, left, right);
    true
}

/// The diagnostics with context messages of the shared type analyzer
/// (Dart `DiagnosticFactory().duplicateAssignmentPatternVariable`,
/// `duplicatePatternField`, `duplicateRestElementInPattern`) and
/// `inconsistentJoinedPatternVariable`.
pub(crate) fn shared_error_diagnostic(
    rv: &ResolverVisitor<'_>,
    error: &crate::shared_type_analyzer::SharedError,
) -> Option<dartr_diagnostics::LocatedDiagnostic> {
    use crate::shared_type_analyzer::SharedError;
    let ctx = &rv.ctx;
    let ast = &*rv.ast;
    let source = &ctx.fragment(rv.unit.fragment).source;
    let message = |offset: u32, length: u32, text: &str| DiagnosticMessage {
        file_path: source.path.to_string(),
        offset: offset as i64,
        length: length as i64,
        message: text.to_string(),
        url: Some(source.uri.to_string()),
    };
    let token_range = |token: dartr_syntax::TokenId| {
        let t = ast.tokens.get(token);
        (t.offset, t.end() - t.offset)
    };
    Some(match *error {
        SharedError::DuplicateAssignmentPatternVariable {
            variable,
            original,
            duplicate,
        } => {
            let name = ctx
                .element_data(variable.raw())
                .and_then(|d| d.name)
                .map(|n| ctx.name_str(n))
                .unwrap_or("");
            diag::duplicate_pattern_assignment_variable(name)
                .with_context_messages([message(
                    ast.offset(original),
                    ast.length(original),
                    "The first assigned variable pattern.",
                )])
                .at_offset(
                    ast.offset(duplicate) as usize,
                    ast.length(duplicate) as usize,
                )
        }
        SharedError::DuplicateRecordPatternField {
            name,
            original,
            duplicate,
            ..
        } => {
            // Dart `field.name!.name ?? field.name!.colon`.
            let target = |field: NodeId| {
                let field = ast.cast::<PatternField>(field)?;
                let field_name = ast[field].name?;
                Some(token_range(
                    ast[field_name].name.unwrap_or(ast[field_name].colon),
                ))
            };
            let (original_offset, original_length) = target(original)?;
            let (duplicate_offset, duplicate_length) = target(duplicate)?;
            diag::duplicate_pattern_field(ctx.name_str(name))
                .with_context_messages([message(
                    original_offset,
                    original_length,
                    "The first field.",
                )])
                .at_offset(duplicate_offset as usize, duplicate_length as usize)
        }
        SharedError::DuplicateRestPattern {
            original,
            duplicate,
        } => diag::duplicate_rest_element_in_pattern()
            .with_context_messages([message(
                ast.offset(original),
                ast.length(original),
                "The first rest element.",
            )])
            .at_offset(
                ast.offset(duplicate) as usize,
                ast.length(duplicate) as usize,
            ),
        SharedError::InconsistentJoinedPatternVariable {
            variable,
            component,
        } => {
            let name = ctx
                .element_data(variable.raw())
                .and_then(|d| d.name)
                .map(|n| ctx.name_str(n))
                .unwrap_or("");
            let component_data = ctx.element_data(component.raw());
            // Dart `component.firstFragment.nameOffset ?? 0`.
            let offset = component_data
                .and_then(|d| ctx.fragment_data(d.first_fragment))
                .and_then(|f| f.name_offset)
                .unwrap_or(0);
            // Dart `component.name?.length ?? 1`.
            let length = component_data
                .and_then(|d| d.name)
                .map_or(1, |n| ctx.name_str(n).encode_utf16().count());
            diag::inconsistent_pattern_variable_logical_or(name).at_offset(offset as usize, length)
        }
        _ => return None,
    })
}

// ------------------------------------------------------------ SwitchExhaustiveness

/// Dart `SwitchExhaustiveness`: tracks whether a `switch` statement has
/// `default`, or is on an enumeration and all the enum constants are
/// covered (language versions without patterns).
#[derive(Clone, Debug)]
pub struct SwitchExhaustiveness {
    /// Dart `_enumConstants`: if the switch is on an enumeration, the enum
    /// constants (fields) to cover.
    enum_constants: Option<IndexSet<ElementId>>,
    /// Dart `_isNullEnumValueCovered`.
    is_null_enum_value_covered: bool,
    /// Dart `isExhaustive`.
    pub is_exhaustive: bool,
}

impl SwitchExhaustiveness {
    /// Dart `SwitchExhaustiveness(expressionType)`.
    pub fn new(rv: &ResolverVisitor<'_>, expression_type: TypeId) -> SwitchExhaustiveness {
        if let TypeKind::Interface {
            element,
            nullability,
            ..
        } = *rv.ctx.ty(expression_type)
            && element.raw().is::<dartr_element::EnumElement>()
        {
            return SwitchExhaustiveness {
                enum_constants: Some(
                    element_ext::enum_constants(&rv.ctx, element.upcast())
                        .into_iter()
                        .collect(),
                ),
                is_null_enum_value_covered: nullability == Nullability::None,
                is_exhaustive: false,
            };
        }
        SwitchExhaustiveness {
            enum_constants: None,
            is_null_enum_value_covered: false,
            is_exhaustive: false,
        }
    }

    /// Dart `visitSwitchExpressionCase`.
    pub fn visit_switch_expression_case(
        &mut self,
        rv: &ResolverVisitor<'_>,
        node: Id<SwitchExpressionCase>,
    ) {
        if self.enum_constants.is_some() {
            let ast = &*rv.ast;
            let guarded_pattern = ast[node].guarded_pattern;
            let mut case_constant = None;
            if ast[guarded_pattern].when_clause.is_none() {
                let pattern = un_parenthesized_pattern(ast, ast[guarded_pattern].pattern);
                if let Some(p) = ast.cast::<ConstantPattern>(pattern) {
                    case_constant = Some(ast[p].expression);
                }
            }
            self.handle_case_constant(rv, case_constant);
        }
    }

    /// Dart `visitSwitchMember`.
    pub fn visit_switch_member(
        &mut self,
        rv: &ResolverVisitor<'_>,
        group: &SwitchStatementCaseGroup,
    ) {
        let ast = &*rv.ast;
        for &node in &group.members {
            if self.enum_constants.is_some() {
                let mut case_constant = None;
                if let Some(m) = ast.cast::<SwitchCase>(node) {
                    case_constant = Some(ast[m].expression);
                } else if let Some(m) = ast.cast::<SwitchPatternCase>(node) {
                    let guarded_pattern = ast[m].guarded_pattern;
                    if ast[guarded_pattern].when_clause.is_none() {
                        let pattern = un_parenthesized_pattern(ast, ast[guarded_pattern].pattern);
                        if let Some(p) = ast.cast::<ConstantPattern>(pattern) {
                            case_constant = Some(ast[p].expression);
                        }
                    }
                }
                self.handle_case_constant(rv, case_constant);
            } else if ast.is::<SwitchDefault>(node) {
                self.is_exhaustive = true;
            }
        }
    }

    /// Dart `_handleCaseConstant`.
    fn handle_case_constant(
        &mut self,
        rv: &ResolverVisitor<'_>,
        case_constant: Option<Id<Expression>>,
    ) {
        let Some(case_constant) = case_constant else {
            return;
        };
        let Some(enum_constants) = self.enum_constants.as_mut() else {
            return;
        };
        if let Some(element) = referenced_element(rv, case_constant)
            && member::base_element(&rv.ctx, element).is::<PropertyAccessorElement>()
            && let Some(variable) = member::variable(&rv.ctx, element)
        {
            enum_constants.shift_remove(&member::base_element(&rv.ctx, variable));
        }
        if rv.ast.is::<NullLiteral>(case_constant) {
            self.is_null_enum_value_covered = true;
        }
        if enum_constants.is_empty() && self.is_null_enum_value_covered {
            self.is_exhaustive = true;
        }
    }
}

/// Dart `SwitchExhaustiveness._referencedElement`.
fn referenced_element(rv: &ResolverVisitor<'_>, expression: Id<Expression>) -> Option<ElemRef> {
    let ast = &*rv.ast;
    if let Some(e) = ast.cast::<ParenthesizedExpression>(expression) {
        referenced_element(rv, ast[e].expression)
    } else if let Some(e) = ast.cast::<PrefixedIdentifier>(expression) {
        rv.element(e)
    } else if let Some(e) = ast.cast::<PropertyAccess>(expression) {
        rv.element(ast[e].property_name)
    } else if let Some(e) = ast.cast::<SimpleIdentifier>(expression) {
        rv.element(e)
    } else {
        None
    }
}

/// Dart `DartPattern.unParenthesized`.
pub fn un_parenthesized_pattern(ast: &Ast, mut pattern: Id<DartPattern>) -> Id<DartPattern> {
    while let Some(p) = ast.cast::<ParenthesizedPattern>(pattern) {
        pattern = ast[p].pattern;
    }
    pattern
}
