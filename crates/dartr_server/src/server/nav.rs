// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_definition.dart
// Dart source: pkg/analysis_server/lib/src/utilities/navigation/keyword_navigation_computer.dart
// Dart source: pkg/analysis_server/lib/src/lsp/mapping.dart (navigationTargetToLocation, navigationTargetToLocationLink)

//! The navigation requests of the server: `textDocument/definition`.

use std::sync::Arc;

use dartr_ast::*;
use dartr_cli::driver_provider::ResolvedLibraryResult;
use dartr_element::{Ctx, FragmentId, NoopSink, Tag};
use dartr_syntax::LineInfo;
use serde_json::{Value, json};

use super::Server;
use crate::mapping::{self, ErrorOr, ResponseError, codes};
use crate::navigation::{self, Collector, Target};
use crate::uri::path_to_uri;

/// A resolved unit: its library and its index (Dart `ResolvedUnitResult`).
pub(crate) struct ResolvedUnitRef {
    pub library: Arc<ResolvedLibraryResult>,
    pub index: usize,
}

impl ResolvedUnitRef {
    pub fn unit(&self) -> &dartr_resolver::library_analyzer::ResolvedUnit {
        &self.library.library.units[self.index]
    }

    pub fn line_info(&self) -> &LineInfo {
        &self.library.inputs[self.index].parsed.line_info
    }

    pub fn ctx<'a>(&'a self, sink: &'a NoopSink) -> Ctx<'a> {
        self.library.ctx(self.index, sink)
    }
}

impl Server {
    /// Dart `requireResolvedUnit(path)`.
    pub(crate) fn require_resolved_unit(&mut self, path: &str) -> ErrorOr<ResolvedUnitRef> {
        self.require_resolved_unit_in(path, None)
    }

    /// [Self::require_resolved_unit] with the driver of [context] (the
    /// context of [path] when `None`).
    pub(crate) fn require_resolved_unit_in(
        &mut self,
        path: &str,
        context: Option<usize>,
    ) -> ErrorOr<ResolvedUnitRef> {
        let not_analyzed = || {
            ResponseError::with_data(codes::FILE_NOT_ANALYZED, "File is not being analyzed", path)
        };
        if !path.ends_with(".dart") {
            return Err(not_analyzed());
        }
        let Some(collection) = &self.collection else {
            return Err(not_analyzed());
        };
        if self.content(path).is_none() {
            return Err(ResponseError::with_data(
                codes::INVALID_FILE_PATH,
                "File does not exist",
                path,
            ));
        }
        let library = match context {
            Some(context) => self.session.resolved_library_in(collection, context, path),
            None => self.session.resolved_library(collection, path),
        }
        .ok_or_else(not_analyzed)?;
        let index = library.unit_index(path).ok_or_else(not_analyzed)?;
        Ok(ResolvedUnitRef { library, index })
    }

    /// Dart `server.getLineInfo(path)`: the line info of the content of
    /// [path] (the overlay or the file).
    pub(crate) fn line_info_of(&self, path: &str) -> Option<LineInfo> {
        self.content(path).map(|c| LineInfo::from_content(&c))
    }

    /// The offset of `params.position` in [line_info] (Dart `toOffset`).
    pub(crate) fn position_offset(&self, line_info: &LineInfo, params: &Value) -> ErrorOr<u32> {
        let (line, character) = params
            .get("position")
            .and_then(mapping::read_position)
            .ok_or_else(|| ResponseError::new(codes::INVALID_PARAMS, "Invalid params"))?;
        mapping::to_offset(line_info, line, character, false)
    }

