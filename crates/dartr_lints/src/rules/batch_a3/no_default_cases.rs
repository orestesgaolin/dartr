// Dart source: pkg/linter/lib/src/rules/no_default_cases.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ClassElement, ElemRef, FragmentFlags, TypeKind};
use dartr_typesystem::{TypeExt, member};
pub fn register(registry: &mut RuleVisitorRegistry) {
    registry.add_switch_statement("no_default_cases", check);
}

fn is_enum_like(ctx: &LinterContext<'_>, class: dartr_element::EId<ClassElement>) -> bool {
    let Some(resolved) = ctx.resolved else {
        return false;
    };
    if dartr_link::dump::is_abstract(&resolved.ctx, class.raw()) {
        return false;
    }
    for &constructor in &resolved.ctx.get(class).constructors {
        let name = resolved.ctx.element_name(constructor.raw()).unwrap_or("");
        if !name.starts_with('_') || dartr_link::dump::is_factory(&resolved.ctx, constructor.raw())
        {
            return false;
        }
    }

    let class_type = resolved.ctx.interface_this_type(class.upcast());
    let mut constants = 0usize;
    for &field in &resolved.ctx.get(class).fields {
        let origin_getter_setter = resolved
            .ctx
            .element_data(field.raw())
            .and_then(|data| resolved.ctx.fragment_data(data.first_fragment))
            .is_some_and(|fragment| {
                fragment
                    .flags
                    .has(FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_GETTER_SETTER)
            });
        if origin_getter_setter
            || !dartr_link::dump::is_static(&resolved.ctx, field.raw())
            || !dartr_link::dump::is_const(&resolved.ctx, field.raw())
            || member::type_(&resolved.ctx, ElemRef::Base(field.raw())) != class_type
        {
            continue;
        }
        let has_value = (0..ctx.resolved_units.len()).any(|unit_index| {
            let Some(unit) = ctx.resolved_unit(unit_index) else {
                return false;
            };
            let initializer = (0..unit.ast.node_count())
                .map(NodeId::from_index)
                .find(|&node| unit.declared_element(node) == Some(field.raw()))
                .and_then(|node| unit.ast.cast::<VariableDeclaration>(node))
                .and_then(|variable| unit.ast[variable].initializer);
            initializer
                .and_then(|initializer| unit.constant_value(initializer.raw()))
                .is_some()
        });
        if has_value {
            constants += 1;
        }
    }
    if constants < 2 {
        return false;
    }

    let Some(library) = resolved
        .ctx
        .element_data(class.raw())
        .and_then(|data| data.library)
    else {
        return false;
    };
    !resolved.ctx.get(library).classes.iter().any(|&candidate| {
        let mut supertype = resolved.ctx.element_supertype(candidate.upcast());
        while let Some(ty) = supertype {
            let Some(element) = resolved.ctx.interface_element(ty) else {
                break;
            };
            if element == class.upcast() {
                return true;
            }
            supertype = resolved.ctx.element_supertype(element);
        }
        false
    })
}

fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &ctx.ast[Id::<SwitchStatement>::from_raw(node)];
    let Some(r) = ctx.resolved else {
        return;
    };
    let Some(t) = ctx.static_type(n.expression) else {
        return;
    };
    let TypeKind::Interface { element, .. } = *r.ctx.ty(t) else {
        return;
    };
    if element.raw().kind() != dartr_element::ElementKind::Enum
        && !element
            .cast::<ClassElement>()
            .is_some_and(|class| is_enum_like(ctx, class))
    {
        return;
    }
    if let Some(default) = ctx
        .ast
        .list(n.members)
        .iter()
        .find(|m| ctx.ast.kind(**m) == NodeKind::SwitchDefault)
    {
        ctx.report_node(out, &diag::NO_DEFAULT_CASES, *default, &[]);
    }
}
