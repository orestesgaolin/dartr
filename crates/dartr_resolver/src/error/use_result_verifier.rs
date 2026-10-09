// Dart source: pkg/analyzer/lib/src/error/use_result_verifier.dart

//! `UseResultVerifier`: the result of an invocation or property access of
//! an element annotated with `@useResult` must be used (`unused_result`).
//!
//! The error verifier (`generated/error_verifier.dart`) calls the
//! `check_*` functions on single nodes; they are generic over
//! [`VerifierHost`].
//!
//! `useResultMessage` reads the field `message` of the annotation value;
//! `UseResult` of `package:meta` has no such field (`reason`), so the
//! message is always absent and `unused_result_with_message` is never
//! reported, as in Dart.

use dartr_ast::{
    ArgumentList, AsExpression, AssertInitializer, AssertStatement, AssignmentExpression, Ast,
    AwaitExpression, BinaryExpression, CascadeExpression, CommentReference, ConditionalExpression,
    ConstructorFieldInitializer, DoStatement, ExpressionFunctionBody, ForEachParts, ForElement,
    ForLoopParts, ForPartsWithDeclarations, ForPartsWithExpression, ForPartsWithPattern,
    FunctionExpressionInvocation, HideCombinator, Id, IfElement, IfStatement, IndexExpression,
    InstanceCreationExpression, InterpolationExpression, ListLiteral, MapLiteralEntry,
    MethodInvocation, NamedArgument, NodeId, ParenthesizedExpression, PatternAssignment,
    PatternVariableDeclaration, PostfixExpression, PrefixExpression, PrefixedIdentifier,
    PropertyAccess, RecordLiteral, RecordLiteralNamedField, ReturnStatement, SetOrMapLiteral,
    ShowCombinator, SimpleIdentifier, SpreadElement, SwitchExpression, SwitchExpressionCase,
    SwitchStatement, ThrowExpression, VariableDeclaration, WhenClause, WhileStatement,
    YieldStatement,
};
use dartr_diagnostics::diag;
use dartr_element::{ElementId, Tag};
use dartr_syntax::TokenType;
use dartr_typesystem::member;

use super::VerifierHost;
use super::support::{corresponding_parameter, display_name, in_declaration_context, node_range};
use crate::element_metadata::{AnnotationRef, UnitAst, accessor_variable, element_annotations, flags};

/// Dart `UseResultVerifier.checkFunctionExpressionInvocation(node)`.
pub fn check_function_expression_invocation<'a, H: VerifierHost<'a>>(
    host: &mut H,
    node: Id<FunctionExpressionInvocation>,
) {
    let Some(element) = base_element(host, node.raw()) else {
        return;
    };
    check(host, node.raw(), element);
}

/// Dart `UseResultVerifier.checkInstanceCreationExpression(node)`.
pub fn check_instance_creation_expression<'a, H: VerifierHost<'a>>(
    host: &mut H,
    node: Id<InstanceCreationExpression>,
) {
    let constructor_name = host.ast()[node].constructor_name;
    let Some(element) = base_element(host, constructor_name.raw()) else {
        return;
    };
    check(host, node.raw(), element);
}

/// Dart `UseResultVerifier.checkMethodInvocation(node)`.
pub fn check_method_invocation<'a, H: VerifierHost<'a>>(host: &mut H, node: Id<MethodInvocation>) {
    let method_name = host.ast()[node].method_name;
    let Some(element) = base_element(host, method_name.raw()) else {
        return;
    };
    check(host, node.raw(), element);
}

/// Dart `UseResultVerifier.checkPropertyAccess(node)`.
pub fn check_property_access<'a, H: VerifierHost<'a>>(host: &mut H, node: Id<PropertyAccess>) {
    let property_name = host.ast()[node].property_name;
    let Some(element) = base_element(host, property_name.raw()) else {
        return;
    };
    check(host, node.raw(), element);
}

/// Dart `UseResultVerifier.checkSimpleIdentifier(node)`.
pub fn check_simple_identifier<'a, H: VerifierHost<'a>>(host: &mut H, node: Id<SimpleIdentifier>) {
    let ast = host.ast();
    if in_declaration_context(ast, node) {
        return;
    }
    // Covered by checkPropertyAccess, checkMethodInvocation and
    // checkFunctionExpressionInvocation respectively.
    if let Some(parent) = ast.parent(node)
        && (ast.is::<PropertyAccess>(parent)
            || ast.is::<MethodInvocation>(parent)
            || ast.is::<FunctionExpressionInvocation>(parent))
    {
        return;
    }
    let Some(element) = base_element(host, node.raw()) else {
        return;
    };
    check(host, node.raw(), element);
}

