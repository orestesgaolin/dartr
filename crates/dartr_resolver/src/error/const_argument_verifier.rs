// Dart source: pkg/analyzer/lib/src/error/const_argument_verifier.dart

//! Checks that the arguments for a parameter annotated with `@mustBeConst`
//! are constant, and that functions with such parameters are not torn off
//! (Dart `ConstArgumentsVerifier`).
//!
//! The error verifier calls the `visit_*` functions on single nodes (Dart
//! `ErrorVerifier.visitX` -> `_constArgumentsVerifier.visitX(node)`).
//!
//! Open point: Dart `Metadata.hasMustBeConst` needs the resolved
//! annotations of the parameter. The annotations of linked elements are in
//! the `ConstExprs` of their cycle, which the resolver cannot read, and they
//! are not resolved. [`has_must_be_const`] reads the resolved annotations of
//! the declarations in the unit of the host only; a parameter declared in
//! another unit or library is never `@mustBeConst` here.

use dartr_ast::{
    AdjacentStrings, AnonymousMethodInvocation, ArgumentList, AssignmentExpression, Ast,
    BinaryExpression, BooleanLiteral, CascadeExpression, CommentReference, ConstructorName,
    ConstructorReference, DotShorthandInvocation, DotShorthandPropertyAccess, DoubleLiteral,
    Expression, FunctionExpressionInvocation, FunctionReference, Id, IndexExpression,
    InstanceCreationExpression, IntegerLiteral, InvocationExpression, ListLiteral,
    MethodInvocation, NamedArgument, NodeId, NodeKind, NullLiteral, ParenthesizedExpression,
    PostfixExpression, PrefixExpression, PrefixedIdentifier, PropertyAccess, RecordLiteral,
    RedirectingConstructorInvocation, SetOrMapLiteral, SimpleIdentifier, SimpleStringLiteral,
    StringInterpolation, SuperConstructorInvocation, SymbolLiteral,
};
use dartr_diagnostics::diag;
use dartr_element::{ElemRef, ElementId, ExecutableElement, Tag, TypeKind};
use dartr_syntax::TokenId;
use dartr_typesystem::TypeExt;
use dartr_typesystem::member;

use super::VerifierHost;
use crate::ast_ext::{in_constant_context, instance_creation_is_const};

/// Dart `visitAnonymousMethodInvocation(node)`.
pub fn visit_anonymous_method_invocation<'a, H: VerifierHost<'a>>(
    host: &mut H,
    node: Id<AnonymousMethodInvocation>,
) {
    let ast = host.ast();
    let Some(parameters) = ast[node].parameters else {
        return;
    };
    let Some(&parameter) = ast.list_raw(ast[parameters].parameters).first() else {
        return;
    };
    let ctx = host.ctx();
    let Some(&fragment) = host.tables().declared_fragment.get(parameter) else {
        return;
    };
    let Some(&element) = ctx
        .fragment_data(fragment)
        .and_then(|f| f.element.try_get())
    else {
        return;
    };
    if has_must_be_const(host, ElemRef::Base(element), node.raw()) {
        let Some(target) = anonymous_method_invocation_real_target(ast, node) else {
            return;
        };
        if !is_const(host, target) {
            let name = ctx.element_name(element).unwrap_or("").to_string();
            let d = host.at(diag::non_const_argument_for_const_parameter(&name), target);
            host.report(d);
        }
    }
}

/// Dart `visitAssignmentExpression(node)`.
pub fn visit_assignment_expression<'a, H: VerifierHost<'a>>(
    host: &mut H,
    node: Id<AssignmentExpression>,
) {
    let n = &host.ast()[node];
    check(host, &[n.right_hand_side.raw()], Entity::Token(n.operator));
}

/// Dart `visitBinaryExpression(node)`.
pub fn visit_binary_expression<'a, H: VerifierHost<'a>>(host: &mut H, node: Id<BinaryExpression>) {
    let n = &host.ast()[node];
    check(host, &[n.right_operand.raw()], Entity::Token(n.operator));
}

/// Dart `visitConstructorReference(node)`.
pub fn visit_constructor_reference<'a, H: VerifierHost<'a>>(
    host: &mut H,
    node: Id<ConstructorReference>,
) {
    let constructor_name = host.ast()[node].constructor_name;
    let element = host.element(constructor_name);
    check_tearoff(host, node.upcast(), element);
}

