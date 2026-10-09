// Dart source: pkg/linter/lib/src/rules/library_annotations.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::{TypeExt, member};
pub fn register(r: &mut RuleVisitorRegistry) {
    r.add(NodeKind::Annotation, "library_annotations", check);
}
fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &ctx.ast[Id::<Annotation>::from_raw(node)];
    let Some(r) = ctx.resolved else {
        return;
    };
    let Some(e) = ctx.element(node) else {
        return;
    };
    let constructor = member::base_element(&r.ctx, e);
    let Some(class) = r.ctx.element_data(constructor).and_then(|d| d.enclosing) else {
        return;
    };
    if !r.ctx.is_element(class, "dart.core", "pragma") {
        return;
    }
    let Some(args) = n.arguments else {
        return;
    };
    let Some(first) = ctx.ast.list(ctx.ast[args].arguments).first() else {
        return;
    };
    let expression = ctx
        .ast
        .cast::<NamedArgument>(*first)
        .map_or(first.raw(), |a| ctx.ast[a].argument_expression.raw());
    if ctx
        .constant_value(expression)
        .and_then(|value| value.to_string_value().map(str::to_owned))
        .as_deref()
        == Some("dart2js:late:trust")
    {
        ctx.report_node(out, &diag::LIBRARY_ANNOTATIONS, node, &[]);
    }
}
