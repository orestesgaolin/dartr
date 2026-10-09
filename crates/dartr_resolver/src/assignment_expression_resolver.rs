// Dart source: pkg/analyzer/lib/src/dart/resolver/assignment_expression_resolver.dart,
// pkg/analyzer/lib/src/generated/resolver.dart (visitAssignmentExpression,
// resolveForWrite, setReadElement, setWriteElement),
// pkg/analyzer/lib/src/error/assignment_verifier.dart (AssignmentVerifier)

//! `AssignmentExpressionResolver`: assignments `a = b`, compound
//! assignments `a += b` (with the operator element and the numeric
//! refinement), logical `a &&= b` / `a ||= b` and if-null `a ??= b`; the
//! resolution of a left-hand side (`resolveForWrite`, shared with the
//! prefix and postfix increments), the read and write elements and types,
//! `AssignmentExpressionShared.checkFinalAlreadyAssigned` and the
//! `AssignmentVerifier`.

use dartr_ast::{
    AssignmentExpression, Expression, Id, IndexExpression, MethodInvocation, NodeId,
    ParenthesizedExpression, PostfixExpression, PrefixExpression, PrefixedIdentifier,
    PropertyAccess, SimpleIdentifier,
};
use dartr_diagnostics::diag;
use dartr_element::diagnostics::type_arg;
use dartr_element::{
    ElemRef, ElementId, FragmentFlags, MethodElement, PromotableElement, Tag, TypeId, TypeKind,
    VariableElement,
};
use dartr_flow::flow_analysis::FlowAnalysis;
use dartr_flow::shared_type::{SharedTypeSchemaView, SharedTypeView};
use dartr_flow::type_analyzer::TypeAnalyzer;
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_syntax::TokenType;
use dartr_typesystem::TypeExt;
use dartr_typesystem::member;

use crate::ast_ext;
use crate::element_ext;
use crate::property_element_resolver::{self, PropertyElementResolverResult};
use crate::resolver::ResolverVisitor;
use crate::type_property_resolver::{self, PropertyQuery};

/// Dart `ResolverVisitor.visitAssignmentExpression(node, contextType:)`
/// (after `checkUnreachableNode`) = `AssignmentExpressionResolver.resolve`,
/// then the implicit call reference.
pub fn visit_assignment_expression(
    rv: &mut ResolverVisitor<'_>,
    node: Id<AssignmentExpression>,
    context_type: TypeId,
) {
    resolve(rv, node, context_type);
    let e = rv.insert_generic_function_instantiation(node.upcast(), context_type);
    rv.insert_implicit_call_reference(e, context_type);
}

/// Dart `AssignmentExpressionResolver.resolve(node, contextType:)`.
fn resolve(rv: &mut ResolverVisitor<'_>, node: Id<AssignmentExpression>, context_type: TypeId) {
    let operator = rv.ast.tokens.ty(rv.ast[node].operator);
    let has_read = operator != TokenType::EQ;
    let is_if_null = operator == TokenType::QUESTION_QUESTION_EQ;

    let left = rv.ast[node].left_hand_side;
    let left_resolution = resolve_for_write(rv, left, has_read);

    // The left-hand side may have been rewritten.
    let left = rv.ast[node].left_hand_side;
    let right = rv.ast[node].right_hand_side;

    let read_element = left_resolution.read_element();
    let write_element = left_resolution.write_element();

    if has_read {
        set_read_element(rv, left, read_element, left_resolution.at_dynamic_target);
        if let Some(record_field) = left_resolution.record_field {
            rv.tables.read_type.insert(node, record_field.ty);
        }
        resolve_operator(rv, node);
    }
    set_write_element(rv, left, write_element, left_resolution.at_dynamic_target);

    // TODO(scheglov): Use VariableElement and do in resolveForWrite() ?
    check_final_already_assigned(rv, left, false);

    let rhs_context = {
        let ctx = rv.ctx;
        let mut left_type = rv
            .tables
            .write_type
            .get(node)
            .copied()
            .unwrap_or(TypeId::INVALID);
        if let Some(e) = write_element
            && member::base_element(&ctx, e).is::<VariableElement>()
            && let Some(identifier) = rv.ast.cast::<SimpleIdentifier>(left)
        {
            left_type = local_variable_type(rv, identifier, member::base_element(&ctx, e), false);
        }
        compute_rhs_context(rv, node, left_type, operator)
    };

    let flow_active = rv.flow_analysis.flow.is_some();
    if flow_active && is_if_null {
        let info = rv.flow_analysis.get_expression_info(Some(left));
        let read_type = rv
            .tables
            .read_type
            .get(node)
            .copied()
            .unwrap_or(TypeId::DYNAMIC);
        rv.flow()
            .if_null_expression_right_begin(info, SharedTypeView::new(read_type));
    }

    let right = rv.resolve_expression(right, rhs_context);
    // Dart `flow?.whyNotPromoted(...)`: the why-not-promoted messages are
    // not ported yet.

    resolve_types(rv, node, context_type);

    if flow_active {
        if let Some(ElemRef::Base(e)) = write_element
            && let Some(variable) = e.cast::<PromotableElement>()
        {
            let ty = rv.type_or_throw(node);
            let right_info = if has_read {
                None
            } else {
                rv.flow_analysis.get_expression_info(Some(right))
            };
            let info = {
                let ast = &*rv.ast;
                rv.flow_analysis
                    .write(ast, node.raw(), variable, ty, right_info)
            };
            rv.flow_analysis.store_expression_info(node.upcast(), info);
        }
        if is_if_null {
            rv.flow().if_null_expression_end();
        }
    }
}