/// Dart `visitFunctionExpressionInvocation(node)`.
pub fn visit_function_expression_invocation<'a, H: VerifierHost<'a>>(
    host: &mut H,
    node: Id<FunctionExpressionInvocation>,
) {
    let invoke_type = host.tables().invoke_type.get(node).copied();
    if invoke_type.is_some_and(|t| matches!(host.ctx().ty(t), TypeKind::Function(_))) {
        let arguments = arguments(host.ast(), host.ast()[node].argument_list);
        check(host, &arguments, Entity::Node(node.raw()));
    }
}

/// Dart `visitIndexExpression(node)`.
pub fn visit_index_expression<'a, H: VerifierHost<'a>>(host: &mut H, node: Id<IndexExpression>) {
    let n = &host.ast()[node];
    check(host, &[n.index.raw()], Entity::Token(n.left_bracket));
}

/// Dart `visitInstanceCreationExpression(node)`.
pub fn visit_instance_creation_expression<'a, H: VerifierHost<'a>>(
    host: &mut H,
    node: Id<InstanceCreationExpression>,
) {
    let ast = host.ast();
    if in_constant_context(ast, node.raw()) {
        return;
    }
    let arguments = arguments(ast, ast[node].argument_list);
    let constructor_name = ast[node].constructor_name;
    check(host, &arguments, Entity::Node(constructor_name.raw()));
}

/// Dart `visitMethodInvocation(node)`.
pub fn visit_method_invocation<'a, H: VerifierHost<'a>>(host: &mut H, node: Id<MethodInvocation>) {
    let ast = host.ast();
    let arguments = arguments(ast, ast[node].argument_list);
    let method_name = ast[node].method_name;
    check(host, &arguments, Entity::Node(method_name.raw()));
}

/// Dart `visitPrefixedIdentifier(node)`.
pub fn visit_prefixed_identifier<'a, H: VerifierHost<'a>>(
    host: &mut H,
    node: Id<PrefixedIdentifier>,
) {
    let identifier = host.ast()[node].identifier;
    let element = host.element(node).or_else(|| host.element(identifier));
    check_tearoff(host, identifier.upcast(), element);
}

/// Dart `visitPropertyAccess(node)`.
pub fn visit_property_access<'a, H: VerifierHost<'a>>(host: &mut H, node: Id<PropertyAccess>) {
    let property_name = host.ast()[node].property_name;
    let element = host.element(property_name);
    check_tearoff(host, property_name.upcast(), element);
}

/// Dart `visitRedirectingConstructorInvocation(node)`.
pub fn visit_redirecting_constructor_invocation<'a, H: VerifierHost<'a>>(
    host: &mut H,
    node: Id<RedirectingConstructorInvocation>,
) {
    let ast = host.ast();
    let n = &ast[node];
    let error_node = match n.constructor_name {
        Some(name) => Entity::Node(name.raw()),
        None => Entity::Token(n.this_keyword),
    };
    let arguments = arguments(ast, n.argument_list);
    check(host, &arguments, error_node);
}

/// Dart `visitSimpleIdentifier(node)`.
pub fn visit_simple_identifier<'a, H: VerifierHost<'a>>(host: &mut H, node: Id<SimpleIdentifier>) {
    let ast = host.ast();
    if let Some(parent) = ast.parent(node) {
        let raw = node.raw();
        if let Some(p) = ast.cast::<PropertyAccess>(parent)
            && ast[p].property_name.raw() == raw
        {
            return;
        }
        if let Some(p) = ast.cast::<PrefixedIdentifier>(parent)
            && ast[p].identifier.raw() == raw
        {
            return;
        }
        if let Some(p) = ast.cast::<DotShorthandPropertyAccess>(parent)
            && ast[p].property_name.raw() == raw
        {
            return;
        }
        if let Some(p) = ast.cast::<DotShorthandInvocation>(parent)
            && ast[p].member_name.raw() == raw
        {
            return;
        }
        if let Some(p) = ast.cast::<MethodInvocation>(parent)
            && ast[p].method_name.raw() == raw
        {
            return;
        }
    }
    let element = host.element(node);
    check_tearoff(host, node.upcast(), element);
}

