// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_type_hierarchy.dart
// Dart source: pkg/analysis_server/lib/src/computer/computer_lazy_type_hierarchy.dart
// Dart source: pkg/analysis_server/lib/src/utilities/element_location2.dart

//! `textDocument/prepareTypeHierarchy`, `typeHierarchy/supertypes` and
//! `typeHierarchy/subtypes`.

use dartr_ast::*;
use dartr_element::display_string::{DisplayOptions, type_display_string_with};
use dartr_element::{Ctx, ElementId, NoopSink, Tag, TypeId, TypeKind};
use dartr_resolver::error::support;
use dartr_typesystem::type_ext::TypeExt;
use serde_json::{Value, json};

use super::Server;
use super::search::SElem;
use crate::mapping::{self, ErrorOr, ResponseError, codes};
use crate::uri::path_to_uri;

/// LSP `SymbolKind.Class`.
const SYMBOL_KIND_CLASS: i64 = 5;

/// Dart `ElementLocation.forElement(element)?.encoding`.
pub(crate) fn element_location(ctx: &Ctx<'_>, element: ElementId) -> Option<String> {
    let library = support::library_of(ctx, element)?;
    let library_uri = ctx.fragment(ctx.get(library).first_fragment()).source.uri.to_string();
    let enclosing = ctx.element_data(element)?.enclosing;
    let lookup = |e: ElementId| dartr_typesystem::member::lookup_name(ctx, dartr_element::ElemRef::Base(e));
    if enclosing == Some(library.raw()) {
        let top = lookup(element)?;
        return Some(format!("{library_uri};{top}"));
    }
    let enclosing = enclosing?;
    if ctx.element_data(enclosing)?.enclosing == Some(library.raw()) {
        let member = lookup(element)?;
        let top = lookup(enclosing)?;
        return Some(format!("{library_uri};{top};{member}"));
    }
    None
}

/// Dart `LibraryElement.children` (the top-level elements).
pub(crate) fn library_children(ctx: &Ctx<'_>, library: dartr_element::EId<dartr_element::LibraryElement>) -> Vec<ElementId> {
    let l = ctx.get(library);
    let mut out: Vec<ElementId> = Vec::new();
    out.extend(l.classes.iter().map(|e| e.raw()));
    out.extend(l.enums.iter().map(|e| e.raw()));
    out.extend(l.extensions.iter().map(|e| e.raw()));
    out.extend(l.extension_types.iter().map(|e| e.raw()));
    out.extend(l.getters.iter().map(|e| e.raw()));
    out.extend(l.mixins.iter().map(|e| e.raw()));
    out.extend(l.setters.iter().map(|e| e.raw()));
    out.extend(l.top_level_functions.iter().map(|e| e.raw()));
    out.extend(l.top_level_variables.iter().map(|e| e.raw()));
    out.extend(l.type_aliases.iter().map(|e| e.raw()));
    out
}

/// Dart `Element.children` of a top-level element (its members).
pub(crate) fn element_children(ctx: &Ctx<'_>, element: ElementId) -> Vec<ElementId> {
    let Some(instance) = element.cast::<dartr_element::InstanceElement>() else {
        return Vec::new();
    };
    let data = ctx.instance(instance);
    let mut out: Vec<ElementId> = Vec::new();
    if let Some(interface) = element.cast::<dartr_element::InterfaceElement>() {
        out.extend(ctx.interface(interface).constructors.iter().map(|e| e.raw()));
    }
    out.extend(data.fields.iter().map(|e| e.raw()));
    out.extend(data.getters.iter().map(|e| e.raw()));
    out.extend(data.methods.iter().map(|e| e.raw()));
    out.extend(data.setters.iter().map(|e| e.raw()));
    out.extend(data.type_params.iter().map(|e| e.raw()));
    out
}

/// Dart `ElementLocation.decode(ref).locateIn(session)`: the element of
/// [encoded] in the element model of [ctx].
pub(crate) fn locate_element(ctx: &Ctx<'_>, encoded: &str) -> Option<ElementId> {
    let parts: Vec<&str> = encoded.split(';').collect();
    let (uri, top, member) = match parts.as_slice() {
        [uri, top] => (*uri, *top, None),
        [uri, top, member] => (*uri, *top, Some(*member)),
        _ => return None,
    };
    let library = ctx.world.libraries.get(uri).copied()?;
    let lookup = |e: ElementId| dartr_typesystem::member::lookup_name(ctx, dartr_element::ElemRef::Base(e));
    let top = library_children(ctx, library)
        .into_iter()
        .find(|&e| lookup(e).as_deref() == Some(top))?;
    match member {
        None => Some(top),
        Some(m) => element_children(ctx, top)
            .into_iter()
            .find(|&e| lookup(e).as_deref() == Some(m)),
    }
}

