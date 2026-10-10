// Dart source: pkg/analyzer/lib/dart/element/element.dart (the getters that completion reads)

//! Element predicates that completion reads (Dart `isStatic`, `isConst`,
//! `isOriginDeclaration`, `isVisibleIn`, ...).

#[allow(unused_imports)]
use dartr_typesystem::TypeExt;
use dartr_element::{Ctx, EId, ElemRef, ElementId, FragmentFlags, LibraryElement, Tag};
use dartr_resolver::error::support;
use dartr_typesystem::member;

fn first_fragment_flags(ctx: &Ctx<'_>, e: ElementId) -> FragmentFlags {
    ctx.element_data(e)
        .and_then(|d| ctx.fragment_data(d.first_fragment))
        .map(|f| f.flags.get())
        .unwrap_or_default()
}

fn has(ctx: &Ctx<'_>, e: ElementId, flag: FragmentFlags) -> bool {
    first_fragment_flags(ctx, e).contains(flag)
}

/// Dart `isStatic` of a member, field or variable.
pub fn is_static(ctx: &Ctx<'_>, e: ElementId) -> bool {
    match e.tag() {
        Tag::Field | Tag::TopLevelVariable => has(ctx, e, FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC),
        Tag::Method | Tag::Getter | Tag::Setter | Tag::Constructor | Tag::TopLevelFunction => {
            has(ctx, e, FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC)
        }
        _ => false,
    }
}

/// Dart `isStatic` of a top-level element: top-level functions and
/// accessors are static.
pub fn is_static_member(ctx: &Ctx<'_>, e: ElementId) -> bool {
    let enclosing_is_library = ctx
        .element_data(e)
        .and_then(|d| d.enclosing)
        .is_some_and(|p| p.tag() == Tag::Library);
    if enclosing_is_library {
        return matches!(
            e.tag(),
            Tag::TopLevelFunction | Tag::Getter | Tag::Setter | Tag::TopLevelVariable
        );
    }
    is_static(ctx, e)
}

/// Dart `VariableElement.isConst`.
pub fn is_const_variable(ctx: &Ctx<'_>, e: ElementId) -> bool {
    has(ctx, e, FragmentFlags::VARIABLE_FRAGMENT_IS_CONST)
}

/// Dart `VariableElement.isFinal`.
pub fn is_final_variable(ctx: &Ctx<'_>, e: ElementId) -> bool {
    has(ctx, e, FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL)
}

/// Dart `ConstructorElement.isConst`.
pub fn is_const_constructor(ctx: &Ctx<'_>, e: ElementId) -> bool {
    has(ctx, e, FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST)
}

/// Dart `ConstructorElement.isFactory`.
pub fn is_factory(ctx: &Ctx<'_>, e: ElementId) -> bool {
    has(ctx, e, FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY)
}

/// Dart `ClassElement.isAbstract`.
pub fn is_abstract_class(ctx: &Ctx<'_>, e: ElementId) -> bool {
    e.tag() == Tag::Class
        && (has(ctx, e, FragmentFlags::CLASS_FRAGMENT_IS_ABSTRACT)
            || has(ctx, e, FragmentFlags::CLASS_FRAGMENT_IS_SEALED))
}

/// Dart `MethodElement.isOperator`.
pub fn is_operator(ctx: &Ctx<'_>, e: ElementId) -> bool {
    if e.tag() != Tag::Method {
        return false;
    }
    let name = ctx.element_name(e).unwrap_or("");
    let first = name.chars().next();
    !name.is_empty() && !first.is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$')
}

/// Dart `PropertyAccessorElement.isOriginVariable`.
pub fn is_origin_variable(ctx: &Ctx<'_>, e: ElementId) -> bool {
    matches!(e.tag(), Tag::Getter | Tag::Setter)
        && has(ctx, e, FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE)
}

/// Dart `isOriginDeclaration` of accessors, fields and variables.
pub fn is_origin_declaration(ctx: &Ctx<'_>, e: ElementId) -> bool {
    match e.tag() {
        Tag::Getter | Tag::Setter => {
            has(ctx, e, FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_DECLARATION)
        }
        Tag::Field | Tag::TopLevelVariable => {
            has(ctx, e, FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_DECLARATION)
        }
        Tag::Method => has(ctx, e, FragmentFlags::METHOD_FRAGMENT_IS_ORIGIN_DECLARATION),
        Tag::TopLevelFunction => {
            has(ctx, e, FragmentFlags::TOP_LEVEL_FUNCTION_FRAGMENT_IS_ORIGIN_DECLARATION)
        }
        Tag::Constructor => has(ctx, e, FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_ORIGIN_DECLARATION),
        _ => true,
    }
}

/// Dart `FieldElement.isOriginGetterSetter`.
pub fn is_origin_getter_setter(ctx: &Ctx<'_>, e: ElementId) -> bool {
    matches!(e.tag(), Tag::Field | Tag::TopLevelVariable)
        && has(ctx, e, FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_GETTER_SETTER)
}

/// Dart `FieldElement.isOriginEnumValues`.
pub fn is_origin_enum_values(ctx: &Ctx<'_>, e: ElementId) -> bool {
    e.tag() == Tag::Field && has(ctx, e, FragmentFlags::FIELD_FRAGMENT_IS_ORIGIN_ENUM_VALUES)
}

/// Dart `FieldElement.isOriginDeclaringFormalParameter`.
pub fn is_origin_declaring_formal_parameter(ctx: &Ctx<'_>, e: ElementId) -> bool {
    e.tag() == Tag::Field
        && has(
            ctx,
            e,
            FragmentFlags::FIELD_FRAGMENT_IS_ORIGIN_DECLARING_FORMAL_PARAMETER,
        )
}