/// Dart `visitSuperConstructorInvocation(node)`.
pub fn visit_super_constructor_invocation<'a, H: VerifierHost<'a>>(
    host: &mut H,
    node: Id<SuperConstructorInvocation>,
) {
    let ast = host.ast();
    let n = &ast[node];
    let error_node = match n.constructor_name {
        Some(name) => Entity::Node(name.raw()),
        None => Entity::Token(n.super_keyword),
    };
    let arguments = arguments(ast, n.argument_list);
    check(host, &arguments, error_node);
}

/// Dart `SyntacticEntity` of `_check(errorNode:)` (unused by Dart `_check`,
/// which reports at the argument; kept for the signature).
#[derive(Clone, Copy)]
enum Entity {
    #[allow(dead_code)]
    Node(NodeId),
    #[allow(dead_code)]
    Token(TokenId),
}

/// The arguments of [list] (Dart `argumentList.arguments`).
fn arguments(ast: &Ast, list: Id<ArgumentList>) -> Vec<NodeId> {
    ast.list_raw(ast[list].arguments).to_vec()
}

/// Dart `Argument.argumentExpression`: the expression of a named argument,
/// else the argument.
fn argument_expression(ast: &Ast, argument: NodeId) -> Id<Expression> {
    match ast.cast::<NamedArgument>(argument) {
        Some(named) => ast[named].argument_expression,
        None => Id::from_raw(argument),
    }
}

/// Dart `_check(arguments:, errorNode:)`.
fn check<'a, H: VerifierHost<'a>>(host: &mut H, arguments: &[NodeId], _error_node: Entity) {
    for &argument in arguments {
        let Some(parameter) = corresponding_parameter(host, argument) else {
            continue;
        };
        let ctx = host.ctx();
        let Some(parameter_name) = member::name(&ctx, parameter) else {
            continue;
        };
        if has_must_be_const(host, parameter, argument) {
            let resolved_argument = argument_expression(host.ast(), argument);
            if !is_const(host, resolved_argument) {
                let d = host.at(
                    diag::non_const_argument_for_const_parameter(parameter_name),
                    argument,
                );
                host.report(d);
            }
        }
    }
}

/// Dart `_checkTearoff(node, element)`.
fn check_tearoff<'a, H: VerifierHost<'a>>(
    host: &mut H,
    node: Id<Expression>,
    element: Option<ElemRef>,
) {
    let Some(element) = element else {
        return;
    };
    let ctx = host.ctx();
    if !member::base_element(&ctx, element).is::<ExecutableElement>() {
        return;
    }
    let parameters = member::formal_parameters(&ctx, element);
    if !parameters
        .iter()
        .any(|&p| has_must_be_const(host, p, node.raw()))
    {
        return;
    }
    if is_tear_off(host, node)
        && let Some(name) = member::name(&ctx, element)
        && !name.is_empty()
    {
        let d = host.at(diag::tearoff_with_must_be_const_parameter(name), node);
        host.report(d);
    }
}

/// Dart `_isConst(expression)`.
fn is_const<'a, H: VerifierHost<'a>>(host: &H, expression: Id<Expression>) -> bool {
    let ast = host.ast();
    let raw = expression.raw();
    if in_constant_context(ast, raw) {
        return true;
    }
    if let Some(creation) = ast.cast::<InstanceCreationExpression>(raw) {
        return instance_creation_is_const(ast, creation);
    }
    if ast.cast::<BooleanLiteral>(raw).is_some()
        || ast.cast::<DoubleLiteral>(raw).is_some()
        || ast.cast::<IntegerLiteral>(raw).is_some()
        || ast.cast::<NullLiteral>(raw).is_some()
        || ast.cast::<SimpleStringLiteral>(raw).is_some()
        || ast.cast::<AdjacentStrings>(raw).is_some()
        || ast.cast::<SymbolLiteral>(raw).is_some()
    {
        return true;
    }
    // `isConst` of record and typed literals: a `const` keyword (the
    // constant context is checked above).
    if let Some(n) = ast.cast::<RecordLiteral>(raw) {
        return ast[n].const_keyword.is_some();
    }
    if let Some(n) = ast.cast::<ListLiteral>(raw) {
        return ast[n].const_keyword.is_some();
    }
    if let Some(n) = ast.cast::<SetOrMapLiteral>(raw) {
        return ast[n].const_keyword.is_some();
    }
    if ast.cast::<StringInterpolation>(raw).is_some() {
        return false;
    }
    // Dart `expression is Identifier`: `expression.element`.
    let element = if let Some(p) = ast.cast::<PrefixedIdentifier>(raw) {
        host.element(p).or_else(|| host.element(ast[p].identifier))
    } else if ast.cast::<SimpleIdentifier>(raw).is_some() {
        host.element(raw)
    } else {
        None
    };
    let Some(element) = element else {
        return false;
    };
    let ctx = host.ctx();
    let base = member::base_element(&ctx, element);
    match base.tag() {
        Tag::Getter => member::variable(&ctx, element)
            .is_some_and(|v| crate::element_ext::is_const(&ctx, member::base_element(&ctx, v))),
        Tag::Field
        | Tag::TopLevelVariable
        | Tag::LocalVariable
        | Tag::PatternVariable
        | Tag::BindPatternVariable
        | Tag::JoinPatternVariable
        | Tag::FormalParameter
        | Tag::FieldFormalParameter
        | Tag::SuperFormalParameter => crate::element_ext::is_const(&ctx, base),
        _ => false,
    }
}

