// Dart source: pkg/linter/lib/src/rules/avoid_classes_with_only_static_members.dart

use super::helpers::class_name_token;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{ClassDeclaration, Id, NodeId, NodeKind};
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::ClassElement;
use dartr_typesystem::{TypeExt, inheritance_manager3::InheritanceManager3, member};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::ClassDeclaration,
        "avoid_classes_with_only_static_members",
        check,
    );
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let class = Id::<ClassDeclaration>::from_raw(node);
    let n = &c.ast[class];
    if n.augment_keyword.is_some() || n.sealed_keyword.is_some() {
        return;
    }
    if c.ast.kind(n.name_part) == NodeKind::PrimaryConstructorDeclaration {
        return;
    }
    let (Some(r), Some(class_element)) = (
        c.resolved,
        c.declared_element(node)
            .and_then(|e| e.cast::<ClassElement>()),
    ) else {
        return;
    };
    let interface = InheritanceManager3::new(r.ctx).get_interface(class_element.upcast());
    for inherited in interface.map.values() {
        let Some(enclosing) = member::enclosing_element(&r.ctx, *inherited) else {
            continue;
        };
        if enclosing.is::<ClassElement>()
            && !(r.ctx.element_name(enclosing) == Some("Object")
                && member::library(&r.ctx, *inherited)
                    .is_some_and(|library| r.ctx.library_uri(library) == "dart:core"))
        {
            return;
        }
    }
    let data = r.ctx.get(class_element);
    if data
        .constructors
        .iter()
        .any(|constructor| constructor_is_declared(c, constructor.raw()))
        || data
            .methods
            .iter()
            .any(|method| !dartr_link::dump::is_static(&r.ctx, method.raw()))
    {
        return;
    }
    let interesting = !data.methods.is_empty()
        || data
            .fields
            .iter()
            .any(|field| !dartr_link::dump::is_const(&r.ctx, field.raw()));
    if interesting && let Some(name) = class_name_token(c.ast, class) {
        c.report_token(
            out,
            &diag::AVOID_CLASSES_WITH_ONLY_STATIC_MEMBERS,
            name,
            &[],
        );
    }
}

fn constructor_is_declared(c: &LinterContext<'_>, constructor: dartr_element::ElementId) -> bool {
    c.resolved_units.iter().enumerate().any(|(index, _)| {
        let Some(unit) = c.resolved_unit(index) else {
            return false;
        };
        (0..unit.ast.node_count())
            .map(NodeId::from_index)
            .filter(|node| {
                matches!(
                    unit.ast.kind(*node),
                    NodeKind::ConstructorDeclaration | NodeKind::PrimaryConstructorDeclaration
                )
            })
            .any(|node| unit.declared_element(node) == Some(constructor))
    })
}
