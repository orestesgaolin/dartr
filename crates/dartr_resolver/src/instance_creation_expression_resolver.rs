// Dart source: pkg/analyzer/lib/src/dart/resolver/instance_creation_expression_resolver.dart

//! `InstanceCreationExpressionResolver`: instance creation expressions
//! (`new C<int>.named(...)`, `const C(...)`, `C(...)`) and dot shorthand
//! constructor invocations (`.new(...)`, `.named(...)`), with the inference
//! of the type arguments of the constructed type from the context and the
//! arguments.
//!
//! Also here, because they share the constructor inference:
//! - Dart `InvocationInferenceHelper.constructorElementToInfer` and
//!   `ConstructorElementToInfer` (invocation_inference_helper.dart),
//! - the `InstanceCreationInferrer` and
//!   `DotShorthandConstructorInvocationInferrer` subclasses
//!   (invocation_inferrer.dart),
//! - Dart `ResolverVisitor.visitSuperConstructorInvocation`,
//!   `visitRedirectingConstructorInvocation` and the constructor part of
//!   `visitEnumConstantDeclaration` (resolver.dart).

use dartr_ast::{
    ConstructorName, DotShorthandConstructorInvocation, EnumConstantDeclaration, Id,
    InstanceCreationExpression, RedirectingConstructorInvocation, SuperConstructorInvocation,
};
use dartr_diagnostics::diag;
use dartr_element::{
    EId, ElemRef, ElementId, EnumElement, InterfaceElement, Nullability, Tag, TypeAliasElement,
    TypeId, TypeKind, TypeParameterElement,
};
use dartr_flow::type_analyzer::TypeAnalyzer;
use dartr_typesystem::generic_inferrer::{InferenceErrorEntity, InferenceErrorEntityKind};
use dartr_typesystem::{TypeExt, lookup, member};

use crate::ast_ext::instance_creation_is_const;
use crate::constructor_invocation_inferrer::{
    FullInvocation, report_not_enough_positional_arguments, resolve_full_invocation,
    resolve_invocation,
};
use crate::element_resolver;
use crate::resolver::ResolverVisitor;

/// Dart `ConstructorElementToInfer`: the constructor to instantiate and the
/// type parameters to infer.
#[derive(Clone, Debug)]
pub struct ConstructorElementToInfer {
    /// Dart `typeParameters`: the type parameters of the class, or of the
    /// type alias.
    pub type_parameters: Vec<EId<TypeParameterElement>>,
    /// Dart `element`: a raw constructor of the class, or a substituted
    /// constructor of the aliased class.
    pub element: ElemRef,
}

impl ConstructorElementToInfer {
    /// Dart `asType`: the generic function type that forwards to the
    /// constructor (`<T>(T) -> C<T>` for `class C<T> { C(T arg); }`), or the
    /// constructor type for a non-generic type.
    pub fn as_type(&self, rv: &ResolverVisitor<'_>) -> TypeId {
        let ctx = rv.ctx;
        let ty = member::type_(&ctx, self.element);
        if self.type_parameters.is_empty() {
            return ty;
        }
        let TypeKind::Function(f) = *ctx.ty(ty) else {
            return ty;
        };
        ctx.function_type(
            &self.type_parameters,
            ctx.list(f.params),
            f.ret,
            Nullability::None,
            None,
        )
    }
}

