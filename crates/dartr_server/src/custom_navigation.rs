// Dart source: pkg/analysis_server/lib/src/lsp/handlers/custom/handler_super.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/custom/handler_augmentation.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/custom/handler_augmented.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/custom/handler_update_diagnostic_information.dart
// Dart source: pkg/analysis_server/lib/src/lsp/mapping.dart (fragmentToLocation)

//! Custom `dart/textDocument/{super,augmentation,augmented}` and
//! `dart/updateDiagnosticInformation` handlers.

use dartr_ast::*;
use dartr_element::{
    ConstructorElement, ConstructorFragment, Ctx, ElemRef, ElementId, FragmentFlags, FragmentId,
    InterfaceElement, LibraryFragment, NoopSink, Tag, TypeKind,
};
use dartr_resolver::element_ext;
use dartr_syntax::LineInfo;
use dartr_typesystem::inheritance_manager3::{InheritanceManager3, Name};
use dartr_typesystem::member;
use serde_json::{Value, json};

use crate::element_locator::{Unit, is_executable};
use crate::mapping::{ErrorOr, to_range};
use crate::navigation::library_fragment_of;
use crate::server::{Server, is_dart_document};
use crate::uri::path_to_uri;

impl Server {
    /// Handles `dart/textDocument/super` (Dart `SuperHandler.handle`).
    pub(crate) fn dart_super(&mut self, params: &Value) -> ErrorOr<Value> {
        if !is_dart_document(params) {
            return Ok(Value::Null);
        }
        let path = self.path_of_doc(params)?;
        let resolved = self.require_resolved_unit(&path)?;
        let offset = self.position_offset(resolved.line_info(), params)?;

        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let r_unit = resolved.unit();
        let ast = &r_unit.ast;
        let unit = Unit {
            ctx: &ctx,
            ast,
            tables: &r_unit.tables,
        };

        let Some(covering) = ast.node_covering(r_unit.unit, offset, 0) else {
            return Ok(Value::Null);
        };
        let Some(node) = ast.this_or_ancestor_matching(covering, can_have_super) else {
            return Ok(Value::Null);
        };
        let Some(mut element) = unit.locate(node) else {
            return Ok(Value::Null);
        };

        if let Some(primary) = ast.cast::<PrimaryConstructorDeclaration>(node)
            && element.tag() == Tag::Class
            && offset > ast.tokens.get(ast[primary].type_name).end()
        {
            if let Some(ctor) = unit.declared_element(primary.raw()).or_else(|| {
                let iface = element.cast::<InterfaceElement>()?;
                ctx.interface(iface)
                    .constructors
                    .iter()
                    .find(|&&c| {
                        element_ext::first_fragment_flags(&ctx, c.raw())
                            .contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_PRIMARY)
                    })
                    .map(|c| c.raw())
            }) {
                element = ctor;
            }
        }

        let target_fragment = compute_super(&ctx, element);
        Ok(fragment_to_location(&ctx, target_fragment).unwrap_or(Value::Null))
    }

    /// Handles `dart/textDocument/augmentation` (Dart `AugmentationHandler.handle`).
    pub(crate) fn dart_augmentation(&mut self, params: &Value) -> ErrorOr<Value> {
        if !is_dart_document(params) {
            return Ok(Value::Null);
        }
        let path = self.path_of_doc(params)?;
        let resolved = self.require_resolved_unit(&path)?;
        let offset = self.position_offset(resolved.line_info(), params)?;

        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let r_unit = resolved.unit();
        let ast = &r_unit.ast;

        let target_fragment = ast
            .node_covering(r_unit.unit, offset, 0)
            .and_then(|c| ast.this_or_ancestor_of_type::<Declaration>(c))
            .and_then(|decl| r_unit.tables.declared_fragment.get(decl.raw()).copied())
            .and_then(|frag| ctx.fragment_data(frag)?.next_fragment);

        Ok(fragment_to_location(&ctx, target_fragment).unwrap_or(Value::Null))
    }

    /// Handles `dart/textDocument/augmented` (Dart `AugmentedHandler.handle`).
    pub(crate) fn dart_augmented(&mut self, params: &Value) -> ErrorOr<Value> {
        if !is_dart_document(params) {
            return Ok(Value::Null);
        }
        let path = self.path_of_doc(params)?;
        let resolved = self.require_resolved_unit(&path)?;
        let offset = self.position_offset(resolved.line_info(), params)?;

        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let r_unit = resolved.unit();
        let ast = &r_unit.ast;

        let target_fragment = ast
            .node_covering(r_unit.unit, offset, 0)
            .and_then(|c| ast.this_or_ancestor_of_type::<Declaration>(c))
            .and_then(|decl| r_unit.tables.declared_fragment.get(decl.raw()).copied())
            .and_then(|frag| ctx.fragment_data(frag)?.previous_fragment);

        Ok(fragment_to_location(&ctx, target_fragment).unwrap_or(Value::Null))
    }

    /// Handles `dart/updateDiagnosticInformation`
    /// (Dart `UpdateDiagnosticInformationHandler.handle`).
    pub(crate) fn update_diagnostic_information(&mut self, params: &Value) -> ErrorOr<Value> {
        self.client_diagnostic_information = Some(params.clone());
        Ok(Value::Null)
    }
}

