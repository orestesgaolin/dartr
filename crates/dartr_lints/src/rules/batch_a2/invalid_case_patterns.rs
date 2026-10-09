// Dart source: pkg/linter/lib/src/rules/invalid_case_patterns.dart

use crate::{ExperimentalFlag, LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};

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
            context.ast.tokens.lexeme(context.ast[n.method_name].token) == "identical"
                && context.element(n.method_name).is_some_and(|e| {
                    context
                        .resolved
                        .is_some_and(|r| dartr_typesystem::member::is_object_member(&r.ctx, e))
                })
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
            context
                .ast
                .tokens
                .lexeme(context.ast[n.property_name].token)
                == "length"
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