/// Dart `_checkForInvalidAssignment(writeType, right, rightType,
/// whyNotPromoted:)`.
fn check_for_invalid_assignment(
    rv: &mut ResolverVisitor<'_>,
    write_type: TypeId,
    right: Id<Expression>,
    right_type: TypeId,
) {
    let ctx = rv.ctx;
    if !matches!(ctx.ty(write_type), TypeKind::Void) && check_for_use_of_void_result(rv, right) {
        return;
    }

    let strict_casts = rv.unit.options.strict_casts;
    let ts = rv.type_system;
    if ts.is_assignable_to(right_type, write_type, strict_casts) {
        return;
    }

    if let TypeKind::Record { positional, .. } = *ctx.ty(write_type)
        && ctx.list(positional).len() == 1
        && !matches!(ctx.ty(right_type), TypeKind::Record { .. })
        && rv.ast.is::<ParenthesizedExpression>(right)
    {
        let field = ctx.list(positional)[0];
        if ts.is_assignable_to(field, right_type, strict_casts) {
            let d = rv.at(
                diag::record_literal_one_positional_no_trailing_comma_by_type(),
                right,
            );
            rv.report(d);
            return;
        }
    }

    // Dart `.withContextMessages(computeWhyNotPromotedMessages(...))`: the
    // context messages are not ported yet.
    let d = diag::invalid_assignment(type_arg(&ctx, right_type), type_arg(&ctx, write_type));
    let d = rv.at(d, right);
    rv.report(d);
}

/// Dart `_checkForUseOfVoidResult(expression)`: reports `useOfVoidResult`
/// if the type of [expression] is `void`.
pub fn check_for_use_of_void_result(
    rv: &mut ResolverVisitor<'_>,
    expression: Id<Expression>,
) -> bool {
    let Some(ty) = rv.static_type(expression) else {
        return false;
    };
    if !matches!(rv.ctx.ty(ty), TypeKind::Void) {
        return false;
    }
    let d = match rv.ast.cast::<MethodInvocation>(expression) {
        Some(invocation) => {
            let method_name = rv.ast[invocation].method_name;
            rv.at(diag::use_of_void_result(), method_name)
        }
        None => rv.at(diag::use_of_void_result(), expression),
    };
    rv.report(d);
    true
}

