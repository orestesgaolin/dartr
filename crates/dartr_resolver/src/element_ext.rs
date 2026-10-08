// Dart source: pkg/analyzer/lib/src/dart/element/element.dart (getters of
// the element classes that resolution reads: isLate, isConst, isFinal,
// hasImplicitType, VariableElementImpl.type, ...)

//! Element getters over [`Ctx`] that the resolver needs and that are not in
//! `dartr_element` / `dartr_typesystem`. Add a function here when a ported
//! file needs another getter; keep the Dart name.

use dartr_element::{
    AnyElement, Ctx, EId, ElementId, FragmentFlags, LocalVariableElement, PromotableElement, Tag,
    TypeId,
};

/// The flags of the first fragment of [e] (most `isX` getters of
/// elements read the first fragment).
pub fn first_fragment_flags(ctx: &Ctx<'_>, e: ElementId) -> FragmentFlags {
    let Some(data) = ctx.element_data(e) else {
        return FragmentFlags::EMPTY;
    };
    match ctx.fragment_data(data.first_fragment) {
        Some(f) => f.flags.get(),
        None => FragmentFlags::EMPTY,
    }
}

/// Dart `VariableElement.isLate`.
pub fn is_late(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_flags(ctx, e).contains(FragmentFlags::VARIABLE_FRAGMENT_IS_LATE)
}

/// Dart `VariableElement.isConst`.
pub fn is_const(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_flags(ctx, e).contains(FragmentFlags::VARIABLE_FRAGMENT_IS_CONST)
}

/// Dart `VariableElement.isFinal`.
pub fn is_final(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_flags(ctx, e).contains(FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL)
}

/// Dart `VariableElement.hasImplicitType`.
pub fn has_implicit_type(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_flags(ctx, e).contains(FragmentFlags::VARIABLE_FRAGMENT_HAS_IMPLICIT_TYPE)
}

/// Whether [e] is a local variable element (Dart `LocalVariableElementImpl`,
/// including the pattern variables).
pub fn is_local_variable(e: ElementId) -> bool {
    matches!(
        e.tag(),
        Tag::LocalVariable | Tag::PatternVariable | Tag::BindPatternVariable | Tag::JoinPatternVariable
    )
}

/// Whether [e] is a Dart `PromotableElementImpl` (local variables and
/// formal parameters).
pub fn is_promotable_element(e: ElementId) -> bool {
    e.is::<PromotableElement>()
}

/// Dart `VariableElementImpl.type` of a variable element (local variable,
/// formal parameter, field, top-level variable); `InvalidType` when not
/// set yet.
pub fn variable_type(ctx: &Ctx<'_>, e: ElementId) -> TypeId {
    let t = match ctx.any(e) {
        AnyElement::LocalVariable(v) => v.type_.get(),
        AnyElement::FormalParameter(p) => p.type_.get(),
        AnyElement::Field(f) => f.type_.get(),
        AnyElement::TopLevelVariable(v) => v.type_.get(),
        _ => None,
    };
    t.unwrap_or(TypeId::INVALID)
}

/// Dart `LocalVariableElementImpl.type = type`.
pub fn set_local_variable_type(ctx: &Ctx<'_>, e: EId<LocalVariableElement>, ty: TypeId) {
    ctx.get(e).type_.set(Some(ty));
}

/// Dart `InstanceElement.getField(name)`.
pub fn get_field(
    ctx: &Ctx<'_>,
    element: EId<dartr_element::InstanceElement>,
    name: &str,
) -> Option<EId<dartr_element::FieldElement>> {
    ctx.instance(element)
        .fields
        .iter()
        .copied()
        .find(|&f| ctx.get(f).name.is_some_and(|n| ctx.name_str(n) == name))
}