    /// Dart `DefinitionHandler.handle`.
    pub(crate) fn definition(&mut self, params: &Value) -> ErrorOr<Value> {
        let supports_link = self
            .client
            .raw
            .pointer("/textDocument/definition/linkSupport")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let path = self.path_of_doc(params)?;
        let resolved = self.require_resolved_unit(&path).ok();
        let line_info = match &resolved {
            Some(r) => Some(r.line_info().clone()),
            None => self.line_info_of(&path),
        };
        let Some(line_info) = line_info else {
            return Ok(json!([]));
        };
        let offset = self.position_offset(&line_info, params)?;
        let Some(resolved) = resolved else {
            return Ok(json!([]));
        };
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let mut collector = navigation::compute(&ctx, &unit.ast, &unit.tables, unit.unit, offset);
        if collector.regions.is_empty() {
            keyword_navigation(
                &ctx,
                &unit.ast,
                &unit.tables,
                unit.unit,
                offset,
                &path,
                &mut collector,
            );
        }
        let code_locations: Vec<Option<(u32, u32)>> = if supports_link {
            collector
                .targets
                .iter()
                .map(|t| t.fragment.and_then(|f| self.code_location(&ctx, f)))
                .collect()
        } else {
            vec![None; collector.targets.len()]
        };
        let regions = collector.sorted_regions();
        let Some(((region_offset, region_length), _)) = regions.first().cloned() else {
            return Ok(json!([]));
        };
        let source_uri = path_to_uri(&path);
        let source_line = params
            .get("position")
            .and_then(mapping::read_position)
            .map(|(l, _)| l)
            .unwrap_or(0);
        let mut results: Vec<(String, u32, Value)> = Vec::new();
        for (target, code) in collector.targets.iter().zip(&code_locations) {
            let Some(target_lines) = self.line_info_of(&target.file) else {
                continue;
            };
            let uri = path_to_uri(&target.file);
            let name_range = mapping::to_range(&target_lines, target.offset, target.length);
            let start_line = target_lines.get_location(target.offset).line_number - 1;
            let value = if supports_link {
                let code_range = match code {
                    Some((o, l)) => mapping::to_range(&target_lines, *o, *l),
                    None => name_range.clone(),
                };
                json!({
                    "originSelectionRange": mapping::to_range(&line_info, region_offset, region_length),
                    "targetUri": uri,
                    "targetRange": code_range,
                    "targetSelectionRange": name_range,
                })
            } else {
                json!({"uri": uri, "range": name_range})
            };
            results.push((uri, start_line, value));
        }
        // Dart `_filterResults`: drop results on the same line of the same
        // file (for example `var` and the variable name).
        let others: Vec<Value> = results
            .iter()
            .filter(|(uri, line, _)| *uri != source_uri || *line != source_line)
            .map(|(_, _, v)| v.clone())
            .collect();
        if !others.is_empty() {
            return Ok(Value::Array(others));
        }
        Ok(Value::Array(
            results.into_iter().map(|(_, _, v)| v).collect(),
        ))
    }

    /// Dart `DefinitionHandler._getCodeLocation`: the code range of the
    /// declaration of [fragment], without its documentation comment and
    /// annotations.
    fn code_location(&mut self, ctx: &Ctx<'_>, fragment: FragmentId) -> Option<(u32, u32)> {
        let mut code_fragment = fragment;
        // A synthetic getter or setter of a variable: the variable.
        if matches!(fragment.tag(), Tag::Getter | Tag::Setter)
            && let Some(e) = ctx
                .fragment_data(fragment)
                .and_then(|d| d.element.try_get().copied())
        {
            let non_synthetic = dartr_element::diagnostics::non_synthetic(ctx, e);
            if non_synthetic != e
                && let Some(d) = ctx.element_data(non_synthetic)
            {
                code_fragment = d.first_fragment;
            }
        }
        // The primary constructor of an extension type: the declaration.
        if code_fragment.tag() == Tag::Constructor
            && let Some(e) = ctx
                .fragment_data(code_fragment)
                .and_then(|d| d.element.try_get().copied())
            && let Some(enclosing) = ctx.element_data(e).and_then(|d| d.enclosing)
            && enclosing.tag() == Tag::ExtensionType
            && dartr_resolver::scope_context::primary_constructor_of(ctx, enclosing.cast()?)
                .map(|c| c.raw())
                == Some(e)
        {
            code_fragment = ctx.fragment_data(code_fragment)?.enclosing_fragment?;
        }
        let data = ctx.fragment_data(code_fragment)?;
        let (mut code_offset, mut code_length) = (data.code_offset?, data.code_length?);
        let path = navigation::fragment_path(ctx, code_fragment)?;
        let Some(parsed) = self.parsed_unit(&path) else {
            return Some((code_offset, code_length));
        };
        let ast = &parsed.unit.ast;
        let Some(mut node) = declaration_node(ctx, ast, parsed.unit.unit, code_fragment) else {
            return Some((code_offset, code_length));
        };
        if ast.is::<VariableDeclaration>(node)
            && let Some(parent) = ast.parent(node)
            && let Some(list) = ast.cast::<VariableDeclarationList>(parent)
            && ast.list(ast[list].variables).len() == 1
        {
            node = parent;
        }
        if let Some(token) =
            crate::computer::outline::first_token_after_comment_and_metadata(ast, node)
        {
            let offset_after_docs = ast.tokens.offset(token);
            code_length = code_length.saturating_sub(offset_after_docs.saturating_sub(code_offset));
            code_offset = offset_after_docs;
        }
        Some((code_offset, code_length))
    }
}