/// Dart `InvocationInferenceHelper.constructorElementToInfer(typeElement:,
/// constructorName:, definingLibrary:)`.
pub fn constructor_element_to_infer(
    rv: &ResolverVisitor<'_>,
    type_element: Option<ElementId>,
    constructor_name: Option<&str>,
) -> Option<ConstructorElementToInfer> {
    let ctx = rv.ctx;
    let library = rv.unit.library;
    let type_element = type_element?;
    let type_parameters;
    let raw_element;
    if let Some(interface) = type_element.cast::<InterfaceElement>() {
        type_parameters = ctx.interface_type_parameters(interface).to_vec();
        raw_element = match constructor_name {
            None => lookup::get_named_constructor(&ctx, interface, "new")
                .map(|c| ElemRef::Base(c.raw())),
            Some(name) => lookup::get_named_constructor(&ctx, interface, name)
                .map(|c| ElemRef::Base(c.raw()))
                .filter(|&c| member::is_accessible_in(&ctx, c, library)),
        };
    } else {
        let alias = type_element.cast::<TypeAliasElement>()?;
        let data = ctx.get(alias);
        type_parameters = data.type_params.clone();
        let aliased_type = data.aliased_type.get().unwrap_or(TypeId::INVALID);
        raw_element = match ctx.ty(aliased_type) {
            TypeKind::Interface { .. } => {
                lookup::type_look_up_constructor(&ctx, aliased_type, constructor_name, library)
            }
            _ => None,
        };
    }
    Some(ConstructorElementToInfer {
        type_parameters,
        element: raw_element?,
    })
}

/// Dart `ResolverVisitor.visitInstanceCreationExpression(node, contextType:)`.
pub fn visit_instance_creation_expression(
    rv: &mut ResolverVisitor<'_>,
    node: Id<InstanceCreationExpression>,
    context_type: TypeId,
) {
    rv.check_unreachable_node(node);
    resolve(rv, node, context_type);
    rv.insert_implicit_call_reference(node.upcast(), context_type);
}

/// Dart `InstanceCreationExpressionResolver.resolve(node, contextType:)`.
///
/// The parser can parse `a.m<int>.apply()` as an instance creation
/// expression when `a.m<int>` is a function reference or constructor
/// reference; Dart does not rewrite it yet either
/// (`_resolveWithTypeNameWithTypeArguments`).
pub fn resolve(
    rv: &mut ResolverVisitor<'_>,
    node: Id<InstanceCreationExpression>,
    context_type: TypeId,
) {
    resolve_instance_creation_expression(rv, node, context_type);
}

/// The `element` of the named type of a constructor name (Dart
/// `constructorName.type.element`).
fn named_type_element(
    rv: &ResolverVisitor<'_>,
    constructor_name: Id<ConstructorName>,
) -> Option<ElementId> {
    let named_type = rv.ast[constructor_name].type_;
    rv.base_element(named_type)
}

/// The inference error entity of a constructor name (Dart
/// `_errorEntity => node.constructorName`).
fn constructor_name_error_entity(
    rv: &ResolverVisitor<'_>,
    constructor_name: Id<ConstructorName>,
) -> InferenceErrorEntity {
    let named_type = rv.ast[constructor_name].type_;
    let name = match rv.ast[constructor_name].name {
        None => {
            let mut qualified = String::new();
            if let Some(prefix) = rv.ast[named_type].import_prefix {
                qualified.push_str(rv.lexeme(rv.ast[prefix].name));
                qualified.push('.');
            }
            qualified.push_str(rv.lexeme(rv.ast[named_type].name));
            qualified
        }
        Some(name) => format!(
            "{}.{}",
            dartr_ast::to_source::to_source(rv.ast, named_type),
            rv.lexeme(rv.ast[name].token)
        ),
    };
    InferenceErrorEntity {
        offset: rv.ast.offset(constructor_name) as usize,
        length: rv.ast.length(constructor_name) as usize,
        is_invocation_in_as_expression: false,
        kind: InferenceErrorEntityKind::ConstructorName {
            // `metadata.hasOptionalTypeArgs` (package:meta): not tracked.
            type_element_has_optional_type_args: false,
            constructor_name: name,
        },
    }
}

