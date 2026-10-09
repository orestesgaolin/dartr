// Dart source: pkg/analyzer/lib/src/dart/resolver/annotation_resolver.dart

//! Resolution of metadata annotations.

use dartr_ast::{Annotation, ArgumentList, Id, PrefixedIdentifier, SimpleIdentifier};
use dartr_diagnostics::diag;
use dartr_element::{
    EId, ElemRef, ElementId, FragmentFlags, InterfaceElement, PrefixElement,
    PropertyAccessorElement, Tag, TypeAliasElement, TypeId, TypeKind, TypeParameterElement,
    VariableElement,
};
use dartr_typesystem::{TypeExt, lookup, member};

use crate::ast_ext::identifier_name;
use crate::element_ext;
use crate::invocation_inference_helper::ConstructorElementToInfer;
use crate::invocation_inferrer::{
    InferrerKind, InvocationInferrer, InvocationTarget, function_type_parameters,
    record_corresponding_parameters, resolve_arguments_to_parameters,
};
use crate::resolver::ResolverVisitor;

/// Dart `ResolverVisitor.visitAnnotation(node)` =
/// `AnnotationResolver.resolve(node, whyNotPromotedArguments)`.
pub fn visit_annotation(rv: &mut ResolverVisitor<'_>, node: Id<Annotation>) {
    rv.with_flow_analysis(node.raw(), |rv| {
        if let Some(type_arguments) = rv.ast[node].type_arguments {
            rv.visit_node(type_arguments.raw());
        }
        resolve(rv, node);
        if let Some(arguments) = rv.ast[node].arguments {
            rv.check_for_argument_types_not_assignable_in_list(arguments);
        }
    });
}

/// Dart `AnnotationResolver._resolve`.
fn resolve(rv: &mut ResolverVisitor<'_>, node: Id<Annotation>) {
    let name_node = rv.ast[node].name;
    let (name1, name2, name3) = match rv.ast.cast::<PrefixedIdentifier>(name_node.raw()) {
        Some(name) => (
            rv.ast[name].prefix,
            Some(rv.ast[name].identifier),
            rv.ast[node].constructor_name,
        ),
        None => (
            rv.ast
                .cast::<SimpleIdentifier>(name_node.raw())
                .expect("annotation name is an identifier"),
            rv.ast[node].constructor_name,
            None,
        ),
    };
    let argument_list = rv.ast[node].arguments;

    let element1 = rv
        .rt
        .scope_lookup_result
        .get(name1)
        .and_then(|result| result.getter);
    set_identifier_element(rv, name1, element1.map(ElemRef::Base));

    let Some(element1) = element1 else {
        let name = identifier_name(rv.ast, name1).to_string();
        let diagnostic = rv.at(diag::undefined_annotation(&name), node);
        rv.report(diagnostic);
        visit_arguments(rv, node, argument_list);
        return;
    };

    // Class(args) or Class.CONST.
    if let Some(class_element) = element1.cast::<InterfaceElement>() {
        if let Some(argument_list) = argument_list {
            class_constructor_invocation(rv, node, class_element, name2, argument_list);
        } else {
            class_getter(rv, node, class_element, name2);
        }
        return;
    }

    // Extension.CONST.
    if element1.tag() == Tag::Extension {
        extension_getter(rv, node, element1, name2);
        return;
    }

    // prefix.*
    if let Some(prefix) = element1.cast::<PrefixElement>()
        && let Some(name2) = name2
    {
        let name = identifier_name(rv.ast, name2).to_string();
        let element = rv.unit.scopes.prefix_lookup(&rv.ctx, prefix, &name).getter;
        set_identifier_element(rv, name2, element.map(ElemRef::Base));

        if let Some(element) = element {
            // prefix.Class(args) or prefix.Class.CONST. The Dart resolver
            // accepts constructor invocation syntax here only for classes.
            if let Some(interface) = element.cast::<InterfaceElement>() {
                if element.tag() == Tag::Class
                    && let Some(argument_list) = argument_list
                {
                    class_constructor_invocation(rv, node, interface, name3, argument_list);
                } else {
                    class_getter(rv, node, interface, name3);
                }
                return;
            }

            // prefix.Extension.CONST.
            if element.tag() == Tag::Extension {
                extension_getter(rv, node, element, name3);
                return;
            }

            // prefix.CONST.
            if element.is::<PropertyAccessorElement>() {
                property_accessor_element(rv, node, name2, ElemRef::Base(element));
                return;
            }

            // prefix.TypeAlias(args) or prefix.TypeAlias.CONST.
            if let Some(alias) = element.cast::<TypeAliasElement>() {
                type_alias(rv, node, alias, name3, argument_list);
                return;
            }
        } else {
            let diagnostic = rv.at(diag::undefined_annotation(&name), node);
            rv.report(diagnostic);
            visit_arguments(rv, node, argument_list);
            return;
        }
    }

    // CONST.
    if element1.is::<PropertyAccessorElement>() {
        property_accessor_element(rv, node, name1, ElemRef::Base(element1));
        return;
    }

    // TypeAlias(args) or TypeAlias.CONST.
    if let Some(alias) = element1.cast::<TypeAliasElement>() {
        type_alias(rv, node, alias, name2, argument_list);
        return;
    }

    if element1.is::<VariableElement>() {
        local_variable(rv, node, element1, argument_list);
        return;
    }

    report_invalid_annotation(rv, node);
    visit_arguments(rv, node, argument_list);
}

