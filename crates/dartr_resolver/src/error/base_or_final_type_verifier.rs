// Dart source: pkg/analyzer/lib/src/error/base_or_final_type_verifier.dart

//! `BaseOrFinalTypeVerifier`: the subtypes of a `base` or `final` type
//! must be `base`, `final` or `sealed`, and a `base` type must not be
//! implemented outside of its library. The resolver calls
//! [`check_element`] after the visit of a class, class type alias or mixin
//! declaration (Dart `ResolverVisitor.baseOrFinalTypeVerifier`).

use dartr_ast::{Id, ImplementsClause, NamedType};
use dartr_diagnostics::{DiagnosticMessage, LocatableDiagnostic, diag};
use dartr_element::{
    ClassElement, Ctx, EId, ElementFlags, FragmentFlags, InterfaceElement, LibraryElement,
    MixinElement, Tag, TypeId,
};
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_typesystem::TypeExt;

use super::VerifierHost;
use crate::scope::library_feature_enabled;

/// Dart `BaseOrFinalTypeVerifier.checkElement(element, implementsClause)`:
/// checks that the subelement of a base or final element is base, final,
/// or sealed and that base elements are not implemented outside of their
/// library. Otherwise, an error is reported on that element.
pub fn check_element<'a, H: VerifierHost<'a>>(
    host: &mut H,
    element: EId<InterfaceElement>,
    implements_clause: Option<Id<ImplementsClause>>,
) {
    let ctx = host.ctx();
    if let Some(supertype) = ctx.element_supertype(element)
        && check_supertypes(host, &[supertype], element)
    {
        return;
    }
    if let Some(implements_clause) = implements_clause {
        let ast = host.ast();
        let interfaces = ast.list(ast[implements_clause].interfaces).to_vec();
        if check_interface_supertypes(host, &interfaces, element) {
            return;
        }
    }
    if check_supertypes(host, ctx.element_mixins(element), element) {
        return;
    }
    if element.tag() == Tag::Mixin
        && check_supertypes(host, ctx.element_superclass_constraints(element), element)
    {
        // Dart: `return;`
    }
}

/// Dart `_checkInterfaceSupertypes(interfaces, subElement,
/// areImplementedInterfaces: true)`: whether a 'base' or 'final' subtype
/// modifier error is reported for an interface in [interfaces].
fn check_interface_supertypes<'a, H: VerifierHost<'a>>(
    host: &mut H,
    interfaces: &[Id<NamedType>],
    sub_element: EId<InterfaceElement>,
) -> bool {
    let ctx = host.ctx();
    for &interface in interfaces {
        let Some(interface_type) = host.tables().annotation_type.get(interface).copied() else {
            continue;
        };
        if let Some(interface_element) = ctx.interface_element(interface_type) {
            // Return early if an error has been reported to prevent
            // reporting multiple errors on one element.
            if report_restriction_error(host, sub_element, interface_element, Some(interface)) {
                return true;
            }
        }
    }
    false
}

/// Dart `_checkSupertypes(supertypes, subElement)`: whether a 'base' or
/// 'final' subtype modifier error is reported for a supertype in
/// [supertypes].
fn check_supertypes<'a, H: VerifierHost<'a>>(
    host: &mut H,
    supertypes: &[TypeId],
    sub_element: EId<InterfaceElement>,
) -> bool {
    let ctx = host.ctx();
    for &supertype in supertypes {
        if let Some(supertype_element) = ctx.interface_element(supertype) {
            // Return early if an error has been reported to prevent
            // reporting multiple errors on one element.
            if report_restriction_error(host, sub_element, supertype_element, None) {
                return true;
            }
        }
    }
    false
}

