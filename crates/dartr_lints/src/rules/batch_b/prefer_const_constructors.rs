// Dart source: pkg/linter/lib/src/rules/prefer_const_constructors.dart
use super::util::*;
use crate::rules::batch_a2::avoid_types_on_closure_parameters::approximate_context_type;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{FragmentFlags, Tag, TypeKind};
use dartr_typesystem::TypeExt;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::DotShorthandConstructorInvocation,
        "prefer_const_constructors",
        dot_shorthand,
    );
    r.add(
        NodeKind::InstanceCreationExpression,
        "prefer_const_constructors",
        creation,
    );
}

/// Common checks of the constructor element; returns the base element.
fn const_constructor(c: &LinterContext<'_>, name: NodeId) -> Option<dartr_element::ElementId> {
    let element = base(c, c.element(name)?);
    if element.tag() != Tag::Constructor
        || !flags(c, element).contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST)
        || c.has_package_meta_getter(element, "literal")
    {
        return None;
    }
    let enclosing = enclosing(c, element)?;
    if enclosing.tag() == Tag::Class && name_is_dart_core_object(c, enclosing) {
        return None;
    }
    Some(element)
}

fn name_is_dart_core_object(c: &LinterContext<'_>, element: dartr_element::ElementId) -> bool {
    name(c, element) == Some("Object") && library_uri(c, element) == Some("dart:core")
}

fn dot_shorthand(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<DotShorthandConstructorInvocation>::from_raw(node)];
    if n.const_keyword.is_some() || in_constant_context(c, node) {
        return;
    }
    if const_constructor(c, n.constructor_name.raw()).is_none() {
        return;
    }
    if c.can_be_const(node) {
        c.report_node(out, &diag::PREFER_CONST_CONSTRUCTORS, node, &[]);
    }
}

fn creation(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    let n = &c.ast[Id::<InstanceCreationExpression>::from_raw(node)];
    if n.keyword.is_some_and(|k| lexeme(c, k) == "const") || in_constant_context(c, node) {
        return;
    }
    let named_type = c.ast[n.constructor_name].type_;
    // Dart `NamedType.isDeferred`.
    if let Some(prefix) = c.ast[named_type].import_prefix
        && let Some(element) = c.element(prefix).map(|e| base(c, e))
        && element.tag() == Tag::Prefix
        && prefix_is_deferred(c, element)
    {
        return;
    }
    let Some(element) = const_constructor(c, n.constructor_name.raw()) else {
        return;
    };
    let interface = enclosing(c, element).and_then(|e| e.cast::<dartr_element::InterfaceElement>());
    if let Some(interface) = interface
        && !ctx.interface_type_parameters(interface).is_empty()
        && c.ast[named_type].type_arguments.is_none()
        && let Some(context) = approximate_context_type(c, node)
        && let Some(instance) = ctx.as_instance_of(context, interface)
        && ctx
            .type_arguments(instance)
            .iter()
            .any(|&t| matches!(*ctx.ty(t), TypeKind::TypeParameter { .. }))
    {
        return;
    }
    if c.can_be_const(node) {
        c.report_node(out, &diag::PREFER_CONST_CONSTRUCTORS, node, &[]);
    }
}

/// Dart `PrefixElement.isDeferred` (any fragment of the prefix is deferred).
pub(crate) fn prefix_is_deferred(c: &LinterContext<'_>, prefix: dartr_element::ElementId) -> bool {
    let Some(ctx) = rctx(c) else { return false };
    let Some(data) = ctx.element_data(prefix) else { return false };
    let mut fragment = Some(data.first_fragment);
    while let Some(f) = fragment {
        if let Some(pf) = f.cast::<dartr_element::PrefixFragment>()
            && ctx.fragment(pf).is_deferred
        {
            return true;
        }
        fragment = ctx.fragment_data(f).and_then(|d| d.next_fragment);
    }
    false
}
