// Dart source: pkg/analyzer/lib/src/dart/resolver/comment_reference_resolver.dart

//! Resolution of references in documentation comments.

use dartr_ast::{CommentReference, Id, PrefixedIdentifier, PropertyAccess, SimpleIdentifier};
use dartr_element::{
    EId, ElemRef, ExtensionElement, InterfaceElement, PrefixElement, TypeAliasElement, TypeKind,
};
use dartr_typesystem::TypeExt;
use dartr_typesystem::inheritance_manager3::{InheritanceManager3, Name};
use dartr_typesystem::lookup;

use crate::ast_ext::identifier_name;
use crate::resolver::ResolverVisitor;
use crate::type_property_resolver::{self, PropertyQuery};

/// Dart `ResolverVisitor.visitCommentReference(node)`.
pub fn visit_comment_reference(rv: &mut ResolverVisitor<'_>, node: Id<CommentReference>) {
    // Dart locks the reporter because unresolved comment references are not
    // resolver diagnostics.
    rv.lock_level += 1;
    resolve(rv, node);
    rv.lock_level -= 1;
}

/// Dart `CommentReferenceResolver.resolve`.
fn resolve(rv: &mut ResolverVisitor<'_>, node: Id<CommentReference>) {
    let expression = rv.ast[node].expression;
    let has_new_keyword = rv.ast[node].new_keyword.is_some();
    if let Some(expression) = rv.ast.cast::<SimpleIdentifier>(expression) {
        resolve_simple_identifier_reference(rv, expression, has_new_keyword);
    } else if let Some(expression) = rv.ast.cast::<PrefixedIdentifier>(expression) {
        resolve_prefixed_identifier_reference(rv, expression, has_new_keyword);
    } else if let Some(expression) = rv.ast.cast::<PropertyAccess>(expression) {
        resolve_property_access_reference(rv, expression, has_new_keyword);
    }
}

/// Dart `_resolveSimpleIdentifier`.
fn resolve_simple_identifier(
    rv: &mut ResolverVisitor<'_>,
    identifier: Id<SimpleIdentifier>,
) -> Option<ElemRef> {
    let lookup = rv.rt.scope_lookup_result.get(identifier.raw()).copied();
    let element = lookup
        .and_then(|result| result.getter.or(result.setter))
        .map(ElemRef::Base);
    if element.is_some() {
        // Dart also calls `notifyPrefixUsedInCommentReference` for prefixes.
        // Import-usage tracking is not represented in the Rust resolver.
        return element;
    }

    let enclosing_type = if let Some(enclosing_class) = rv.enclosing_class {
        rv.ctx.interface_this_type(enclosing_class)
    } else {
        let enclosing_extension = rv.enclosing_extension?;
        let extended_type = rv.ctx.get(enclosing_extension).extended_type.get()?;
        let extended_type = rv.type_system.resolve_to_bound(extended_type);
        match rv.ctx.ty(extended_type) {
            TypeKind::Interface { .. } => extended_type,
            TypeKind::Function(_) => rv.ctx.tp.function_type(),
            _ => return None,
        }
    };

    let name = identifier_name(rv.ast, identifier).to_string();
    let result = type_property_resolver::resolve(
        rv,
        PropertyQuery {
            receiver: None,
            receiver_type: enclosing_type,
            name: &name,
            has_read: true,
            has_write: true,
            property_error_entity: identifier.raw(),
            name_error_entity: identifier.raw(),
            parent_node: None,
        },
    );
    result.getter.or(result.setter)
}

/// When [element] is a type alias, returns the element of its aliased type.
fn aliased_element(rv: &ResolverVisitor<'_>, element: ElemRef) -> ElemRef {
    let base = dartr_typesystem::member::base_element(&rv.ctx, element);
    let Some(alias) = base.cast::<TypeAliasElement>() else {
        return element;
    };
    rv.ctx
        .get(alias)
        .aliased_type
        .get()
        .and_then(|ty| rv.ctx.type_element(ty))
        .map(ElemRef::Base)
        .unwrap_or(element)
}

/// The member selected by a documentation reference on an interface.
fn interface_member(
    rv: &ResolverVisitor<'_>,
    interface: EId<InterfaceElement>,
    name: &str,
) -> Option<ElemRef> {
    let ctx = rv.ctx;
    let library = ctx.interface(interface).library;
    InheritanceManager3::new(ctx)
        .get_member(interface, Name::for_library(&ctx, library, name))
        .or_else(|| {
            lookup::get_method(&ctx, interface.upcast(), name).map(|e| ElemRef::Base(e.raw()))
        })
        .or_else(|| {
            lookup::get_getter(&ctx, interface.upcast(), name).map(|e| ElemRef::Base(e.raw()))
        })
        .or_else(|| {
            lookup::get_setter(&ctx, interface.upcast(), name).map(|e| ElemRef::Base(e.raw()))
        })
        .or_else(|| {
            lookup::get_named_constructor(&ctx, interface, name).map(|e| ElemRef::Base(e.raw()))
        })
}