/// Dart `_getExplicitlyBaseOrFinalElement(element)`: the nearest explicitly
/// declared 'base' or 'final' element in the element hierarchy of
/// [element].
fn get_explicitly_base_or_final_element(
    ctx: &Ctx<'_>,
    element: EId<InterfaceElement>,
) -> Option<EId<InterfaceElement>> {
    // The current element has an explicit 'base' or 'final' modifier.
    if (is_base(ctx, element) || is_final(ctx, element)) && !is_sealed(ctx, element) {
        return Some(element);
    }

    let mut base_or_final_super_element = None;
    if let Some(supertype) = ctx.element_supertype(element) {
        base_or_final_super_element =
            get_explicitly_base_or_final_element_from_super_types(ctx, &[supertype]);
    }
    if base_or_final_super_element.is_none() {
        base_or_final_super_element = get_explicitly_base_or_final_element_from_super_types(
            ctx,
            ctx.element_interfaces(element),
        );
    }
    if base_or_final_super_element.is_none() {
        base_or_final_super_element =
            get_explicitly_base_or_final_element_from_super_types(ctx, ctx.element_mixins(element));
    }
    if base_or_final_super_element.is_none() && element.tag() == Tag::Mixin {
        base_or_final_super_element = get_explicitly_base_or_final_element_from_super_types(
            ctx,
            ctx.element_superclass_constraints(element),
        );
    }
    base_or_final_super_element
}

/// Dart `_getExplicitlyBaseOrFinalElementFromSuperTypes(supertypes)`: the
/// first explicitly declared 'base' or 'final' element found in the class
/// hierarchies of a supertype in [supertypes].
fn get_explicitly_base_or_final_element_from_super_types(
    ctx: &Ctx<'_>,
    supertypes: &[TypeId],
) -> Option<EId<InterfaceElement>> {
    for &supertype in supertypes {
        if let Some(supertype_element) = ctx.interface_element(supertype)
            && let Some(result) = get_explicitly_base_or_final_element(ctx, supertype_element)
        {
            return Some(result);
        }
    }
    None
}

/// Dart `LibraryElement.isInSdk`.
fn is_in_sdk(ctx: &Ctx<'_>, library: EId<LibraryElement>) -> bool {
    ctx.library_uri(library).starts_with("dart:")
}

/// Dart `library.featureSet.isEnabled(Feature.class_modifiers)`.
fn class_modifiers_enabled(ctx: &Ctx<'_>, library: Option<EId<LibraryElement>>) -> bool {
    library.is_some_and(|l| library_feature_enabled(ctx, l, ExperimentalFlag::ClassModifiers))
}

/// Dart `element.library`.
fn library_of(ctx: &Ctx<'_>, element: EId<InterfaceElement>) -> Option<EId<LibraryElement>> {
    ctx.element_data(element.raw()).and_then(|d| d.library)
}

/// Dart `_mayIgnoreClassModifiers(superLibrary)`: whether a subclass in the
/// current library can ignore a class modifier of a declaration in
/// [super_library]. Only true if the supertype library is a platform
/// library, and either the current library is also a platform library, or
/// the current library has a language version which predates class
/// modifiers.
fn may_ignore_class_modifiers(
    ctx: &Ctx<'_>,
    defining_library: EId<LibraryElement>,
    super_library: Option<EId<LibraryElement>>,
) -> bool {
    // Only modifiers in platform libraries can be ignored.
    if !super_library.is_some_and(|l| is_in_sdk(ctx, l)) {
        return false;
    }
    // Other platform libraries can ignore modifiers.
    if is_in_sdk(ctx, defining_library) {
        return true;
    }
    // Libraries predating class modifiers can ignore platform modifiers.
    !class_modifiers_enabled(ctx, Some(defining_library))
}