/// Dart `_getFragmentNameOffset`.
fn fragment_name_offset(ctx: &Ctx<'_>, fragment: FragmentId) -> Option<u32> {
    let data = ctx.fragment_data(fragment)?;
    if let Some(o) = data.name_offset {
        return Some(o);
    }
    if let Some(c) = fragment.cast::<dartr_element::ConstructorFragment>() {
        let c = ctx.fragment(c);
        return c
            .type_name_offset
            .or(c.new_keyword_offset)
            .or(c.factory_keyword_offset);
    }
    if fragment.tag() == Tag::Extension {
        return data.first_token_offset;
    }
    None
}

/// Dart `ParsedLibraryResult.getFragmentDeclaration(fragment).node`
/// (`DeclarationByElementLocator`).
fn declaration_node(
    ctx: &Ctx<'_>,
    ast: &Ast,
    unit: Id<CompilationUnit>,
    fragment: FragmentId,
) -> Option<NodeId> {
    if fragment.tag() == Tag::Library {
        return None;
    }
    let name_offset = fragment_name_offset(ctx, fragment)?;
    let mut locator = DeclarationLocator {
        tag: fragment.tag(),
        name_offset,
        result: None,
    };
    locator.visit(ast, unit.raw());
    locator.result
}

struct DeclarationLocator {
    tag: Tag,
    name_offset: u32,
    result: Option<NodeId>,
}

impl DeclarationLocator {
    fn has_offset(&self, ast: &Ast, token: Option<dartr_syntax::TokenId>) -> bool {
        token.is_some_and(|t| ast.tokens.offset(t) == self.name_offset)
    }

