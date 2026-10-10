// Dart source: pkg/analysis_server/lib/src/domains/analysis/implemented_dart.dart
// Dart source: pkg/analysis_server/lib/src/services/search/search_engine_internal.dart (membersOfSubtypes)

//! Computes `analysis.implemented` notifications (`ImplementedComputer`).

use std::collections::HashSet;

use dartr_element::{Ctx, ElemRef, ElementId, FragmentFlags, InterfaceElement, NoopSink, Tag};
use dartr_resolver::error::support;
use dartr_typesystem::member;

use crate::protocol;
use crate::search::{ResolvedUnitRef, SElem};

/// Computes `analysis.implemented` for `resolved`.
pub fn compute_implemented(
    resolved: &ResolvedUnitRef,
    file: &str,
    mut members_of_subtypes: impl FnMut(&SElem) -> Option<HashSet<String>>,
) -> protocol::AnalysisImplementedParams {
    let sink = NoopSink;
    let ctx = resolved.ctx(&sink);
    let unit = resolved.unit();
    let unit_frag = ctx.fragment(unit.fragment);

    let mut interface_elements = Vec::new();
    let mut push_frag = |frag_id: dartr_element::FragmentId| {
        if let Some(el) = ctx
            .fragment_data(frag_id)
            .and_then(|d| d.element.try_get().copied())
        {
            interface_elements.push(el);
        }
    };
    for &f in &unit_frag.classes {
        push_frag(f.raw());
    }
    for &f in &unit_frag.enums {
        push_frag(f.raw());
    }
    for &f in &unit_frag.extension_types {
        push_frag(f.raw());
    }
    for &f in &unit_frag.mixins {
        push_frag(f.raw());
    }

    let mut classes = Vec::new();
    let mut members = Vec::new();

    for element in interface_elements {
        let Some(intf) = element.cast::<InterfaceElement>() else {
            continue;
        };
        if element.tag() == Tag::Class
            && support::display_name(&ctx, element) == "Object"
            && support::library_of(&ctx, element).is_some_and(|l| {
                ctx.fragment(ctx.get(l).first_fragment())
                    .source
                    .uri
                    .as_ref()
                    == "dart:core"
            })
        {
            add_implemented_class(&ctx, element, &mut classes);
            let d = ctx.interface(intf);
            for &g in &d.getters {
                add_implemented_member(&ctx, g.raw(), &mut members);
            }
            for &s in &d.setters {
                add_implemented_member(&ctx, s.raw(), &mut members);
            }
            for &f in &d.fields {
                add_implemented_member(&ctx, f.raw(), &mut members);
            }
            for &m in &d.methods {
                add_implemented_member(&ctx, m.raw(), &mut members);
            }
            continue;
        }

        let selem = SElem {
            lib: resolved.library.clone(),
            unit: resolved.index,
            id: element,
        };
        if let Some(subtype_members) = members_of_subtypes(&selem) {
            add_implemented_class(&ctx, element, &mut classes);
            let d = ctx.interface(intf);
            let mut add_if_implemented = |member_id: ElementId| {
                if member::is_static(&ctx, ElemRef::Base(member_id)) {
                    return;
                }
                let name = support::display_name(&ctx, member_id);
                if subtype_members.contains(&name) {
                    add_implemented_member(&ctx, member_id, &mut members);
                }
            };
            for &g in &d.getters {
                let flags = dartr_resolver::element_ext::first_fragment_flags(&ctx, g.raw());
                if flags.contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_DECLARATION) {
                    add_if_implemented(g.raw());
                }
            }
            for &s in &d.setters {
                let flags = dartr_resolver::element_ext::first_fragment_flags(&ctx, s.raw());
                if flags.contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_DECLARATION) {
                    add_if_implemented(s.raw());
                }
            }
            for &f in &d.fields {
                let flags = dartr_resolver::element_ext::first_fragment_flags(&ctx, f.raw());
                if flags.contains(FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_DECLARATION) {
                    add_if_implemented(f.raw());
                }
            }
            for &m in &d.methods {
                add_if_implemented(m.raw());
            }
        }
    }

    protocol::AnalysisImplementedParams {
        file: file.to_string(),
        classes,
        members,
    }
}

fn add_implemented_class(
    ctx: &Ctx<'_>,
    element: ElementId,
    classes: &mut Vec<protocol::ImplementedClass>,
) {
    let mut frag = ctx.element_data(element).map(|d| d.first_fragment);
    while let Some(f) = frag {
        let Some(data) = ctx.fragment_data(f) else {
            break;
        };
        if let (Some(offset), Some(name)) = (data.name_offset, data.name) {
            let len = ctx.name_str(name).encode_utf16().count();
            classes.push(protocol::ImplementedClass {
                offset: offset as i64,
                length: len as i64,
            });
        }
        frag = data.next_fragment;
    }
}

fn add_implemented_member(
    ctx: &Ctx<'_>,
    element: ElementId,
    members: &mut Vec<protocol::ImplementedMember>,
) {
    let mut frag = ctx.element_data(element).map(|d| d.first_fragment);
    while let Some(f) = frag {
        let Some(data) = ctx.fragment_data(f) else {
            break;
        };
        if let (Some(offset), Some(name)) = (data.name_offset, data.name) {
            let len = ctx.name_str(name).encode_utf16().count();
            members.push(protocol::ImplementedMember {
                offset: offset as i64,
                length: len as i64,
            });
        }
        frag = data.next_fragment;
    }
}

/// Collects the declared non-static member names of `subtype` (`DirectSubtypeWithMembers.members`).
pub fn subtype_declared_members(subtype: &SElem) -> (Option<String>, Vec<String>) {
    subtype.with(|ctx| {
        let lib_path = support::library_of(ctx, subtype.id).map(|l| {
            ctx.fragment(ctx.get(l).first_fragment())
                .source
                .path
                .to_string()
        });
        let Some(intf) = subtype.id.cast::<InterfaceElement>() else {
            return (lib_path, Vec::new());
        };
        let d = ctx.interface(intf);
        let mut names = Vec::new();
        let mut add = |id: ElementId| {
            if !member::is_static(ctx, ElemRef::Base(id)) {
                let name = support::display_name(ctx, id);
                if !name.is_empty() {
                    names.push(name);
                }
            }
        };
        for &g in &d.getters {
            let flags = dartr_resolver::element_ext::first_fragment_flags(ctx, g.raw());
            if flags.contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_DECLARATION) {
                add(g.raw());
            }
        }
        for &s in &d.setters {
            let flags = dartr_resolver::element_ext::first_fragment_flags(ctx, s.raw());
            if flags.contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_DECLARATION) {
                add(s.raw());
            }
        }
        for &f in &d.fields {
            let flags = dartr_resolver::element_ext::first_fragment_flags(ctx, f.raw());
            if flags.contains(FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_DECLARATION) {
                add(f.raw());
            }
        }
        for &m in &d.methods {
            add(m.raw());
        }
        (lib_path, names)
    })
}