/// Dart `_isTearOff(node)`.
fn is_tear_off<'a, H: VerifierHost<'a>>(host: &H, node: Id<Expression>) -> bool {
    let ast = host.ast();
    let raw = node.raw();
    if ast.cast::<ConstructorReference>(raw).is_some()
        || ast.cast::<FunctionReference>(raw).is_some()
        || ast.cast::<DotShorthandPropertyAccess>(raw).is_some()
    {
        return true;
    }
    if in_comment_reference(ast, raw) {
        return false;
    }
    if ast.cast::<SimpleIdentifier>(raw).is_some() {
        let mut parent = ast.parent(raw);
        if let Some(p) = parent
            && ast.cast::<ConstructorName>(p).is_some()
        {
            parent = ast.parent(p);
        }
        while let Some(p) = parent
            && ast.cast::<ParenthesizedExpression>(p).is_some()
        {
            parent = ast.parent(p);
        }
        if parent.is_some_and(|p| ast.cast::<InvocationExpression>(p).is_some()) {
            return false;
        }
        let ctx = host.ctx();
        if let Some(element) = host.element(raw) {
            let tag = member::base_element(&ctx, element).tag();
            if matches!(tag, Tag::TopLevelFunction | Tag::Method) {
                return true;
            }
        }
    }
    false
}

/// Dart `AstNode.inCommentReference` (utilities/extensions/ast.dart).
fn in_comment_reference(ast: &Ast, node: NodeId) -> bool {
    let mut current = ast.parent(node);
    while let Some(p) = current {
        if ast.cast::<CommentReference>(p).is_some() {
            return true;
        }
        current = ast.parent(p);
    }
    false
}

/// Dart `AnonymousMethodInvocationImpl.realTarget`.
fn anonymous_method_invocation_real_target(
    ast: &Ast,
    node: Id<AnonymousMethodInvocation>,
) -> Option<Id<Expression>> {
    let operator = ast.tokens.lexeme(ast[node].operator);
    if operator == ".." || operator == "?.." {
        let mut current = ast.parent(node);
        while let Some(p) = current {
            if let Some(c) = ast.cast::<CascadeExpression>(p) {
                return Some(ast[c].target);
            }
            current = ast.parent(p);
        }
        return None;
    }
    ast[node].target
}