/// Dart `FieldElement.hasInitializer`.
pub fn has_initializer(ctx: &Ctx<'_>, e: ElementId) -> bool {
    has(ctx, e, FragmentFlags::NON_PARAMETER_VARIABLE_FRAGMENT_HAS_INITIALIZER)
}

/// Dart `Element.library`.
pub fn library_of(ctx: &Ctx<'_>, e: ElementId) -> Option<EId<LibraryElement>> {
    support::library_of(ctx, e)
}

/// Dart `isVisibleIn(referencingLibrary)` (the completion extension):
/// declared in [library], or public.
pub fn is_visible_in(ctx: &Ctx<'_>, e: ElementId, library: EId<LibraryElement>) -> bool {
    if library_of(ctx, e) == Some(library) {
        return true;
    }
    ctx.element_name(e).is_some_and(|n| !n.starts_with('_'))
}

/// Dart `Element.isAccessibleIn(library)`.
pub fn is_accessible_in(ctx: &Ctx<'_>, e: ElementId, library: EId<LibraryElement>) -> bool {
    let name = ctx.element_name(e);
    if name.is_some_and(|n| n.starts_with('_')) {
        return library_of(ctx, e) == Some(library);
    }
    true
}

/// The variable of an accessor (Dart `PropertyAccessorElement.variable`).
pub fn accessor_variable(ctx: &Ctx<'_>, accessor: ElementId) -> Option<ElementId> {
    dartr_resolver::element_metadata::accessor_variable_any(ctx, accessor)
}

/// Dart `PropertyInducingElement.getter`.
pub fn variable_getter(ctx: &Ctx<'_>, variable: ElementId) -> Option<ElementId> {
    match ctx.any(variable) {
        dartr_element::AnyElement::Field(f) => f.getter.map(|g| g.raw()),
        dartr_element::AnyElement::TopLevelVariable(v) => v.getter.map(|g| g.raw()),
        _ => None,
    }
}

/// Dart `PropertyInducingElement.setter`.
pub fn variable_setter(ctx: &Ctx<'_>, variable: ElementId) -> Option<ElementId> {
    match ctx.any(variable) {
        dartr_element::AnyElement::Field(f) => f.setter.map(|g| g.raw()),
        dartr_element::AnyElement::TopLevelVariable(v) => v.setter.map(|g| g.raw()),
        _ => None,
    }
}

/// Dart `GetterElement.correspondingSetter`.
pub fn corresponding_setter(ctx: &Ctx<'_>, getter: ElementId) -> Option<ElementId> {
    let v = accessor_variable(ctx, getter)?;
    variable_setter(ctx, v)
}

/// Dart `SetterElement.correspondingGetter`.
pub fn corresponding_getter(ctx: &Ctx<'_>, setter: ElementId) -> Option<ElementId> {
    let v = accessor_variable(ctx, setter)?;
    variable_getter(ctx, v)
}

/// Dart `PropertyAccessorElement.isConst` (the completion extension).
pub fn accessor_is_const(ctx: &Ctx<'_>, accessor: ElementId) -> bool {
    if is_origin_variable(ctx, accessor) {
        return accessor_variable(ctx, accessor).is_some_and(|v| is_const_variable(ctx, v));
    }
    false
}

/// Dart `Element.isDeprecatedWithKind('use')`.
pub fn is_deprecated(ctx: &Ctx<'_>, e: ElementId) -> bool {
    dartr_resolver::element_metadata::is_deprecated_with_kind(ctx, e, "use", None)
}

/// Dart `hasOrInheritsDeprecated` (analysis_server element extensions).
pub fn has_or_inherits_deprecated(ctx: &Ctx<'_>, e: ElementId) -> bool {
    if is_deprecated(ctx, e) {
        return true;
    }
    let mut ancestor = ctx.element_data(e).and_then(|d| d.enclosing);
    if let Some(a) = ancestor {
        if a.is::<dartr_element::InterfaceElement>() {
            if is_deprecated(ctx, a) {
                return true;
            }
            ancestor = ctx.element_data(a).and_then(|d| d.enclosing);
        }
    }
    ancestor.is_some_and(|a| a.tag() == Tag::Library && is_deprecated(ctx, a))
}

/// Dart `element.metadata.has<flag>`.
pub fn metadata_has(ctx: &Ctx<'_>, e: ElementId, flag: u32) -> bool {
    dartr_resolver::element_metadata::element_has(ctx, e, flag, None)
}

/// Dart `metadata.hasRequired`.
pub fn has_required(ctx: &Ctx<'_>, e: ElementId) -> bool {
    metadata_has(ctx, e, dartr_resolver::element_metadata::flags::REQUIRED)
}

/// The kind of a formal parameter (base element).
pub fn parameter_kind(ctx: &Ctx<'_>, p: ElemRef) -> dartr_ast::ParameterKind {
    let base = member::base_element(ctx, p);
    match base.cast::<dartr_element::FormalParameterElement>() {
        Some(fp) => ctx.get(fp).kind,
        None => dartr_ast::ParameterKind::Required,
    }
}

/// Dart `Element.name` of [e] (base).
pub fn name<'a>(ctx: &Ctx<'a>, e: ElementId) -> Option<&'a str> {
    ctx.element_name(e)
}

/// Dart `Element.enclosingElement`.
pub fn enclosing(ctx: &Ctx<'_>, e: ElementId) -> Option<ElementId> {
    ctx.element_data(e).and_then(|d| d.enclosing)
}

/// The URI of a library element.
pub fn library_uri(ctx: &Ctx<'_>, library: EId<LibraryElement>) -> String {
    let first = ctx.get(library).first_fragment();
    ctx.fragment(first).source.uri.to_string()
}
