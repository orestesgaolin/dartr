// Dart source: pkg/linter/lib/src/rules/invalid_case_patterns.dart

use crate::{ExperimentalFlag, LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::TypeExt;

pub fn register(registry: &mut RuleVisitorRegistry, context: &LinterContext<'_>) {
    if !context.is_feature_enabled(ExperimentalFlag::Patterns) {
        registry.add(NodeKind::SwitchCase, "invalid_case_patterns", check);
    }
}
fn unparenthesized(ast: &Ast, mut node: NodeId) -> NodeId {
    while ast.kind(node) == NodeKind::ParenthesizedExpression {
        node = ast[ast.cast::<ParenthesizedExpression>(node).unwrap()]
            .expression
            .raw();
    }
    node
}

fn is_dart_core_identifier(
    context: &LinterContext<'_>,
    identifier: Id<SimpleIdentifier>,
    name: &str,
) -> bool {
    if context.ast.tokens.lexeme(context.ast[identifier].token) != name {
        return false;
    }
    let Some(resolved) = context.resolved else {
        return false;
    };
    context.element(identifier).is_some_and(|element| {
        dartr_typesystem::member::library(&resolved.ctx, element)
            .is_some_and(|library| resolved.ctx.library_uri(library) == "dart:core")
    })
}

fn check(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let case = &context.ast[context.ast.cast::<SwitchCase>(node).unwrap()];
    let expression = unparenthesized(context.ast, case.expression.raw());
    let invalid = match context.ast.kind(expression) {
        NodeKind::SetOrMapLiteral => context.ast
            [context.ast.cast::<SetOrMapLiteral>(expression).unwrap()]
        .const_keyword
        .is_none(),
        NodeKind::ListLiteral => context.ast[context.ast.cast::<ListLiteral>(expression).unwrap()]
            .const_keyword
            .is_none(),
        NodeKind::MethodInvocation => {
            let n = &context.ast[context.ast.cast::<MethodInvocation>(expression).unwrap()];
            is_dart_core_identifier(context, n.method_name, "identical")
        }
        NodeKind::PrefixExpression => {
            let n = &context.ast[context.ast.cast::<PrefixExpression>(expression).unwrap()];
            context.ast.kind(n.operand) != NodeKind::IntegerLiteral
        }
        NodeKind::BinaryExpression | NodeKind::ConditionalExpression | NodeKind::IsExpression => {
            true
        }
        NodeKind::PropertyAccess => {
            let n = &context.ast[context.ast.cast::<PropertyAccess>(expression).unwrap()];
            is_dart_core_identifier(context, n.property_name, "length")
        }
        NodeKind::InstanceCreationExpression => {
            let n = &context.ast[context
                .ast
                .cast::<InstanceCreationExpression>(expression)
                .unwrap()];
            let is_const = context
                .element(n.constructor_name)
                .and_then(|e| super::helpers::base_element(context, e))
                .and_then(|e| {
                    context.resolved.and_then(|r| {
                        r.ctx
                            .element_data(e)
                            .and_then(|d| r.ctx.fragment_data(d.first_fragment))
                    })
                })
                .is_some_and(|f| {
                    f.flags
                        .has(dartr_element::FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST)
                });
            is_const
                && n.keyword
                    .is_none_or(|t| context.ast.tokens.lexeme(t) != "const")
        }
        NodeKind::SimpleIdentifier => {
            context.ast.tokens.lexeme(
                context.ast[context.ast.cast::<SimpleIdentifier>(expression).unwrap()].token,
            ) == "_"
        }
        _ => false,
    };
    if invalid {
        context.report_node(out, &diag::INVALID_CASE_PATTERNS, expression, &[]);
    }
}
