// Dart source: pkg/analyzer/lib/src/utilities/extensions/flutter.dart (isWidget, isWidgetExpression, findWidgetExpression, isExactWidgetType*, isFlexWidget, isWidgetCreation)

//! The Flutter helpers of the corrections: widget classes and widget
//! expressions.

use dartr_ast::*;
use dartr_element::{Ctx, ElementId, InterfaceElement, Tag, TypeId, TypeKind};
use dartr_typesystem::type_ext::TypeExt;

use super::producer::ProducerContext;

pub const WIDGETS_URI: &str = "package:flutter/widgets.dart";
pub const URI_BASIC: &str = "package:flutter/src/widgets/basic.dart";
pub const URI_CONTAINER: &str = "package:flutter/src/widgets/container.dart";
pub const URI_FRAMEWORK: &str = "package:flutter/src/widgets/framework.dart";

/// The URI of the library of [element].
fn element_library_uri(ctx: &Ctx<'_>, element: ElementId) -> Option<String> {
    let library = ctx.element_data(element).and_then(|d| d.library)?;
    Some(super::imports::library_uri(ctx, library))
}

/// Dart `_isExactly(type, uri)`.
pub fn is_exactly(ctx: &Ctx<'_>, element: ElementId, name: &str, uri: &str) -> bool {
    element.tag() == Tag::Class
        && ctx.element_name(element) == Some(name)
        && element_library_uri(ctx, element).as_deref() == Some(uri)
}

fn any_supertype_exactly(ctx: &Ctx<'_>, element: ElementId, name: &str, uri: &str) -> bool {
    let Some(interface) = element.cast::<InterfaceElement>() else {
        return false;
    };
    ctx.element_all_supertypes(interface).iter().any(|&t| {
        ctx.interface_element(t)
            .is_some_and(|e| is_exactly(ctx, e.raw(), name, uri))
    })
}

/// Dart `Element.isWidget`.
pub fn is_widget(ctx: &Ctx<'_>, element: ElementId) -> bool {
    if element.tag() != Tag::Class {
        return false;
    }
    is_exactly(ctx, element, "Widget", URI_FRAMEWORK)
        || any_supertype_exactly(ctx, element, "Widget", URI_FRAMEWORK)
}

/// Dart `Element.isFlexWidget`.
pub fn is_flex_widget(ctx: &Ctx<'_>, element: ElementId) -> bool {
    if !is_widget(ctx, element) {
        return false;
    }
    is_exactly(ctx, element, "Flex", URI_BASIC)
        || any_supertype_exactly(ctx, element, "Flex", URI_BASIC)
}

fn type_element(ctx: &Ctx<'_>, ty: TypeId) -> Option<ElementId> {
    match *ctx.ty(ty) {
        TypeKind::Interface { element, .. } => Some(element.raw()),
        _ => None,
    }
}

/// Dart `DartType.isWidgetType`.
pub fn is_widget_type(ctx: &Ctx<'_>, ty: Option<TypeId>) -> bool {
    ty.and_then(|t| type_element(ctx, t))
        .is_some_and(|e| is_widget(ctx, e))
}

/// Dart `DartType.isExactWidgetType<name>`.
pub fn is_exact_widget_type(ctx: &Ctx<'_>, ty: Option<TypeId>, name: &str, uri: &str) -> bool {
    ty.and_then(|t| type_element(ctx, t))
        .is_some_and(|e| is_exactly(ctx, e, name, uri))
}

/// Dart `AstNode.isWidgetExpression`.
pub fn is_widget_expression(c: &ProducerContext<'_>, node: NodeId) -> bool {
    let ast = c.ast;
    let parent = ast.parent(node);
    if parent.is_some_and(|p| ast.is::<NamedType>(p)) {
        return false;
    }
    if parent
        .and_then(|p| ast.parent(p))
        .is_some_and(|g| ast.is::<NamedType>(g))
    {
        return false;
    }
    if parent.is_some_and(|p| ast.is::<ConstructorName>(p)) {
        return false;
    }
    if ast.is::<NamedArgument>(node) {
        return false;
    }
    if !ast.is::<Expression>(node) {
        return false;
    }
    is_widget_type(c.ctx, super::producers::create::static_type(c, node))
}

/// Dart `AstNode.findWidgetExpression`.
pub fn find_widget_expression(c: &ProducerContext<'_>, start: NodeId) -> Option<NodeId> {
    let ast = c.ast;
    let mut current = Some(start);
    while let Some(node) = current {
        current = ast.parent(node);
        if !is_widget_expression(c, node) {
            if ast.is::<ArgumentList>(node)
                || ast.is::<Statement>(node)
                || ast.is::<FunctionBody>(node)
            {
                return None;
            }
            continue;
        }
        if ast.is::<AssignmentExpression>(node) {
            return None;
        }
        let Some(parent) = ast.parent(node) else {
            continue;
        };
        if let Some(a) = ast.cast::<AssignmentExpression>(parent) {
            if ast[a].right_hand_side.raw() == node {
                return Some(node);
            }
            return None;
        }
        let matches = ast.is::<ArgumentList>(parent)
            || ast.cast::<ConditionalExpression>(parent).is_some_and(|p| {
                ast[p].then_expression.raw() == node || ast[p].else_expression.raw() == node
            })
            || ast
                .cast::<ExpressionFunctionBody>(parent)
                .is_some_and(|p| ast[p].expression.raw() == node)
            || ast
                .cast::<ForElement>(parent)
                .is_some_and(|p| ast[p].body.raw() == node)
            || ast.cast::<IfElement>(parent).is_some_and(|p| {
                ast[p].then_element.raw() == node
                    || ast[p].else_element.map(|e| e.raw()) == Some(node)
            })
            || ast.is::<ListLiteral>(parent)
            || ast
                .cast::<NamedArgument>(parent)
                .is_some_and(|p| ast[p].argument_expression.raw() == node)
            || ast.is::<Statement>(parent)
            || ast
                .cast::<SwitchExpressionCase>(parent)
                .is_some_and(|p| ast[p].expression.raw() == node)
            || ast.is::<VariableDeclaration>(parent);
        if matches {
            return Some(node);
        }
    }
    None
}

/// Dart `InstanceCreationExpression.isWidgetCreation`.
pub fn is_widget_creation(c: &ProducerContext<'_>, node: Id<InstanceCreationExpression>) -> bool {
    let ast = c.ast;
    let constructor = c.locator().element(ast[node].constructor_name);
    let Some(enclosing) = constructor.and_then(|e| c.ctx.element_data(e).and_then(|d| d.enclosing))
    else {
        return false;
    };
    is_widget(c.ctx, enclosing)
}

/// The class [name] exported by the library [uri] (Dart
/// `sessionHelper.getClass`).
pub fn get_class(c: &ProducerContext<'_>, uri: &str, name: &str) -> Option<ElementId> {
    let ctx = c.ctx;
    let library = ctx.library_by_uri(uri)?;
    let element = super::producers::import_library::exported_element(ctx, library, name)?;
    (element.tag() == Tag::Class).then_some(element)
}
