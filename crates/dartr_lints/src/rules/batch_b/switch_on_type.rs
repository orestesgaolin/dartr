// Dart source: pkg/linter/lib/src/rules/switch_on_type.dart
use super::util::*;
use crate::{ExperimentalFlag, LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{TypeId, TypeKind};

pub fn register(r: &mut RuleVisitorRegistry, c: &LinterContext<'_>) {
    if !c.is_feature_enabled(ExperimentalFlag::Patterns) {
        return;
    }
    r.add(NodeKind::SwitchExpression, "switch_on_type", check);
    r.add(NodeKind::SwitchStatement, "switch_on_type", check);
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let expression = match kind(c, node) {
        NodeKind::SwitchExpression => c.ast[Id::<SwitchExpression>::from_raw(node)].expression.raw(),
        _ => c.ast[Id::<SwitchStatement>::from_raw(node)].expression.raw(),
    };
    process_expression(c, expression, expression, out);
}

/// Dart `_isAssignableToType`.
fn is_assignable_to_type(c: &LinterContext<'_>, ty: Option<TypeId>) -> bool {
    let (Some(ty), Some(ctx), Some(ts)) = (ty, rctx(c), c.type_system()) else {
        return false;
    };
    if matches!(*ctx.ty(ty), TypeKind::Dynamic) {
        return false;
    }
    ts.is_assignable_to(ty, ctx.tp.type_type(), false)
}

/// Dart `_processExpression`.
fn process_expression(c: &LinterContext<'_>, expression: NodeId, error_node: NodeId, out: &mut Vec<Diagnostic>) -> bool {
    if let Some(interpolation) = c.ast.cast::<StringInterpolation>(expression) {
        for &element in c.ast.list_raw(c.ast[interpolation].elements) {
            if let Some(e) = c.ast.cast::<InterpolationExpression>(element)
                && process_expression(c, c.ast[e].expression.raw(), error_node, out)
            {
                return true;
            }
        }
        return false;
    }
    if let Some(conditional) = c.ast.cast::<ConditionalExpression>(expression) {
        let n = &c.ast[conditional];
        return process_expression(c, n.then_expression.raw(), error_node, out)
            || process_expression(c, n.else_expression.raw(), error_node, out);
    }
    if let Some(switch) = c.ast.cast::<SwitchExpression>(expression) {
        for &case in c.ast.list(c.ast[switch].cases) {
            if process_expression(c, c.ast[case].expression.raw(), error_node, out) {
                return true;
            }
        }
        return false;
    }
    if let Some(binary) = c.ast.cast::<BinaryExpression>(expression)
        && lexeme(c, c.ast[binary].operator) == "+"
    {
        let b = &c.ast[binary];
        return process_expression(c, b.left_operand.raw(), error_node, out)
            || process_expression(c, b.right_operand.raw(), error_node, out);
    }
    let ty = match kind(c, expression) {
        NodeKind::PrefixedIdentifier => {
            c.static_type(c.ast[Id::<PrefixedIdentifier>::from_raw(expression)].identifier)
        }
        NodeKind::PropertyAccess => {
            c.static_type(c.ast[Id::<PropertyAccess>::from_raw(expression)].property_name)
        }
        NodeKind::SimpleIdentifier | NodeKind::TypeLiteral => c.static_type(expression),
        NodeKind::MethodInvocation => {
            let n = &c.ast[Id::<MethodInvocation>::from_raw(expression)];
            match real_target(c, expression) {
                Some(target) => {
                    let is_to_string = c.element(n.method_name).is_some_and(|e| {
                        let e = base(c, e);
                        e.tag() == dartr_element::Tag::Method && name(c, e) == Some("toString")
                    });
                    if is_to_string { c.static_type(target) } else { None }
                }
                None => None,
            }
        }
        _ => None,
    };
    if is_assignable_to_type(c, ty) {
        c.report_node(out, &diag::SWITCH_ON_TYPE, error_node, &[]);
        return true;
    }
    false
}