    fn visit(&mut self, ast: &Ast, node: NodeId) {
        if self.result.is_some() {
            return;
        }
        if ast.end(node) < self.name_offset || ast.offset(node) > self.name_offset {
            return;
        }
        let name = |ast: &Ast| -> Option<dartr_syntax::TokenId> {
            if let Some(n) = ast.cast::<ClassDeclaration>(node) {
                return Some(dartr_resolver::error::support::class_name_token(
                    ast,
                    ast[n].name_part,
                ));
            }
            None
        };
        let matched = match self.tag {
            Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType => {
                if ast.is::<ClassDeclaration>(node) {
                    self.has_offset(ast, name(ast))
                } else if let Some(n) = ast.cast::<ClassTypeAlias>(node) {
                    self.has_offset(ast, Some(ast[n].name))
                } else if let Some(n) = ast.cast::<EnumDeclaration>(node) {
                    self.has_offset(
                        ast,
                        Some(dartr_resolver::error::support::class_name_token(
                            ast,
                            ast[n].name_part,
                        )),
                    )
                } else if let Some(n) = ast.cast::<MixinDeclaration>(node) {
                    self.has_offset(ast, Some(ast[n].name))
                } else if let Some(n) = ast.cast::<ExtensionTypeDeclaration>(node) {
                    self.has_offset(
                        ast,
                        Some(dartr_resolver::error::support::class_name_token(
                            ast,
                            ast[n].name_part,
                        )),
                    )
                } else {
                    false
                }
            }
            Tag::Constructor => {
                if let Some(n) = ast.cast::<ConstructorDeclaration>(node) {
                    let token = ast[n]
                        .name
                        .or(ast[n].type_name.map(|t| ast[t].token))
                        .or(ast[n].new_keyword)
                        .or(ast[n].factory_keyword);
                    self.has_offset(ast, token)
                } else if let Some(n) = ast.cast::<PrimaryConstructorDeclaration>(node) {
                    let token = ast[n]
                        .constructor_name
                        .map(|c| ast[c].name)
                        .or(Some(ast[n].type_name));
                    self.has_offset(ast, token)
                } else {
                    false
                }
            }
            Tag::Extension => ast.cast::<ExtensionDeclaration>(node).is_some_and(|n| {
                self.has_offset(ast, ast[n].name.or(Some(ast[n].extension_keyword)))
            }),
            Tag::Field => {
                if let Some(n) = ast.cast::<EnumConstantDeclaration>(node) {
                    self.has_offset(ast, Some(ast[n].name))
                } else if let Some(n) = ast.cast::<VariableDeclaration>(node) {
                    self.has_offset(ast, Some(ast[n].name))
                } else {
                    false
                }
            }
            Tag::TopLevelFunction | Tag::LocalFunction => ast
                .cast::<FunctionDeclaration>(node)
                .is_some_and(|n| self.has_offset(ast, Some(ast[n].name))),
            Tag::LocalVariable | Tag::TopLevelVariable => ast
                .cast::<VariableDeclaration>(node)
                .is_some_and(|n| self.has_offset(ast, Some(ast[n].name))),
            Tag::Method => ast
                .cast::<MethodDeclaration>(node)
                .is_some_and(|n| self.has_offset(ast, Some(ast[n].name))),
            Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter => {
                ast.is::<FormalParameter>(node)
                    && self.has_offset(
                        ast,
                        dartr_resolver::ast_ext::formal_parameter_parts(ast, node).name,
                    )
            }
            Tag::Getter | Tag::Setter => {
                if let Some(n) = ast.cast::<FunctionDeclaration>(node) {
                    self.has_offset(ast, Some(ast[n].name))
                } else if let Some(n) = ast.cast::<MethodDeclaration>(node) {
                    self.has_offset(ast, Some(ast[n].name))
                } else {
                    false
                }
            }
            Tag::TypeAlias => ast
                .cast::<GenericTypeAlias>(node)
                .is_some_and(|n| self.has_offset(ast, Some(ast[n].name))),
            _ => false,
        };
        if matched {
            self.result = Some(node);
            return;
        }
        for c in ast.children(node) {
            self.visit(ast, c);
        }
    }
}