fn is_interface(e: ElementId) -> bool {
    matches!(e.tag(), Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType)
}

/// Dart `TypeHierarchyItem.forElement` as an LSP item.
fn type_hierarchy_item(server: &Server, ctx: &Ctx<'_>, element: ElementId) -> Option<Value> {
    let location = element_location(ctx, element)?;
    let interface = element.cast::<dartr_element::InterfaceElement>()?;
    let this_type = ctx.interface_this_type(interface);
    let name = type_display_string_with(ctx, this_type, DisplayOptions::default());
    let non_synthetic = dartr_element::diagnostics::non_synthetic(ctx, element);
    let first = ctx.element_data(non_synthetic)?.first_fragment;
    let data = ctx.fragment_data(first)?;
    let path = crate::navigation::fragment_path(ctx, first)?;
    let lines = server.line_info_of(&path)?;
    let name_length = data
        .name
        .map(|n| ctx.name_str(n).encode_utf16().count() as u32)
        .unwrap_or(0);
    let name_offset = data.name_offset.unwrap_or(0);
    let code = (data.code_offset.unwrap_or(0), data.code_length.unwrap_or(0));
    Some(json!({
        "name": name,
        "kind": SYMBOL_KIND_CLASS,
        "uri": path_to_uri(&path),
        "range": mapping::to_range(&lines, code.0, code.1),
        "selectionRange": mapping::to_range(&lines, name_offset, name_length),
        "data": {"ref": location},
    }))
}

impl Server {
    /// Dart `PrepareTypeHierarchyHandler.handle`.
    pub(crate) fn prepare_type_hierarchy(&mut self, params: &Value) -> ErrorOr<Value> {
        if !super::is_dart_document(params) {
            return Ok(json!([]));
        }
        let path = self.path_of_doc(params)?;
        let resolved = self.require_resolved_unit(&path)?;
        let line_info = resolved.line_info().clone();
        let offset = self.position_offset(&line_info, params)?;
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let ast = &unit.ast;
        let Some(node) = ast.node_covering(unit.unit.raw(), offset, 0) else {
            return Ok(Value::Null);
        };
        // Dart `findTarget`.
        let mut current = Some(node);
        let mut target = None;
        while let Some(n) = current {
            if ast.is::<NamedType>(n)
                || ast.is::<CommentReference>(n)
                || ast.is::<ClassDeclaration>(n)
                || ast.is::<MixinDeclaration>(n)
                || ast.is::<ExtensionTypeDeclaration>(n)
                || ast.is::<EnumDeclaration>(n)
            {
                target = Some(n);
                break;
            }
            current = ast.parent(n);
        }
        let Some(target) = target else {
            return Ok(Value::Null);
        };
        let element = if ast.is::<NamedType>(target) {
            unit.tables
                .annotation_type
                .get(target)
                .and_then(|&t| match *ctx.ty(t) {
                    TypeKind::Interface { element, .. } => Some(element.raw()),
                    _ => None,
                })
        } else if let Some(c) = ast.cast::<CommentReference>(target) {
            let expression = ast[c].expression.raw();
            if ast.is::<Identifier>(expression) {
                let element = unit.tables.element.get(expression).copied().or_else(|| {
                    let p = ast.cast::<PrefixedIdentifier>(expression)?;
                    unit.tables.element.get(ast[p].identifier.raw()).copied()
                });
                element
                    .map(|e| dartr_typesystem::member::base_element(&ctx, e))
                    .filter(|&e| is_interface(e))
            } else {
                None
            }
        } else {
            support::declared_element(&ctx, &unit.tables, target).filter(|&e| is_interface(e))
        };
        let Some(element) = element else {
            return Ok(Value::Null);
        };
        match type_hierarchy_item(self, &ctx, element) {
            Some(item) => Ok(json!([item])),
            None => Ok(Value::Null),
        }
    }