/// Dart `_reportRestrictionError(element, superElement,
/// implementsNamedType:)`: whether an element modifier restriction error
/// has been reported. Reports an error based on the modifier of the
/// [super_element].
fn report_restriction_error<'a, H: VerifierHost<'a>>(
    host: &mut H,
    element: EId<InterfaceElement>,
    super_element: EId<InterfaceElement>,
    implements_named_type: Option<Id<NamedType>>,
) -> bool {
    let ctx = host.ctx();
    let defining_library = host.library();
    let element_library = library_of(&ctx, element);
    let super_library = library_of(&ctx, super_element);
    // Only report errors on elements within the current library.
    if element_library != Some(defining_library) {
        return false;
    }

    let base_or_final_super_element = if is_base(&ctx, super_element)
        || is_final(&ctx, super_element)
        || (!class_modifiers_enabled(&ctx, super_library)
            && class_modifiers_enabled(&ctx, element_library))
    {
        // The 'base' or 'final' modifier may be an induced modifier. Find
        // the explicitly declared 'base' or 'final' in the hierarchy. In
        // the case where the super element is in a pre-feature library, we
        // need to check if there's an indirect core library super element.
        get_explicitly_base_or_final_element(&ctx, super_element)
    } else {
        // There are no restrictions on this element's modifiers.
        return false;
    };

    let Some(base_or_final_super_element) = base_or_final_super_element else {
        return false;
    };
    let base_or_final_library = library_of(&ctx, base_or_final_super_element);

    if may_ignore_class_modifiers(&ctx, defining_library, base_or_final_library) {
        return false;
    }

    // Dart `firstFragmentLocation`.
    let Some(super_data) = ctx.element_data(base_or_final_super_element.raw()) else {
        return false;
    };
    let super_fragment = ctx.fragment_data(super_data.first_fragment);
    let super_library_fragment =
        dartr_element::diagnostics::library_fragment_of(&ctx, super_data.first_fragment);
    let super_name = super_fragment.and_then(|f| f.name).map(|n| ctx.name_str(n));
    let super_name_offset = super_fragment.and_then(|f| f.name_offset);
    let (Some(super_library_fragment), Some(super_name), Some(super_name_offset)) =
        (super_library_fragment, super_name, super_name_offset)
    else {
        return false;
    };

    let super_element_name = display_name(&ctx, super_element);
    let base_or_final_name = display_name(&ctx, base_or_final_super_element);

    // The context message links to the explicitly declared 'base' or
    // 'final' super element and is only added onto the error if 'base' or
    // 'final' is an induced modifier of the direct super element.
    let context_messages = vec![DiagnosticMessage {
        file_path: ctx.fragment(super_library_fragment).source.path.to_string(),
        offset: super_name_offset as i64,
        length: super_name.encode_utf16().count() as i64,
        message: format!(
            "The type '{super_element_name}' is a subtype of '{base_or_final_name}', and \
             '{base_or_final_name}' is defined here."
        ),
        url: None,
    }];

    // It's an error to implement a class if it has a supertype from a
    // different library which is marked base.
    if let Some(implements_named_type) = implements_named_type
        && is_sealed(&ctx, super_element)
        && base_or_final_library != element_library
        && is_base(&ctx, base_or_final_super_element)
    {
        let error = if base_or_final_super_element.tag() == Tag::Mixin {
            diag::base_mixin_implemented_outside_of_library(&base_or_final_name)
        } else {
            diag::base_class_implemented_outside_of_library(&base_or_final_name)
        };
        let d = host.at(
            error.with_context_messages(context_messages),
            implements_named_type,
        );
        host.report(d);
        return true;
    }

    if !is_base(&ctx, element) && !is_final(&ctx, element) && !is_sealed(&ctx, element) {
        let element_name = display_name(&ctx, element);
        let error: LocatableDiagnostic;
        if is_final(&ctx, base_or_final_super_element) {
            // If you can't extend, implement or mix in a final element
            // outside of its library anyways, it's not helpful to report a
            // subelement modifier error.
            if base_or_final_library != element_library {
                // In the case where the 'baseOrFinalSuperElement' is a core
                // library element and we are subtyping from a super element
                // that's from a pre-feature library, we want to produce a
                // final transitivity error.
                //
                // For implements clauses with the above scenario, we avoid
                // over-reporting since there will already be a
                // [FinalClassImplementedOutsideOfLibrary] error.
                if class_modifiers_enabled(&ctx, super_library)
                    || !base_or_final_library.is_some_and(|l| is_in_sdk(&ctx, l))
                    || implements_named_type.is_some()
                {
                    return false;
                }
            }
            error = if element.tag() == Tag::Mixin {
                diag::mixin_subtype_of_final_is_not_base(&element_name, &base_or_final_name)
            } else {
                diag::subtype_of_final_is_not_base_final_or_sealed(
                    &element_name,
                    &base_or_final_name,
                )
            };
        } else if is_base(&ctx, base_or_final_super_element) {
            error = if element.tag() == Tag::Mixin {
                diag::mixin_subtype_of_base_is_not_base(&element_name, &base_or_final_name)
            } else {
                diag::subtype_of_base_is_not_base_final_or_sealed(
                    &element_name,
                    &base_or_final_name,
                )
            };
        } else {
            return false;
        }
        let error = if is_sealed(&ctx, super_element) {
            error.with_context_messages(context_messages)
        } else {
            error
        };
        // Dart `atSourceRange(element.diagnosticRange(diagnosticSource))`.
        let (offset, length) = diagnostic_range(&ctx, element);
        host.report(error.at_offset(offset, length));
        return true;
    }

    false
}