/// Dart `KeywordNavigationComputer.compute(unit.nodeCovering(offset))`.
fn keyword_navigation(
    ctx: &Ctx<'_>,
    ast: &Ast,
    tables: &dartr_element::ResolutionTables,
    unit: Id<CompilationUnit>,
    offset: u32,
    path: &str,
    collector: &mut Collector,
) {
    let Some(node) = ast.node_covering(unit, offset, 0) else {
        return;
    };
    if !ast.is::<Statement>(node) {
        return;
    }
    let target = match ast.kind(node) {
        NodeKind::BreakStatement | NodeKind::ContinueStatement => {
            let label = ast
                .cast::<BreakStatement>(node)
                .and_then(|b| ast[b].label)
                .or_else(|| {
                    ast.cast::<ContinueStatement>(node)
                        .and_then(|c| ast[c].label)
                });
            let element = label
                .and_then(|l| tables.element.get(l.raw()))
                .map(|&e| dartr_typesystem::member::base_element(ctx, e));
            dartr_resolver::flow_analysis_visitor::get_label_target(
                ast,
                tables,
                ctx,
                node,
                element,
                ast.is::<BreakStatement>(node),
            )
            .map(|s| ast.begin_token(s.raw()))
        }
        NodeKind::ReturnStatement | NodeKind::YieldStatement => {
            let mut body = ast.parent(node);
            while let Some(b) = body {
                if ast.is::<FunctionBody>(b) {
                    break;
                }
                body = ast.parent(b);
            }
            let function = body.and_then(|b| ast.parent(b));
            function.and_then(|f| {
                if let Some(fe) = ast.cast::<FunctionExpression>(f) {
                    match ast
                        .parent(fe)
                        .and_then(|p| ast.cast::<FunctionDeclaration>(p))
                    {
                        Some(d) => Some(ast[d].name),
                        None => Some(ast.begin_token(fe.raw())),
                    }
                } else if let Some(m) = ast.cast::<MethodDeclaration>(f) {
                    Some(ast[m].name)
                } else if let Some(c) = ast.cast::<ConstructorDeclaration>(f) {
                    ast[c]
                        .name
                        .or(ast[c].type_name.map(|t| ast.begin_token(t.raw())))
                        .or(ast[c].new_keyword)
                        .or(ast[c].factory_keyword)
                } else {
                    None
                }
            })
        }
        _ => None,
    };
    let Some(target) = target else { return };
    let source = ast.begin_token(node);
    let t = ast.tokens.get(source);
    let target_token = ast.tokens.get(target);
    collector.add_region(
        t.offset,
        t.end() - t.offset,
        Target {
            kind: "UNKNOWN",
            file: path.to_string(),
            offset: target_token.offset,
            length: target_token.end() - target_token.offset,
            fragment: None,
        },
    );
}

impl Server {
    /// Dart `server.getResolvedUnit(path)`: `None` instead of an error.
    pub(crate) fn resolved_unit_or_none(&mut self, path: &str) -> Option<ResolvedUnitRef> {
        self.require_resolved_unit(path).ok()
    }