/// Dart `_computeRhsContext(node, leftType, operator, right)`.
fn compute_rhs_context(
    rv: &ResolverVisitor<'_>,
    node: Id<AssignmentExpression>,
    left_type: TypeId,
    operator: TokenType,
) -> TypeId {
    match operator {
        TokenType::EQ | TokenType::QUESTION_QUESTION_EQ => left_type,
        TokenType::AMPERSAND_AMPERSAND_EQ | TokenType::BAR_BAR_EQ => rv.ctx.tp.bool_type(),
        _ => {
            let ctx = rv.ctx;
            if let Some(method) = rv.element(node) {
                let parameters = member::formal_parameters(&ctx, method);
                if let Some(&first) = parameters.first() {
                    let method_element = member::base_element(&ctx, method).cast::<MethodElement>();
                    return rv.type_system.refine_numeric_invocation_context(
                        Some(left_type),
                        method_element,
                        left_type,
                        member::type_(&ctx, first),
                    );
                }
            }
            TypeId::UNKNOWN
        }
    }
}

/// Dart `_resolveOperator(node)`.
fn resolve_operator(rv: &mut ResolverVisitor<'_>, node: Id<AssignmentExpression>) {
    let left = rv.ast[node].left_hand_side;
    let operator = rv.ast[node].operator;
    let operator_type = rv.ast.tokens.ty(operator);

    let Some(left_type) = rv.tables.read_type.get(node).copied() else {
        return;
    };
    if left_type == TypeId::NEVER {
        return;
    }

    // Values of the type void cannot be used.
    // Example: `y += 0`, is not allowed.
    if operator_type != TokenType::EQ && matches!(rv.ctx.ty(left_type), TypeKind::Void) {
        let d = rv.at_token(diag::use_of_void_result(), operator);
        rv.report(d);
        return;
    }

    if operator_type == TokenType::AMPERSAND_AMPERSAND_EQ
        || operator_type == TokenType::BAR_BAR_EQ
        || operator_type == TokenType::EQ
        || operator_type == TokenType::QUESTION_QUESTION_EQ
    {
        return;
    }

    let Some(binary_operator_type) = operator_type.binary_operator_of_compound_assignment() else {
        return;
    };
    let method_name = binary_operator_type.lexeme();

    let result = type_property_resolver::resolve_at_token(
        rv,
        PropertyQuery {
            receiver: Some(left),
            receiver_type: left_type,
            name: method_name,
            has_read: operator_type != TokenType::EQ,
            has_write: true,
            property_error_entity: node.raw(),
            name_error_entity: node.raw(),
            parent_node: None,
        },
        operator,
    );
    rv.set_element(node, result.getter);
    if result.needs_getter_error {
        let d = diag::undefined_operator(method_name, type_arg(&rv.ctx, left_type));
        let d = rv.at_token(d, operator);
        rv.report(d);
    }
}

/// Dart `_resolveTypes(node, whyNotPromoted:, contextType:)`.
fn resolve_types(
    rv: &mut ResolverVisitor<'_>,
    node: Id<AssignmentExpression>,
    context_type: TypeId,
) {
    let ctx = rv.ctx;
    let ts = rv.type_system;
    let right_hand_side = rv.ast[node].right_hand_side;
    let operator = rv.ast.tokens.ty(rv.ast[node].operator);
    let read_type = rv
        .tables
        .read_type
        .get(node)
        .copied()
        .unwrap_or(TypeId::DYNAMIC);

    let assigned_type = if operator == TokenType::EQ || operator == TokenType::QUESTION_QUESTION_EQ
    {
        rv.type_or_throw(right_hand_side)
    } else if operator == TokenType::AMPERSAND_AMPERSAND_EQ || operator == TokenType::BAR_BAR_EQ {
        ctx.tp.bool_type()
    } else {
        let left_type = read_type;
        let operator_element = rv.element(node);
        if matches!(ctx.ty(left_type), TypeKind::Dynamic) {
            TypeId::DYNAMIC
        } else if let Some(operator_element) = operator_element {
            let right_type = rv.type_or_throw(right_hand_side);
            ts.refine_binary_expression_type(
                left_type,
                right_type,
                member::return_type(&ctx, operator_element),
                member::base_element(&ctx, operator_element).cast::<MethodElement>(),
            )
        } else {
            TypeId::INVALID
        }
    };

    let node_type = if operator == TokenType::QUESTION_QUESTION_EQ {
        // - An if-null assignment `E` of the form `lvalue ??= e` with context
        //   type `K` is analyzed as follows:
        //
        //   - Let `T1` be the read type the lvalue.
        let t1 = read_type;
        //   - Let `T2` be the type of `e` inferred with context type `T1`.
        let t2 = assigned_type;
        //   - Let `T` be `UP(NonNull(T1), T2)`.
        let non_null_t1 = ts.promote_to_non_null(t1);
        let t = ts.least_upper_bound(non_null_t1, t2);
        //   - Let `S` be the greatest closure of `K`.
        let s = ts.greatest_closure_of_schema(context_type);
        // If `inferenceUpdate3` is not enabled, then the type of `E` is `T`.
        if !rv.is_enabled(ExperimentalFlag::InferenceUpdate3) {
            t
        } else if ts.is_subtype_of(t, s) {
            //   - If `T <: S`, then the type of `E` is `T`.
            t
        } else if ts.is_subtype_of(non_null_t1, s) && ts.is_subtype_of(t2, s) {
            //   - Otherwise, if `NonNull(T1) <: S` and `T2 <: S`, then the
            //     type of `E` is `S`.
            s
        } else {
            //   - Otherwise, the type of `E` is `T`.
            t
        }
    } else {
        assigned_type
    };
    rv.record_static_type(node, node_type);

    // TODO(scheglov): Remove from ErrorVerifier?
    let write_type = rv
        .tables
        .write_type
        .get(node)
        .copied()
        .unwrap_or(TypeId::INVALID);
    check_for_invalid_assignment(rv, write_type, right_hand_side, assigned_type);
    if operator != TokenType::EQ && operator != TokenType::QUESTION_QUESTION_EQ {
        let right_hand_side = rv.ast[node].right_hand_side;
        rv.check_for_argument_type_not_assignable_for_argument(right_hand_side.raw(), false);
    }
}

