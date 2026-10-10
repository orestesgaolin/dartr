// Dart source: pkg/analyzer/lib/src/error/listener.dart (convertTypeNames,
// _TypeToConvert.allElements, _DiagnosticLocation.forElement) and the
// `nonSynthetic` getters of pkg/analyzer/lib/src/dart/element/element.dart

//! The hookup to `dartr_diagnostics`: semantic code reports through
//! [`dartr_diagnostics::DiagnosticReporter`] (one per unit). Type and element
//! arguments are converted here into [`TypeArg`] / [`ElementArg`], which
//! carry the display string and the elements needed by `convertTypeNames`
//! to disambiguate equal names:
//!
//! ```ignore
//! reporter.report(
//!     diag::argument_type_not_assignable(type_arg(ctx, actual), type_arg(ctx, expected), "")
//!         .at_offset(node_offset, node_length),
//! );
//! ```

use dartr_diagnostics::{ElementArg, ElementRef, TypeArg};

use crate::LibraryFragment;
use crate::ctx::Ctx;
use crate::display_string::{self, DisplayOptions};
use crate::flags::FragmentFlags;
use crate::ids::FId;
use crate::ids::{ElementId, FragmentId, Tag};
use crate::name::Name;
use crate::store::AnyElement;
use crate::types::{TypeId, TypeKind};

/// `type.getDisplayString(preferTypeAlias: true)` (unit A1).
pub fn type_display_string(ctx: &Ctx<'_>, ty: TypeId, prefer_type_alias: bool) -> String {
    display_string::type_display_string_with(
        ctx,
        ty,
        DisplayOptions {
            multiline: false,
            prefer_type_alias,
        },
    )
}

/// `element.displayString()` (unit A1).
pub fn element_display_string(ctx: &Ctx<'_>, element: ElementId) -> String {
    display_string::element_display_string_with(ctx, element, DisplayOptions::default())
}

/// A `DartType` diagnostic argument.
pub fn type_arg(ctx: &Ctx<'_>, ty: TypeId) -> TypeArg {
    TypeArg {
        display: type_display_string(ctx, ty, true),
        elements: type_elements(ctx, ty)
            .into_iter()
            .map(|e| element_ref(ctx, e))
            .collect(),
    }
}

/// An `Element` diagnostic argument.
pub fn element_arg(ctx: &Ctx<'_>, element: ElementId) -> ElementArg {
    ElementArg {
        display: element_display_string(ctx, element),
        element: Some(element_ref(ctx, element)),
    }
}

