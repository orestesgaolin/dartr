// Dart source: pkg/linter/lib/src/rules/prefer_const_constructors_in_immutables.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{EId, ElementId, FragmentFlags, InterfaceElement, Tag};
use dartr_typesystem::TypeExt;

const RULE: &str = "prefer_const_constructors_in_immutables";

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::ConstructorDeclaration, RULE, check);
    r.add(NodeKind::PrimaryConstructorDeclaration, RULE, check);
}

fn is_const(c: &LinterContext<'_>, constructor: ElementId) -> bool {
    flags(c, constructor).contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST)
}

/// Dart `InterfaceElement.hasImmutableAnnotation` of the rule (the
/// superclass chain).
fn has_immutable_annotation(c: &LinterContext<'_>, element: EId<InterfaceElement>) -> bool {
    let Some(ctx) = rctx(c) else { return false };
    let mut seen = indexmap::IndexSet::new();
    let mut current = Some(element);
    while let Some(e) = current {
        if !seen.insert(e) {
            break;
        }
        if c.has_immutable(e.raw()) {
            return true;
        }
        current = ctx.element_supertype(e).and_then(|t| ctx.interface_element(t));
    }
    false
}

/// Dart `_hasConstConstructorInvocation`.
fn has_const_constructor_invocation(c: &LinterContext<'_>, element: ElementId, initializers: &[NodeId]) -> bool {
    let Some(ctx) = rctx(c) else { return false };
    let Some(interface) = enclosing(c, element).and_then(|e| e.cast::<InterfaceElement>()) else {
        return false;
    };
    let element_is_const = |n: NodeId| c.element(n).is_some_and(|e| is_const(c, base(c, e)));
    if let Some(&s) = initializers.iter().find(|&&i| kind(c, i) == NodeKind::SuperConstructorInvocation) {
        return element_is_const(s);
    }
    if let Some(&r) = initializers.iter().find(|&&i| kind(c, i) == NodeKind::RedirectingConstructorInvocation) {
        return element_is_const(r);
    }
    if interface.raw().tag() == Tag::ExtensionType {
        return ctx
            .interface(interface)
            .constructors
            .iter()
            .find(|k| flags(c, k.raw()).contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_PRIMARY))
            .is_some_and(|k| is_const(c, k.raw()));
    }
    let Some(supertype) = ctx.element_supertype(interface).and_then(|t| ctx.interface_element(t)) else {
        return false;
    };
    ctx.interface(supertype)
        .constructors
        .iter()
        .find(|k| name(c, k.raw()) == Some("new"))
        .is_some_and(|k| is_const(c, k.raw()))
}

/// Dart `isFactory` with a const `redirectedConstructor`.
fn redirected_const(c: &LinterContext<'_>, element: ElementId) -> Option<bool> {
    let ctx = rctx(c)?;
    if !flags(c, element).contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY) {
        return None;
    }
    let redirected = ctx
        .get(EId::<dartr_element::ConstructorElement>::from_raw(element))
        .redirected_constructor
        .get()?;
    Some(is_const(c, base(c, redirected)))
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(ctx) = rctx(c) else { return };
    let Some(element) = c.declared_element(node) else { return };
    if is_const(c, element) {
        return;
    }
    let (initializers, range) = match kind(c, node) {
        NodeKind::ConstructorDeclaration => {
            let n = &c.ast[Id::<ConstructorDeclaration>::from_raw(node)];
            if kind(c, n.body) != NodeKind::EmptyFunctionBody {
                return;
            }
            // Dart `firstTokenAfterCommentAndMetadata`.
            let first = [
                n.augment_keyword,
                n.external_keyword,
                n.const_keyword,
                n.factory_keyword,
                n.new_keyword,
                n.type_name.map(|t| c.ast.begin_token(t)),
                n.name,
            ]
            .into_iter()
            .flatten()
            .min_by_key(|&t| c.ast.tokens.get(t).offset)
            .unwrap();
            let t = c.ast.tokens.get(first);
            (
                c.ast.list_raw(n.initializers).to_vec(),
                (t.offset as usize, t.length as usize),
            )
        }
        _ => {
            let body = super::tighten_type_of_initializing_formals::primary_constructor_body(c, node);
            if let Some(body) = body
                && kind(c, c.ast[body].body) != NodeKind::EmptyFunctionBody
            {
                return;
            }
            let n = &c.ast[Id::<PrimaryConstructorDeclaration>::from_raw(node)];
            // Dart `PrimaryConstructorDeclaration.errorRange`.
            let begin = c.ast.tokens.get(c.ast.begin_token(node));
            let end = match n.constructor_name {
                Some(name) => c.ast.end(name) as usize,
                None => begin.offset as usize + begin.length as usize,
            };
            (
                body.map(|b| c.ast.list_raw(c.ast[b].initializers).to_vec()).unwrap_or_default(),
                (begin.offset as usize, end - begin.offset as usize),
            )
        }
    };
    let Some(interface) = enclosing(c, element).and_then(|e| e.cast::<InterfaceElement>()) else {
        return;
    };
    if !ctx.element_mixins(interface).is_empty() || !has_immutable_annotation(c, interface) {
        return;
    }
    if kind(c, node) == NodeKind::PrimaryConstructorDeclaration && interface.raw().tag() == Tag::ExtensionType {
        c.report_offset(out, &diag::PREFER_CONST_CONSTRUCTORS_IN_IMMUTABLES, range.0, range.1, &[]);
        return;
    }
    let report = match redirected_const(c, element) {
        Some(r) => r,
        None => has_const_constructor_invocation(c, element, &initializers) && c.can_be_const(node),
    };
    if report {
        c.report_offset(out, &diag::PREFER_CONST_CONSTRUCTORS_IN_IMMUTABLES, range.0, range.1, &[]);
    }
}
