// Dart source: tools/oracle/bin/interface.dart (oracle mode `interface`,
// docs/design/semantics.md §5.2) and the `ref` / `refName` helpers of
// tools/oracle/bin/elements.dart

//! The JSON line of `dartr dump interface` for one library: the same bytes
//! as the oracle mode `interface` (see `tools/oracle/bin/interface.dart` for
//! the format). The driver (`dartr dump interface`) calls
//! [`interface_library_json`] once the linker builds the library elements.

use dartr_element::{
    Ctx, DisplayOptions, EId, ElemRef, ElementId, InterfaceElement, LibraryElement, Tag,
};

use crate::inheritance_manager3::{Conflict, InheritanceManager3, Name, NameMap};
use crate::member;
use crate::type_ext::TypeExt;

/// One line of the `interface` dump for [library]; [path] is the input path
/// (or `dart:` URI) as given.
pub fn interface_library_json(ctx: &Ctx<'_>, path: &str, library: EId<LibraryElement>) -> String {
    let mut out = String::new();
    out.push_str("{\"path\":");
    write_string(&mut out, path);
    out.push_str(",\"uri\":");
    write_string(&mut out, ctx.library_uri(library));
    out.push_str(",\"interfaces\":[");
    let data = ctx.get(library);
    let elements: Vec<(ElementId, &str)> = data
        .classes
        .iter()
        .map(|e| (e.raw(), "class"))
        .chain(data.enums.iter().map(|e| (e.raw(), "enum")))
        .chain(data.mixins.iter().map(|e| (e.raw(), "mixin")))
        .chain(
            data.extension_types
                .iter()
                .map(|e| (e.raw(), "extensionType")),
        )
        .collect();
    for (i, (element, kind)) in elements.into_iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        interface_json(ctx, &mut out, EId::from_raw(element), kind);
    }
    out.push_str("]}");
    out
}

/// `interfaceJson(element, kind)`.
fn interface_json(ctx: &Ctx<'_>, out: &mut String, element: EId<InterfaceElement>, kind: &str) {
    let manager = InheritanceManager3::new(*ctx);
    let interface = manager.get_interface(element);
    out.push_str("{\"n\":");
    write_opt_string(out, ctx.element_name(element.raw()));
    out.push_str(",\"k\":");
    write_string(out, kind);
    out.push_str(",\"map\":");
    name_map_json(ctx, out, &interface.map);
    out.push_str(",\"implemented\":");
    name_map_json(ctx, out, &interface.implemented);
    out.push_str(",\"inherited\":");
    name_map_json(ctx, out, manager.get_inherited_map(element));
    out.push_str(",\"forwarders\":[");
    let mut forwarders: Vec<String> = interface
        .no_such_method_forwarders
        .iter()
        .map(|n| n.to_display(ctx))
        .collect();
    forwarders.sort_by(|a, b| dart_compare(a, b));
    for (i, f) in forwarders.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write_string(out, f);
    }
    out.push_str("],\"conflicts\":[");
    for (i, conflict) in interface.conflicts.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        conflict_json(ctx, out, conflict);
    }
    out.push_str("]}");
}

/// `nameMapJson(map)`: entries sorted by `Name.toString()`.
fn name_map_json(ctx: &Ctx<'_>, out: &mut String, map: &NameMap) {
    let mut entries: Vec<(String, ElemRef)> =
        map.iter().map(|(n, &e)| (n.to_display(ctx), e)).collect();
    entries.sort_by(|a, b| dart_compare(&a.0, &b.0));
    out.push('[');
    for (i, (name, element)) in entries.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('[');
        write_string(out, name);
        out.push(',');
        write_opt_string(
            out,
            reference(ctx, member::base_element(ctx, *element)).as_deref(),
        );
        out.push(',');
        let t = member::type_(ctx, *element);
        write_string(
            out,
            &dartr_element::type_display_string_with(ctx, t, DisplayOptions::default()),
        );
        out.push(']');
    }
    out.push(']');
}

/// `conflictJson(conflict)`.
fn conflict_json(ctx: &Ctx<'_>, out: &mut String, conflict: &Conflict) {
    let members: Vec<ElemRef> = match conflict {
        Conflict::Candidates { candidates, .. }
        | Conflict::NotUniqueExtensionMember { candidates, .. } => candidates.clone(),
        Conflict::GetterMethod { getter, method, .. } => vec![*getter, *method],
        Conflict::HasNonExtensionAndExtensionMember {
            non_extension,
            extension,
            ..
        } => non_extension.iter().chain(extension).copied().collect(),
        Conflict::ExtensionTypeConflictingInheritedMethodAndSetter { method, setter, .. } => {
            vec![*method, *setter]
        }
        Conflict::ExtensionTypeConflictingStaticAndInstance {
            declared,
            inherited,
            ..
        } => vec![*declared, *inherited],
    };
    out.push_str("{\"k\":");
    write_string(out, conflict.kind_name());
    out.push_str(",\"n\":");
    write_string(out, &Name::to_display(&conflict.name(), ctx));
    out.push_str(",\"members\":[");
    for (i, m) in members.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write_opt_string(
            out,
            reference(ctx, member::base_element(ctx, *m)).as_deref(),
        );
    }
    out.push_str("]}");
}

/// R(e) of the oracle: `<libraryUri>::<names of the enclosing elements>`.
pub fn reference(ctx: &Ctx<'_>, element: ElementId) -> Option<String> {
    let mut names = Vec::new();
    let mut current = Some(element);
    while let Some(e) = current {
        if e.tag() == Tag::Library {
            break;
        }
        names.push(ref_name(ctx, e));
        current = ctx.element_data(e).and_then(|d| d.enclosing);
    }
    let uri = match ctx.element_data(element).and_then(|d| d.library) {
        Some(library) => ctx.library_uri(library).to_string(),
        None => "<no-library>".to_string(),
    };
    names.reverse();
    Some(format!("{uri}::{}", names.join(".")))
}

/// `refName(e)`.
fn ref_name(ctx: &Ctx<'_>, e: ElementId) -> String {
    let mut name = ctx.element_name(e).map(str::to_string);
    if e.tag() == Tag::Constructor && name.as_deref().is_none_or(str::is_empty) {
        name = Some("new".into());
    }
    let mut name = name.unwrap_or_else(|| "<unnamed>".into());
    if e.tag() == Tag::Setter {
        name.push('=');
    }
    name
}

/// Dart `String.compareTo` (UTF-16 code units).
fn dart_compare(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

fn write_opt_string(out: &mut String, s: Option<&str>) {
    match s {
        Some(s) => write_string(out, s),
        None => out.push_str("null"),
    }
}

/// A JSON string exactly as Dart `jsonEncode` writes it (same as
/// `crates/dartr/src/json.rs`).
fn write_string(out: &mut String, s: &str) {
    use std::fmt::Write as _;
    out.push('"');
    let bytes = s.as_bytes();
    let mut start = 0;
    for (i, &b) in bytes.iter().enumerate() {
        if b >= 0x20 && b != b'"' && b != b'\\' {
            continue;
        }
        out.push_str(&s[start..i]);
        start = i + 1;
        match b {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            8 => out.push_str("\\b"),
            9 => out.push_str("\\t"),
            10 => out.push_str("\\n"),
            12 => out.push_str("\\f"),
            13 => out.push_str("\\r"),
            _ => {
                let _ = write!(out, "\\u{:04x}", b);
            }
        }
    }
    out.push_str(&s[start..]);
    out.push('"');
}