fn base_element<'a, H: VerifierHost<'a>>(host: &H, node: NodeId) -> Option<ElementId> {
    let e = host.tables().element.get(node).copied()?;
    Some(member::base_element(&host.ctx(), e))
}

/// Dart `_check(node, element)`.
fn check<'a, H: VerifierHost<'a>>(host: &mut H, node: NodeId, element: ElementId) {
    let ast = host.ast();
    let ctx = host.ctx();
    let mut parent = ast.parent(node);
    if let Some(p) = parent
        && ast.is::<PrefixedIdentifier>(p)
    {
        parent = ast.parent(p);
    }
    if let Some(p) = parent {
        if ast.is::<CommentReference>(p) {
            // Don't flag references in comments.
            return;
        }
        if ast.is::<ShowCombinator>(p) || ast.is::<HideCombinator>(p) {
            return;
        }
    }

    let unit = Some(UnitAst {
        ast: host.ast(),
        tables: host.tables(),
    });
    let Some(annotation) = get_use_result_metadata(&ctx, element, unit) else {
        return;
    };
    if passes_using_param(host, node, &annotation) {
        return;
    }
    let ast = host.ast();
    if is_used(ast, node) {
        return;
    }

    let to_annotate = node_to_annotate(ast, node);
    let name = match ast.cast::<SimpleIdentifier>(to_annotate) {
        Some(s) => ast.tokens.lexeme(ast[s].token).to_string(),
        None => display_name(&ctx, element),
    };
    let range = node_range(ast, to_annotate);
    // Dart `annotation.useResultMessage`: the field `message`, which
    // `UseResult` does not have.
    let message: Option<String> = None;
    let d = match message {
        Some(message) if !message.is_empty() => diag::unused_result_with_message(&name, &message),
        _ => diag::unused_result(&name),
    };
    host.report(d.at_offset(range.0 as usize, range.1 as usize));
}

/// Dart `_passesUsingParam(node, annotation)`.
fn passes_using_param<'a, H: VerifierHost<'a>>(
    host: &H,
    node: NodeId,
    annotation: &AnnotationRef<'_>,
) -> bool {
    let ast = host.ast();
    let Some(invocation) = ast.cast::<MethodInvocation>(node) else {
        return false;
    };
    // Dart `annotation.useResultUnlessParameter`.
    let Some(unless_param) = annotation.string_argument(None, Some("parameterDefined")) else {
        return false;
    };
    let ctx = host.ctx();
    let argument_list: Id<ArgumentList> = ast[invocation].argument_list;
    for &argument in ast.list(ast[argument_list].arguments) {
        let parameter = corresponding_parameter(&ctx, ast, host.tables(), argument.raw());
        let name = parameter.and_then(|p| dartr_typesystem::TypeExt::element_name(&ctx, p));
        if name == Some(unless_param.as_str()) {
            return true;
        }
    }
    false
}

/// Dart `_getUseResultMetadata(element)`.
fn get_use_result_metadata<'a>(
    ctx: &dartr_element::Ctx<'a>,
    mut element: ElementId,
    unit: Option<UnitAst<'a>>,
) -> Option<AnnotationRef<'a>> {
    // Implicit getters/setters.
    if let Some(variable) = accessor_variable(ctx, element) {
        element = variable;
    }
    element_annotations(ctx, element, unit)
        .into_iter()
        .find(|a| a.is(ctx, flags::USE_RESULT))
}

/// Dart `AstNode.nodeToAnnotate`.
fn node_to_annotate(ast: &Ast, node: NodeId) -> NodeId {
    if let Some(m) = ast.cast::<MethodInvocation>(node) {
        return ast[m].method_name.raw();
    }
    if let Some(p) = ast.cast::<PropertyAccess>(node) {
        return ast[p].property_name.raw();
    }
    if let Some(f) = ast.cast::<FunctionExpressionInvocation>(node) {
        return node_to_annotate(ast, ast[f].function.raw());
    }
    node
}