/// Dart `_classConstructorInvocation`.
fn class_constructor_invocation(
    rv: &mut ResolverVisitor<'_>,
    node: Id<Annotation>,
    class_element: EId<InterfaceElement>,
    constructor_name: Option<Id<SimpleIdentifier>>,
    argument_list: Id<ArgumentList>,
) {
    let name = constructor_name.map(|name| identifier_name(rv.ast, name).to_string());
    let constructor =
        lookup::get_named_constructor(&rv.ctx, class_element, name.as_deref().unwrap_or("new"))
            .map(|element| ElemRef::Base(element.raw()));
    constructor_invocation(
        rv,
        node,
        constructor_name,
        rv.ctx.interface_type_parameters(class_element).to_vec(),
        constructor,
        argument_list,
    );
}

/// Dart `_classGetter`.
fn class_getter(
    rv: &mut ResolverVisitor<'_>,
    node: Id<Annotation>,
    class_element: EId<InterfaceElement>,
    getter_name: Option<Id<SimpleIdentifier>>,
) {
    let getter = match getter_name {
        Some(name) => {
            let name_text = identifier_name(rv.ast, name).to_string();
            lookup::get_getter(&rv.ctx, class_element.upcast(), &name_text)
                .map(|element| ElemRef::Base(element.raw()))
                .or_else(|| {
                    lookup::get_named_constructor(&rv.ctx, class_element, &name_text)
                        .map(|element| ElemRef::Base(element.raw()))
                })
        }
        None => lookup::get_named_constructor(&rv.ctx, class_element, "new")
            .map(|element| ElemRef::Base(element.raw())),
    };

    if let Some(name) = getter_name {
        set_identifier_element(rv, name, getter);
    }
    rv.set_element(node, getter);

    if let (Some(name), Some(getter)) = (getter_name, getter)
        && member::base_element(&rv.ctx, getter).is::<PropertyAccessorElement>()
    {
        property_accessor_element(rv, node, name, getter);
        resolve_annotation_element_getter(rv, node, getter);
    } else if getter.is_none_or(|e| member::base_element(&rv.ctx, e).tag() != Tag::Constructor) {
        report_invalid_annotation(rv, node);
    }
    visit_arguments(rv, node, rv.ast[node].arguments);
}

/// Dart `_extensionGetter`.
fn extension_getter(
    rv: &mut ResolverVisitor<'_>,
    node: Id<Annotation>,
    extension: ElementId,
    getter_name: Option<Id<SimpleIdentifier>>,
) {
    let getter = getter_name.and_then(|name| {
        let text = identifier_name(rv.ast, name).to_string();
        lookup::get_getter(
            &rv.ctx,
            extension.cast().expect("extension instance element"),
            &text,
        )
        .map(|element| ElemRef::Base(element.raw()))
    });
    if let Some(name) = getter_name {
        set_identifier_element(rv, name, getter);
    }
    rv.set_element(node, getter);

    if let (Some(name), Some(getter)) = (getter_name, getter) {
        property_accessor_element(rv, node, name, getter);
        resolve_annotation_element_getter(rv, node, getter);
    } else {
        report_invalid_annotation(rv, node);
    }
    visit_arguments(rv, node, rv.ast[node].arguments);
}

