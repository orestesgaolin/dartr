// Dart source: pkg/linter/lib/src/rules/matching_super_parameters.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{FormalParameterElement, ParameterKind};
use dartr_typesystem::TypeExt;

pub fn register(registry: &mut RuleVisitorRegistry) {
    registry.add_constructor_declaration("matching_super_parameters", check);
    registry.add_primary_constructor_declaration("matching_super_parameters", check);
}

fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let list = match ctx.ast.kind(node) {
        NodeKind::ConstructorDeclaration => {
            ctx.ast[Id::<ConstructorDeclaration>::from_raw(node)].parameters
        }
        NodeKind::PrimaryConstructorDeclaration => {
            ctx.ast[Id::<PrimaryConstructorDeclaration>::from_raw(node)].formal_parameters
        }
        _ => return,
    };
    let Some(resolved) = ctx.resolved else {
        return;
    };
    for parameter in ctx.ast.list(ctx.ast[list].parameters) {
        let Some(super_parameter) = ctx.ast.cast::<SuperFormalParameter>(*parameter) else {
            continue;
        };
        if ctx.ast[super_parameter].kind == ParameterKind::Named {
            continue;
        }
        let Some(element) = ctx
            .declared_element(parameter.raw())
            .and_then(|e| e.cast::<FormalParameterElement>())
        else {
            continue;
        };
        let Some(super_constructor_parameter) =
            dartr_link::outline::super_constructor_parameter(&resolved.ctx, element)
        else {
            continue;
        };
        let Some(expected) = resolved.ctx.element_name(super_constructor_parameter.raw()) else {
            continue;
        };
        let actual = ctx.ast.tokens.lexeme(ctx.ast[super_parameter].name);
        if actual != expected {
            ctx.report_node(
                out,
                &diag::MATCHING_SUPER_PARAMETERS,
                *parameter,
                &[actual, expected],
            );
        }
    }
}
