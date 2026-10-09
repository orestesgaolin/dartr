// Dart source: pkg/analyzer/lib/src/generated/element_resolver.dart

//! `ElementResolver`: the elements of the nodes that are not expressions
//! (constructor names, `super(...)` / `this(...)` constructor invocations,
//! combinators of imports and exports, ...). The Dart methods that do
//! nothing are not ported; their call sites in the resolver are comments.
//!
//! Partly STUB (unit C2): `visitConstructorName`,
//! `visitSuperConstructorInvocation`, `visitRedirectingConstructorInvocation`,
//! `visitImportDirective`, `visitExportDirective` (combinators) and
//! `visitCommentReference` are ported with the units that own those nodes
//! (C8 constructors, C9 comment references).

use dartr_ast::{
    ConstructorDeclaration, ConstructorName, DotShorthandConstructorInvocation, Id,
    InstanceCreationExpression, RedirectingConstructorInvocation, SuperConstructorInvocation,
};
use dartr_diagnostics::diag;
use dartr_element::diagnostics::{element_arg, type_arg};
use dartr_element::{ElemRef, FragmentFlags, TypeKind};
use dartr_typesystem::{TypeExt, lookup, member};

use crate::constructor_invocation_inferrer::resolve_arguments_to_parameters;
use crate::resolver::ResolverVisitor;

/// Dart `ConstructorElement.isFactory` of [element].
pub fn is_factory(rv: &ResolverVisitor<'_>, element: ElemRef) -> bool {
    let base = member::base_element(&rv.ctx, element);
    crate::element_ext::first_fragment_flags(&rv.ctx, base)
        .contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY)
}

/// Dart `ElementResolver.visitConstructorName(node)`.
pub fn visit_constructor_name(rv: &mut ResolverVisitor<'_>, node: Id<ConstructorName>) {
    let named_type = rv.ast[node].type_;
    let Some(&ty) = rv.tables.annotation_type.get(named_type) else {
        return;
    };
    if let TypeKind::Interface { .. } = rv.ctx.ty(ty) {
        // look up ConstructorElement
        let library = rv.unit.library;
        let constructor = match rv.ast[node].name {
            None => lookup::type_look_up_constructor(&rv.ctx, ty, None, library),
            Some(name) => {
                let lexeme = rv.lexeme(rv.ast[name].token).to_string();
                let constructor =
                    lookup::type_look_up_constructor(&rv.ctx, ty, Some(&lexeme), library);
                rv.set_element(name, constructor);
                constructor
            }
        };
        rv.set_element(node, constructor);
    }
}

/// Dart `ElementResolver.visitInstanceCreationExpression(node)`.
pub fn visit_instance_creation_expression(
    rv: &mut ResolverVisitor<'_>,
    node: Id<InstanceCreationExpression>,
) {
    let constructor_name = rv.ast[node].constructor_name;
    let invoked_constructor = rv.element(constructor_name);
    let argument_list = rv.ast[node].argument_list;
    resolve_arguments_to_function(rv, argument_list, invoked_constructor, None);
}

/// Dart `ElementResolver.visitDotShorthandConstructorInvocation(node)`.
pub fn visit_dot_shorthand_constructor_invocation(
    rv: &mut ResolverVisitor<'_>,
    node: Id<DotShorthandConstructorInvocation>,
) {
    let invoked_constructor = rv.element(node);
    let argument_list = rv.ast[node].argument_list;
    resolve_arguments_to_function(rv, argument_list, invoked_constructor, None);
}