    /// The element of `params.item` (Dart `_findTargetElement`), with the
    /// resolved unit of the item file.
    fn type_hierarchy_target(&mut self, params: &Value) -> ErrorOr<Option<SElem>> {
        let item = params.get("item").cloned().unwrap_or(Value::Null);
        let uri = item.get("uri").and_then(Value::as_str).unwrap_or_default();
        let path = self.path_of_uri(uri)?;
        let resolved = self.require_resolved_unit(&path)?;
        let Some(reference) = item.pointer("/data/ref").and_then(Value::as_str) else {
            return Err(ResponseError::new(
                codes::INVALID_PARAMS,
                "TypeHierarchyItem is missing the data field",
            ));
        };
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let element = locate_element(&ctx, reference).filter(|&e| is_interface(e));
        Ok(element.map(|id| SElem {
            lib: resolved.library.clone(),
            unit: resolved.index,
            id,
        }))
    }

    /// Dart `TypeHierarchySupertypesHandler.handle`.
    pub(crate) fn type_hierarchy_supertypes(&mut self, params: &Value) -> ErrorOr<Value> {
        let Some(target) = self.type_hierarchy_target(params)? else {
            return Ok(Value::Null);
        };
        let items = target.with(|ctx| {
            let interface = target.id.cast::<dartr_element::InterfaceElement>().unwrap();
            let data = ctx.interface(interface);
            let mut types: Vec<TypeId> = Vec::new();
            // Dart `InterfaceType.superclass`.
            if let Some(s) = data.supertype.get() {
                types.push(s);
            }
            if let Some(mixin) = target.id.cast::<dartr_element::MixinElement>()
                && let Some(list) = ctx.get(mixin).superclass_constraints.get()
            {
                types.extend(ctx.list(list).iter().copied());
            }
            if let Some(list) = data.interfaces.get() {
                types.extend(ctx.list(list).iter().copied());
            }
            if let Some(list) = data.mixins.get() {
                types.extend(ctx.list(list).iter().copied());
            }
            types
                .into_iter()
                .filter_map(|t| match *ctx.ty(t) {
                    TypeKind::Interface { element, .. } => type_hierarchy_item(self, ctx, element.raw()),
                    _ => None,
                })
                .collect::<Vec<Value>>()
        });
        Ok(Value::Array(items))
    }

    /// Dart `TypeHierarchySubtypesHandler.handle`.
    pub(crate) fn type_hierarchy_subtypes(&mut self, params: &Value) -> ErrorOr<Value> {
        let Some(target) = self.type_hierarchy_target(params)? else {
            return Ok(Value::Null);
        };
        let matches = self.direct_subtypes_with_kinds(&target);
        let mut seen = Vec::new();
        let mut items = Vec::new();
        for (sub, _) in matches {
            let Some(identity) = sub.identity() else { continue };
            if seen.contains(&identity) {
                continue;
            }
            seen.push(identity);
            if let Some(item) = sub.with(|ctx| type_hierarchy_item(self, ctx, sub.id)) {
                items.push(item);
            }
        }
        Ok(Value::Array(items))
    }
}

// ---- call hierarchy ----
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_call_hierarchy.dart
// Dart source: pkg/analysis_server/lib/src/computer/computer_call_hierarchy.dart

/// Dart `CallHierarchyKind`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CallKind {
    Class,
    Constructor,
    Extension,
    File,
    Function,
    Method,
    Mixin,
    Property,
    Unknown,
}

impl CallKind {
    fn for_element(e: ElementId) -> CallKind {
        match e.tag() {
            Tag::Class => CallKind::Class,
            Tag::Library => CallKind::File,
            Tag::Constructor => CallKind::Constructor,
            Tag::Extension => CallKind::Extension,
            Tag::TopLevelFunction | Tag::LocalFunction => CallKind::Function,
            Tag::Getter | Tag::Setter => CallKind::Property,
            Tag::Method => CallKind::Method,
            Tag::Mixin => CallKind::Mixin,
            _ => CallKind::Unknown,
        }
    }

    /// `toSymbolKindMapping`.
    fn symbol_kind(self) -> Option<i64> {
        Some(match self {
            CallKind::Class | CallKind::Extension | CallKind::Mixin => 5,
            CallKind::Constructor => 9,
            CallKind::File => 1,
            CallKind::Function => 12,
            CallKind::Method => 6,
            CallKind::Property => 7,
            CallKind::Unknown => return None,
        })
    }

    fn from_symbol_kind(kind: i64) -> CallKind {
        match kind {
            5 => CallKind::Class,
            9 => CallKind::Constructor,
            1 => CallKind::File,
            12 => CallKind::Function,
            6 => CallKind::Method,
            7 => CallKind::Property,
            _ => CallKind::Unknown,
        }
    }
}