/// Dart `Argument.correspondingParameter` (an argument in an argument list)
/// and `Expression.correspondingParameter` (the index of an index
/// expression, the right operand of a binary expression, the right-hand
/// side of an assignment).
fn corresponding_parameter<'a, H: VerifierHost<'a>>(host: &H, argument: NodeId) -> Option<ElemRef> {
    let ast = host.ast();
    let parent = ast.parent(argument)?;
    if ast.kind(parent) == NodeKind::ArgumentList {
        // Dart `ArgumentList._getStaticParameterElementFor(argument)`; the
        // resolver records it keyed by the argument expression.
        let expression = argument_expression(ast, argument);
        return host.tables().param_element.get(expression).copied();
    }
    let ctx = host.ctx();
    let first_parameter = |element: Option<ElemRef>| -> Option<ElemRef> {
        let element = element?;
        if !member::base_element(&ctx, element).is::<ExecutableElement>() {
            return None;
        }
        member::formal_parameters(&ctx, element).first().copied()
    };
    if let Some(index) = ast.cast::<IndexExpression>(parent) {
        if ast[index].index.raw() != argument {
            return None;
        }
        // Dart `IndexExpressionImpl._staticParameterElementForIndex`.
        let mut element = host.element(index);
        if let Some(compound) = ast.parent(index)
            && (ast.cast::<AssignmentExpression>(compound).is_some()
                || ast.cast::<PrefixExpression>(compound).is_some()
                || ast.cast::<PostfixExpression>(compound).is_some())
        {
            let tables = host.tables();
            element = tables
                .write_element
                .get(compound)
                .or_else(|| tables.read_element.get(compound))
                .copied();
        }
        return first_parameter(element);
    }
    if let Some(binary) = ast.cast::<BinaryExpression>(parent) {
        if ast[binary].right_operand.raw() != argument {
            return None;
        }
        let invoke_type = *host.tables().invoke_type.get(binary)?;
        let TypeKind::Function(function) = ctx.ty(invoke_type) else {
            return None;
        };
        return ctx.list(function.params).first().and_then(|p| p.element);
    }
    if let Some(assignment) = ast.cast::<AssignmentExpression>(parent) {
        if ast[assignment].right_hand_side.raw() != argument {
            return None;
        }
        // Dart `AssignmentExpressionImpl._staticParameterElementForRightHandSide`.
        let is_eq = ast.tokens.lexeme(ast[assignment].operator) == "=";
        let executable = if is_eq {
            host.tables().write_element.get(assignment).copied()
        } else {
            host.element(assignment)
        }?;
        if !member::base_element(&ctx, executable).is::<ExecutableElement>() {
            return None;
        }
        let parameters = member::formal_parameters(&ctx, executable);
        if parameters.is_empty() {
            return None;
        }
        let lhs = ast[assignment].left_hand_side;
        if is_eq && ast.cast::<IndexExpression>(lhs).is_some() {
            return if parameters.len() == 2 {
                Some(parameters[1])
            } else {
                None
            };
        }
        return Some(parameters[0]);
    }
    None
}

/// Dart `parameter.metadata.hasMustBeConst`: whether an annotation of
/// [parameter] is the getter `mustBeConst` of `package:meta` (Dart
/// `ElementAnnotation.isMustBeConst`).
///
/// Reads the resolved annotation nodes of the declaration in the unit of
/// [host] (found from the offset of the fragment); see the module doc.
/// [any_node] is a node of the unit (to find its root).
pub fn has_must_be_const<'a, H: VerifierHost<'a>>(
    host: &H,
    parameter: ElemRef,
    any_node: NodeId,
) -> bool {
    let ctx = host.ctx();
    let base = member::base_element(&ctx, parameter);
    let Some(data) = ctx.element_data(base) else {
        return false;
    };
    let mut fragment = Some(data.first_fragment);
    while let Some(f) = fragment {
        let Some(fragment_data) = ctx.fragment_data(f) else {
            break;
        };
        let annotations = &fragment_data.metadata.annotations;
        if annotations
            .iter()
            .any(|a| a.library_fragment == host.fragment())
            && let Some(offset) = fragment_data.first_token_offset
        {
            for node in annotation_nodes(host.ast(), any_node, offset) {
                if let Some(element) = host.element(node)
                    && is_must_be_const_getter(&ctx, member::base_element(&ctx, element))
                {
                    return true;
                }
            }
        }
        fragment = fragment_data.next_fragment;
    }
    false
}

/// Dart `ElementAnnotationImpl.isMustBeConst`
/// (`_isPackageMetaGetter('mustBeConst')`).
fn is_must_be_const_getter(ctx: &dartr_element::Ctx<'_>, element: ElementId) -> bool {
    element.tag() == Tag::Getter && ctx.is_element(element, "meta", "mustBeConst")
}

/// The `Annotation` nodes of the declaration that starts at [offset] (the
/// first token offset of a fragment with metadata: its first annotation).
fn annotation_nodes(ast: &Ast, any_node: NodeId, offset: u32) -> Vec<NodeId> {
    let root = ast.root(any_node);
    let Some(mut node) = ast.node_covering(root, offset, 0) else {
        return Vec::new();
    };
    while ast.kind(node) != NodeKind::Annotation {
        match ast.parent(node) {
            Some(p) => node = p,
            None => return Vec::new(),
        }
    }
    let Some(declaration) = ast.parent(node) else {
        return Vec::new();
    };
    ast.children(declaration)
        .into_iter()
        .filter(|&c| ast.kind(c) == NodeKind::Annotation)
        .collect()
}