/// Direct interface member lookup used by Dart's property-access branch.
fn direct_interface_member(
    rv: &ResolverVisitor<'_>,
    interface: EId<InterfaceElement>,
    name: &str,
) -> Option<ElemRef> {
    let ctx = rv.ctx;
    lookup::get_method(&ctx, interface.upcast(), name)
        .map(|e| ElemRef::Base(e.raw()))
        .or_else(|| {
            lookup::get_getter(&ctx, interface.upcast(), name).map(|e| ElemRef::Base(e.raw()))
        })
        .or_else(|| {
            lookup::get_setter(&ctx, interface.upcast(), name).map(|e| ElemRef::Base(e.raw()))
        })
        .or_else(|| {
            lookup::get_named_constructor(&ctx, interface, name).map(|e| ElemRef::Base(e.raw()))
        })
}

/// The member selected by a documentation reference on an extension.
fn extension_member(
    rv: &ResolverVisitor<'_>,
    extension: EId<ExtensionElement>,
    name: &str,
) -> Option<ElemRef> {
    let ctx = rv.ctx;
    lookup::get_method(&ctx, extension.upcast(), name)
        .map(|e| ElemRef::Base(e.raw()))
        .or_else(|| {
            lookup::get_getter(&ctx, extension.upcast(), name).map(|e| ElemRef::Base(e.raw()))
        })
        .or_else(|| {
            lookup::get_setter(&ctx, extension.upcast(), name).map(|e| ElemRef::Base(e.raw()))
        })
}

/// Dart `_resolvePrefixedIdentifierReference`.
fn resolve_prefixed_identifier_reference(
    rv: &mut ResolverVisitor<'_>,
    expression: Id<PrefixedIdentifier>,
    has_new_keyword: bool,
) {
    let prefix = rv.ast[expression].prefix;
    let Some(mut prefix_element) = resolve_simple_identifier(rv, prefix) else {
        return;
    };
    rv.set_element(prefix, Some(prefix_element));
    prefix_element = aliased_element(rv, prefix_element);
    let prefix_base = dartr_typesystem::member::base_element(&rv.ctx, prefix_element);

    let identifier = rv.ast[expression].identifier;
    let name = identifier_name(rv.ast, identifier).to_string();
    if let Some(prefix) = prefix_base.cast::<PrefixElement>() {
        let result = rv.unit.scopes.prefix_lookup(&rv.ctx, prefix, &name);
        rv.set_element(
            identifier,
            result.getter.or(result.setter).map(ElemRef::Base),
        );
        return;
    }

    let element = if !has_new_keyword {
        if let Some(interface) = prefix_base.cast::<InterfaceElement>() {
            interface_member(rv, interface, &name)
        } else if let Some(extension) = prefix_base.cast::<ExtensionElement>() {
            extension_member(rv, extension, &name)
        } else {
            None
        }
    } else if let Some(interface) = prefix_base.cast::<InterfaceElement>() {
        lookup::get_named_constructor(&rv.ctx, interface, &name).map(|e| ElemRef::Base(e.raw()))
    } else {
        None
    };
    rv.set_element(identifier, element);
}

/// Dart `_resolvePropertyAccessReference`.
fn resolve_property_access_reference(
    rv: &mut ResolverVisitor<'_>,
    expression: Id<PropertyAccess>,
    _has_new_keyword: bool,
) {
    let Some(target) = rv.ast[expression].target else {
        return;
    };
    let Some(target) = rv.ast.cast::<PrefixedIdentifier>(target) else {
        return;
    };

    let prefix = rv.ast[target].prefix;
    let Some(prefix_element) = resolve_simple_identifier(rv, prefix) else {
        return;
    };
    rv.set_element(prefix, Some(prefix_element));
    let prefix_base = dartr_typesystem::member::base_element(&rv.ctx, prefix_element);
    let Some(prefix_element) = prefix_base.cast::<PrefixElement>() else {
        return;
    };

    let identifier = rv.ast[target].identifier;
    let name = identifier_name(rv.ast, identifier).to_string();
    let result = rv.unit.scopes.prefix_lookup(&rv.ctx, prefix_element, &name);
    let element = result.getter.or(result.setter).map(ElemRef::Base);
    rv.set_element(identifier, element);

    let Some(element) = element else {
        return;
    };
    let element = aliased_element(rv, element);
    let element_base = dartr_typesystem::member::base_element(&rv.ctx, element);
    let property_name = rv.ast[expression].property_name;
    let name = identifier_name(rv.ast, property_name).to_string();
    let property_element = if let Some(interface) = element_base.cast::<InterfaceElement>() {
        direct_interface_member(rv, interface, &name)
    } else if let Some(extension) = element_base.cast::<ExtensionElement>() {
        extension_member(rv, extension, &name)
    } else {
        None
    };
    rv.set_element(property_name, property_element);
}

/// Dart `_resolveSimpleIdentifierReference`.
fn resolve_simple_identifier_reference(
    rv: &mut ResolverVisitor<'_>,
    expression: Id<SimpleIdentifier>,
    has_new_keyword: bool,
) {
    let Some(element) = resolve_simple_identifier(rv, expression) else {
        return;
    };
    rv.set_element(expression, Some(element));
    if has_new_keyword {
        let base = dartr_typesystem::member::base_element(&rv.ctx, element);
        let Some(interface) = base.cast::<InterfaceElement>() else {
            return;
        };
        let Some(constructor) = lookup::get_named_constructor(&rv.ctx, interface, "new") else {
            return;
        };
        rv.set_element(expression, Some(ElemRef::Base(constructor.raw())));
    }
}