/// Dart `_getContainer`: [element] or its nearest ancestor of a container
/// kind.
fn container_of(ctx: &Ctx<'_>, element: ElementId) -> Option<ElementId> {
    let mut current = Some(element);
    while let Some(e) = current {
        if matches!(
            e.tag(),
            Tag::Class
                | Tag::Library
                | Tag::Constructor
                | Tag::Enum
                | Tag::Extension
                | Tag::ExtensionType
                | Tag::TopLevelFunction
                | Tag::LocalFunction
                | Tag::Getter
                | Tag::Method
                | Tag::Mixin
                | Tag::Setter
        ) {
            return Some(e);
        }
        current = ctx.element_data(e).and_then(|d| d.enclosing).or_else(|| {
            // Dart `firstFragment.enclosingFragment?.element`.
            let first = ctx.element_data(e)?.first_fragment;
            let enclosing = ctx.fragment_data(first)?.enclosing_fragment?;
            ctx.fragment_data(enclosing)?.element.try_get().copied()
        });
    }
    None
}

/// Dart `_getDisplayName`.
fn call_display_name(ctx: &Ctx<'_>, e: ElementId) -> String {
    match e.tag() {
        Tag::Library => {
            let library = e.cast::<dartr_element::LibraryElement>().unwrap();
            let path = ctx.fragment(ctx.get(library).first_fragment()).source.path.to_string();
            path.rsplit('/').next().unwrap_or_default().to_string()
        }
        Tag::Getter => format!("get {}", support::display_name(ctx, e)),
        Tag::Setter => format!("set {}", support::display_name(ctx, e)),
        _ => support::display_name(ctx, e),
    }
}

/// Dart `CallHierarchyItem`.
#[derive(Clone, Debug)]
struct CallItem {
    display_name: String,
    container_name: Option<String>,
    kind: CallKind,
    file: String,
    name_range: (u32, u32),
    code_range: (u32, u32),
}

/// Dart `CallHierarchyItem.forElement`.
fn call_item(ctx: &Ctx<'_>, element: ElementId) -> Option<CallItem> {
    let non_synthetic = dartr_element::diagnostics::non_synthetic(ctx, element);
    let first = ctx.element_data(non_synthetic)?.first_fragment;
    let data = ctx.fragment_data(first)?;
    let code_range = (data.code_offset.unwrap_or(0), data.code_length.unwrap_or(0));
    let name_range = match (data.name_offset, data.name) {
        (Some(o), Some(n)) => (o, ctx.name_str(n).encode_utf16().count() as u32),
        _ => {
            // A default constructor: the type name or the keyword.
            let mut range = (0, 0);
            if let Some(f) = first.cast::<dartr_element::ConstructorFragment>() {
                let c = ctx.fragment(f);
                if let (Some(o), Some(n)) = (c.type_name_offset, c.type_name) {
                    range = (o, ctx.name_str(n).encode_utf16().count() as u32);
                } else if let Some(o) = c.new_keyword_offset {
                    range = (o, 3);
                } else if let Some(o) = c.factory_keyword_offset {
                    range = (o, 7);
                }
            }
            range
        }
    };
    let file = crate::navigation::fragment_path(ctx, ctx.element_data(element)?.first_fragment)?;
    let enclosing = ctx.element_data(element).and_then(|d| d.enclosing).or_else(|| {
        let first = ctx.element_data(element)?.first_fragment;
        let enclosing = ctx.fragment_data(first)?.enclosing_fragment?;
        ctx.fragment_data(enclosing)?.element.try_get().copied()
    });
    let container_name = enclosing
        .and_then(|e| container_of(ctx, e))
        .map(|c| call_display_name(ctx, c));
    Some(CallItem {
        display_name: call_display_name(ctx, element),
        container_name,
        kind: CallKind::for_element(element),
        file,
        name_range,
        code_range,
    })
}

/// Dart `ElementLocator.locate` with the call hierarchy adjustments
/// (`_getElementOfNode`).
fn call_element_of_node(unit: &crate::element_locator::Unit<'_, '_>, node: NodeId) -> Option<ElementId> {
    let ast = unit.ast;
    let ctx = unit.ctx;
    let parent = ast.parent(node);
    let element_of = |n: NodeId| {
        unit.tables
            .element
            .get(n)
            .map(|&e| dartr_typesystem::member::base_element(ctx, e))
    };
    let mut node = node;
    if ast.is::<NamedType>(node) && parent.is_some_and(|p| ast.is::<ConstructorName>(p)) {
        return element_of(parent.unwrap());
    } else if ast.is::<ConstructorName>(node) {
        return element_of(node);
    } else if let Some(p) = ast.cast::<PropertyAccess>(node) {
        node = ast[p].property_name.raw();
    }
    let mut element = unit.locate(node)?;
    if element.tag() == Tag::Class && ast.is::<PrimaryConstructorDeclaration>(node) {
        element = dartr_resolver::scope_context::primary_constructor_of(ctx, element.cast()?)?.raw();
    }
    if matches!(element.tag(), Tag::Getter | Tag::Setter)
        && dartr_resolver::element_metadata::is_origin_variable(ctx, element)
    {
        return None;
    }
    Some(element)
}

