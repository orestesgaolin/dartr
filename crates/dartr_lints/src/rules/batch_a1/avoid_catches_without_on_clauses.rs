// Dart source: pkg/linter/lib/src/rules/avoid_catches_without_on_clauses.dart

use super::helpers::descendants;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{CatchClause, Id, InstanceCreationExpression, MethodInvocation, NodeId, NodeKind};
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{AnyElement, EId, InterfaceElement, TypeKind};
use dartr_typesystem::TypeExt;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::CatchClause,
        "avoid_catches_without_on_clauses",
        check,
    );
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<CatchClause>::from_raw(node)];
    if n.on_keyword.is_some() {
        return;
    }
    let Some(parameter) = n.exception_parameter.and_then(|p| c.declared_element(p)) else {
        return;
    };
    let mut valid = false;
    let mut can_rethrow = true;
    let uses_parameter = |root: NodeId| {
        descendants(c.ast, root)
            .into_iter()
            .chain(std::iter::once(root))
            .any(|used| {
                c.element(used)
                    .is_some_and(|e| super::helpers::base_element(c, e) == Some(parameter))
            })
    };
    for child in descendants(c.ast, n.body) {
        if c.ast.kind(child) == NodeKind::CatchClause {
            // The upstream recursive visitor disables rethrow handling after
            // it enters a nested catch clause and does not restore the flag.
            can_rethrow = false;
        }
        if c.ast.kind(child) == NodeKind::RethrowExpression && can_rethrow {
            valid = true;
            break;
        }
        if c.ast.kind(child) == NodeKind::ThrowExpression && uses_parameter(child) {
            valid = true;
            break;
        }
        let invocation_arguments = match c.ast.kind(child) {
            NodeKind::FunctionExpressionInvocation => Some(
                c.ast[Id::<dartr_ast::FunctionExpressionInvocation>::from_raw(child)]
                    .argument_list
                    .raw(),
            ),
            NodeKind::MethodInvocation => Some(
                c.ast[Id::<MethodInvocation>::from_raw(child)]
                    .argument_list
                    .raw(),
            ),
            _ => None,
        };
        if invocation_arguments.is_some()
            && c.static_type(child).is_some_and(|ty| {
                c.resolved
                    .is_some_and(|r| matches!(r.ctx.ty(ty), TypeKind::Never(_)))
            })
            && invocation_arguments.is_some_and(uses_parameter)
        {
            valid = true;
            break;
        }
        if let Some(invocation) = c.ast.cast::<MethodInvocation>(child) {
            let invocation = &c.ast[invocation];
            let name = c.ast.tokens.lexeme(c.ast[invocation.method_name].token);
            let real_target = invocation.target.or_else(|| {
                super::helpers::ancestor(c.ast, child, NodeKind::CascadeExpression).map(|cascade| {
                    c.ast[Id::<dartr_ast::CascadeExpression>::from_raw(cascade)].target
                })
            });
            let is_flutter_report = name == "reportError"
                && real_target.is_some_and(|target| {
                    let (Some(r), Some(element)) = (
                        c.resolved,
                        c.element(target)
                            .and_then(|element| super::helpers::base_element(c, element)),
                    ) else {
                        return false;
                    };
                    matches!(r.ctx.any(element), AnyElement::Class(_))
                        && r.ctx.element_name(element) == Some("FlutterError")
                });
            let is_completer = name == "completeError"
                && real_target.is_some_and(|target| {
                    let (Some(r), Some(ty)) = (c.resolved, c.static_type(target)) else {
                        return false;
                    };
                    r.ctx.library_by_uri("dart:async").is_some_and(|library| {
                        r.ctx.get(library).classes.iter().copied().any(|class| {
                            r.ctx.element_name(class.raw()) == Some("Completer")
                                && extends_class(c, ty, class.upcast())
                        })
                    })
                });
            if (is_flutter_report || is_completer) && uses_parameter(invocation.argument_list.raw())
            {
                valid = true;
                break;
            }
        }
        if let Some(creation) = c.ast.cast::<InstanceCreationExpression>(child) {
            let creation = &c.ast[creation];
            let constructor = &c.ast[creation.constructor_name];
            let is_future_error = constructor.name.is_some_and(|name| {
                c.ast.tokens.lexeme(c.ast[name].token) == "error"
                    && c.static_type(child).is_some_and(|ty| {
                        c.resolved.is_some_and(|r| r.ctx.is_dart_async_future(ty))
                    })
            });
            if is_future_error && uses_parameter(creation.argument_list.raw()) {
                valid = true;
                break;
            }
        }
    }
    if !valid && let Some(catch) = n.catch_keyword {
        c.report_token(out, &diag::AVOID_CATCHES_WITHOUT_ON_CLAUSES, catch, &[]);
    }
}

fn extends_class(
    c: &LinterContext<'_>,
    ty: dartr_element::TypeId,
    base: EId<InterfaceElement>,
) -> bool {
    let Some(r) = c.resolved else { return false };
    let Some(mut current) = r.ctx.interface_element(ty) else {
        return false;
    };
    let mut visited = indexmap::IndexSet::new();
    loop {
        if current == base {
            return true;
        }
        if !visited.insert(current) {
            return false;
        }
        let Some(supertype) = r.ctx.interface(current).supertype.get() else {
            return false;
        };
        let Some(next) = r.ctx.interface_element(supertype) else {
            return false;
        };
        current = next;
    }
}