/// Dart `_resolveInstanceCreationExpression`.
fn resolve_instance_creation_expression(
    rv: &mut ResolverVisitor<'_>,
    node: Id<InstanceCreationExpression>,
    context_type: TypeId,
) {
    let constructor_name = rv.ast[node].constructor_name;
    rv.visit_node(constructor_name.raw());
    // Re-assign constructorName in case the node got replaced.
    let constructor_name = rv.ast[node].constructor_name;
    element_resolver::visit_instance_creation_expression(rv, node);
    let name = rv.ast[constructor_name]
        .name
        .map(|n| rv.lexeme(rv.ast[n].token).to_string());
    let element_to_infer = constructor_element_to_infer(
        rv,
        named_type_element(rv, constructor_name),
        name.as_deref(),
    );
    let raw_type = element_to_infer.as_ref().map(|e| e.as_type(rv));
    let named_type = rv.ast[constructor_name].type_;
    let inv = FullInvocation {
        node: node.raw(),
        argument_list: rv.ast[node].argument_list,
        context_type,
        raw_type,
        // For an instance creation expression the type arguments are on
        // the constructor name.
        type_arguments: rv.ast[named_type].type_arguments,
        error_entity: constructor_name_error_entity(rv, constructor_name),
        is_const: instance_creation_is_const(rv.ast, node),
        is_generic_inference_disabled: false,
        needs_type_argument_bounds_check: true,
        // Error reporting for instance creations is done elsewhere.
        wrong_number_of_type_arguments: None,
    };
    resolve_full_invocation(rv, inv, &mut |rv, _type_argument_types, invoke_type| {
        let constructed_type = invoke_return_type(rv, invoke_type?)?;
        rv.tables
            .annotation_type
            .insert(named_type, constructed_type);
        let base = member::base_element(&rv.ctx, rv.element(constructor_name)?);
        let constructor_element = constructed_element(rv, base, constructed_type);
        rv.set_element(constructor_name, Some(constructor_element));
        Some(member::formal_parameters(&rv.ctx, constructor_element))
    });
    let ty = rv
        .tables
        .annotation_type
        .get(named_type)
        .copied()
        .unwrap_or(TypeId::DYNAMIC);
    rv.record_static_type(node, ty);
    // Dart `checkForArgumentTypesNotAssignableInList` (wave D).
}

/// The return type of the function type [invoke_type].
fn invoke_return_type(rv: &ResolverVisitor<'_>, invoke_type: TypeId) -> Option<TypeId> {
    match *rv.ctx.ty(invoke_type) {
        TypeKind::Function(f) => Some(f.ret),
        _ => None,
    }
}

/// Dart `SubstitutedConstructorElementImpl.from2(baseElement,
/// constructedType as InterfaceType)`.
fn constructed_element(
    rv: &ResolverVisitor<'_>,
    base: ElementId,
    constructed_type: TypeId,
) -> ElemRef {
    match rv.ctx.ty(constructed_type) {
        TypeKind::Interface { .. } => member::constructor_from2(&rv.ctx, base, constructed_type),
        _ => ElemRef::Base(base),
    }
}

/// Dart `ResolverVisitor.visitDotShorthandConstructorInvocation(node,
/// contextType:)`.
pub fn visit_dot_shorthand_constructor_invocation(
    rv: &mut ResolverVisitor<'_>,
    node: Id<DotShorthandConstructorInvocation>,
    context_type: TypeId,
) {
    // If [isDotShorthand] is set, cache the context type for resolution.
    let is_dot_shorthand = rv.rt.is_dot_shorthand(node.raw());
    if is_dot_shorthand {
        rv.push_dot_shorthand_context(node.raw(), crate::resolver::SchemaOf::new(context_type));
    }
    resolve_dot_shorthand(rv, node, context_type);
    if is_dot_shorthand {
        rv.pop_dot_shorthand_context();
    }
}

