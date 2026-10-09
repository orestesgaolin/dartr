// Dart source: pkg/linter/lib/src/rules/null_closures.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_typesystem::{TypeExt, member};

pub fn register(r: &mut RuleVisitorRegistry) {
    r.add_instance_creation_expression("null_closures", check);
    r.add_method_invocation("null_closures", check);
}

fn type_is(ctx: &LinterContext<'_>, ty: dartr_element::TypeId, lib: &str, name: &str) -> bool {
    let r = ctx.resolved.unwrap();
    if r.ctx.interface_element(ty).is_none() {
        return false;
    }
    std::iter::once(ty)
        .chain(r.ctx.all_supertypes(ty))
        .any(|t| {
            r.ctx
                .interface_element(t)
                .is_some_and(|e| r.ctx.is_element(e.raw(), lib, name))
        })
}

fn extends_class(
    ctx: &LinterContext<'_>,
    ty: dartr_element::TypeId,
    lib: &str,
    name: &str,
) -> bool {
    let resolved = ctx.resolved.unwrap();
    let mut ty = ctx.type_system().unwrap().extension_type_erasure(ty);
    loop {
        let Some(element) = resolved.ctx.interface_element(ty) else {
            return false;
        };
        if resolved.ctx.is_element(element.raw(), lib, name) {
            return true;
        }
        let Some(supertype) = resolved.ctx.element_supertype(element) else {
            return false;
        };
        ty = supertype;
    }
}
fn report_args(
    ctx: &LinterContext<'_>,
    list: Id<ArgumentList>,
    positions: &[usize],
    names: &[&str],
    out: &mut Vec<Diagnostic>,
) {
    for (i, arg) in ctx.ast.list(ctx.ast[list].arguments).iter().enumerate() {
        let (raw, name) = if let Some(n) = ctx.ast.cast::<NamedArgument>(*arg) {
            (
                ctx.ast[n].argument_expression.raw(),
                Some(ctx.ast.tokens.lexeme(ctx.ast[n].name)),
            )
        } else {
            (arg.raw(), None)
        };
        if ctx.ast.kind(raw) == NodeKind::NullLiteral
            && (name.is_some_and(|n| names.contains(&n))
                || name.is_none() && positions.contains(&i))
        {
            ctx.report_node(out, &diag::NULL_CLOSURES, *arg, &[]);
        }
    }
}
fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    match ctx.ast.kind(node) {
        NodeKind::InstanceCreationExpression => {
            let n = &ctx.ast[Id::<InstanceCreationExpression>::from_raw(node)];
            let Some(ty) = ctx.static_type(node) else {
                return;
            };
            let cname = ctx.ast[n.constructor_name]
                .name
                .map(|id| ctx.ast.tokens.lexeme(ctx.ast[id].token));
            for (lib, class, name, pos) in [
                ("dart.async", "Future", None, &[0][..]),
                ("dart.async", "Future", Some("microtask"), &[0][..]),
                ("dart.async", "Future", Some("sync"), &[0][..]),
                ("dart.async", "Timer", None, &[1][..]),
                ("dart.async", "Timer", Some("periodic"), &[1][..]),
                ("dart.core", "List", Some("generate"), &[1][..]),
            ] {
                if cname == name && extends_class(ctx, ty, lib, class) {
                    report_args(ctx, n.argument_list, pos, &[], out);
                }
            }
        }
        NodeKind::MethodInvocation => {
            let n = &ctx.ast[Id::<MethodInvocation>::from_raw(node)];
            let method = ctx.ast.tokens.lexeme(ctx.ast[n.method_name].token);
            let specs: &[(&str, &str, &[usize], &[&str])] = match method {
                "any" | "every" | "expand" | "map" | "reduce" | "skipWhile" | "takeWhile"
                | "where" => &[("dart.core", "Iterable", &[0], &[])],
                "complete" => &[("dart.async", "Future", &[0], &[])],
                "firstWhere" | "lastWhere" | "singleWhere" => {
                    &[("dart.core", "Iterable", &[0], &["orElse"])]
                }
                "forEach" => &[
                    ("dart.core", "Iterable", &[0], &[]),
                    ("dart.core", "Map", &[0], &[]),
                ],
                "fold" => &[("dart.core", "Iterable", &[1], &[])],
                "putIfAbsent" => &[("dart.core", "Map", &[1], &[])],
                "removeWhere" | "retainWhere" => &[
                    ("dart.collection", "Queue", &[0], &[]),
                    ("dart.core", "List", &[0], &[]),
                    ("dart.core", "Set", &[0], &[]),
                ],
                "replaceAllMapped" | "replaceFirstMapped" => &[("dart.core", "String", &[1], &[])],
                "splitMapJoin" => &[("dart.core", "String", &[], &["onMatch", "onNonMatch"])],
                "then" => &[("dart.async", "Future", &[0], &["onError"])],
                _ => &[],
            };
            if let Some(target) = n.target {
                if let Some(e) = ctx.element(target) {
                    let r = ctx.resolved.unwrap();
                    let base = member::base_element(&r.ctx, e);
                    if r.ctx.is_element(base, "dart.async", "Timer") && method == "run" {
                        report_args(ctx, n.argument_list, &[0], &[], out);
                        return;
                    } else if r.ctx.is_element(base, "dart.async", "Future") {
                        match method {
                            "doWhile" => report_args(ctx, n.argument_list, &[0], &[], out),
                            "forEach" => report_args(ctx, n.argument_list, &[1], &[], out),
                            "wait" => report_args(ctx, n.argument_list, &[], &["cleanUp"], out),
                            _ => {}
                        }
                        return;
                    }
                }
                if let Some(ty) = ctx.static_type(target) {
                    for (lib, class, pos, names) in specs {
                        if type_is(ctx, ty, lib, class) {
                            report_args(ctx, n.argument_list, pos, names, out);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}
