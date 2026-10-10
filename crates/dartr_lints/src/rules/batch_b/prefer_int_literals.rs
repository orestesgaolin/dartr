// Dart source: pkg/linter/lib/src/rules/prefer_int_literals.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::TypeId;
use dartr_typesystem::TypeExt;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::DoubleLiteral, "prefer_int_literals", check);
}

fn is_double(c: &LinterContext<'_>, ty: Option<TypeId>) -> bool {
    ty.is_some_and(|t| rctx(c).is_some_and(|ctx| ctx.is_dart_core_double(t)))
}

fn is_double_annotation(c: &LinterContext<'_>, annotation: Option<NodeId>) -> bool {
    is_double(c, annotation.and_then(|a| annotation_type(c, a)))
}

/// Dart `hasReturnTypeDouble`.
fn has_return_type_double(c: &LinterContext<'_>, node: Option<NodeId>) -> bool {
    let Some(node) = node else { return false };
    if kind(c, node) == NodeKind::FunctionExpression {
        if let Some(declaration) = c
            .ast
            .parent(node)
            .and_then(|p| c.ast.cast::<FunctionDeclaration>(p))
        {
            return is_double_annotation(c, c.ast[declaration].return_type.map(|t| t.raw()));
        }
    } else if let Some(method) = c.ast.cast::<MethodDeclaration>(node) {
        return is_double_annotation(c, c.ast[method].return_type.map(|t| t.raw()));
    }
    false
}

/// Dart `hasTypeDouble`.
fn has_type_double(c: &LinterContext<'_>, expression: NodeId) -> bool {
    let Some(parent) = c.ast.parent(expression) else {
        return false;
    };
    match kind(c, parent) {
        NodeKind::ArgumentList => is_double(c, c.corresponding_parameter_type(expression)),
        NodeKind::ListLiteral => {
            let type_arguments = c.ast[Id::<ListLiteral>::from_raw(parent)].type_arguments;
            let arguments = type_arguments.map(|t| c.ast.list_raw(c.ast[t].arguments).to_vec());
            arguments.as_ref().is_some_and(|a| a.len() == 1)
                && is_double_annotation(c, Some(arguments.unwrap()[0]))
        }
        NodeKind::NamedArgument => {
            c.ast
                .parent(parent)
                .is_some_and(|p| kind(c, p) == NodeKind::ArgumentList)
                && is_double(c, c.corresponding_parameter_type(parent))
        }
        NodeKind::ExpressionFunctionBody => has_return_type_double(c, c.ast.parent(parent)),
        NodeKind::ReturnStatement => {
            let body = this_or_ancestor_kind(c, parent, NodeKind::BlockFunctionBody);
            body.is_some() && has_return_type_double(c, body.and_then(|b| c.ast.parent(b)))
        }
        NodeKind::VariableDeclaration => c
            .ast
            .parent(parent)
            .and_then(|l| c.ast.cast::<VariableDeclarationList>(l))
            .is_some_and(|l| is_double_annotation(c, c.ast[l].type_.map(|t| t.raw()))),
        _ => false,
    }
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let value = c.ast[Id::<DoubleLiteral>::from_raw(node)].value;
    if !value.is_finite() || value != value.trunc() {
        return;
    }
    let replaceable = match c
        .ast
        .parent(node)
        .and_then(|p| c.ast.cast::<PrefixExpression>(p))
    {
        Some(prefix) => {
            lexeme(c, c.ast[prefix].operator) == "-" && has_type_double(c, prefix.raw())
        }
        None => has_type_double(c, node),
    };
    if replaceable {
        c.report_node(out, &diag::PREFER_INT_LITERALS, node, &[]);
    }
}