/// Dart `InstanceCreationExpressionResolver.resolveDotShorthand(node,
/// contextType:)`.
pub fn resolve_dot_shorthand(
    rv: &mut ResolverVisitor<'_>,
    node: Id<DotShorthandConstructorInvocation>,
    context_type: TypeId,
) {
    let ctx = rv.ctx;
    let mut dot_shorthand_context_type = if rv.is_dot_shorthand_context_empty() {
        TypeId::UNKNOWN
    } else {
        rv.get_dot_shorthand_context().unwrap_type_schema_view()
    };
    // The static namespace denoted by `S` is also the namespace denoted by
    // `FutureOr<S>`.
    dot_shorthand_context_type = rv.type_system.future_or_base(dot_shorthand_context_type);

    let library = rv.unit.library;
    let context_element = ctx
        .interface_element(dot_shorthand_context_type)
        .filter(|&e| member::is_accessible_in(&ctx, ElemRef::Base(e.raw()), library));
    let constructor_name_node = rv.ast[node].constructor_name;
    let constructor_name = rv.lexeme(rv.ast[constructor_name_node].token).to_string();
    if let Some(context_element) = context_element {
        // This branch will be true if we're resolving an explicitly marked
        // const constructor invocation. It's completely unresolved, unlike a
        // rewritten [DotShorthandConstructorInvocation] that resulted from
        // resolving a [DotShorthandInvocation].
        if rv.element(node).is_none() {
            match lookup::get_named_constructor(&ctx, context_element, &constructor_name)
                .map(|c| ElemRef::Base(c.raw()))
                .filter(|&c| member::is_accessible_in(&ctx, c, library))
            {
                Some(element) => rv.set_element(node, Some(element)),
                None => {
                    let class_name = ctx
                        .element_name(context_element.raw())
                        .unwrap_or("")
                        .to_string();
                    let d = diag::const_with_undefined_constructor(&class_name, &constructor_name);
                    let d = rv.at(d, constructor_name_node);
                    rv.report(d);
                }
            }
        }

        let type_arguments = rv.ast[node].type_arguments;
        let constructor_element = rv.element(node);
        if context_element.raw().tag() == Tag::Class
            && is_abstract_class(rv, context_element.raw())
            && constructor_element.is_some_and(|c| !element_resolver::is_factory(rv, c))
        {
            let d = rv.at(diag::instantiate_abstract_class(), node);
            rv.report(d);
        } else if let Some(type_arguments) = type_arguments {
            let class_name = ctx
                .element_name(context_element.raw())
                .unwrap_or("")
                .to_string();
            let d = diag::wrong_number_of_type_arguments_dot_shorthand_constructor(
                &class_name,
                &constructor_name,
            );
            let d = rv.at(d, type_arguments);
            rv.report(d);
        }
    } else {
        let d = rv.at(diag::dot_shorthand_missing_context(), node);
        rv.report(d);
        // Prevents `constructorElementToInfer` (called by
        // `_resolveDotShorthandConstructorInvocation`) from considering the
        // context type to be valid.
        dot_shorthand_context_type = TypeId::INVALID;
    }

    resolve_dot_shorthand_constructor_invocation(
        rv,
        node,
        context_type,
        dot_shorthand_context_type,
    );
}

/// Dart `ClassElementImpl.isAbstract`.
fn is_abstract_class(rv: &ResolverVisitor<'_>, class: ElementId) -> bool {
    crate::element_ext::first_fragment_flags(&rv.ctx, class)
        .contains(dartr_element::FragmentFlags::CLASS_FRAGMENT_IS_ABSTRACT)
}