/// Dart `_findTargetNode`.
fn call_target_node(ast: &Ast, root: NodeId, offset: u32) -> Option<NodeId> {
    let node = ast.node_covering(root, offset, 0)?;
    let parent = ast.parent(node);
    if ast.is::<NamedType>(node)
        && let Some(cn) = parent.and_then(|p| ast.cast::<ConstructorName>(p))
        && let Some(name) = ast[cn].name
        && offset < ast.offset(name.raw())
    {
        return None;
    }
    if ast.is::<Identifier>(node)
        && let Some(cd) = parent.and_then(|p| ast.cast::<ConstructorDeclaration>(p))
    {
        match ast[cd].name {
            Some(name) if offset < ast.tokens.get(name).offset => return None,
            None => return parent,
            _ => {}
        }
    }
    if let Some(pc) = ast.cast::<PrimaryConstructorDeclaration>(node)
        && let Some(cn) = ast[pc].constructor_name
        && offset < ast.tokens.get(ast[cn].name).offset
    {
        return None;
    }
    if ast.is::<PrimaryConstructorName>(node) {
        return parent;
    }
    Some(node)
}

/// Dart `_OutboundCallVisitor`.
struct OutboundCalls<'u, 'c, 'a> {
    unit: &'u crate::element_locator::Unit<'c, 'a>,
    root: NodeId,
    nodes: Vec<NodeId>,
}

impl OutboundCalls<'_, '_, '_> {
    fn collect(&mut self, node: NodeId) {
        if !self.nodes.contains(&node) {
            self.nodes.push(node);
        }
    }
}

impl AstVisitor for OutboundCalls<'_, '_, '_> {
    fn visit_constructor_name(&mut self, ast: &Ast, node: Id<ConstructorName>) {
        self.collect(ast[node].name.map(|n| n.raw()).unwrap_or(node.raw()));
        ast.visit_children(node.raw(), self);
    }

    fn visit_dot_shorthand_constructor_invocation(
        &mut self,
        ast: &Ast,
        node: Id<DotShorthandConstructorInvocation>,
    ) {
        self.collect(ast[node].constructor_name.raw());
        ast.visit_children(node.raw(), self);
    }

    fn visit_dot_shorthand_invocation(&mut self, ast: &Ast, node: Id<DotShorthandInvocation>) {
        self.collect(ast[node].member_name.raw());
        ast.visit_children(node.raw(), self);
    }

    fn visit_dot_shorthand_property_access(&mut self, ast: &Ast, node: Id<DotShorthandPropertyAccess>) {
        self.collect(ast[node].property_name.raw());
        ast.visit_children(node.raw(), self);
    }

    fn visit_function_declaration(&mut self, ast: &Ast, node: Id<FunctionDeclaration>) {
        if node.raw() == self.root {
            ast.visit_children(node.raw(), self);
        }
    }

    fn visit_function_reference(&mut self, ast: &Ast, node: Id<FunctionReference>) {
        self.collect(node.raw());
        ast.visit_children(node.raw(), self);
    }

    fn visit_method_invocation(&mut self, ast: &Ast, node: Id<MethodInvocation>) {
        self.collect(ast[node].method_name.raw());
        ast.visit_children(node.raw(), self);
    }

    fn visit_prefixed_identifier(&mut self, ast: &Ast, node: Id<PrefixedIdentifier>) {
        if !ast.parent(node.raw()).is_some_and(|p| ast.is::<NamedType>(p)) {
            self.collect(ast[node].identifier.raw());
        }
        ast.visit_children(node.raw(), self);
    }

    fn visit_property_access(&mut self, ast: &Ast, node: Id<PropertyAccess>) {
        self.collect(ast[node].property_name.raw());
        ast.visit_children(node.raw(), self);
    }

    fn visit_simple_identifier(&mut self, ast: &Ast, node: Id<SimpleIdentifier>) {
        let element = self
            .unit
            .tables
            .element
            .get(node.raw())
            .map(|&e| dartr_typesystem::member::base_element(self.unit.ctx, e));
        if element.is_some_and(|e| matches!(e.tag(), Tag::LocalFunction | Tag::TopLevelFunction))
            && !in_declaration_context(ast, node)
        {
            self.collect(node.raw());
        }
        ast.visit_children(node.raw(), self);
    }
}