/// Dart `ElementResolver.visitRedirectingConstructorInvocation(node)`.
pub fn visit_redirecting_constructor_invocation(
    rv: &mut ResolverVisitor<'_>,
    node: Id<RedirectingConstructorInvocation>,
) {
    let Some(enclosing_class) = rv.enclosing_class else {
        return;
    };
    let name = rv.ast[node].constructor_name;
    let lexeme = match name {
        None => "new".to_string(),
        Some(name) => rv.lexeme(rv.ast[name].token).to_string(),
    };
    let Some(element) = lookup::get_named_constructor(&rv.ctx, enclosing_class, &lexeme) else {
        return;
    };
    let element = ElemRef::Base(element.raw());
    if let Some(name) = name {
        rv.set_element(name, Some(element));
    }
    rv.set_element(node, Some(element));
    let argument_list = rv.ast[node].argument_list;
    resolve_arguments_to_function(rv, argument_list, Some(element), None);
}

/// Dart `ElementResolver.visitSuperConstructorInvocation(node)`.
pub fn visit_super_constructor_invocation(
    rv: &mut ResolverVisitor<'_>,
    node: Id<SuperConstructorInvocation>,
) {
    let Some(enclosing_class) = rv.enclosing_class else {
        return;
    };
    let Some(super_type) = rv.ctx.interface(enclosing_class).supertype.get() else {
        return;
    };
    if !matches!(rv.ctx.ty(super_type), TypeKind::Interface { .. }) {
        return;
    }
    let library = rv.unit.library;
    let name = rv.ast[node].constructor_name;
    let super_name = name.map(|n| rv.lexeme(rv.ast[n].token).to_string());
    let element =
        lookup::type_look_up_constructor(&rv.ctx, super_type, super_name.as_deref(), library)
            .filter(|&e| member::is_accessible_in(&rv.ctx, e, library));
    let Some(element) = element else {
        let d = match &super_name {
            Some(super_name) => diag::undefined_constructor_in_initializer(
                type_arg(&rv.ctx, super_type),
                super_name,
            ),
            None => {
                let class_name = rv
                    .ctx
                    .interface_element(super_type)
                    .and_then(|e| rv.ctx.element_name(e.raw()))
                    .unwrap_or("<unknown>")
                    .to_string();
                diag::undefined_constructor_in_initializer_default(&class_name)
            }
        };
        let d = rv.at(d, node);
        rv.report(d);
        return;
    };
    if is_factory(rv, element) {
        // Check if we've reported [NO_GENERATIVE_CONSTRUCTORS_IN_SUPERCLASS].
        let all_factories = member::enclosing_interface(&rv.ctx, element).is_none_or(|class| {
            rv.ctx
                .interface(class)
                .constructors
                .iter()
                .all(|&c| is_factory(rv, ElemRef::Base(c.raw())))
        });
        if !all_factories {
            let base = member::base_element(&rv.ctx, element);
            let d = rv.at(
                diag::non_generative_constructor(element_arg(&rv.ctx, base)),
                node,
            );
            rv.report(d);
        }
    }
    if let Some(name) = name {
        rv.set_element(name, Some(element));
    }
    rv.set_element(node, Some(element));
    // Dart: if the extended type is an undefined name that the library
    // fragment ignores (`shouldIgnoreUndefinedNamedType`), the arguments are
    // not resolved to parameters. Not ported (it needs the ignored
    // undefined names of the library fragment).
    let argument_list = rv.ast[node].argument_list;
    let enclosing_list = rv
        .ast
        .parent(node)
        .and_then(|p| rv.ast.cast::<ConstructorDeclaration>(p))
        .map(|c| rv.ast[c].parameters);
    resolve_arguments_to_function(rv, argument_list, Some(element), enclosing_list);
}

/// Dart `ElementResolver._resolveArgumentsToFunction`.
fn resolve_arguments_to_function(
    rv: &mut ResolverVisitor<'_>,
    argument_list: Id<dartr_ast::ArgumentList>,
    executable_element: Option<ElemRef>,
    enclosing_constructor_formal_parameter_list: Option<Id<dartr_ast::FormalParameterList>>,
) {
    let Some(executable_element) = executable_element else {
        return;
    };
    let parameters = member::formal_parameters(&rv.ctx, executable_element);
    resolve_arguments_to_parameters(
        rv,
        argument_list,
        &parameters,
        true,
        enclosing_constructor_formal_parameter_list,
    );
}