/// Dart `element.displayName` of an interface element.
fn display_name(ctx: &Ctx<'_>, element: EId<InterfaceElement>) -> String {
    ctx.element_name(element.raw()).unwrap_or("").to_string()
}

/// Dart `Element.diagnosticRange(source)`: the name offset and the name
/// length of the first fragment (`-1` and `0` without a name).
fn diagnostic_range(ctx: &Ctx<'_>, element: EId<InterfaceElement>) -> (usize, usize) {
    let fragment = ctx
        .element_data(element.raw())
        .and_then(|d| ctx.fragment_data(d.first_fragment));
    let offset = fragment.and_then(|f| f.name_offset);
    let length = fragment
        .and_then(|f| f.name)
        .map(|n| ctx.name_str(n).encode_utf16().count())
        .unwrap_or(0);
    match offset {
        Some(offset) => (offset as usize, length),
        // Dart `SourceRange(-1, length)`: an unnamed element is recovery
        // code; report at the start of the file.
        None => (0, length),
    }
}

/// The flags of the first fragment of [element].
fn first_fragment_flags(ctx: &Ctx<'_>, element: EId<InterfaceElement>) -> FragmentFlags {
    crate::element_ext::first_fragment_flags(ctx, element.raw())
}

/// Dart `InterfaceElementImpl.isBase` (the extension of the Dart file):
/// classes and mixins.
fn is_base(ctx: &Ctx<'_>, element: EId<InterfaceElement>) -> bool {
    if let Some(class) = element.cast::<ClassElement>() {
        ctx.get(class)
            .flags
            .get()
            .contains(ElementFlags::CLASS_ELEMENT_IS_BASE)
    } else if element.cast::<MixinElement>().is_some() {
        first_fragment_flags(ctx, element).contains(FragmentFlags::MIXIN_FRAGMENT_IS_BASE)
    } else {
        false
    }
}

/// Dart `InterfaceElementImpl.isFinal`: classes only.
fn is_final(ctx: &Ctx<'_>, element: EId<InterfaceElement>) -> bool {
    match element.cast::<ClassElement>() {
        Some(class) => ctx
            .get(class)
            .flags
            .get()
            .contains(ElementFlags::CLASS_ELEMENT_IS_FINAL),
        None => false,
    }
}

/// Dart `InterfaceElementImpl.isSealed`: classes only.
fn is_sealed(ctx: &Ctx<'_>, element: EId<InterfaceElement>) -> bool {
    element.tag() == Tag::Class
        && first_fragment_flags(ctx, element).contains(FragmentFlags::CLASS_FRAGMENT_IS_SEALED)
}