/// Dart `_rangeForNode`.
fn range_for_node(ast: &Ast, node: NodeId) -> (u32, u32) {
    if let Some(m) = ast.cast::<MethodInvocation>(node) {
        let n = ast[m].method_name.raw();
        return (ast.offset(n), ast.length(n));
    }
    if let Some(i) = ast.cast::<InstanceCreationExpression>(node) {
        let n = ast[i].constructor_name.raw();
        return (ast.offset(n), ast.length(n));
    }
    if let Some(p) = ast.cast::<PropertyAccess>(node) {
        let n = ast[p].property_name.raw();
        return (ast.offset(n), ast.length(n));
    }
    (ast.offset(node), ast.length(node))
}

/// The innermost element whose declaration contains [offset] (Dart
/// `_getEnclosingFragment(...).element` of a search result).
fn enclosing_element(unit: &crate::element_locator::Unit<'_, '_>, root: NodeId, offset: u32) -> Option<ElementId> {
    let ast = unit.ast;
    let mut node = ast.node_covering(root, offset, 0);
    while let Some(n) = node {
        if ast.is::<ConstructorDeclaration>(n)
            || ast.is::<FunctionDeclaration>(n)
            || ast.is::<MethodDeclaration>(n)
            || ast.is::<ClassDeclaration>(n)
            || ast.is::<ClassTypeAlias>(n)
            || ast.is::<EnumDeclaration>(n)
            || ast.is::<ExtensionDeclaration>(n)
            || ast.is::<ExtensionTypeDeclaration>(n)
            || ast.is::<MixinDeclaration>(n)
            || ast.is::<VariableDeclaration>(n)
            || ast.is::<TopLevelVariableDeclaration>(n)
        {
            if let Some(e) = unit.declared_element(n) {
                return Some(e);
            }
        }
        node = ast.parent(n);
    }
    let unit_node = ast.cast::<CompilationUnit>(root)?;
    let fragment = unit.tables.declared_fragment.get(unit_node.raw())?;
    let library = fragment.cast::<dartr_element::LibraryFragment>()?;
    Some(unit.ctx.fragment(library).library.raw())
}

impl Server {
    fn call_item_json(&self, item: &CallItem) -> Option<Value> {
        let lines = self.line_info_of(&item.file)?;
        let supported = self.client.document_symbol_kinds();
        let mut kind = item.kind.symbol_kind();
        if let Some(k) = kind
            && !supported.contains(&k)
        {
            kind = if k == 1 { Some(2) } else { None };
        }
        let mut v = json!({
            "name": item.display_name,
            "kind": kind.unwrap_or(19),
            "uri": path_to_uri(&item.file),
            "range": mapping::to_range(&lines, item.code_range.0, item.code_range.1),
            "selectionRange": mapping::to_range(&lines, item.name_range.0, item.name_range.1),
        });
        if let Some(c) = &item.container_name {
            v["detail"] = json!(c);
        }
        Some(v)
    }

    /// Dart `PrepareCallHierarchyHandler.handle`.
    pub(crate) fn prepare_call_hierarchy(&mut self, params: &Value) -> ErrorOr<Value> {
        if !super::is_dart_document(params) {
            return Ok(json!([]));
        }
        let path = self.path_of_doc(params)?;
        let resolved = self.require_resolved_unit(&path)?;
        let line_info = resolved.line_info().clone();
        let offset = self.position_offset(&line_info, params)?;
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let u = crate::element_locator::Unit {
            ctx: &ctx,
            ast: &unit.ast,
            tables: &unit.tables,
        };
        let element = call_target_node(&unit.ast, unit.unit.raw(), offset)
            .and_then(|n| call_element_of_node(&u, n))
            .filter(|&e| crate::element_locator::is_executable(e));
        let Some(item) = element.and_then(|e| call_item(&ctx, e)) else {
            return Ok(Value::Null);
        };
        match self.call_item_json(&item) {
            Some(v) => Ok(json!([v])),
            None => Err(ResponseError::new(
                codes::INTERNAL_ERROR,
                format!(
                    "Call Hierarchy target was in an unavailable file: {} in {}",
                    item.display_name, item.file
                ),
            )),
        }
    }

