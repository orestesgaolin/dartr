// Dart source: pkg/linter/lib/src/rules/no_logic_in_create_state.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ClassElement, TypeKind};
use dartr_typesystem::TypeExt;

pub fn register(registry: &mut RuleVisitorRegistry) {
    registry.add_method_declaration("no_logic_in_create_state", check);
}

fn is_stateful_widget(ctx: &LinterContext<'_>, class: dartr_element::EId<ClassElement>) -> bool {
    let Some(r) = ctx.resolved else {
        return false;
    };
    let this = r.ctx.interface_this_type(class.upcast());
    std::iter::once(this).chain(r.ctx.all_supertypes(this)).any(|t| {
        matches!(*r.ctx.ty(t),TypeKind::Interface{element,..} if r.ctx.element_name(element.raw())==Some("StatefulWidget") && r.ctx.element_library_uri(element.raw())==Some("package:flutter/src/widgets/framework.dart"))
    })
}

fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &ctx.ast[Id::<MethodDeclaration>::from_raw(node)];
    if ctx.ast.tokens.lexeme(n.name) != "createState" {
        return;
    }
    let Some(class_node) = ctx
        .ast
        .parent(node)
        .and_then(|b| ctx.ast.parent(b))
        .and_then(|c| ctx.ast.cast::<ClassDeclaration>(c))
    else {
        return;
    };
    let Some(class) = ctx
        .declared_element(class_node.raw())
        .and_then(|e| e.cast::<ClassElement>())
    else {
        return;
    };
    if !is_stateful_widget(ctx, class) {
        return;
    }
    let body = n.body.raw();
    let expression = match ctx.ast.kind(body) {
        NodeKind::ExpressionFunctionBody => Some(
            ctx.ast[Id::<ExpressionFunctionBody>::from_raw(body)]
                .expression
                .raw(),
        ),
        NodeKind::BlockFunctionBody => {
            let block = ctx.ast[Id::<BlockFunctionBody>::from_raw(body)].block;
            let statements = ctx.ast.list(ctx.ast[block].statements);
            if statements.len() == 1 {
                ctx.ast
                    .cast::<ReturnStatement>(statements[0])
                    .and_then(|r| ctx.ast[r].expression.map(Id::raw))
            } else {
                None
            }
        }
        NodeKind::EmptyFunctionBody => return,
        _ => None,
    };
    if let Some(expression) = expression {
        let empty_creation = match ctx.ast.kind(expression) {
            NodeKind::InstanceCreationExpression => ctx
                .ast
                .list(
                    ctx.ast[ctx.ast[Id::<InstanceCreationExpression>::from_raw(expression)]
                        .argument_list]
                        .arguments,
                )
                .is_empty(),
            NodeKind::DotShorthandConstructorInvocation => ctx
                .ast
                .list(
                    ctx.ast[ctx.ast[Id::<DotShorthandConstructorInvocation>::from_raw(expression)]
                        .argument_list]
                        .arguments,
                )
                .is_empty(),
            _ => false,
        };
        if empty_creation {
            return;
        }
        ctx.report_node(out, &diag::NO_LOGIC_IN_CREATE_STATE, expression, &[]);
    } else {
        ctx.report_node(out, &diag::NO_LOGIC_IN_CREATE_STATE, body, &[]);
    }
}
