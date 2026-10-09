// Dart source: pkg/analyzer/lib/src/dart/resolver/constructor_reference_resolver.dart

//! `ConstructorReferenceResolver`: constructor tear-offs (`C.new`,
//! `C<int>.named`, `prefix.C.named`), with the inference of the type
//! arguments of a generic class from the context.

use dartr_ast::{ConstructorReference, Id};
use dartr_diagnostics::diag;
use dartr_element::{
    ElemRef, InstanceElement, InterfaceElement, Tag, TypeAliasElement, TypeId, TypeKind,
};
use dartr_typesystem::{TypeExt, lookup, member};

use crate::element_resolver::is_factory;
use crate::instance_creation_expression_resolver::constructor_element_to_infer;
use crate::invocation_inference_helper::infer_tear_off;
use crate::resolver::ResolverVisitor;

/// Dart `ResolverVisitor.visitConstructorReference(node, contextType:)`.
pub fn visit_constructor_reference(
    rv: &mut ResolverVisitor<'_>,
    node: Id<ConstructorReference>,
    context_type: TypeId,
) {
    resolve(rv, node, context_type);
    rv.insert_implicit_call_reference(node.upcast(), context_type);
}

/// Dart `ConstructorReferenceResolver.resolve(node, contextType:)`.
pub fn resolve(rv: &mut ResolverVisitor<'_>, node: Id<ConstructorReference>, context_type: TypeId) {
    let ctx = rv.ctx;
    let constructor_name = rv.ast[node].constructor_name;
    let named_type = rv.ast[constructor_name].type_;
    if !rv.is_constructor_tearoffs_enabled() && rv.ast[named_type].type_arguments.is_none() {
        // Only report this if [node] has no explicit type arguments; otherwise
        // the parser has already reported an error.
        let d = rv.at(diag::sdk_version_constructor_tearoffs(), node);
        rv.report(d);
    }
    rv.visit_node(constructor_name.raw());
    let element = rv.element(constructor_name);
    if let Some(element) = element
        && !is_factory(rv, element)
        && let Some(enclosing) = member::enclosing_element(&ctx, element)
        && enclosing.tag() == Tag::Class
        && crate::element_ext::first_fragment_flags(&ctx, enclosing)
            .contains(dartr_element::FragmentFlags::CLASS_FRAGMENT_IS_ABSTRACT)
    {
        let d = rv.at(
            diag::tearoff_of_generative_constructor_of_abstract_class(),
            node,
        );
        rv.report(d);
    }
    let name = rv.ast[constructor_name].name;
    if element.is_none()
        && let Some(name) = name
        && rv.is_constructor_tearoffs_enabled()
    {
        // The illegal construction, which looks like a type-instantiated
        // constructor tearoff, may be an attempt to reference a member on
        // [enclosingElement]. Try to provide a helpful error, and fall back to
        // "unknown constructor."
        //
        // Only report errors when the constructor tearoff feature is enabled,
        // to avoid reporting redundant errors.
        let mut enclosing_element = rv.base_element(named_type);
        if let Some(alias) = enclosing_element.and_then(|e| e.cast::<TypeAliasElement>()) {
            let aliased_type = ctx.get(alias).aliased_type.get().unwrap_or(TypeId::INVALID);
            enclosing_element = ctx.interface_element(aliased_type).map(|e| e.raw());
        }
        if let Some(interface) = enclosing_element.and_then(|e| e.cast::<InterfaceElement>()) {
            let name_str = rv.lexeme(rv.ast[name].token).to_string();
            let instance = interface.upcast::<InstanceElement>();
            let method = lookup::get_method(&ctx, instance, &name_str)
                .map(|m| m.raw())
                .or_else(|| lookup::get_getter(&ctx, instance, &name_str).map(|g| g.raw()))
                .or_else(|| lookup::get_setter(&ctx, instance, &name_str).map(|s| s.raw()));
            if let Some(method) = method {
                let d = if member::is_static(&ctx, ElemRef::Base(method)) {
                    diag::class_instantiation_access_to_static_member(&name_str)
                } else {
                    diag::class_instantiation_access_to_instance_member(&name_str)
                };
                let d = rv.at(d, node);
                rv.report(d);
            } else if !crate::ast_ext::token_is_synthetic(rv.ast, rv.ast[name].token) {
                let class_name = ctx.element_name(interface.raw()).unwrap_or("").to_string();
                let d = diag::class_instantiation_access_to_unknown_member(&class_name, &name_str);
                let d = rv.at(d, node);
                rv.report(d);
            }
        }
    }
    infer_argument_types(rv, node, context_type);
}

/// Dart `_inferArgumentTypes(node, contextType:)`.
fn infer_argument_types(
    rv: &mut ResolverVisitor<'_>,
    node: Id<ConstructorReference>,
    context_type: TypeId,
) {
    let ctx = rv.ctx;
    let constructor_name = rv.ast[node].constructor_name;
    let named_type = rv.ast[constructor_name].type_;
    let name = rv.ast[constructor_name].name;
    let name_str = name.map(|n| rv.lexeme(rv.ast[n].token).to_string());
    let element_to_infer =
        constructor_element_to_infer(rv, rv.base_element(named_type), name_str.as_deref());

    // If the constructor is generic, we'll have a
    // SubstitutedConstructorElementImpl that substitutes in type arguments
    // (possibly `dynamic`) from earlier in resolution.
    //
    // Otherwise we'll have a ConstructorElement, and we can skip inference
    // because there's nothing to infer in a non-generic type.
    if let Some(element_to_infer) = element_to_infer
        && !element_to_infer.type_parameters.is_empty()
        && rv.ast[named_type].type_arguments.is_none()
    {
        // Get back to the uninstantiated generic constructor.
        let raw_element = member::base_element(&ctx, element_to_infer.element);
        let constructor_type = element_to_infer.as_type(rv);
        let inferred = match name {
            Some(name) => infer_tear_off(rv, node.upcast(), name, constructor_type, context_type),
            // Dart: `constructorName.name!` (a constructor reference always
            // has a name).
            None => constructor_type,
        };
        if let TypeKind::Function(f) = *ctx.ty(inferred) {
            let inferred_return_type = f.ret;
            // Update the static element as well. This is used in some cases,
            // such as computing constant values. It is stored in two places.
            let constructor_element = match ctx.ty(inferred_return_type) {
                TypeKind::Interface { .. } => {
                    member::constructor_from2(&ctx, raw_element, inferred_return_type)
                }
                _ => ElemRef::Base(raw_element),
            };
            let base = ElemRef::Base(member::base_element(&ctx, constructor_element));
            rv.set_element(constructor_name, Some(base));
            if let Some(name) = name {
                rv.set_element(name, Some(base));
            }
            rv.record_static_type(node, inferred);
            // The NamedType child of `constructorName` doesn't have a static
            // type.
            rv.tables.annotation_type.remove(named_type);
        }
    } else {
        let ty = match rv.element(constructor_name) {
            None => TypeId::INVALID,
            Some(constructor_element) => member::type_(&ctx, constructor_element),
        };
        rv.record_static_type(node, ty);
        // The NamedType child of `constructorName` doesn't have a static type.
        rv.tables.annotation_type.remove(named_type);
    }
}
