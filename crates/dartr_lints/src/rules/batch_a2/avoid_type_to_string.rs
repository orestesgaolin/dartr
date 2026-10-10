// Dart source: pkg/linter/lib/src/rules/avoid_type_to_string.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ExtensionElement, InterfaceElement, TypeId, TypeKind};
use dartr_typesystem::{TypeExt, member};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::MethodInvocation,
        "avoid_type_to_string",
        check_invocation,
    );
    registry.add(
        NodeKind::ArgumentList,
        "avoid_type_to_string",
        check_arguments,
    );
}
fn implicit_receiver_type(context: &LinterContext<'_>, node: NodeId) -> Option<TypeId> {
    let resolved = context.resolved?;
    for ancestor in super::helpers::ancestors(context.ast, node) {
        match context.ast.kind(ancestor) {
            NodeKind::ClassDeclaration | NodeKind::MixinDeclaration | NodeKind::EnumDeclaration => {
                let element = context
                    .declared_element(ancestor)?
                    .cast::<InterfaceElement>()?;
                return Some(resolved.ctx.interface_this_type(element));
            }
            NodeKind::ExtensionDeclaration => {
                let element = context
                    .declared_element(ancestor)?
                    .cast::<ExtensionElement>()?;
                return resolved
                    .ctx
                    .get(element)
                    .extended_type
                    .get()
                    .filter(|&ty| matches!(resolved.ctx.ty(ty), TypeKind::Interface { .. }));
            }
            _ => {}
        }
    }
    None
}

fn receiver_type(
    context: &LinterContext<'_>,
    node: NodeId,
    explicit_target: Option<Id<Expression>>,
) -> Option<TypeId> {
    if let Some(ty) = explicit_target.and_then(|target| context.static_type(target)) {
        return Some(ty);
    }
    for ancestor in super::helpers::ancestors(context.ast, node) {
        if let Some(cascade) = context.ast.cast::<CascadeExpression>(ancestor) {
            return context.static_type(context.ast[cascade].target);
        }
        if FunctionBody::test(context.ast.kind(ancestor)) {
            break;
        }
    }
    implicit_receiver_type(context, node)
}

fn is_bad(
    context: &LinterContext<'_>,
    target_type: Option<TypeId>,
    method: Id<SimpleIdentifier>,
) -> bool {
    let Some(resolved) = context.resolved else {
        return false;
    };
    if context.ast.tokens.lexeme(context.ast[method].token) != "toString" {
        return false;
    }
    let Some(element) = context.element(method) else {
        return false;
    };
    if !member::is_object_member(&resolved.ctx, element) {
        return false;
    }
    let Some(ty) = target_type else {
        return false;
    };
    if !matches!(resolved.ctx.ty(ty), TypeKind::Interface { .. }) {
        return false;
    }
    context
        .type_system()
        .is_some_and(|ts| ts.is_subtype_of(ty, resolved.ctx.tp.type_type()))
}
fn check_invocation(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &context.ast[context.ast.cast::<MethodInvocation>(node).unwrap()];
    let target_type = receiver_type(context, node, n.target);
    if is_bad(context, target_type, n.method_name) {
        context.report_node(out, &diag::AVOID_TYPE_TO_STRING, n.method_name, &[]);
    }
}
fn check_arguments(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &context.ast[context.ast.cast::<ArgumentList>(node).unwrap()];
    for &argument in context.ast.list(n.arguments) {
        let expression = context
            .ast
            .cast::<NamedArgument>(argument)
            .map(|named| context.ast[named].argument_expression.raw())
            .unwrap_or_else(|| argument.raw());
        match context.ast.kind(expression) {
            NodeKind::PropertyAccess => {
                let p = &context.ast[context.ast.cast::<PropertyAccess>(expression).unwrap()];
                let target_type = receiver_type(context, expression, p.target);
                if is_bad(context, target_type, p.property_name) {
                    context.report_node(out, &diag::AVOID_TYPE_TO_STRING, p.property_name, &[]);
                }
            }
            NodeKind::PrefixedIdentifier => {
                let p = &context.ast[context.ast.cast::<PrefixedIdentifier>(expression).unwrap()];
                if is_bad(context, context.static_type(p.prefix), p.identifier) {
                    context.report_node(out, &diag::AVOID_TYPE_TO_STRING, p.identifier, &[]);
                }
            }
            NodeKind::SimpleIdentifier => {
                let identifier = context.ast.cast::<SimpleIdentifier>(expression).unwrap();
                if is_bad(
                    context,
                    implicit_receiver_type(context, expression),
                    identifier,
                ) {
                    context.report_node(out, &diag::AVOID_TYPE_TO_STRING, identifier, &[]);
                }
            }
            _ => {}
        }
    }
}