/// Dart `_localVariable`.
fn local_variable(
    rv: &mut ResolverVisitor<'_>,
    node: Id<Annotation>,
    element: ElementId,
    argument_list: Option<Id<ArgumentList>>,
) {
    if !element_ext::is_const(&rv.ctx, element) || argument_list.is_some() {
        report_invalid_annotation(rv, node);
    }
    visit_arguments(rv, node, argument_list);
}

/// Dart `_propertyAccessorElement`.
fn property_accessor_element(
    rv: &mut ResolverVisitor<'_>,
    node: Id<Annotation>,
    name: Id<SimpleIdentifier>,
    element: ElemRef,
) {
    set_identifier_element(rv, name, Some(element));
    rv.set_element(node, Some(element));
    resolve_annotation_element_getter(rv, node, element);
    visit_arguments(rv, node, rv.ast[node].arguments);
}

/// Dart `_resolveAnnotationElementGetter`.
fn resolve_annotation_element_getter(
    rv: &mut ResolverVisitor<'_>,
    annotation: Id<Annotation>,
    accessor: ElemRef,
) {
    let base = member::base_element(&rv.ctx, accessor);
    let is_synthetic = element_ext::first_fragment_flags(&rv.ctx, base)
        .contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE);
    let is_const = member::variable(&rv.ctx, accessor).is_some_and(|variable| {
        element_ext::is_const(&rv.ctx, member::base_element(&rv.ctx, variable))
    });
    if !is_synthetic || !is_const || rv.ast[annotation].arguments.is_some() {
        report_invalid_annotation(rv, annotation);
    }
}

fn type_alias(
    rv: &mut ResolverVisitor<'_>,
    node: Id<Annotation>,
    alias: EId<TypeAliasElement>,
    name: Option<Id<SimpleIdentifier>>,
    argument_list: Option<Id<ArgumentList>>,
) {
    let aliased_type = rv
        .ctx
        .get(alias)
        .aliased_type
        .get()
        .unwrap_or(TypeId::INVALID);
    if matches!(rv.ctx.ty(aliased_type), TypeKind::Interface { .. }) {
        if let Some(argument_list) = argument_list {
            type_alias_constructor_invocation(rv, node, alias, name, aliased_type, argument_list);
        } else {
            type_alias_getter(rv, node, name, aliased_type);
        }
    } else {
        type_alias_getter(rv, node, name, aliased_type);
    }
}

/// Dart `_typeAliasConstructorInvocation`.
fn type_alias_constructor_invocation(
    rv: &mut ResolverVisitor<'_>,
    node: Id<Annotation>,
    alias: EId<TypeAliasElement>,
    constructor_name: Option<Id<SimpleIdentifier>>,
    aliased_type: TypeId,
    argument_list: Id<ArgumentList>,
) {
    let name = constructor_name.map(|name| identifier_name(rv.ast, name).to_string());
    let constructor =
        lookup::type_look_up_constructor(&rv.ctx, aliased_type, name.as_deref(), rv.unit.library);
    constructor_invocation(
        rv,
        node,
        constructor_name,
        rv.ctx.get(alias).type_params.clone(),
        constructor,
        argument_list,
    );
}

/// Dart `_typeAliasGetter`.
fn type_alias_getter(
    rv: &mut ResolverVisitor<'_>,
    node: Id<Annotation>,
    getter_name: Option<Id<SimpleIdentifier>>,
    aliased_type: TypeId,
) {
    let getter = match (rv.ctx.interface_element(aliased_type), getter_name) {
        (Some(class_element), Some(name)) => {
            let text = identifier_name(rv.ast, name).to_string();
            lookup::get_getter(&rv.ctx, class_element.upcast(), &text)
                .map(|element| ElemRef::Base(element.raw()))
        }
        _ => None,
    };
    if let Some(name) = getter_name {
        set_identifier_element(rv, name, getter);
    }
    rv.set_element(node, getter);

    if let (Some(name), Some(getter)) = (getter_name, getter) {
        property_accessor_element(rv, node, name, getter);
        resolve_annotation_element_getter(rv, node, getter);
    } else if getter.is_none_or(|e| member::base_element(&rv.ctx, e).tag() != Tag::Constructor) {
        report_invalid_annotation(rv, node);
    }
    visit_arguments(rv, node, rv.ast[node].arguments);
}

