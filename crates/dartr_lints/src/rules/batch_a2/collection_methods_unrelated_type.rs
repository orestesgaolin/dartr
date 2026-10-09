// Dart source: pkg/linter/lib/src/rules/collection_methods_unrelated_type.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ClassElement, EId, EnumElement, InterfaceElement, TypeId, TypeKind};
use dartr_typesystem::TypeExt;
use indexmap::IndexSet;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::IndexExpression,
        "collection_methods_unrelated_type",
        check,
    );
    registry.add(
        NodeKind::MethodInvocation,
        "collection_methods_unrelated_type",
        check,
    );
}

fn types_are_unrelated(context: &LinterContext<'_>, left: TypeId, right: TypeId) -> bool {
    types_are_unrelated_inner(context, left, right, &mut IndexSet::new())
}

fn types_are_unrelated_inner(
    context: &LinterContext<'_>,
    left: TypeId,
    right: TypeId,
    visited: &mut IndexSet<(TypeId, TypeId)>,
) -> bool {
    if !visited.insert((left, right)) {
        return false;
    }
    let Some(resolved) = context.resolved else {
        return false;
    };
    let Some(ts) = context.type_system() else {
        return false;
    };
    let left = ts.extension_type_erasure(left);
    let right = ts.extension_type_erasure(right);
    if resolved.ctx.is_bottom(left)
        || resolved.ctx.is_bottom(right)
        || matches!(resolved.ctx.ty(left), TypeKind::Dynamic)
        || matches!(resolved.ctx.ty(right), TypeKind::Dynamic)
    {
        return false;
    }
    if (resolved.ctx.is_dart_core_null(left) && ts.is_non_nullable(right))
        || (resolved.ctx.is_dart_core_null(right) && ts.is_non_nullable(left))
    {
        return true;
    }
    let left = ts.promote_to_non_null(left);
    let right = ts.promote_to_non_null(right);
    if ts.dart_eq(left, right) || ts.is_subtype_of(left, right) || ts.is_subtype_of(right, left) {
        return false;
    }
    match (*resolved.ctx.ty(left), *resolved.ctx.ty(right)) {
        (TypeKind::Interface { element: le, .. }, TypeKind::Interface { element: re, .. }) => {
            if le == re {
                let la = resolved.ctx.type_arguments(left);
                let ra = resolved.ctx.type_arguments(right);
                la.len() == ra.len()
                    && la
                        .iter()
                        .zip(ra)
                        .any(|(&l, &r)| types_are_unrelated_inner(context, l, r, visited))
            } else {
                let ls = resolved.ctx.interface(le).supertype.get();
                let rs = resolved.ctx.interface(re).supertype.get();
                match (ls, rs) {
                    (None, _) => true,
                    (Some(_), None) => true,
                    (Some(l), Some(r)) if !ts.dart_eq(l, r) => true,
                    (Some(l), Some(_)) => {
                        le.raw().is::<EnumElement>()
                            || is_protobuf_enum(context, l)
                            || resolved.ctx.is_dart_core_object(l)
                    }
                }
            }
        }
        (
            TypeKind::TypeParameter { param: left, .. },
            TypeKind::TypeParameter { param: right, .. },
        ) => {
            let (Some(left), Some(right)) = (
                resolved.ctx.get(left).bound.get(),
                resolved.ctx.get(right).bound.get(),
            ) else {
                return false;
            };
            types_are_unrelated_inner(context, left, right, visited)
        }
        (TypeKind::Function(_), TypeKind::Function(_)) => false,
        (TypeKind::Function(_), _) => unrelated_to_function(context, right),
        (_, TypeKind::Function(_)) => unrelated_to_function(context, left),
        (TypeKind::Record { .. }, _) | (_, TypeKind::Record { .. }) => {
            !ts.is_assignable_to(left, right, false) && !ts.is_assignable_to(right, left, false)
        }
        _ => false,
    }
}

fn is_protobuf_enum(context: &LinterContext<'_>, ty: TypeId) -> bool {
    let Some(resolved) = context.resolved else {
        return false;
    };
    resolved.ctx.interface_element(ty).is_some_and(|element| {
        resolved.ctx.element_name(element.raw()) == Some("ProtobufEnum")
            && resolved.ctx.element_library_uri(element.raw())
                == Some("package:protobuf/src/protobuf/protobuf_enum.dart")
    })
}

fn unrelated_to_function(context: &LinterContext<'_>, ty: TypeId) -> bool {
    let Some(resolved) = context.resolved else {
        return false;
    };
    match *resolved.ctx.ty(ty) {
        TypeKind::Function(_) => false,
        TypeKind::Interface { element, .. } => {
            element.raw().cast::<ClassElement>().is_none()
                || context
                    .type_system()
                    .is_none_or(|ts| ts.get_call_method_type(ty).is_none())
        }
        _ => true,
    }
}