/// `_TypeToConvert.allElements`: the interface elements in [ty] (through
/// function return and parameter types, record fields and type arguments),
/// in Dart's order, without unnamed elements.
pub fn type_elements(ctx: &Ctx<'_>, ty: TypeId) -> Vec<ElementId> {
    fn add(ctx: &Ctx<'_>, ty: TypeId, out: &mut Vec<ElementId>) {
        match *ctx.ty(ty) {
            TypeKind::Function(f) => {
                add(ctx, f.ret, out);
                for p in ctx.list(f.params) {
                    add(ctx, p.ty, out);
                }
            }
            TypeKind::Record {
                positional, named, ..
            } => {
                // `RecordType.fields`: positional fields, then named fields.
                for &t in ctx.list(positional) {
                    add(ctx, t, out);
                }
                for n in ctx.list(named) {
                    add(ctx, n.ty, out);
                }
            }
            // A Dart `Set<Element>` (insertion order).
            TypeKind::Interface { element, args, .. } if !out.contains(&element.raw()) => {
                out.push(element.raw());
                for &a in ctx.list(args) {
                    add(ctx, a, out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    add(ctx, ty, &mut out);
    out.retain(|&e| {
        ctx.element_data(e)
            .and_then(|d| d.name)
            .is_some_and(|n| n != Name::EMPTY)
    });
    out
}

const ORIGIN_VARIABLE: FragmentFlags = FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE;

/// Dart `FieldElementImpl.declaringFormalParameter`: the declaring field
/// formal parameter (of the primary constructor) of [field].
pub fn declaring_formal_parameter(ctx: &Ctx<'_>, field: ElementId) -> Option<ElementId> {
    let enclosing = ctx.element_data(field)?.enclosing?;
    let interface = enclosing.cast::<crate::InterfaceElement>()?;
    for &c in &ctx.interface(interface).constructors {
        for &p in &ctx.executable(c.upcast()).formal_params {
            let p = p.raw();
            if p.tag() != Tag::FieldFormalParameter {
                continue;
            }
            let AnyElement::FormalParameter(fp) = ctx.any(p) else {
                continue;
            };
            if fp.field.get().map(|f| f.raw()) != Some(field) {
                continue;
            }
            let declaring = ctx
                .element_data(p)
                .and_then(|d| ctx.fragment_data(d.first_fragment))
                .is_some_and(|f| {
                    f.flags
                        .get()
                        .contains(FragmentFlags::FIELD_FORMAL_PARAMETER_FRAGMENT_IS_DECLARING)
                });
            if declaring {
                return Some(p);
            }
        }
    }
    None
}

/// Dart `element.nonSynthetic`.
pub fn non_synthetic(ctx: &Ctx<'_>, element: ElementId) -> ElementId {
    let first_flags = |e: ElementId| {
        ctx.element_data(e)
            .and_then(|d| ctx.fragment_data(d.first_fragment))
            .map(|f| f.flags.get())
            .unwrap_or_default()
    };
    match ctx.any(element) {
        AnyElement::Constructor(c) => {
            if first_flags(element)
                .contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_ORIGIN_DECLARATION)
            {
                element
            } else {
                c.enclosing.expect("constructor without enclosing element")
            }
        }
        AnyElement::Getter(g) if first_flags(element).contains(ORIGIN_VARIABLE) => {
            non_synthetic(ctx, g.variable.expect().raw())
        }
        AnyElement::Setter(s) if first_flags(element).contains(ORIGIN_VARIABLE) => {
            non_synthetic(ctx, s.variable.expect().raw())
        }
        AnyElement::Field(_) | AnyElement::TopLevelVariable(_) => {
            let flags = first_flags(element);
            if flags.contains(FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_DECLARATION) {
                return element;
            }
            if element.tag() == Tag::Field
                && flags
                    .contains(FragmentFlags::FIELD_FRAGMENT_IS_ORIGIN_DECLARING_FORMAL_PARAMETER)
            {
                // FieldElementImpl.nonSynthetic returns the declaring
                // formal parameter of the primary constructor.
                if let Some(p) = declaring_formal_parameter(ctx, element) {
                    return p;
                }
                return element;
            }
            let data = ctx.element_data(element).expect("variable data");
            if let Some(enclosing) = data.enclosing
                && enclosing.tag() == Tag::Enum
                && (data.name == Some(Name::INDEX) || data.name == Some(Name::VALUES))
            {
                return enclosing;
            }
            let p = match ctx.any(element) {
                AnyElement::Field(f) => &f.property,
                AnyElement::TopLevelVariable(v) => &v.property,
                _ => unreachable!(),
            };
            match (p.getter, p.setter) {
                (Some(g), _) => g.raw(),
                (None, Some(s)) => s.raw(),
                (None, None) => panic!("synthetic variable without accessors"),
            }
        }
        _ => element,
    }
}

/// The library fragment that contains [fragment] (Dart `libraryFragment`).
pub fn library_fragment_of(ctx: &Ctx<'_>, fragment: FragmentId) -> Option<FId<LibraryFragment>> {
    let mut f = fragment;
    loop {
        if f.tag() == Tag::Library {
            return Some(FId::from_raw(f));
        }
        f = ctx.fragment_data(f)?.enclosing_fragment?;
    }
}

/// The [`ElementRef`] of [element] (`_DiagnosticLocation.forElement` of its
/// non-synthetic element).
pub fn element_ref(ctx: &Ctx<'_>, element: ElementId) -> ElementRef {
    let data = ctx.element_data(element);
    let name = data
        .and_then(|d| d.name)
        .map(|n| ctx.name_str(n).to_string());
    let non_synthetic = non_synthetic(ctx, element);
    let ns_data = ctx.element_data(non_synthetic);
    let ns_name = ns_data
        .and_then(|d| d.name)
        .map(|n| ctx.name_str(n).to_string());
    let fragment = ns_data.and_then(|d| ctx.fragment_data(d.first_fragment));
    let source_path = ns_data
        .and_then(|d| library_fragment_of(ctx, d.first_fragment))
        .map(|lf| ctx.fragment(lf).source.path.to_string())
        .unwrap_or_default();
    let (offset, length) = match fragment {
        Some(f) => match (f.name_offset, f.first_token_offset) {
            (Some(o), _) => (
                o as i64,
                f.name
                    .map(|n| ctx.name_str(n).encode_utf16().count() as i64)
                    .unwrap_or(0),
            ),
            (None, Some(o)) => (o as i64, 0),
            (None, None) => (-1, 0),
        },
        None => (-1, 0),
    };
    ElementRef {
        id: element.raw(),
        name,
        is_extension: element.tag() == Tag::Extension,
        non_synthetic_name: ns_name,
        non_synthetic_is_extension: non_synthetic.tag() == Tag::Extension,
        source_path,
        offset,
        length,
    }
}