    /// Dart `toServerItem` and the target element of the item
    /// (`findIncomingCalls`/`findOutgoingCalls`): the node at the name and its
    /// element, when the name still matches.
    fn call_target(&mut self, params: &Value) -> ErrorOr<Option<(super::nav::ResolvedUnitRef, NodeId, ElementId, CallKind)>> {
        let item = params.get("item").cloned().unwrap_or(Value::Null);
        let uri = item.get("uri").and_then(Value::as_str).unwrap_or_default();
        let path = self.path_of_uri(uri)?;
        let resolved = self.require_resolved_unit(&path)?;
        let line_info = resolved.line_info().clone();
        let read = |key: &str| -> Option<(u32, u32)> {
            let start = item.pointer(&format!("/{key}/start")).and_then(mapping::read_position)?;
            let end = item.pointer(&format!("/{key}/end")).and_then(mapping::read_position)?;
            let s = mapping::to_offset(&line_info, start.0, start.1, false).ok()?;
            let e = mapping::to_offset(&line_info, end.0, end.1, false).ok()?;
            Some((s, e.saturating_sub(s)))
        };
        let (Some(name_range), Some(_)) = (read("selectionRange"), read("range")) else {
            return Err(ResponseError::new(
                codes::CONTENT_MODIFIED,
                "Content was modified since Call Hierarchy node was produced",
            ));
        };
        let name = item.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
        let kind = CallKind::from_symbol_kind(item.get("kind").and_then(Value::as_i64).unwrap_or(0));
        let found = {
            let sink = NoopSink;
            let ctx = resolved.ctx(&sink);
            let unit = resolved.unit();
            let u = crate::element_locator::Unit {
                ctx: &ctx,
                ast: &unit.ast,
                tables: &unit.tables,
            };
            let node = call_target_node(&unit.ast, unit.unit.raw(), name_range.0);
            node.and_then(|n| call_element_of_node(&u, n).map(|e| (n, e)))
                .filter(|(_, e)| call_display_name(&ctx, *e) == name)
        };
        Ok(found.map(|(n, e)| (resolved, n, e, kind)))
    }

    /// Dart `IncomingCallHierarchyHandler.handle`.
    pub(crate) fn call_hierarchy_incoming(&mut self, params: &Value) -> ErrorOr<Value> {
        let Some((resolved, _, mut element, kind)) = self.call_target(params)? else {
            return Ok(json!([]));
        };
        if is_interface(element) && kind == CallKind::Constructor {
            let unnamed = {
                let sink = NoopSink;
                let ctx = resolved.ctx(&sink);
                let interface = element.cast::<dartr_element::InterfaceElement>().unwrap();
                ctx.interface(interface)
                    .constructors
                    .iter()
                    .map(|c| c.raw())
                    .find(|&c| support::display_name(&ctx, c).is_empty() || ctx.element_data(c).and_then(|d| d.name).map(|n| ctx.name_str(n) == "new").unwrap_or(false))
            };
            match unnamed {
                Some(c) => element = c,
                None => return Ok(json!([])),
            }
        }
        if !crate::element_locator::is_executable(element) {
            return Ok(json!([]));
        }
        let target = SElem {
            lib: resolved.library.clone(),
            unit: resolved.index,
            id: element,
        };
        let matches = self.element_references(&target);
        // Group by container (Dart: a map by element identity).
        let mut groups: Vec<((usize, crate::index::ElementKey), CallItem, Vec<(u32, u32)>)> = Vec::new();
        for m in matches {
            let Ok(r) = self.require_resolved_unit_in(&m.path, Some(m.context)) else {
                continue;
            };
            let sink = NoopSink;
            let ctx = r.ctx(&sink);
            let unit = r.unit();
            let u = crate::element_locator::Unit {
                ctx: &ctx,
                ast: &unit.ast,
                tables: &unit.tables,
            };
            let Some(enclosing) = enclosing_element(&u, unit.unit.raw(), m.offset) else {
                continue;
            };
            let Some(container) = container_of(&ctx, enclosing) else {
                continue;
            };
            let Some(key) = crate::index::element_key(&ctx, container) else {
                continue;
            };
            let identity = (m.context, key);
            // Dart `_rangeForSearchMatch`.
            let ast = &unit.ast;
            let mut range = (m.offset, m.length);
            if let Some(node) = ast.node_covering(unit.unit.raw(), m.offset, 0) {
                let parent = ast.parent(node);
                if ast.is::<SimpleIdentifier>(node)
                    && let Some(mi) = parent.and_then(|p| ast.cast::<MethodInvocation>(p))
                {
                    let n = ast[mi].method_name.raw();
                    range = (ast.offset(n), ast.length(n));
                } else if m.length == 0 {
                    range = (ast.offset(node), ast.length(node));
                }
            }
            match groups.iter_mut().find(|g| g.0 == identity) {
                Some(g) => g.2.push(range),
                None => {
                    let Some(item) = call_item(&ctx, container) else { continue };
                    groups.push((identity, item, vec![range]));
                }
            }
        }
        let mut out = Vec::new();
        for (_, item, ranges) in groups {
            let Some(lines) = self.line_info_of(&item.file) else { continue };
            let Some(from) = self.call_item_json(&item) else { continue };
            let from_ranges: Vec<Value> =
                ranges.iter().map(|r| mapping::to_range(&lines, r.0, r.1)).collect();
            out.push(json!({"from": from, "fromRanges": from_ranges}));
        }
        Ok(Value::Array(out))
    }