/// Dart `_resolveDotShorthandConstructorInvocation`.
fn resolve_dot_shorthand_constructor_invocation(
    rv: &mut ResolverVisitor<'_>,
    node: Id<DotShorthandConstructorInvocation>,
    context_type: TypeId,
    dot_shorthand_context_type: TypeId,
) {
    element_resolver::visit_dot_shorthand_constructor_invocation(rv, node);
    let constructor_name_node = rv.ast[node].constructor_name;
    let constructor_name = rv.lexeme(rv.ast[constructor_name_node].token).to_string();
    let type_element = rv.ctx.type_element(dot_shorthand_context_type);
    let element_to_infer = constructor_element_to_infer(rv, type_element, Some(&constructor_name));
    let raw_type = element_to_infer.as_ref().map(|e| e.as_type(rv));
    let is_const = rv.ast[node]
        .const_keyword
        .is_some_and(|k| rv.lexeme(k) == "const")
        || crate::ast_ext::in_constant_context(rv.ast, node.raw());
    let inv = FullInvocation {
        node: node.raw(),
        argument_list: rv.ast[node].argument_list,
        context_type,
        raw_type,
        type_arguments: rv.ast[node].type_arguments,
        error_entity: InferenceErrorEntity {
            offset: rv.ast.offset(constructor_name_node) as usize,
            length: rv.ast.length(constructor_name_node) as usize,
            is_invocation_in_as_expression: false,
            kind: InferenceErrorEntityKind::SimpleIdentifier {
                name: constructor_name.clone(),
                element: None,
            },
        },
        is_const,
        is_generic_inference_disabled: false,
        needs_type_argument_bounds_check: true,
        // Error reporting for dot shorthand constructor invocations is done
        // within the [InstanceCreationExpressionResolver].
        wrong_number_of_type_arguments: None,
    };
    let return_type =
        resolve_full_invocation(rv, inv, &mut |rv, _type_argument_types, invoke_type| {
            let constructed_type = invoke_return_type(rv, invoke_type?)?;
            let base = member::base_element(&rv.ctx, rv.element(node)?);
            let constructor_element = constructed_element(rv, base, constructed_type);
            rv.set_element(constructor_name_node, Some(constructor_element));
            Some(member::formal_parameters(&rv.ctx, constructor_element))
        });
    rv.record_static_type(node, return_type);
    // Dart `checkForArgumentTypesNotAssignableInList` (wave D).
}

/// Dart `ResolverVisitor.visitSuperConstructorInvocation(node)`.
pub fn visit_super_constructor_invocation(
    rv: &mut ResolverVisitor<'_>,
    node: Id<SuperConstructorInvocation>,
) {
    // We visit the argument list, but do not visit the optional identifier
    // because it needs to be visited in the context of the constructor
    // invocation.
    element_resolver::visit_super_constructor_invocation(rv, node);
    let raw_type = rv.element(node).map(|e| member::type_(&rv.ctx, e));
    let argument_list = rv.ast[node].argument_list;
    resolve_invocation(rv, node.raw(), argument_list, raw_type);
    // Dart `checkForArgumentTypesNotAssignableInList` (wave D).
}

/// Dart `ResolverVisitor.visitRedirectingConstructorInvocation(node)`.
pub fn visit_redirecting_constructor_invocation(
    rv: &mut ResolverVisitor<'_>,
    node: Id<RedirectingConstructorInvocation>,
) {
    // We visit the argument list, but do not visit the optional identifier
    // because it needs to be visited in the context of the constructor
    // invocation.
    element_resolver::visit_redirecting_constructor_invocation(rv, node);
    let raw_type = rv.element(node).map(|e| member::type_(&rv.ctx, e));
    let argument_list = rv.ast[node].argument_list;
    resolve_invocation(rv, node.raw(), argument_list, raw_type);
    // Dart `checkForArgumentTypesNotAssignableInList` (wave D).
}