/// Dart `AssignmentExpressionShared.checkFinalAlreadyAssigned(left,
/// isForEachIdentifier:)`.
pub fn check_final_already_assigned(
    rv: &mut ResolverVisitor<'_>,
    left: Id<Expression>,
    is_for_each_identifier: bool,
) {
    if rv.flow_analysis.flow.is_none() {
        return;
    }
    let Some(identifier) = rv.ast.cast::<SimpleIdentifier>(left) else {
        return;
    };
    let Some(ElemRef::Base(element)) = rv.element(identifier) else {
        return;
    };
    let Some(promotable) = element.cast::<PromotableElement>() else {
        return;
    };
    let assigned = rv.flow_analysis.is_definitely_assigned(promotable);
    let unassigned = rv.flow_analysis.is_definitely_unassigned(promotable);

    let ctx = rv.ctx;
    if element_ext::is_final(&ctx, element) {
        if element_ext::is_late(&ctx, element) {
            if is_for_each_identifier || assigned {
                let d = rv.at(diag::late_final_local_already_assigned(), left);
                rv.report(d);
            }
        } else if is_for_each_identifier || !unassigned {
            let name = ctx.element_name(element).unwrap_or("").to_string();
            let d = rv.at(diag::assignment_to_final_local(&name), left);
            rv.report(d);
        }
    }
}

// ------------------------------------------------------------------ resolver.dart