/// Dart `_constructorInvocation`.
fn constructor_invocation(
    rv: &mut ResolverVisitor<'_>,
    node: Id<Annotation>,
    constructor_name: Option<Id<SimpleIdentifier>>,
    type_parameters: Vec<EId<TypeParameterElement>>,
    constructor: Option<ElemRef>,
    argument_list: Id<ArgumentList>,
) {
    if let Some(name) = constructor_name {
        set_identifier_element(rv, name, constructor);
    }
    rv.set_element(node, constructor);

    let Some(constructor) = constructor else {
        report_invalid_annotation(rv, node);
        resolve_annotation_invocation(rv, node, constructor_name, argument_list, None);
        return;
    };

    let element_to_infer = ConstructorElementToInfer {
        type_parameters,
        element: constructor,
    };
    let constructor_raw_type = element_to_infer.as_type(rv);
    resolve_annotation_invocation(
        rv,
        node,
        constructor_name,
        argument_list,
        Some(InvocationTarget::ConstructorElement {
            element: constructor,
            raw_type: constructor_raw_type,
        }),
    );
}

/// Dart `_visitArguments` for an unresolved annotation target.
fn visit_arguments(
    rv: &mut ResolverVisitor<'_>,
    node: Id<Annotation>,
    argument_list: Option<Id<ArgumentList>>,
) {
    let Some(argument_list) = argument_list else {
        return;
    };
    resolve_annotation_invocation(rv, node, None, argument_list, None);
}

/// Dart `AnnotationInferrer(...).resolveInvocation()`.
fn resolve_annotation_invocation(
    rv: &mut ResolverVisitor<'_>,
    node: Id<Annotation>,
    constructor_name: Option<Id<SimpleIdentifier>>,
    argument_list: Id<ArgumentList>,
    target: Option<InvocationTarget>,
) {
    let has_target = target.is_some();
    InvocationInferrer {
        kind: InferrerKind::Annotation {
            node,
            constructor_name,
        },
        argument_list,
        context_type: TypeId::UNKNOWN,
        target,
    }
    .resolve_invocation(rv);

    if has_target && let Some(constructor) = rv.element(node) {
        if let Some(name) = constructor_name {
            // `AnnotationInferrer._storeResult` updates the constructor
            // identifier. Keep the enclosing `PrefixedIdentifier` result in
            // sync for inferred `@A.named(...)` metadata.
            set_identifier_element(rv, name, Some(constructor));
        }
        // Dart `AnnotationInferrer._storeResult` returns the substituted
        // constructor's formal parameter elements. The shared Rust inferrer
        // carries the substituted parameter types but retains the raw
        // `FnParam.element`, so attach the substituted elements before
        // recording `Expression.correspondingParameter`.
        let mut parameters =
            function_type_parameters(rv, Some(member::type_(&rv.ctx, constructor)));
        for (parameter, element) in parameters
            .iter_mut()
            .zip(member::formal_parameters(&rv.ctx, constructor))
        {
            parameter.element = Some(element);
        }
        let corresponding = resolve_arguments_to_parameters(rv, argument_list, &parameters, false);
        record_corresponding_parameters(rv, argument_list, &corresponding);
    }
}

fn set_identifier_element(
    rv: &mut ResolverVisitor<'_>,
    identifier: Id<SimpleIdentifier>,
    element: Option<ElemRef>,
) {
    rv.set_element(identifier, element);
    if let Some(parent) = rv.ast.parent(identifier)
        && rv.ast.is::<PrefixedIdentifier>(parent)
    {
        rv.set_element(parent, element);
    }
}

fn report_invalid_annotation(rv: &mut ResolverVisitor<'_>, node: Id<Annotation>) {
    let diagnostic = rv.at(diag::invalid_annotation(), node);
    rv.report(diagnostic);
}
