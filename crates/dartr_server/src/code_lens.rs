// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_code_lens.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/code_lens/augmentations.dart

//! `textDocument/codeLens` handler and augmentation CodeLens computer.

use dartr_element::{
    AnyElement, ClassFragment, Ctx, ElementId, EnumFragment, ExecutableFragment, ExtensionFragment,
    ExtensionTypeFragment, FId, FragmentId, GenericFunctionTypeFragment, InterfaceFragmentData,
    LibraryFragment, MixinFragment, NoopSink, Tag, TypeAliasFragment,
};
use dartr_syntax::LineInfo;
use serde_json::{Value, json};

use crate::client_configuration::CodeLensConfiguration;
use crate::mapping::{ErrorOr, to_range};
use crate::navigation::library_fragment_of;
use crate::server::{ResolvedUnitRef, Server, is_dart_document};
use crate::uri::path_to_uri;

impl Server {
    /// Handles `textDocument/codeLens` (Dart `CodeLensHandler.handle`).
    pub(crate) fn code_lens(&mut self, params: &Value) -> ErrorOr<Value> {
        let config = self.client_configuration.global().code_lens();
        let is_available = is_dart_document(params)
            && (config.augmentation || config.augmented)
            && self.client.supports_go_to_location_command();
        if !is_available {
            return Ok(json!([]));
        }

        let path = self.path_of_doc(params)?;
        let resolved = self.require_resolved_unit(&path)?;
        Ok(Value::Array(compute_augmentation_code_lenses(
            &resolved, config,
        )))
    }
}

/// Computes augmentation/augmented `CodeLens` items for `resolved`
/// (Dart `AugmentationCodeLensComputer.compute`).
pub fn compute_augmentation_code_lenses(
    resolved: &ResolvedUnitRef,
    config: CodeLensConfiguration,
) -> Vec<Value> {
    let sink = NoopSink;
    let ctx = resolved.ctx(&sink);
    let unit_fragment = resolved.unit().fragment.raw();

    let mut code_lenses = Vec::new();
    add_code_lenses(&ctx, config, unit_fragment, &mut code_lenses);
    code_lenses
}

fn add_code_lenses(
    ctx: &Ctx<'_>,
    config: CodeLensConfiguration,
    fragment: FragmentId,
    code_lenses: &mut Vec<Value>,
) {
    let Some(data) = ctx.fragment_data(fragment) else {
        return;
    };
    if config.augmented
        && let Some(prev) = data.previous_fragment
    {
        add_code_lens(ctx, "Go to Augmented", fragment, prev, code_lenses);
    }
    if config.augmentation
        && let Some(next) = data.next_fragment
    {
        add_code_lens(ctx, "Go to Augmentation", fragment, next, code_lenses);
    }
    for child in fragment_children(ctx, fragment) {
        add_code_lenses(ctx, config, child, code_lenses);
    }
}

fn add_code_lens(
    ctx: &Ctx<'_>,
    title: &str,
    from: FragmentId,
    to: FragmentId,
    code_lenses: &mut Vec<Value>,
) {
    let Some(from_loc) = get_location(ctx, from) else {
        return;
    };
    let Some(to_loc) = get_location(ctx, to) else {
        return;
    };
    code_lenses.push(json!({
        "range": from_loc["range"],
        "command": {
            "command": "dart.goToLocation",
            "title": title,
            "arguments": [to_loc],
        },
    }));
}

/// Dart `AugmentationCodeLensComputer._getLocation`.
fn get_location(ctx: &Ctx<'_>, target_fragment: FragmentId) -> Option<Value> {
    let data = ctx.fragment_data(target_fragment)?;
    let name_offset = data.name_offset?;
    let element = data.element.try_get().copied()?;
    let name_length = element_display_name_len(ctx, element);

    let lib_frag = library_fragment_of(ctx, target_fragment)?.cast::<LibraryFragment>()?;
    let lib_data = ctx.fragment(lib_frag);
    let uri = path_to_uri(&lib_data.source.path);
    let line_info = LineInfo::new(lib_data.line_starts.to_vec());
    let range = to_range(&line_info, name_offset, name_length);

    Some(json!({
        "uri": uri,
        "range": range,
    }))
}