/// Dart `ResolverVisitor.resolveForWrite(node:, hasRead:)`: resolves the
/// left-hand side [node] of an assignment, an explicit
/// [AssignmentExpression], or implicit [PrefixExpression] or
/// [PostfixExpression].
pub fn resolve_for_write(
    rv: &mut ResolverVisitor<'_>,
    node: Id<Expression>,
    has_read: bool,
) -> PropertyElementResolverResult {
    if let Some(index_expression) = rv.ast.cast::<IndexExpression>(node) {
        if let Some(target) = rv.ast[index_expression].target {
            if rv.is_dot_shorthand(node) {
                // Recovery.
                // It's a compile-time error to use postfix or prefix operators
                // with dot shorthands. We provide an unknown type since this
                // shouldn't be valid code, but we want to prevent any crashes.
                rv.push_dot_shorthand_context(
                    target.raw(),
                    SharedTypeSchemaView::new(TypeId::UNKNOWN),
                );
            }
            rv.analyze_expression(
                target,
                SharedTypeSchemaView::new(TypeId::UNKNOWN),
                true,
                false,
                false,
            );
            rv.pop_rewrite();
        }

        if property_element_resolver::index_expression_is_null_aware(rv, index_expression) {
            let target = rv.ast[index_expression].target;
            crate::method_invocation_resolver::start_null_aware_access(rv, target);
            // Dart `nullSafetyDeadCodeVerifier.visitNode(node.index)` (wave D).
        }

        let result = property_element_resolver::resolve_index_expression(
            rv,
            index_expression,
            has_read,
            true,
        );

        let index = rv.ast[index_expression].index;
        let index =
            rv.resolve_expression(index, result.index_context_type.unwrap_or(TypeId::UNKNOWN));
        let read_element = if has_read { result.read_element() } else { None };
        rv.check_index_expression_index(index, read_element, result.write_element());
        result
    } else if let Some(prefixed) = rv.ast.cast::<PrefixedIdentifier>(node) {
        let prefix = rv.ast[prefixed].prefix;
        rv.analyze_expression(
            prefix.upcast(),
            SharedTypeSchemaView::new(TypeId::UNKNOWN),
            true,
            false,
            false,
        );
        rv.pop_rewrite();

        // TODO(scheglov): It would be nice to rewrite all such cases.
        if rv
            .static_type(prefix)
            .is_some_and(|t| matches!(rv.ctx.ty(t), TypeKind::Record { .. }))
        {
            let period = rv.ast[prefixed].period;
            let identifier = rv.ast[prefixed].identifier;
            let property_access = rv.ast.add(PropertyAccess {
                target: Some(prefix.upcast()),
                operator: period,
                property_name: identifier,
            });
            // Dart `node.replaceWith(propertyAccess)`.
            rv.ast.replace_with(prefixed, property_access);
            return property_element_resolver::resolve_property_access(
                rv,
                property_access,
                has_read,
                true,
                None,
            );
        }

        property_element_resolver::resolve_prefixed_identifier(rv, prefixed, has_read, true, false)
    } else if let Some(property_access) = rv.ast.cast::<PropertyAccess>(node) {
        if let Some(target) = rv.ast[property_access].target {
            if rv.is_dot_shorthand(node) {
                // Recovery.
                // It's a compile-time error to use a dot shorthand as the
                // target of a write, but to prevent any crashing we provide
                // an unknown context type since this shouldn't be valid code.
                rv.push_dot_shorthand_context(
                    target.raw(),
                    SharedTypeSchemaView::new(TypeId::UNKNOWN),
                );
            }
            rv.analyze_expression(
                target,
                SharedTypeSchemaView::new(TypeId::UNKNOWN),
                true,
                false,
                false,
            );
            rv.pop_rewrite();
        }
        if property_element_resolver::property_access_is_null_aware(rv, property_access) {
            let target = rv.ast[property_access].target;
            crate::method_invocation_resolver::start_null_aware_access(rv, target);
            // Dart `nullSafetyDeadCodeVerifier.visitNode(node.propertyName)`
            // (wave D).
        }

        property_element_resolver::resolve_property_access(
            rv,
            property_access,
            has_read,
            true,
            None,
        )
    } else if let Some(identifier) = rv.ast.cast::<SimpleIdentifier>(node) {
        let result =
            property_element_resolver::resolve_simple_identifier(rv, identifier, has_read, true);

        if has_read && result.read_element_requested.is_none() {
            let name = ast_ext::identifier_name(rv.ast, identifier).to_string();
            let d = rv.at(diag::undefined_identifier(&name), identifier);
            rv.report(d);
        }

        result
    } else {
        rv.resolve_expression(node, TypeId::UNKNOWN);
        PropertyElementResolverResult::default()
    }
}

/// Dart `localVariableTypeProvider.getType(node, isRead:)`: the type of
/// the variable [variable] at [node], promoted by flow analysis.
fn local_variable_type(
    rv: &mut ResolverVisitor<'_>,
    node: Id<SimpleIdentifier>,
    variable: ElementId,
    is_read: bool,
) -> TypeId {
    if variable.is::<PromotableElement>() && rv.flow_analysis.is_active() {
        let ctx = rv.ctx;
        return rv
            .flow_analysis
            .local_variable_type(&ctx, node.upcast(), variable, is_read);
    }
    element_ext::variable_type(&rv.ctx, variable)
}

