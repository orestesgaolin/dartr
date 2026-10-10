// Dart source: pkg/linter/lib/src/rules/avoid_positional_boolean_parameters.dart

use super::helpers::{declared_type, element_name};
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElemRef, InterfaceElement};
use dartr_typesystem::TypeExt;
use dartr_typesystem::inheritance_manager3::{InheritanceManager3, Name};

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    for kind in [
        NodeKind::ConstructorDeclaration,
        NodeKind::FunctionDeclaration,
        NodeKind::GenericFunctionType,
        NodeKind::MethodDeclaration,
        NodeKind::PrimaryConstructorDeclaration,
    ] {
        registry.add(kind, "avoid_positional_boolean_parameters", check);
    }
}

fn parameters(c: &LinterContext<'_>, node: NodeId) -> Option<Id<FormalParameterList>> {
    match c.ast.kind(node) {
        NodeKind::ConstructorDeclaration => {
            Some(c.ast[Id::<ConstructorDeclaration>::from_raw(node)].parameters)
        }
        NodeKind::FunctionDeclaration => {
            c.ast[c.ast[Id::<FunctionDeclaration>::from_raw(node)].function_expression].parameters
        }
        NodeKind::GenericFunctionType => {
            Some(c.ast[Id::<GenericFunctionType>::from_raw(node)].parameters)
        }
        NodeKind::MethodDeclaration => c.ast[Id::<MethodDeclaration>::from_raw(node)].parameters,
        NodeKind::PrimaryConstructorDeclaration => {
            Some(c.ast[Id::<PrimaryConstructorDeclaration>::from_raw(node)].formal_parameters)
        }
        _ => None,
    }
}

fn is_augmentation(c: &LinterContext<'_>, node: NodeId) -> bool {
    match c.ast.kind(node) {
        NodeKind::ConstructorDeclaration => c.ast[Id::<ConstructorDeclaration>::from_raw(node)]
            .augment_keyword
            .is_some(),
        NodeKind::FunctionDeclaration => c.ast[Id::<FunctionDeclaration>::from_raw(node)]
            .augment_keyword
            .is_some(),
        NodeKind::MethodDeclaration => c.ast[Id::<MethodDeclaration>::from_raw(node)]
            .augment_keyword
            .is_some(),
        NodeKind::PrimaryConstructorDeclaration => {
            super::helpers::ancestor(c.ast, node, NodeKind::ClassDeclaration).is_some_and(|class| {
                c.ast[Id::<ClassDeclaration>::from_raw(class)]
                    .augment_keyword
                    .is_some()
            }) || super::helpers::ancestor(c.ast, node, NodeKind::EnumDeclaration).is_some_and(
                |enum_| {
                    c.ast[Id::<EnumDeclaration>::from_raw(enum_)]
                        .augment_keyword
                        .is_some()
                },
            ) || super::helpers::ancestor(c.ast, node, NodeKind::ExtensionTypeDeclaration)
                .is_some_and(|extension| {
                    c.ast[Id::<ExtensionTypeDeclaration>::from_raw(extension)]
                        .augment_keyword
                        .is_some()
                })
        }
        _ => false,
    }
}

/// Dart `_isOverridingMember`: the enclosing class (a `ClassElement`, not a
/// mixin, enum or extension) inherits a member with the name.
fn is_overriding_member(c: &LinterContext<'_>, member: dartr_element::ElementId) -> bool {
    let Some(r) = c.resolved else { return false };
    let mut class = r.ctx.element_data(member).and_then(|d| d.enclosing);
    while let Some(e) = class {
        if e.tag() == dartr_element::Tag::Class {
            break;
        }
        class = r.ctx.element_data(e).and_then(|d| d.enclosing);
    }
    let Some(class) = class.and_then(|e| e.cast::<InterfaceElement>()) else {
        return false;
    };
    let Some(name) = element_name(c, ElemRef::Base(member)) else {
        return false;
    };
    let library = r.ctx.element_data(class.raw()).and_then(|d| d.library);
    InheritanceManager3::new(r.ctx)
        .get_inherited(class, Name::new(&r.ctx, library, name))
        .is_some()
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(r) = c.resolved else { return };
    if is_augmentation(c, node) {
        return;
    }
    if c.ast.kind(node) != NodeKind::GenericFunctionType {
        let Some(element) = c.declared_element(node) else {
            return;
        };
        if element_name(c, ElemRef::Base(element)).is_some_and(|name| name.starts_with('_')) {
            return;
        }
        if c.ast.kind(node) == NodeKind::MethodDeclaration {
            let method = &c.ast[Id::<MethodDeclaration>::from_raw(node)];
            if method
                .property_keyword
                .is_some_and(|k| c.ast.tokens.lexeme(k) == "set")
                || method.operator_keyword.is_some()
            {
                return;
            }
            // Dart `node.hasInheritedMethod`.
            if crate::rules::batch_b::util::look_up_inherited_method(c, node).is_some() {
                return;
            }
            if is_overriding_member(c, element) {
                return;
            }
        }
    }
    let Some(list) = parameters(c, node) else {
        return;
    };
    for parameter in c.ast.list_raw(c.ast[list].parameters) {
        let kind = c
            .ast
            .cast::<RegularFormalParameter>(*parameter)
            .map(|p| c.ast[p].kind)
            .or_else(|| {
                c.ast
                    .cast::<FieldFormalParameter>(*parameter)
                    .map(|p| c.ast[p].kind)
            })
            .or_else(|| {
                c.ast
                    .cast::<SuperFormalParameter>(*parameter)
                    .map(|p| c.ast[p].kind)
            });
        if kind.is_some_and(|k| k.is_positional())
            && declared_type(c, *parameter).is_some_and(|ty| r.ctx.is_dart_core_bool(ty))
        {
            c.report_node(
                out,
                &diag::AVOID_POSITIONAL_BOOLEAN_PARAMETERS,
                *parameter,
                &[],
            );
            return;
        }
    }
}
