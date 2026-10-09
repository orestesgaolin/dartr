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
        Tag::LocalVariable
            | Tag::PatternVariable
            | Tag::BindPatternVariable
            | Tag::JoinPatternVariable
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
    // Dart `PropertyInducingElementImpl.type` infers on demand while
    // linking (summary2 `AstResolver`).
    dartr_element::type_inference::ensure_variable_type(ctx, e);
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

// ------------------------------------------------------------ pattern variables
//
// The pattern variables (Dart `PatternVariableElementImpl`,
// `BindPatternVariableElementImpl`, `JoinPatternVariableElementImpl`) are
// local variable elements with the tags `PatternVariable`,
// `BindPatternVariable` and `JoinPatternVariable`; the element binding pass
// creates them in the local arena. Their pattern data is in the first
// fragment (`LocalVariableFragment.pattern`):
// - a bind variable: `pattern.node` is the `DeclaredVariablePattern`,
//   `pattern.join` is the join variable that uses it (if any);
// - a join variable: `pattern.variables` are the component fragments,
//   `pattern.inconsistency`, `pattern.references` (the identifiers that
//   reference the join variable, written by the resolution visitor).

/// The first fragment of the local variable [e] (also a pattern variable).
pub fn local_variable_fragment<'a>(
    ctx: &Ctx<'a>,
    e: ElementId,
) -> Option<&'a dartr_element::LocalVariableFragment> {
    let local = e.cast::<LocalVariableElement>()?;
    Some(ctx.fragment(ctx.get(local).first_fragment()))
}

/// Whether [e] is a Dart `JoinPatternVariableElementImpl`.
pub fn is_join_pattern_variable(e: ElementId) -> bool {
    e.tag() == Tag::JoinPatternVariable
}

/// Dart `PatternVariableElementImpl.join` (through the first fragment):
/// the join variable that [e] is a component of.
pub fn pattern_variable_join(ctx: &Ctx<'_>, e: ElementId) -> Option<ElementId> {
    let fragment = local_variable_fragment(ctx, e)?;
    let join = fragment.pattern.join.get()?;
    ctx.fragment_data(join.raw())?.element.try_get().copied()
}

/// Dart `JoinPatternVariableElementImpl.variables`: the component
/// variables of the join variable [e].
pub fn join_pattern_variable_components(ctx: &Ctx<'_>, e: ElementId) -> Vec<ElementId> {
    let Some(fragment) = local_variable_fragment(ctx, e) else {
        return Vec::new();
    };
    fragment
        .pattern
        .variables
        .iter()
        .filter_map(|f| ctx.fragment_data(f.raw())?.element.try_get().copied())
        .collect()
}

/// Dart `BindPatternVariableFragmentImpl.node` of the bind variable [e]:
/// the `DeclaredVariablePattern`.
pub fn bind_pattern_variable_node(ctx: &Ctx<'_>, e: ElementId) -> Option<dartr_ast::NodeId> {
    local_variable_fragment(ctx, e)?.pattern.node
}

/// Dart `JoinPatternVariableElementImpl.inconsistency`.
pub fn join_pattern_variable_inconsistency(
    ctx: &Ctx<'_>,
    e: ElementId,
) -> dartr_element::JoinedPatternVariableInconsistency {
    local_variable_fragment(ctx, e)
        .and_then(|f| f.pattern.inconsistency.get())
        .unwrap_or_default()
}

/// Dart `JoinPatternVariableElementImpl.inconsistency = value`.
pub fn set_join_pattern_variable_inconsistency(
    ctx: &Ctx<'_>,
    e: ElementId,
    value: dartr_element::JoinedPatternVariableInconsistency,
) {
    if let Some(f) = local_variable_fragment(ctx, e) {
        f.pattern.inconsistency.set(Some(value));
    }
}

/// Dart `JoinPatternVariableFragmentImpl.references`: the identifiers that
/// reference the join variable [e].
pub fn join_pattern_variable_references(ctx: &Ctx<'_>, e: ElementId) -> Vec<dartr_ast::NodeId> {
    match local_variable_fragment(ctx, e) {
        Some(f) => f.pattern.references.lock().clone(),
        None => Vec::new(),
    }
}

/// Dart `VariableElementImpl.isFinal = value` of the local variable [e]
/// (the flag of the first fragment).
pub fn set_is_final(ctx: &Ctx<'_>, e: ElementId, value: bool) {
    if let Some(data) = ctx.element_data(e)
        && let Some(f) = ctx.fragment_data(data.first_fragment)
    {
        f.flags
            .set(FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL, value);
    }
}

/// Dart `FieldElement.isEnumConstant`.
pub fn is_enum_constant(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_flags(ctx, e).contains(FragmentFlags::FIELD_FRAGMENT_IS_ENUM_CONSTANT)
}

/// Dart `EnumElementImpl.constants`: the fields of [element] that are enum
/// constants, in declaration order.
pub fn enum_constants(
    ctx: &Ctx<'_>,
    element: EId<dartr_element::InstanceElement>,
) -> Vec<ElementId> {
    ctx.instance(element)
        .fields
        .iter()
        .map(|f| f.raw())
        .filter(|&f| is_enum_constant(ctx, f))
        .collect()
}