/// The compound assignment that [node] is the operand of, if any (the
/// `parent` checks of Dart `setReadElement` / `setWriteElement`).
fn compound_assignment_parent(rv: &ResolverVisitor<'_>, node: Id<Expression>) -> Option<NodeId> {
    let parent = rv.ast.parent(node)?;
    if let Some(a) = rv.ast.cast::<AssignmentExpression>(parent) {
        return (rv.ast[a].left_hand_side == node).then_some(parent);
    }
    if let Some(p) = rv.ast.cast::<PostfixExpression>(parent) {
        return ast_ext::is_increment_operator(rv.ast.tokens.ty(rv.ast[p].operator))
            .then_some(parent);
    }
    if let Some(p) = rv.ast.cast::<PrefixExpression>(parent) {
        return ast_ext::is_increment_operator(rv.ast.tokens.ty(rv.ast[p].operator))
            .then_some(parent);
    }
    None
}

/// Whether [node] is a prefixed identifier, a property access or a simple
/// identifier.
fn is_property_like(rv: &ResolverVisitor<'_>, node: Id<Expression>) -> bool {
    rv.ast.is::<PrefixedIdentifier>(node)
        || rv.ast.is::<PropertyAccess>(node)
        || rv.ast.is::<SimpleIdentifier>(node)
}

/// Dart `ResolverVisitor.setReadElement(node, element, atDynamicTarget:)`.
pub fn set_read_element(
    rv: &mut ResolverVisitor<'_>,
    node: Id<Expression>,
    element: Option<ElemRef>,
    at_dynamic_target: bool,
) {
    let ctx = rv.ctx;
    let mut read_type = if at_dynamic_target {
        TypeId::DYNAMIC
    } else {
        TypeId::INVALID
    };
    let base = element.map(|e| member::base_element(&ctx, e));
    if rv.ast.is::<IndexExpression>(node) {
        if let Some(e) = element
            && base.is_some_and(|b| b.tag() == Tag::Method)
        {
            read_type = member::return_type(&ctx, e);
        }
    } else if is_property_like(rv, node) {
        if let Some(e) = element {
            let b = base.unwrap();
            if b.tag() == Tag::Getter {
                read_type = member::return_type(&ctx, e);
            } else if b.is::<VariableElement>()
                && let Some(identifier) = rv.ast.cast::<SimpleIdentifier>(node)
            {
                read_type = local_variable_type(rv, identifier, b, true);
            }
        }
    }

    if let Some(parent) = compound_assignment_parent(rv, node) {
        match element {
            Some(e) => {
                rv.tables.read_element.insert(parent, e);
            }
            None => {
                rv.tables.read_element.remove(parent);
            }
        }
        rv.tables.read_type.insert(parent, read_type);
    }
}

/// Dart `ResolverVisitor.setWriteElement(node, element, atDynamicTarget:)`.
pub fn set_write_element(
    rv: &mut ResolverVisitor<'_>,
    node: Id<Expression>,
    element: Option<ElemRef>,
    at_dynamic_target: bool,
) {
    let ctx = rv.ctx;
    let mut write_type = if at_dynamic_target {
        TypeId::DYNAMIC
    } else {
        TypeId::INVALID
    };
    let base = element.map(|e| member::base_element(&ctx, e));
    if rv.ast.is::<IndexExpression>(node) {
        if let Some(e) = element
            && base.is_some_and(|b| b.tag() == Tag::Method)
        {
            let parameters = member::formal_parameters(&ctx, e);
            if parameters.len() == 2 {
                write_type = member::type_(&ctx, parameters[1]);
            }
        }
    } else if is_property_like(rv, node) {
        if let Some(e) = element {
            let b = base.unwrap();
            if b.tag() == Tag::Setter {
                let is_origin_variable = element_ext::first_fragment_flags(&ctx, b)
                    .contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE);
                if is_origin_variable && let Some(variable) = member::variable(&ctx, e) {
                    write_type = member::type_(&ctx, variable);
                } else {
                    let parameters = member::formal_parameters(&ctx, e);
                    if parameters.len() == 1 {
                        write_type = member::type_(&ctx, parameters[0]);
                    }
                }
            } else if b.is::<VariableElement>() {
                write_type = member::type_(&ctx, e);
            }
        }
    }

    if let Some(parent) = compound_assignment_parent(rv, node) {
        match element {
            Some(e) => {
                rv.tables.write_element.insert(parent, e);
            }
            None => {
                rv.tables.write_element.remove(parent);
            }
        }
        rv.tables.write_type.insert(parent, write_type);
    }
}