fn report_if_unrelated(
    context: &LinterContext<'_>,
    argument: NodeId,
    expected: TypeId,
    out: &mut Vec<Diagnostic>,
) {
    let Some(actual) = context.static_type(argument) else {
        return;
    };
    if types_are_unrelated(context, actual, expected) {
        let actual_text = super::helpers::type_text(context, actual);
        let expected_text = super::helpers::type_text(context, expected);
        context.report_node(
            out,
            &diag::COLLECTION_METHODS_UNRELATED_TYPE,
            argument,
            &[&actual_text, &expected_text],
        );
    }
}

fn as_collection(
    context: &LinterContext<'_>,
    target: TypeId,
    element: EId<InterfaceElement>,
) -> Option<TypeId> {
    context.resolved?.ctx.as_instance_of(target, element)
}

fn implicit_target_type(context: &LinterContext<'_>, node: NodeId) -> Option<TypeId> {
    let resolved = context.resolved?;
    for parent in super::helpers::ancestors(context.ast, node).skip(1) {
        match context.ast.kind(parent) {
            NodeKind::ClassDeclaration | NodeKind::MixinDeclaration | NodeKind::EnumDeclaration => {
                let element = context
                    .declared_element(parent)?
                    .cast::<InterfaceElement>()?;
                return Some(resolved.ctx.interface_this_type(element));
            }
            NodeKind::ExtensionDeclaration => {
                let extension =
                    &context.ast[context.ast.cast::<ExtensionDeclaration>(parent).unwrap()];
                let clause = extension.on_clause?;
                return super::helpers::annotation_type(
                    context,
                    context.ast[clause].extended_type.raw(),
                );
            }
            _ => {}
        }
    }
    None
}

fn queue_type(context: &LinterContext<'_>, target: TypeId) -> Option<TypeId> {
    let resolved = context.resolved?;
    resolved.ctx.interface_element(target)?;
    std::iter::once(target)
        .chain(resolved.ctx.all_supertypes(target))
        .find_map(|ty| {
            let element = resolved.ctx.interface_element(ty)?;
            (resolved.ctx.element_name(element.raw()) == Some("Queue")
                && resolved.ctx.element_library_uri(element.raw()) == Some("dart:collection"))
            .then_some(ty)
        })
}

fn check(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(resolved) = context.resolved else {
        return;
    };
    let (target_node, argument, method) = match context.ast.kind(node) {
        NodeKind::IndexExpression => {
            let n = &context.ast[context.ast.cast::<IndexExpression>(node).unwrap()];
            (n.target.map(Id::raw), n.index.raw(), "[]")
        }
        NodeKind::MethodInvocation => {
            let n = &context.ast[context.ast.cast::<MethodInvocation>(node).unwrap()];
            let args = context.ast.list(context.ast[n.argument_list].arguments);
            if args.len() != 1 {
                return;
            }
            let arg = match context.ast.kind(args[0]) {
                NodeKind::NamedArgument => context.ast
                    [context.ast.cast::<NamedArgument>(args[0]).unwrap()]
                .argument_expression
                .raw(),
                _ => args[0].raw(),
            };
            (
                n.target.map(Id::raw),
                arg,
                context.ast.tokens.lexeme(context.ast[n.method_name].token),
            )
        }
        _ => return,
    };
    let Some(target_type) = target_node
        .and_then(|target| context.static_type(target))
        .or_else(|| implicit_target_type(context, node))
    else {
        return;
    };
    let definition = match method {
        "[]" | "containsKey" => Some((resolved.ctx.tp.map_element().upcast(), 0)),
        "containsValue" => Some((resolved.ctx.tp.map_element().upcast(), 1)),
        "contains" => Some((resolved.ctx.tp.iterable_element().upcast(), 0)),
        "lookup" => Some((resolved.ctx.tp.set_element().upcast(), 0)),
        "remove" => {
            for element in [
                resolved.ctx.tp.list_element().upcast(),
                resolved.ctx.tp.map_element().upcast(),
                resolved.ctx.tp.set_element().upcast(),
            ] {
                if let Some(collection) = as_collection(context, target_type, element) {
                    let expected = resolved.ctx.type_arguments(collection)[0];
                    report_if_unrelated(context, argument, expected, out);
                    return;
                }
            }
            if let Some(collection) = queue_type(context, target_type) {
                let expected = resolved.ctx.type_arguments(collection)[0];
                report_if_unrelated(context, argument, expected, out);
                return;
            }
            None
        }
        _ => None,
    };
    if let Some((element, index)) = definition
        && let Some(collection) = as_collection(context, target_type, element)
        && let Some(&expected) = resolved.ctx.type_arguments(collection).get(index)
    {
        report_if_unrelated(context, argument, expected, out);
    }
}
