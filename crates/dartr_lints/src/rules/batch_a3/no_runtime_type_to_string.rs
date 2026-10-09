// Dart source: pkg/linter/lib/src/rules/no_runtimeType_toString.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ClassElement, ElementFlags, ExtensionElement, TypeKind};

pub fn register(registry: &mut RuleVisitorRegistry) {
    registry.add_interpolation_expression("no_runtimetype_tostring", check);
    registry.add_method_invocation("no_runtimetype_tostring", check);
}

fn identifier_name<'a>(ctx: &'a LinterContext<'_>, id: Id<SimpleIdentifier>) -> &'a str {
    ctx.ast.tokens.lexeme(ctx.ast[id].token)
}

fn is_runtime_type_access(ctx: &LinterContext<'_>, node: NodeId) -> bool {
    match ctx.ast.kind(node) {
        NodeKind::PropertyAccess => {
            let n = &ctx.ast[Id::<PropertyAccess>::from_raw(node)];
            n.target.is_some_and(|t| {
                matches!(
                    ctx.ast.kind(t),
                    NodeKind::ThisExpression | NodeKind::SuperExpression
                )
            }) && identifier_name(ctx, n.property_name) == "runtimeType"
        }
        NodeKind::SimpleIdentifier => {
            identifier_name(ctx, Id::from_raw(node)) == "runtimeType"
                && ctx.element(node).is_some_and(|e| {
                    dartr_typesystem::member::base_element(&ctx.resolved.unwrap().ctx, e).kind()
                        == dartr_element::ElementKind::Getter
                })
        }
        _ => false,
    }
}

fn can_skip(ctx: &LinterContext<'_>, mut node: NodeId) -> bool {
    loop {
        match ctx.ast.kind(node) {
            NodeKind::AssertInitializer
            | NodeKind::AssertStatement
            | NodeKind::ThrowExpression
            | NodeKind::CatchClause
            | NodeKind::MixinDeclaration => return true,
            NodeKind::ClassDeclaration
                if ctx.ast[Id::<ClassDeclaration>::from_raw(node)]
                    .abstract_keyword
                    .is_some() =>
            {
                return true;
            }
            NodeKind::ExtensionDeclaration => {
                let Some(r) = ctx.resolved else {
                    return true;
                };
                let Some(extension) = ctx
                    .declared_element(node)
                    .and_then(|e| e.cast::<ExtensionElement>())
                else {
                    return true;
                };
                let Some(extended) = r.ctx.get(extension).extended_type.get() else {
                    return true;
                };
                let TypeKind::Interface { element, .. } = *r.ctx.ty(extended) else {
                    return true;
                };
                let Some(class) = element.cast::<ClassElement>() else {
                    return true;
                };
                if r.ctx
                    .get(class)
                    .flags
                    .has(ElementFlags::CLASS_ELEMENT_IS_ABSTRACT)
                {
                    return true;
                }
            }
            _ => {}
        }
        let Some(parent) = ctx.ast.parent(node) else {
            return false;
        };
        node = parent;
    }
}

fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    match ctx.ast.kind(node) {
        NodeKind::InterpolationExpression => {
            let expression = ctx.ast[Id::<InterpolationExpression>::from_raw(node)].expression;
            if is_runtime_type_access(ctx, expression.raw()) && !can_skip(ctx, node) {
                ctx.report_node(out, &diag::NO_RUNTIMETYPE_TOSTRING, expression, &[]);
            }
        }
        NodeKind::MethodInvocation => {
            let n = &ctx.ast[Id::<MethodInvocation>::from_raw(node)];
            if identifier_name(ctx, n.method_name) == "toString"
                && n.target
                    .is_some_and(|t| is_runtime_type_access(ctx, t.raw()))
                && !can_skip(ctx, node)
            {
                ctx.report_node(out, &diag::NO_RUNTIMETYPE_TOSTRING, n.method_name, &[]);
            }
        }
        _ => {}
    }
}