    /// Dart `TypeDefinitionHandler.handle`.
    pub(crate) fn type_definition(&mut self, params: &Value) -> ErrorOr<Value> {
        if !super::is_dart_document(params) {
            return Ok(json!([]));
        }
        let supports_link = self
            .client
            .raw
            .pointer("/textDocument/typeDefinition/linkSupport")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let path = self.path_of_doc(params)?;
        let Some(resolved) = self.resolved_unit_or_none(&path) else {
            return Ok(json!([]));
        };
        let line_info = resolved.line_info().clone();
        let offset = self.position_offset(&line_info, params)?;
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let ast = &unit.ast;
        let u = crate::element_locator::Unit {
            ctx: &ctx,
            ast,
            tables: &unit.tables,
        };
        let Some(node) = ast.node_covering(unit.unit, offset, 0) else {
            return Ok(json!([]));
        };
        let token_range = |t: dartr_syntax::TokenId| {
            let t = ast.tokens.get(t);
            (t.offset, t.end() - t.offset)
        };
        let variable_type = |e: Option<dartr_element::ElementId>| {
            e.map(|e| dartr_resolver::element_ext::variable_type(&ctx, e))
        };
        let pattern_type = |n: NodeId| {
            unit.tables
                .pattern_info
                .get(n)
                .and_then(|i| i.matched_value_type)
        };
        // (origin offset, length), type or element.
        let mut origin: Option<(u32, u32)> = None;
        let mut ty: Option<dartr_element::TypeId> = None;
        let mut element: Option<dartr_element::ElementId> = None;
        if let Some(n) = ast.cast::<NamedType>(node) {
            origin = Some(token_range(ast[n].name));
            element = u
                .element(n)
                .filter(|e| e.cast::<dartr_element::InterfaceElement>().is_some());
        } else if let Some(n) = ast.cast::<VariableDeclaration>(node) {
            origin = Some(token_range(ast[n].name));
            ty = variable_type(u.declared_element(n));
        } else if let Some(n) = ast.cast::<DeclaredIdentifier>(node) {
            origin = Some(token_range(ast[n].name));
            ty = variable_type(u.declared_element(n));
        } else if let Some(n) = ast.cast::<DeclaredVariablePattern>(node) {
            origin = Some(token_range(ast[n].name));
            ty = pattern_type(node);
        } else if let Some(n) = ast.cast::<AssignedVariablePattern>(node) {
            origin = Some(token_range(ast[n].name));
            ty = pattern_type(node);
        } else if let Some(n) = ast.cast::<PatternFieldName>(node) {
            if let Some(name) = ast[n].name {
                origin = Some(token_range(name));
            }
            if let Some(field) = ast.parent(n).and_then(|p| ast.cast::<PatternField>(p)) {
                ty = pattern_type(ast[field].pattern.raw());
            }
        } else if let Some(n) = ast.cast::<NamedArgument>(node) {
            origin = Some(token_range(ast[n].name));
            ty = variable_type(dartr_resolver::error::support::corresponding_parameter(
                &ctx,
                ast,
                &unit.tables,
                n.raw(),
            ));
        } else if ast.is::<Expression>(node) {
            origin = Some((ast.offset(node), ast.length(node)));
            let mut done = false;
            if let Some(s) = ast.cast::<SimpleIdentifier>(node) {
                let e = u.element(s);
                if let Some(e) = e
                    && e.cast::<dartr_element::InterfaceElement>().is_some()
                {
                    element = Some(e);
                    done = true;
                } else if let Some(e) = e
                    && e.cast::<dartr_element::VariableElement>().is_some()
                {
                    if dartr_resolver::error::support::in_declaration_context(ast, s) {
                        ty = variable_type(Some(e));
                        done = true;
                    }
                } else if dartr_resolver::ast_ext::simple_identifier_in_setter_context(ast, s) {
                    let write = u.write_or_read_element(s);
                    if let Some(w) = write
                        && matches!(w.tag(), Tag::Getter | Tag::Setter)
                        && let Some(variable) =
                            dartr_resolver::element_metadata::accessor_variable(&ctx, w)
                    {
                        ty = variable_type(Some(variable));
                        done = true;
                    }
                }
            }
            if !done {
                ty = unit.tables.static_type.get(node).copied();
            }
        } else if ast.is::<FormalParameter>(node) {
            origin = dartr_resolver::ast_ext::formal_parameter_parts(ast, node)
                .name
                .map(token_range);
            ty = variable_type(u.declared_element(node));
        }
        let Some((origin_offset, origin_length)) = origin else {
            return Ok(json!([]));
        };
        if element.is_none()
            && let Some(t) = ty
        {
            element = match ctx.ty(t) {
                dartr_element::TypeKind::Interface { element, .. } => Some(element.raw()),
                dartr_element::TypeKind::TypeParameter { param, .. } => Some(param.raw()),
                _ => None,
            };
        }
        let Some(element) = element else {
            return Ok(json!([]));
        };
        let Some(fragment) = navigation::element_fragment(&ctx, element) else {
            return Ok(json!([]));
        };
        let Some(file) = navigation::fragment_path(&ctx, fragment) else {
            return Ok(json!([]));
        };
        let Some(data) = ctx.fragment_data(fragment) else {
            return Ok(json!([]));
        };
        let (Some(name_offset), Some(name)) = (data.name_offset, data.name) else {
            return Ok(json!([]));
        };
        let name_length = ctx.name_str(name).encode_utf16().count() as u32;
        let Some(target_lines) = self.line_info_of(&file) else {
            return Ok(json!([]));
        };
        let name_range = mapping::to_range(&target_lines, name_offset, name_length);
        let uri = path_to_uri(&file);
        if supports_link {
            let code_range = match (data.code_offset, data.code_length) {
                (Some(o), Some(l)) => mapping::to_range(&target_lines, o, l),
                _ => name_range.clone(),
            };
            Ok(json!([{
                "originSelectionRange": mapping::to_range(&line_info, origin_offset, origin_length),
                "targetUri": uri,
                "targetRange": code_range,
                "targetSelectionRange": name_range,
            }]))
        } else {
            Ok(json!({"uri": uri, "range": name_range}))
        }
    }
}

