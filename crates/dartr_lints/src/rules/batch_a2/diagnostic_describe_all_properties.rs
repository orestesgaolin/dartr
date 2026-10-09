// Dart source: pkg/linter/lib/src/rules/diagnostic_describe_all_properties.dart

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{diag, Diagnostic};
use dartr_element::{ElemRef, InterfaceElement, TypeId, TypeKind};
use dartr_typesystem::{member, TypeExt};
use indexmap::IndexMap;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::ClassDeclaration,
        "diagnostic_describe_all_properties",
        check,
    );
}
fn widget_property(context: &LinterContext<'_>, ty: TypeId) -> bool {
    if super::helpers::implements(
        context,
        ty,
        "package:flutter/src/widgets/framework.dart",
        "Widget",
    ) {
        return true;
    }
    let Some(resolved) = context.resolved else {
        return false;
    };
    if matches!(resolved.ctx.ty(ty), TypeKind::Interface { .. })
        && [
            resolved.ctx.tp.list_element().upcast::<InterfaceElement>(),
            resolved.ctx.tp.set_element().upcast(),
            resolved.ctx.tp.map_element().upcast(),
        ]
        .into_iter()
        .any(|e| resolved.ctx.as_instance_of(ty, e).is_some())
    {
        let args = resolved.ctx.type_arguments(ty);
        return args.len() == 1 && widget_property(context, args[0]);
    }
    false
}
fn overrides(context: &LinterContext<'_>, class_type: TypeId, name: &str) -> bool {
    let Some(resolved) = context.resolved else {
        return false;
    };
    let Some(element) = resolved.ctx.interface_element(class_type) else {
        return false;
    };
    dartr_typesystem::inheritance_manager3::InheritanceManager3::new(resolved.ctx)
        .get_overridden(
            element,
            dartr_typesystem::inheritance_manager3::Name::for_library(
                &resolved.ctx,
                resolved
                    .ctx
                    .element_data(element.raw())
                    .and_then(|e| e.library),
                name,
            ),
        )
        .is_some()
}
fn check(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(resolved) = context.resolved else {
        return;
    };
    let Some(class) = context
        .declared_element(node)
        .and_then(|e| e.cast::<dartr_element::ClassElement>())
    else {
        return;
    };
    let class_type = resolved.ctx.interface_this_type(class.upcast());
    if !super::helpers::implements(context, class_type, "", "Diagnosticable") {
        return;
    }
    let declaration = &context.ast[context.ast.cast::<ClassDeclaration>(node).unwrap()];
    let Some(body) = context.ast.cast::<BlockClassBody>(declaration.body) else {
        return;
    };
    let mut properties: IndexMap<String, dartr_syntax::TokenId> = IndexMap::new();
    let mut debug_bodies = vec![];
    for &member_node in context.ast.list(context.ast[body].members) {
        if let Some(method) = context.ast.cast::<MethodDeclaration>(member_node) {
            let method = &context.ast[method];
            let name = context.ast.tokens.lexeme(method.name);
            if matches!(name, "debugFillProperties" | "debugDescribeChildren") {
                debug_bodies.push(method.body.raw());
                continue;
            }
            if method.modifier_keyword.is_some()
                || method.property_keyword.is_none()
                || name.starts_with('_')
                || overrides(context, class_type, name)
            {
                continue;
            }
            let Some(element) = context.declared_element(member_node) else {
                continue;
            };
            let ty = member::return_type(&resolved.ctx, ElemRef::Base(element));
            if !widget_property(context, ty) {
                properties.insert(name.to_string(), method.name);
            }
        } else if let Some(field) = context.ast.cast::<FieldDeclaration>(member_node) {
            let field = &context.ast[field];
            if field.static_keyword.is_some() {
                continue;
            }
            for &variable in context.ast.list(context.ast[field.fields].variables) {
                let name_token = context.ast[variable].name;
                let name = context.ast.tokens.lexeme(name_token);
                if name.starts_with('_') || overrides(context, class_type, name) {
                    continue;
                }
                let Some(element) = context.declared_element(variable) else {
                    continue;
                };
                let ty = member::type_(&resolved.ctx, ElemRef::Base(element));
                if !widget_property(context, ty) {
                    properties.insert(name.to_string(), name_token);
                }
            }
        }
    }
    for root in debug_bodies {
        let mut pending = vec![root];
        while let Some(current) = pending.pop() {
            if let Some(identifier) = context.ast.cast::<SimpleIdentifier>(current) {
                let name = context.ast.tokens.lexeme(context.ast[identifier].token);
                properties.shift_remove(name);
                if let Some(rest) = name.strip_prefix("debug") {
                    let mut chars = rest.chars();
                    if let Some(first) = chars.next() {
                        properties.shift_remove(&format!(
                            "{}{}",
                            first.to_lowercase(),
                            chars.as_str()
                        ));
                    }
                }
            }
            pending.extend(context.ast.children(current));
        }
    }
    for (_, token) in properties {
        context.report_token(out, &diag::DIAGNOSTIC_DESCRIBE_ALL_PROPERTIES, token, &[]);
    }
}