/// Dart `ForParts.condition`.
fn for_parts_condition(ast: &Ast, parts: NodeId) -> Option<NodeId> {
    if let Some(p) = ast.cast::<ForPartsWithDeclarations>(parts) {
        return ast[p].condition.map(|c| c.raw());
    }
    if let Some(p) = ast.cast::<ForPartsWithExpression>(parts) {
        return ast[p].condition.map(|c| c.raw());
    }
    if let Some(p) = ast.cast::<ForPartsWithPattern>(parts) {
        return ast[p].condition.map(|c| c.raw());
    }
    None
}

fn is_for_parts(ast: &Ast, node: NodeId) -> bool {
    ast.is::<ForPartsWithDeclarations>(node)
        || ast.is::<ForPartsWithExpression>(node)
        || ast.is::<ForPartsWithPattern>(node)
}

/// Dart `_isUsed(node)`.
fn is_used(ast: &Ast, node: NodeId) -> bool {
    let Some(parent) = ast.parent(node) else {
        return false;
    };
    if let Some(c) = ast.cast::<CascadeExpression>(parent) {
        return ast[c].target.raw() == node;
    }
    if let Some(p) = ast.cast::<PrefixedIdentifier>(parent) {
        if ast[p].prefix.raw() == node {
            return true;
        }
        return is_used(ast, parent);
    }
    if let Some(p) = ast.cast::<PostfixExpression>(parent) {
        // Null-checking a result is not a "use." Other uses, like `++`, do
        // count.
        return ast.tokens.ty(ast[p].operator) == TokenType::BANG && is_used(ast, parent);
    }
    if ast.is::<AsExpression>(parent)
        || ast.is::<AwaitExpression>(parent)
        || ast.is::<ConditionalExpression>(parent)
        || ast.is::<ForElement>(parent)
        || ast.is::<IfElement>(parent)
        || ast.is::<ParenthesizedExpression>(parent)
        || ast.is::<PrefixExpression>(parent)
        || ast.is::<SpreadElement>(parent)
    {
        return is_used(ast, parent);
    }
    if is_for_parts(ast, parent) {
        // If [node] is the condition of a for-loop, it is used; if it is one
        // of the updaters, it is not.
        return for_parts_condition(ast, parent) == Some(node);
    }
    ast.is::<ArgumentList>(parent)
        || ast.is::<AssertInitializer>(parent)
        || ast.is::<AssertStatement>(parent)
        // Node should always be RHS so no need to check for a property
        // assignment.
        || ast.is::<AssignmentExpression>(parent)
        || ast.is::<BinaryExpression>(parent)
        || ast.is::<ConstructorFieldInitializer>(parent)
        || ast.is::<DoStatement>(parent)
        || ast.is::<ExpressionFunctionBody>(parent)
        || ast.is::<ForEachParts>(parent)
        || ast.is::<ForLoopParts>(parent)
        || ast.is::<FunctionExpressionInvocation>(parent)
        || ast.is::<IfStatement>(parent)
        || ast.is::<IndexExpression>(parent)
        || ast.is::<InterpolationExpression>(parent)
        || ast.is::<ListLiteral>(parent)
        || ast.is::<MapLiteralEntry>(parent)
        || ast.is::<MethodInvocation>(parent)
        || ast.is::<NamedArgument>(parent)
        || ast.is::<PatternAssignment>(parent)
        || ast.is::<PatternVariableDeclaration>(parent)
        || ast.is::<PropertyAccess>(parent)
        || ast.is::<RecordLiteral>(parent)
        || ast.is::<RecordLiteralNamedField>(parent)
        || ast.is::<ReturnStatement>(parent)
        || ast.is::<SetOrMapLiteral>(parent)
        || ast.is::<SwitchExpression>(parent)
        || ast.is::<SwitchExpressionCase>(parent)
        || ast.is::<SwitchStatement>(parent)
        || ast.is::<ThrowExpression>(parent)
        || ast.is::<VariableDeclaration>(parent)
        || ast.is::<WhenClause>(parent)
        || ast.is::<WhileStatement>(parent)
        || ast.is::<YieldStatement>(parent)
}

/// Whether [element] is a getter (Dart `useResultMessage` returns `null`
/// for a getter annotation).
#[allow(dead_code)]
fn is_getter(element: ElementId) -> bool {
    element.tag() == Tag::Getter
}