/// Dart `SuperHandler._canHaveSuper`.
fn can_have_super(ast: &Ast, node: NodeId) -> bool {
    let mut test_node = Some(node);
    if ast.is::<VariableDeclaration>(node) {
        if let Some(parent) = ast.parent(node)
            && ast.is::<VariableDeclarationList>(parent)
        {
            test_node = ast.parent(parent);
        }
    } else if ast.is::<VariableDeclarationList>(node) {
        test_node = ast.parent(node);
    }

    let Some(test_node) = test_node else {
        return false;
    };
    ast.is::<ClassDeclaration>(test_node)
        || ast.is::<ClassMember>(test_node)
        || ast.is::<PrimaryConstructorDeclaration>(test_node)
}

/// Dart `_SuperComputer.computeSuper`.
fn compute_super(ctx: &Ctx<'_>, element: ElementId) -> Option<FragmentId> {
    if let Some(ctor) = element.cast::<ConstructorElement>() {
        let super_ctor = ctx.get(ctor).super_constructor.get()?;
        let base = member::base_element(ctx, super_ctor);
        return last_fragment(ctx, base);
    }
    if let Some(iface) = element.cast::<InterfaceElement>() {
        let supertype = ctx.interface(iface).supertype.get()?;
        let TypeKind::Interface {
            element: super_iface,
            ..
        } = *ctx.ty(supertype)
        else {
            return None;
        };
        return Some(ctx.element_data(super_iface.raw())?.first_fragment);
    }
    find_super_member(ctx, element)
}

fn find_super_member(ctx: &Ctx<'_>, element: ElementId) -> Option<FragmentId> {
    if !is_executable(element) && element.tag() != Tag::Field {
        return None;
    }
    let name = Name::for_element(ctx, ElemRef::Base(element))?;
    let interface_element = this_or_ancestor_interface(ctx, element)?;
    let manager = InheritanceManager3::new(ctx.global());
    let inherited = manager.get_inherited(interface_element, name)?;
    last_fragment(ctx, member::base_element(ctx, inherited))
}

fn this_or_ancestor_interface(
    ctx: &Ctx<'_>,
    mut element: ElementId,
) -> Option<dartr_element::EId<InterfaceElement>> {
    loop {
        if let Some(iface) = element.cast::<InterfaceElement>() {
            return Some(iface);
        }
        element = ctx.element_data(element)?.enclosing?;
    }
}

fn last_fragment(ctx: &Ctx<'_>, element: ElementId) -> Option<FragmentId> {
    let mut fragment = ctx.element_data(element)?.first_fragment;
    while let Some(next) = ctx.fragment_data(fragment)?.next_fragment {
        fragment = next;
    }
    Some(fragment)
}

/// Dart `fragmentToLocation` (`pkg/analysis_server/lib/src/lsp/mapping.dart`).
pub fn fragment_to_location(ctx: &Ctx<'_>, fragment: Option<FragmentId>) -> Option<Value> {
    let fragment = fragment?;
    let lib_frag = library_fragment_of(ctx, fragment)?.cast::<LibraryFragment>()?;
    let lib_data = ctx.fragment(lib_frag);
    let source_path = &lib_data.source.path;

    let frag_data = ctx.fragment_data(fragment)?;
    let (mut name_offset, mut name_length) = if matches!(fragment.tag(), Tag::Getter | Tag::Setter)
        && !frag_data
            .flags
            .get()
            .contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_DECLARATION)
    {
        let element = frag_data.element.try_get().copied()?;
        let non_synthetic = dartr_element::diagnostics::non_synthetic(ctx, element);
        let first = ctx.element_data(non_synthetic)?.first_fragment;
        let first_data = ctx.fragment_data(first)?;
        (
            first_data.name_offset,
            first_data
                .name
                .map(|n| ctx.name_str(n).encode_utf16().count() as u32),
        )
    } else {
        (
            frag_data.name_offset,
            frag_data
                .name
                .map(|n| ctx.name_str(n).encode_utf16().count() as u32),
        )
    };

    if name_offset.is_none()
        && let Some(ctor_frag) = fragment.cast::<ConstructorFragment>()
    {
        let c = ctx.fragment(ctor_frag);
        name_offset = c.type_name_offset;
        name_length = c
            .type_name
            .map(|n| ctx.name_str(n).encode_utf16().count() as u32);
    }

    let name_offset = name_offset?;
    let name_length = name_length?;
    let line_info = LineInfo::new(lib_data.line_starts.to_vec());

    Some(json!({
        "uri": path_to_uri(source_path),
        "range": to_range(&line_info, name_offset, name_length),
    }))
}
