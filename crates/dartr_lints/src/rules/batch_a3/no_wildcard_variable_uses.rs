// Dart source: pkg/linter/lib/src/rules/no_wildcard_variable_uses.dart

use crate::{ExperimentalFlag, LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::ElementKind;

pub fn register(registry: &mut RuleVisitorRegistry, context: &LinterContext<'_>) {
    if context.is_feature_enabled(ExperimentalFlag::WildcardVariables) {
        return;
    }
    registry.add_simple_identifier("no_wildcard_variable_uses", check);
}

fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &ctx.ast[Id::<SimpleIdentifier>::from_raw(node)];
    let name = ctx.ast.tokens.lexeme(n.token);
    if !name.bytes().all(|b| b == b'_') {
        return;
    }
    let Some(element) = ctx.element(node) else {
        return;
    };
    let base = dartr_typesystem::member::base_element(&ctx.resolved.unwrap().ctx, element);
    if matches!(
        base.kind(),
        ElementKind::LocalVariable | ElementKind::Parameter
    ) {
        ctx.report_node(out, &diag::NO_WILDCARD_VARIABLE_USES, node, &[]);
    }
}
