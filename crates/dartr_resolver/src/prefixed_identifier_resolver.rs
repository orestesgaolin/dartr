// Dart source: pkg/analyzer/lib/src/dart/resolver/prefixed_identifier_resolver.dart,
// pkg/analyzer/lib/src/generated/resolver.dart (visitPrefixedIdentifier)

//! `PrefixedIdentifierResolver`: `p.x` where `p` is an import prefix, a
//! type, an extension or an expression (then it is a property access; a
//! record-typed prefix is rewritten to a `PropertyAccess`).

use dartr_ast::{
    CommentReference, ConstructorDeclaration, ConstructorName, Expression, Id, MethodInvocation,
    NamedType, PrefixedIdentifier, PropertyAccess,
};
use dartr_diagnostics::diag;
use dartr_element::{ExtensionElement, InterfaceElement, Tag, TypeId, TypeKind, VariableElement};
use dartr_typesystem::member;

use crate::property_element_resolver;
use crate::resolver::ResolverVisitor;

/// Dart `ResolverVisitor.visitPrefixedIdentifier(node, contextType:)`.
pub fn visit_prefixed_identifier(
    rv: &mut ResolverVisitor<'_>,
    node: Id<PrefixedIdentifier>,
    context_type: TypeId,
) {
    rv.check_unreachable_node(node);
    if let Some(rewritten_property_access) = resolve(rv, node, context_type) {
        property_element_resolver::resolve_property_access_rhs(
            rv,
            rewritten_property_access,
            context_type,
            Some(node),
        );
        return;
    }
    let e = rv.insert_generic_function_instantiation(node.upcast(), context_type);
    rv.insert_implicit_call_reference(e, context_type);
}

/// Dart `PrefixedIdentifierResolver.resolve(node, contextType:)`: returns
/// the property access that replaces [node] when the prefix has a record
/// type.
fn resolve(
    rv: &mut ResolverVisitor<'_>,
    node: Id<PrefixedIdentifier>,
    context_type: TypeId,
) -> Option<Id<PropertyAccess>> {
    let ctx = rv.ctx;
    let prefix = rv.ast[node].prefix;
    rv.resolve_expression(prefix.upcast(), TypeId::UNKNOWN);
    let identifier = rv.ast[node].identifier;
    rv.check_unreachable_node(identifier);

    let prefix_element = rv.base_element(prefix);
    if prefix_element.is_none_or(|e| e.tag() != Tag::Prefix)
        && let Some(prefix_type) = rv.static_type(prefix)
    {
        // TODO(scheglov): It would be nice to rewrite all such cases.
        let prefix_type_resolved = rv.type_system.resolve_to_bound(prefix_type);
        if matches!(ctx.ty(prefix_type_resolved), TypeKind::Record { .. }) {
            let period = rv.ast[node].period;
            let property_access = rv.ast.add(PropertyAccess {
                target: Some(prefix.upcast()),
                operator: period,
                property_name: identifier,
            });
            rv.replace_expression(node.upcast(), property_access.upcast(), None);
            return Some(property_access);
        }
    }

    let result =
        property_element_resolver::resolve_prefixed_identifier(rv, node, true, false, false);

    let element = result.read_element();
    rv.set_element(identifier, element);

    let base = element.map(|e| member::base_element(&ctx, e));
    if base.is_some_and(|b| b.is::<ExtensionElement>()) {
        set_extension_identifier_type(rv, node);
        return None;
    }

    if rv.static_type(prefix) == Some(TypeId::NEVER) {
        rv.set_static_type(identifier, TypeId::NEVER);
        rv.record_static_type(node, TypeId::NEVER);
        return None;
    }

    let tp = ctx.tp;
    let mut ty = TypeId::INVALID;
    if result.read_element_requested.is_none() && result.read_element_recovery.is_some() {
        // Since the element came from error recovery logic, its type isn't
        // trustworthy; leave it as `dynamic`.
    } else if let (Some(element), Some(base)) = (element, base) {
        if base.is::<InterfaceElement>() {
            if is_expression_identifier(rv, node.upcast()) {
                let t = tp.type_type();
                rv.record_static_type(node, t);
                rv.set_static_type(identifier, t);
            }
            return None;
        } else if base.tag() == Tag::Dynamic {
            let t = tp.type_type();
            rv.record_static_type(node, t);
            rv.set_static_type(identifier, t);
            return None;
        } else if base.tag() == Tag::TypeAlias {
            if !rv
                .ast
                .parent(node)
                .is_some_and(|p| rv.ast.is::<NamedType>(p))
            {
                let t = tp.type_type();
                rv.record_static_type(node, t);
                rv.set_static_type(identifier, t);
            }
            return None;
        } else if base.tag() == Tag::Method {
            ty = member::type_(&ctx, element);
        } else if base.is::<dartr_element::PropertyAccessorElement>() {
            ty = result.get_type.unwrap_or(TypeId::INVALID);
        } else if base.is::<dartr_element::ExecutableElement>() || base.is::<VariableElement>() {
            ty = member::type_(&ctx, element);
        } else if let Some(t) = result.function_type_call_type {
            ty = t;
        } else if result.at_dynamic_target {
            ty = TypeId::DYNAMIC;
        }
    } else if let Some(t) = result.function_type_call_type {
        ty = t;
    } else if result.at_dynamic_target {
        ty = TypeId::DYNAMIC;
    }

    if !rv.is_constructor_tearoffs_enabled() {
        // Only perform a generic function instantiation on a
        // [PrefixedIdentifier] in pre-constructor-tearoffs code. In
        // constructor-tearoffs-enabled code, generic function instantiation
        // is performed at assignability check sites.
        ty = crate::invocation_inference_helper::infer_tear_off(
            rv,
            node.upcast(),
            identifier,
            ty,
            context_type,
        );
    }
    rv.set_static_type(identifier, ty);
    rv.record_static_type(node, ty);
    None
}