    /// Dart `OutgoingCallHierarchyHandler.handle`.
    pub(crate) fn call_hierarchy_outgoing(&mut self, params: &Value) -> ErrorOr<Value> {
        let Some((resolved, mut node, _, _)) = self.call_target(params)? else {
            return Ok(json!([]));
        };
        let local_lines = resolved.line_info().clone();
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let ast = &unit.ast;
        let u = crate::element_locator::Unit {
            ctx: &ctx,
            ast,
            tables: &unit.tables,
        };
        if let Some(pc) = ast.cast::<PrimaryConstructorDeclaration>(node)
            && let Some(body) = primary_constructor_body(ast, pc)
        {
            node = body;
        }
        if !(ast.is::<FunctionDeclaration>(node)
            || ast.is::<ConstructorDeclaration>(node)
            || ast.is::<MethodDeclaration>(node)
            || ast.is::<PrimaryConstructorBody>(node))
        {
            return Ok(json!([]));
        }
        let mut visitor = OutboundCalls {
            unit: &u,
            root: node,
            nodes: Vec::new(),
        };
        ast.accept(node, &mut visitor);
        let mut groups: Vec<(ElementId, CallItem, Vec<(u32, u32)>)> = Vec::new();
        for n in visitor.nodes {
            let Some(target) = call_element_of_node(&u, n) else { continue };
            let range = range_for_node(ast, n);
            match groups.iter_mut().find(|g| g.0 == target) {
                Some(g) => g.2.push(range),
                None => {
                    let Some(item) = call_item(&ctx, target) else { continue };
                    groups.push((target, item, vec![range]));
                }
            }
        }
        let mut out = Vec::new();
        for (_, item, ranges) in groups {
            if self.line_info_of(&item.file).is_none() {
                continue;
            }
            let Some(to) = self.call_item_json(&item) else { continue };
            let from_ranges: Vec<Value> = ranges
                .iter()
                .map(|r| mapping::to_range(&local_lines, r.0, r.1))
                .collect();
            out.push(json!({"to": to, "fromRanges": from_ranges}));
        }
        Ok(Value::Array(out))
    }
}

/// Dart `PrimaryConstructorDeclaration.body`: the `this` body in the
/// members of the declaration.
fn primary_constructor_body(ast: &Ast, node: Id<PrimaryConstructorDeclaration>) -> Option<NodeId> {
    let declaration = ast.parent(node.raw())?;
    let body = if let Some(c) = ast.cast::<ClassDeclaration>(declaration) {
        ast[c].body.raw()
    } else if let Some(e) = ast.cast::<ExtensionTypeDeclaration>(declaration) {
        ast[e].body.raw()
    } else {
        return None;
    };
    let block = ast.cast::<BlockClassBody>(body)?;
    ast.list_raw(ast[block].members)
        .iter()
        .copied()
        .find(|&m| ast.is::<PrimaryConstructorBody>(m))
}

/// Dart `SimpleIdentifier.inDeclarationContext`.
fn in_declaration_context(ast: &Ast, node: Id<SimpleIdentifier>) -> bool {
    let Some(parent) = ast.parent(node.raw()) else {
        return false;
    };
    if let Some(i) = ast.cast::<ImportDirective>(parent) {
        return ast[i].prefix == Some(node);
    }
    if ast.is::<Label>(parent) {
        return ast
            .parent(parent)
            .is_some_and(|g| ast.is::<Statement>(g) || ast.is::<SwitchMember>(g));
    }
    false
}
