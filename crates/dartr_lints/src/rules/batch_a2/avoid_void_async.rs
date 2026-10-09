// Dart source: pkg/linter/lib/src/rules/avoid_void_async.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::TypeId;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(NodeKind::FunctionDeclaration, "avoid_void_async", check);
    registry.add(NodeKind::MethodDeclaration, "avoid_void_async", check);
}

fn check(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let (name, return_type, body) = match context.ast.kind(node) {
        NodeKind::FunctionDeclaration => {
            let n = &context.ast[context.ast.cast::<FunctionDeclaration>(node).unwrap()];
            if context.ast.tokens.lexeme(n.name) == "main"
                && context
                    .ast
                    .parent(node)
                    .is_some_and(|p| context.ast.kind(p) == NodeKind::CompilationUnit)
            {
                return;
            }
            (
                n.name,
                n.return_type,
                context.ast[n.function_expression].body,
            )
        }
        NodeKind::MethodDeclaration => {
            let n = &context.ast[context.ast.cast::<MethodDeclaration>(node).unwrap()];
            (n.name, n.return_type, n.body)
        }
        _ => return,
    };
    let is_async = match context.ast.kind(body) {
        NodeKind::BlockFunctionBody => {
            let body = &context.ast[context.ast.cast::<BlockFunctionBody>(body).unwrap()];
            body.keyword.is_some() && body.star.is_none()
        }
        NodeKind::ExpressionFunctionBody => {
            let body = &context.ast[context.ast.cast::<ExpressionFunctionBody>(body).unwrap()];
            body.keyword.is_some() && body.star.is_none()
        }
        _ => false,
    };
    if is_async
        && return_type.and_then(|ty| super::helpers::annotation_type(context, ty.raw()))
            == Some(TypeId::VOID)
    {
        context.report_token(out, &diag::AVOID_VOID_ASYNC, name, &[]);
    }
}
