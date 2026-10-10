// Dart source: pkg/linter/lib/src/rules/use_key_in_widget_constructors.dart
use super::flutter::*;
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{EId, ElemRef, FragmentFlags, InterfaceElement, Tag};
use dartr_typesystem::{TypeExt, member};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::ClassDeclaration,
        "use_key_in_widget_constructors",
        class,
    );
    r.add(
        NodeKind::ConstructorDeclaration,
        "use_key_in_widget_constructors",
        constructor,
    );
    r.add(
        NodeKind::PrimaryConstructorDeclaration,
        "use_key_in_widget_constructors",
        constructor,
    );
}

fn class(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    let Some(element) = c
        .declared_element(node)
        .and_then(|e| e.cast::<InterfaceElement>())
    else {
        return;
    };
    let no_origin_declaration = ctx.interface(element).constructors.iter().all(|k| {
        !flags(c, k.raw()).contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_ORIGIN_DECLARATION)
    });
    if is_public(c, element.raw())
        && !has_visible_for_testing(c, element.raw())
        && extends_widget(c, element)
        && no_origin_declaration
    {
        let (offset, length) = node_to_annotate(c, node);
        c.report_offset(
            out,
            &diag::USE_KEY_IN_WIDGET_CONSTRUCTORS,
            offset,
            length,
            &[],
        );
    }
}

fn constructor(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let (parameter_list, initializers) = match kind(c, node) {
        NodeKind::ConstructorDeclaration => {
            let n = &c.ast[Id::<ConstructorDeclaration>::from_raw(node)];
            if n.augment_keyword.is_some() {
                return;
            }
            (n.parameters, c.ast.list_raw(n.initializers).to_vec())
        }
        _ => {
            let n = &c.ast[Id::<PrimaryConstructorDeclaration>::from_raw(node)];
            let initializers =
                super::tighten_type_of_initializing_formals::primary_constructor_body(c, node)
                    .map(|b| c.ast.list_raw(c.ast[b].initializers).to_vec())
                    .unwrap_or_default();
            (n.formal_parameters, initializers)
        }
    };
    let Some(element) = c.declared_element(node) else {
        return;
    };
    if !is_public(c, element)
        || flags(c, element).contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY)
        || has_visible_for_testing(c, element)
    {
        return;
    }
    let Some(class) = enclosing(c, element) else {
        return;
    };
    if !is_public(c, class) || class.tag() != Tag::Class || has_visible_for_testing(c, class) {
        return;
    }
    let class = EId::<InterfaceElement>::from_raw(class);
    if is_exactly_widget(c, class) || !extends_widget(c, class) {
        return;
    }
    // Dart `_hasKeySuperParameterInitializerArg`.
    if parameters(c, Some(parameter_list)).iter().any(|&p| {
        c.declared_element(p)
            .is_some_and(|e| e.tag() == Tag::SuperFormalParameter && name(c, e) == Some("key"))
    }) {
        return;
    }
    if !initializers.iter().any(|&i| is_missing_key(c, i)) {
        let (offset, length) = node_to_annotate(c, node);
        c.report_offset(
            out,
            &diag::USE_KEY_IN_WIDGET_CONSTRUCTORS,
            offset,
            length,
            &[],
        );
    }
}

/// Dart `ConstructorInitializer.isMissingKey`.
fn is_missing_key(c: &LinterContext<'_>, initializer: NodeId) -> bool {
    let argument_list = match kind(c, initializer) {
        NodeKind::SuperConstructorInvocation => {
            c.ast[Id::<SuperConstructorInvocation>::from_raw(initializer)].argument_list
        }
        NodeKind::RedirectingConstructorInvocation => {
            c.ast[Id::<RedirectingConstructorInvocation>::from_raw(initializer)].argument_list
        }
        _ => return false,
    };
    let Some(element) = c.element(initializer) else {
        return false;
    };
    !defines_key_parameter(c, element)
        || arguments(c, argument_list).iter().any(|&a| {
            c.corresponding_parameter(argument_expression(c, a))
                .is_some_and(|p| name(c, base(c, p)) == Some("key"))
        })
}

/// Dart `ConstructorElement.definesKeyParameter`.
fn defines_key_parameter(c: &LinterContext<'_>, element: ElemRef) -> bool {
    let Some(ctx) = rctx(c) else { return false };
    member::formal_parameters(&ctx, element)
        .into_iter()
        .any(|p| name(c, base(c, p)) == Some("key") && implements_key(c, element_type(c, p)))
}

/// Dart `type.implementsInterface('Key', '')`.
fn implements_key(c: &LinterContext<'_>, ty: Option<dartr_element::TypeId>) -> bool {
    let Some(ty) = ty else { return false };
    let Some(ctx) = rctx(c) else { return false };
    let same = |t: dartr_element::TypeId| {
        ctx.interface_element(t).is_some_and(|e| {
            name(c, e.raw()) == Some("Key") && library_name(c, e.raw()).unwrap_or("").is_empty()
        })
    };
    let check = type_for_interface_check(c, ty);
    let Some(element) = ctx.interface_element(check) else {
        return false;
    };
    same(check) || ctx.element_all_supertypes(element).iter().any(|&t| same(t))
}