/// Dart `_isExpressionIdentifier(node)`: `true` if the prefixed identifier
/// [node] is not a type literal.
fn is_expression_identifier(rv: &ResolverVisitor<'_>, node: Id<Expression>) -> bool {
    let Some(parent) = rv.ast.parent(node) else {
        return true;
    };
    if let Some(c) = rv.ast.cast::<ConstructorDeclaration>(parent)
        && rv.ast[c].type_name.map(|t| t.raw()) == Some(node.raw())
    {
        return false;
    }
    if rv.ast.is::<ConstructorName>(parent)
        || rv.ast.is::<MethodInvocation>(parent)
        || rv
            .ast
            .cast::<PrefixedIdentifier>(parent)
            .is_some_and(|p| rv.ast[p].prefix.raw() == node.raw())
        || rv.ast.is::<PropertyAccess>(parent)
        || rv.ast.is::<NamedType>(parent)
    {
        return false;
    }
    true
}

/// Dart `_setExtensionIdentifierType(node)` for a prefixed identifier.
fn set_extension_identifier_type(rv: &mut ResolverVisitor<'_>, node: Id<PrefixedIdentifier>) {
    if let Some(parent) = rv.ast.parent(node) {
        let ast = &*rv.ast;
        let current = node.raw();
        if ast.is::<CommentReference>(parent)
            || ast
                .cast::<MethodInvocation>(parent)
                .is_some_and(|m| ast[m].target.map(|t| t.raw()) == Some(current))
            || ast
                .cast::<PrefixedIdentifier>(parent)
                .is_some_and(|p| ast[p].prefix.raw() == current)
            || ast
                .cast::<PropertyAccess>(parent)
                .is_some_and(|p| ast[p].target.map(|t| t.raw()) == Some(current))
        {
            return;
        }
    }

    // Dart `node.name` of a prefixed identifier: `prefix.identifier`.
    let prefix = rv.ast[node].prefix;
    let identifier = rv.ast[node].identifier;
    let name = format!(
        "{}.{}",
        rv.lexeme(rv.ast[prefix].token),
        rv.lexeme(rv.ast[identifier].token)
    );
    let d = rv.at(diag::extension_as_expression(&name), node);
    rv.report(d);

    rv.set_static_type(identifier, TypeId::DYNAMIC);
    rv.record_static_type(node, TypeId::DYNAMIC);
}