/// Dart `ResolverVisitor.visitEnumConstantDeclaration(node)` (after the
/// documentation comment and the metadata).
///
/// Dart reads the constructor from the constant initializer of the enum
/// constant (an instance creation expression that the linker builds and
/// resolves). Here the same invocation is inferred directly: the
/// constructor of the enclosing enum, with the type arguments of the
/// arguments or inferred from the arguments, in an unknown context.
pub fn visit_enum_constant_declaration(
    rv: &mut ResolverVisitor<'_>,
    node: Id<EnumConstantDeclaration>,
) {
    rv.check_unreachable_node(node);
    let ctx = rv.ctx;
    let enum_element = rv.enclosing_class.filter(|e| e.raw().is::<EnumElement>());
    let arguments = rv.ast[node].arguments;
    let selector_name = arguments
        .and_then(|a| rv.ast[a].constructor_selector)
        .map(|s| rv.ast[s].name);
    let constructor_name = selector_name.map(|n| rv.lexeme(rv.ast[n].token).to_string());
    let element_to_infer = enum_element
        .and_then(|e| constructor_element_to_infer(rv, Some(e.raw()), constructor_name.as_deref()));
    let constructor_element = element_to_infer.as_ref().map(|e| e.element);

    match constructor_element {
        Some(constructor_element) => {
            rv.set_element(node, Some(constructor_element));
            if element_resolver::is_factory(rv, constructor_element) {
                let d = diag::enum_constant_invokes_factory_constructor();
                let d = match selector_name {
                    Some(name) => rv.at(d, name),
                    None => rv.at_token(d, rv.ast[node].name),
                };
                rv.report(d);
            }
        }
        None => {
            if enum_element.is_some() {
                match selector_name {
                    Some(name_node) => {
                        let name = rv.lexeme(rv.ast[name_node].token).to_string();
                        let d = rv.at(diag::undefined_enum_constructor_named(&name), name_node);
                        rv.report(d);
                    }
                    None => {
                        let d = rv.at_token(
                            diag::undefined_enum_constructor_unnamed(),
                            rv.ast[node].name,
                        );
                        rv.report(d);
                    }
                }
            }
        }
    }

    match arguments {
        Some(arguments) => {
            let argument_list = rv.ast[arguments].argument_list;
            let type_arguments = rv.ast[arguments].type_arguments;
            rv.with_flow_analysis(node.raw(), |rv| {
                let raw_type = element_to_infer.as_ref().map(|e| e.as_type(rv));
                let inv = FullInvocation {
                    node: node.raw(),
                    argument_list,
                    context_type: TypeId::UNKNOWN,
                    raw_type,
                    type_arguments,
                    error_entity: InferenceErrorEntity::other(
                        rv.ast.offset(node) as usize,
                        rv.ast.length(node) as usize,
                    ),
                    is_const: true,
                    is_generic_inference_disabled: false,
                    needs_type_argument_bounds_check: false,
                    wrong_number_of_type_arguments: None,
                };
                resolve_full_invocation(rv, inv, &mut |rv, _type_argument_types, invoke_type| {
                    let constructed_type = invoke_return_type(rv, invoke_type?)?;
                    let base = member::base_element(&rv.ctx, rv.element(node)?);
                    let constructor_element = constructed_element(rv, base, constructed_type);
                    rv.set_element(node, Some(constructor_element));
                    Some(member::formal_parameters(&rv.ctx, constructor_element))
                });
            });
            rv.visit_opt(type_arguments);
            // Dart `checkForArgumentTypesNotAssignableInList` (wave D).
        }
        None => {
            // Dart: `definingLibrary.featureSet.isEnabled(enhanced_enums)`.
            if let Some(constructor_element) = constructor_element
                && rv.is_enabled(dartr_parser::experimental_flags::ExperimentalFlag::EnhancedEnums)
            {
                let required_parameter_count = member::formal_parameters(&ctx, constructor_element)
                    .iter()
                    .filter(|&&p| {
                        member::base_element(&ctx, p)
                            .cast::<dartr_element::FormalParameterElement>()
                            .is_some_and(|p| ctx.get(p).kind.is_required_positional())
                    })
                    .count();
                if required_parameter_count != 0 {
                    let token = rv.ast[node].name;
                    report_not_enough_positional_arguments(
                        rv,
                        token,
                        required_parameter_count,
                        0,
                        Some(node.raw()),
                    );
                }
            }
        }
    }
}