/// UTF-16 length of `Element.displayName` in Dart (`ElementImpl.displayName`).
fn element_display_name_len(ctx: &Ctx<'_>, element: ElementId) -> u32 {
    match ctx.any(element) {
        AnyElement::Constructor(c) => {
            let class_name = c
                .enclosing
                .and_then(|enc| ctx.element_data(enc))
                .and_then(|d| d.name)
                .map(|n| ctx.name_str(n))
                .unwrap_or("<null>");
            let name = c.name.map(|n| ctx.name_str(n)).unwrap_or("<null>");
            if name != "new" {
                (class_name.encode_utf16().count() + 1 + name.encode_utf16().count()) as u32
            } else {
                class_name.encode_utf16().count() as u32
            }
        }
        AnyElement::Class(c) => c
            .name
            .map(|n| ctx.name_str(n).encode_utf16().count() as u32)
            .unwrap_or(0),
        AnyElement::Enum(e) => e
            .name
            .map(|n| ctx.name_str(n).encode_utf16().count() as u32)
            .unwrap_or(0),
        AnyElement::Mixin(m) => m
            .name
            .map(|n| ctx.name_str(n).encode_utf16().count() as u32)
            .unwrap_or(0),
        AnyElement::Extension(e) => e
            .name
            .map(|n| ctx.name_str(n).encode_utf16().count() as u32)
            .unwrap_or(0),
        AnyElement::ExtensionType(e) => e
            .name
            .map(|n| ctx.name_str(n).encode_utf16().count() as u32)
            .unwrap_or(0),
        _ => ctx
            .element_data(element)
            .and_then(|d| d.name)
            .map(|n| ctx.name_str(n).encode_utf16().count() as u32)
            .unwrap_or("<unnamed>".len() as u32),
    }
}

fn push_interface_children(out: &mut Vec<FragmentId>, f: &InterfaceFragmentData) {
    out.extend(f.constructors.iter().map(|c| c.raw()));
    out.extend(f.fields.iter().map(|c| c.raw()));
    out.extend(f.getters.iter().map(|c| c.raw()));
    out.extend(f.methods.iter().map(|c| c.raw()));
    out.extend(f.setters.iter().map(|c| c.raw()));
    out.extend(f.type_params.iter().map(|c| c.raw()));
}

/// Dart `Fragment.children` in the exact order of `element.dart`.
fn fragment_children(ctx: &Ctx<'_>, fragment: FragmentId) -> Vec<FragmentId> {
    let mut out = Vec::new();
    match fragment.tag() {
        Tag::Library => {
            let f = ctx.fragment(FId::<LibraryFragment>::from_raw(fragment));
            out.extend(f.classes.iter().map(|c| c.raw()));
            out.extend(f.enums.iter().map(|c| c.raw()));
            out.extend(f.extensions.iter().map(|c| c.raw()));
            out.extend(f.extension_types.iter().map(|c| c.raw()));
            out.extend(f.functions.iter().map(|c| c.raw()));
            out.extend(f.getters.iter().map(|c| c.raw()));
            out.extend(f.mixins.iter().map(|c| c.raw()));
            out.extend(f.setters.iter().map(|c| c.raw()));
            out.extend(f.type_aliases.iter().map(|c| c.raw()));
            out.extend(f.variables.iter().map(|c| c.raw()));
        }
        Tag::Class => {
            let f = ctx.fragment(FId::<ClassFragment>::from_raw(fragment));
            push_interface_children(&mut out, &f.interface);
        }
        Tag::Enum => {
            let f = ctx.fragment(FId::<EnumFragment>::from_raw(fragment));
            push_interface_children(&mut out, &f.interface);
        }
        Tag::Mixin => {
            let f = ctx.fragment(FId::<MixinFragment>::from_raw(fragment));
            push_interface_children(&mut out, &f.interface);
        }
        Tag::ExtensionType => {
            let f = ctx.fragment(FId::<ExtensionTypeFragment>::from_raw(fragment));
            push_interface_children(&mut out, &f.interface);
        }
        Tag::Extension => {
            let f = ctx.fragment(FId::<ExtensionFragment>::from_raw(fragment));
            out.extend(f.fields.iter().map(|c| c.raw()));
            out.extend(f.getters.iter().map(|c| c.raw()));
            out.extend(f.methods.iter().map(|c| c.raw()));
            out.extend(f.setters.iter().map(|c| c.raw()));
            out.extend(f.type_params.iter().map(|c| c.raw()));
        }
        Tag::Constructor
        | Tag::Method
        | Tag::TopLevelFunction
        | Tag::LocalFunction
        | Tag::Getter
        | Tag::Setter => {
            let f = ctx
                .store(fragment.store())
                .executable_fragment(FId::<ExecutableFragment>::from_raw(fragment));
            out.extend(f.type_params.iter().map(|c| c.raw()));
            out.extend(f.formal_params.iter().map(|c| c.raw()));
        }
        Tag::TypeAlias => {
            let f = ctx.fragment(FId::<TypeAliasFragment>::from_raw(fragment));
            out.extend(f.type_params.iter().map(|c| c.raw()));
        }
        Tag::GenericFunctionType => {
            let f = ctx.fragment(FId::<GenericFunctionTypeFragment>::from_raw(fragment));
            out.extend(f.type_params.iter().map(|c| c.raw()));
            out.extend(f.formal_params.iter().map(|c| c.raw()));
        }
        _ => {}
    }
    out
}