// ------------------------------------------------------------------ AssignmentVerifier

/// Dart `AssignmentVerifier.verify(node:, requested:, recovery:,
/// receiverType:)`: we resolved [node] and found that it references the
/// [requested] element. Verifies that this element is actually writable.
///
/// If the [requested] element is `None`, we might have the [recovery]
/// element, which is definitely not a valid write target. We want to
/// report a good error about this.
///
/// When the [receiver_type] is not `None`, we report `undefinedSetter`
/// instead of a more generic `undefinedIdentifier`.
pub fn verify_assignment(
    rv: &mut ResolverVisitor<'_>,
    node: Id<SimpleIdentifier>,
    requested: Option<ElemRef>,
    recovery: Option<ElemRef>,
    receiver_type: Option<TypeId>,
) {
    let ctx = rv.ctx;
    if let Some(requested) = requested {
        let base = member::base_element(&ctx, requested);
        if base.is::<VariableElement>() && element_ext::is_const(&ctx, base) {
            let d = rv.at(diag::assignment_to_const(), node);
            rv.report(d);
        }
        return;
    }

    let recovery_base = recovery.map(|e| member::base_element(&ctx, e));
    let tag = recovery_base.map(|e| e.tag());
    match tag {
        Some(Tag::Dynamic)
        | Some(Tag::Class)
        | Some(Tag::Enum)
        | Some(Tag::Mixin)
        | Some(Tag::ExtensionType)
        | Some(Tag::TypeAlias)
        | Some(Tag::TypeParameter) => {
            let d = rv.at(diag::assignment_to_type(), node);
            rv.report(d);
        }
        Some(Tag::LocalFunction) | Some(Tag::TopLevelFunction) => {
            let d = rv.at(diag::assignment_to_function(), node);
            rv.report(d);
        }
        Some(Tag::Method) => {
            let d = rv.at(diag::assignment_to_method(), node);
            rv.report(d);
        }
        Some(Tag::Prefix) => {
            if let Some(prefix_name) = ctx.element_name(recovery_base.unwrap()) {
                let d = rv.at(
                    diag::prefix_identifier_not_followed_by_dot(prefix_name),
                    node,
                );
                rv.report(d);
            }
        }
        Some(Tag::Getter) => {
            let Some(variable) = member::variable(&ctx, recovery.unwrap()) else {
                return;
            };
            let variable = member::base_element(&ctx, variable);
            let Some(variable_name) = ctx.element_name(variable) else {
                return;
            };
            let variable_name = variable_name.to_string();

            if element_ext::is_const(&ctx, variable) {
                let d = rv.at(diag::assignment_to_const(), node);
                rv.report(d);
            } else if variable.tag() == Tag::Field
                && element_ext::first_fragment_flags(&ctx, variable)
                    .contains(FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_GETTER_SETTER)
            {
                let class_name = ctx
                    .element_data(variable)
                    .and_then(|d| d.enclosing)
                    .and_then(|e| ctx.element_name(e))
                    .unwrap_or("")
                    .to_string();
                let d = rv.at(
                    diag::assignment_to_final_no_setter(&variable_name, &class_name),
                    node,
                );
                rv.report(d);
            } else {
                let d = rv.at(diag::assignment_to_final(&variable_name), node);
                rv.report(d);
            }
        }
        Some(Tag::MultiplyDefined) => {
            // Will be reported in ErrorVerifier.
        }
        _ => {
            if rv.ast.tokens.get(rv.ast[node].token).is_synthetic() {
                return;
            }
            let name = ast_ext::identifier_name(rv.ast, node).to_string();
            let d = match receiver_type {
                Some(receiver_type) => diag::undefined_setter(&name, type_arg(&ctx, receiver_type)),
                None => diag::undefined_identifier(&name),
            };
            let d = rv.at(d, node);
            rv.report(d);
        }
    }
}