impl Server {
    /// The dartdoc templates of the files of the context of [path] (Dart
    /// `FileSystemState.dartdocDirectiveInfo`).
    pub(crate) fn dartdoc_templates(&mut self, path: &str) -> std::collections::HashMap<String, String> {
        let Some(collection) = &self.collection else {
            return Default::default();
        };
        let mut templates = std::collections::HashMap::new();
        for (_, parsed) in self.session.known_parsed_units(collection, path) {
            crate::hover::extract_templates_from_unit(&parsed.ast, parsed.unit, &mut templates);
        }
        templates
    }

    /// Dart `_libraryInfo(element).libraryName`: the URI, or for a `file:`
    /// URI the path relative to the package root of the library.
    fn hover_library_name(&self, library_path: &str, library_uri: &str) -> String {
        if !library_uri.starts_with("file:") {
            return library_uri.to_string();
        }
        let root = self.collection.as_ref().and_then(|c| {
            let context = c.context_for(library_path)?;
            let root = context
                .root
                .workspace
                .find_package_for(library_path)
                .map(|p| p.root().to_string())
                .unwrap_or_else(|| context.root.root.clone());
            Some(root)
        });
        match root {
            Some(root) => library_path
                .strip_prefix(&format!("{root}/"))
                .unwrap_or(library_path)
                .to_string(),
            None => library_path.to_string(),
        }
    }

    /// Dart `HoverHandler.handle`.
    pub(crate) fn hover(&mut self, params: &Value) -> ErrorOr<Value> {
        if !super::is_dart_document(params) {
            return Ok(Value::Null);
        }
        let path = self.path_of_doc(params)?;
        let resolved = self.require_resolved_unit(&path)?;
        let line_info = resolved.line_info().clone();
        let offset = self.position_offset(&line_info, params)?;
        let templates = self.dartdoc_templates(&path);
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let u = crate::element_locator::Unit {
            ctx: &ctx,
            ast: &unit.ast,
            tables: &unit.tables,
        };
        let library_name = |p: &str, uri: &str| self.hover_library_name(p, uri);
        let computer = crate::hover::HoverComputer {
            unit: &u,
            root: unit.unit,
            templates: &templates,
            library_name: &library_name,
        };
        let Some(hover) = computer.compute(offset) else {
            return Ok(Value::Null);
        };
        let content = crate::hover::hover_markdown(&hover);
        let formats: Option<Vec<String>> = self
            .client
            .raw
            .pointer("/textDocument/hover/contentFormat")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            });
        let contents = match formats {
            None => Value::String(content),
            Some(formats) => {
                let markdown = formats.is_empty() || formats.iter().any(|f| f == "markdown");
                let plain = formats.iter().any(|f| f == "plaintext");
                let kind = if plain && !markdown {
                    "plaintext"
                } else {
                    "markdown"
                };
                json!({"kind": kind, "value": content})
            }
        };
        Ok(json!({
            "contents": contents,
            "range": mapping::to_range(&line_info, hover.offset, hover.length),
        }))
    }
}

impl Server {
    /// Dart `DocumentHighlightsHandler.handle`.
    pub(crate) fn document_highlights(&mut self, params: &Value) -> ErrorOr<Value> {
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
        let tokens = crate::highlights::compute(&u, unit.unit, offset);
        let highlights: Vec<Value> = tokens
            .into_iter()
            .map(|(token, kind)| {
                let t = unit.ast.tokens.get(token);
                json!({
                    "range": mapping::to_range(&line_info, t.offset, t.end() - t.offset),
                    "kind": kind,
                })
            })
            .collect();
        Ok(Value::Array(highlights))
    }
}
