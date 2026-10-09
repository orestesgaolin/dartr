// Dart source: pkg/linter/lib/src/rules/implicit_reopen.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ClassElement, ElementFlags, FragmentFlags, TypeKind};
use dartr_typesystem::{TypeExt, member};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(NodeKind::ClassDeclaration, "implicit_reopen", check);
    registry.add(NodeKind::ClassTypeAlias, "implicit_reopen", check);
}
fn check(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let (name_token, metadata) = match context.ast.kind(node) {
        NodeKind::ClassDeclaration => {
            let n = &context.ast[context.ast.cast::<ClassDeclaration>(node).unwrap()];
            let name = match context.ast.kind(n.name_part) {
                NodeKind::NameWithTypeParameters => {
                    context.ast[context
                        .ast
                        .cast::<NameWithTypeParameters>(n.name_part)
                        .unwrap()]
                    .type_name
                }
                NodeKind::PrimaryConstructorDeclaration => {
                    context.ast[context
                        .ast
                        .cast::<PrimaryConstructorDeclaration>(n.name_part)
                        .unwrap()]
                    .type_name
                }
                _ => return,
            };
            (name, n.metadata)
        }
        NodeKind::ClassTypeAlias => {
            let n = &context.ast[context.ast.cast::<ClassTypeAlias>(node).unwrap()];
            (n.name, n.metadata)
        }
        _ => return,
    };
    if has_reopen(context, metadata) {
        return;
    }
    let Some(resolved) = context.resolved else {
        return;
    };
    let Some(class) = context
        .declared_element(node)
        .and_then(|e| e.cast::<ClassElement>())
    else {
        return;
    };
    let data = resolved.ctx.get(class);
    let Some(fragment) = resolved.ctx.fragment_data(data.first_fragment) else {
        return;
    };
    if fragment.flags.has(FragmentFlags::CLASS_FRAGMENT_IS_SEALED)
        || fragment
            .flags
            .has(FragmentFlags::CLASS_FRAGMENT_IS_MIXIN_CLASS)
    {
        return;
    }
    let Some(TypeKind::Interface {
        element: super_element,
        ..
    }) = data.supertype.get().map(|t| *resolved.ctx.ty(t))
    else {
        return;
    };
    let Some(super_class) = super_element.raw().cast::<ClassElement>() else {
        return;
    };
    let super_data = resolved.ctx.get(super_class);
    if data.library != super_data.library {
        return;
    }
    let target_base = data.flags.has(ElementFlags::CLASS_ELEMENT_IS_BASE);
    let target_plain = !target_base
        && !data.flags.has(ElementFlags::CLASS_ELEMENT_IS_INTERFACE)
        && !data.flags.has(ElementFlags::CLASS_ELEMENT_IS_FINAL)
        && !fragment.flags.has(FragmentFlags::CLASS_FRAGMENT_IS_SEALED);
    let reason = if target_base && super_data.flags.has(ElementFlags::CLASS_ELEMENT_IS_FINAL) {
        Some("final")
    } else if (target_base || target_plain)
        && super_data
            .flags
            .has(ElementFlags::CLASS_ELEMENT_IS_INTERFACE)
    {
        Some("interface")
    } else {
        None
    };
    let Some(reason) = reason else {
        return;
    };
    let Some(target_name) = data.name.map(|n| resolved.ctx.name_str(n)) else {
        return;
    };
    let Some(super_name) = super_data.name.map(|n| resolved.ctx.name_str(n)) else {
        return;
    };
    context.report_token(
        out,
        &diag::IMPLICIT_REOPEN,
        name_token,
        &["class", target_name, super_name, reason],
    );
}

fn has_reopen(context: &LinterContext<'_>, metadata: NodeList<Annotation>) -> bool {
    let Some(resolved) = context.resolved else {
        return false;
    };
    context.ast.list(metadata).iter().any(|&annotation| {
        context.element(annotation).is_some_and(|element| {
            let base = member::base_element(&resolved.ctx, element);
            resolved.ctx.element_name(base) == Some("reopen")
                && resolved.ctx.element_library_uri(base) == Some("package:meta/meta.dart")
        })
    })
}
