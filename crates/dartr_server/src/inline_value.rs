// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_inline_value.dart
// Dart source: pkg/analysis_server/lib/src/lsp/extensions/positions.dart

//! `textDocument/inlineValue` handler, collector, and AST visitor.

use dartr_ast::*;
use dartr_element::{AnyElement, Ctx, ElementId, NoopSink, Tag, TypeId, TypeKind};
use dartr_resolver::ast_ext::{ancestor_cascade_target, property_access_is_cascaded};
use dartr_syntax::{LineInfo, TokenId};
use indexmap::IndexMap;
use serde_json::{Value, json};

use crate::element_locator::Unit;
use crate::mapping::{self, ErrorOr, ResponseError, codes};
use crate::server::{ResolvedUnitRef, Server, is_dart_document};

fn property_access_real_target(ast: &Ast, node: Id<PropertyAccess>) -> Option<Id<Expression>> {
    if property_access_is_cascaded(ast, node) {
        return ancestor_cascade_target(ast, node.raw());
    }
    ast[node].target
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct LspPos {
    line: u32,
    character: u32,
}

impl LspPos {
    fn is_after_or_equal(self, other: Self) -> bool {
        self >= other
    }

    fn is_before_or_equal(self, other: Self) -> bool {
        self <= other
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LspRange {
    start: LspPos,
    end: LspPos,
}

impl LspRange {
    /// Dart `RangeExtension.intersects`.
    fn intersects(self, other: Self) -> bool {
        let ends_before = self.end.is_before_or_equal(other.start);
        let starts_after = self.start.is_after_or_equal(other.end);
        !(ends_before || starts_after)
    }

    fn to_json(self) -> Value {
        json!({
            "start": {"line": self.start.line, "character": self.start.character},
            "end": {"line": self.end.line, "character": self.end.character},
        })
    }
}

#[derive(Clone, Copy, Debug)]
enum InlineValueItem {
    EvaluatableExpression { range: LspRange },
    VariableLookup { range: LspRange },
}

impl InlineValueItem {
    fn range(self) -> LspRange {
        match self {
            InlineValueItem::EvaluatableExpression { range }
            | InlineValueItem::VariableLookup { range } => range,
        }
    }

    fn to_json(self) -> Value {
        match self {
            InlineValueItem::EvaluatableExpression { range } => json!({
                "range": range.to_json(),
            }),
            InlineValueItem::VariableLookup { range } => json!({
                "caseSensitiveLookup": true,
                "range": range.to_json(),
            }),
        }
    }
}

impl Server {
    /// Handles `textDocument/inlineValue` (Dart `InlineValueHandler.handle`).
    pub(crate) fn inline_value(&mut self, params: &Value) -> ErrorOr<Value> {
        if !is_dart_document(params) {
            return Ok(Value::Null);
        }

        let file_path = self.path_of_doc(params)?;
        let Some(resolved) = self.require_resolved_unit(&file_path).ok() else {
            return Ok(Value::Null);
        };

        let invalid = || ResponseError::new(codes::INVALID_PARAMS, "Invalid params");
        let visible_range =
            read_lsp_range(params.get("range").ok_or_else(invalid)?).ok_or_else(invalid)?;
        let stopped_location = read_lsp_range(
            params
                .pointer("/context/stoppedLocation")
                .ok_or_else(invalid)?,
        )
        .ok_or_else(invalid)?;

        let range_already_executed = LspRange {
            start: visible_range.start,
            end: stopped_location.end,
        };
        let range_including_current_line = LspRange {
            start: visible_range.start,
            end: LspPos {
                line: stopped_location.end.line + 1,
                character: 0,
            },
        };

        let line_info = resolved.line_info();
        let stopped_offset = mapping::to_offset(
            line_info,
            stopped_location.end.line,
            stopped_location.end.character,
            false,
        )?;

        let properties_enabled = self
            .client_configuration
            .global()
            .experimental_inline_values_properties();

        Ok(compute_inline_values(
            &resolved,
            range_already_executed,
            range_including_current_line,
            stopped_offset,
            properties_enabled,
        ))
    }
}

fn read_lsp_pos(v: &Value) -> Option<LspPos> {
    let (line, character) = mapping::read_position(v)?;
    Some(LspPos { line, character })
}

fn read_lsp_range(v: &Value) -> Option<LspRange> {
    Some(LspRange {
        start: read_lsp_pos(v.get("start")?)?,
        end: read_lsp_pos(v.get("end")?)?,
    })
}

fn to_lsp_range(line_info: &LineInfo, offset: u32, length: u32) -> LspRange {
    let start_loc = line_info.get_location(offset);
    let end_loc = line_info.get_location(offset + length);
    LspRange {
        start: LspPos {
            line: start_loc.line_number - 1,
            character: start_loc.column_number - 1,
        },
        end: LspPos {
            line: end_loc.line_number - 1,
            character: end_loc.column_number - 1,
        },
    }
}

fn compute_inline_values(
    resolved: &ResolvedUnitRef,
    range_already_executed: LspRange,
    range_including_current_line: LspRange,
    stopped_offset: u32,
    experimental_inline_values_properties: bool,
) -> Value {
    let sink = NoopSink;
    let ctx = resolved.ctx(&sink);
    let r_unit = resolved.unit();
    let ast = &r_unit.ast;
    let unit = Unit {
        ctx: &ctx,
        ast,
        tables: &r_unit.tables,
    };

    let Some(covering) = ast.node_covering(r_unit.unit, stopped_offset, 0) else {
        return Value::Null;
    };
    let Some(function) = ast.this_or_ancestor_matching(covering, |ast, n| {
        ast.is::<FunctionExpression>(n) || ast.is::<MethodDeclaration>(n)
    }) else {
        return Value::Null;
    };

    let collector = InlineValueCollector {
        ctx: &ctx,
        line_info: resolved.line_info(),
        range_already_executed,
        range_including_current_line,
        values: IndexMap::new(),
    };
    let mut visitor = InlineValueVisitor {
        unit: &unit,
        collector,
        root_node: function,
        current_execution_offset: stopped_offset,
        experimental_inline_values_properties,
    };
    ast.accept_generalizing(function, &mut visitor);

    let items: Vec<Value> = visitor
        .collector
        .values
        .values()
        .map(|v| v.to_json())
        .collect();
    Value::Array(items)
}

struct InlineValueCollector<'a, 'c> {
    ctx: &'a Ctx<'c>,
    line_info: &'a LineInfo,
    range_already_executed: LspRange,
    range_including_current_line: LspRange,
    values: IndexMap<ElementId, InlineValueItem>,
}

impl<'a, 'c> InlineValueCollector<'a, 'c> {
    fn record_expression(&mut self, element: Option<ElementId>, offset: u32, length: u32) {
        let Some(element) = element else {
            return;
        };
        let range = to_lsp_range(self.line_info, offset, length);
        if !range.intersects(self.range_already_executed) {
            return;
        }
        let value = InlineValueItem::EvaluatableExpression { range };
        self.record(value, element);
    }

    fn record_variable_lookup(&mut self, element: Option<ElementId>, offset: u32, length: u32) {
        let Some(element) = element else {
            return;
        };
        if is_wildcard_variable(self.ctx, element) {
            return;
        }
        let range = to_lsp_range(self.line_info, offset, length);
        if !range.intersects(self.range_including_current_line) {
            return;
        }
        let value = InlineValueItem::VariableLookup { range };
        self.record(value, element);
    }

    fn is_excluded_element(&self, element: ElementId) -> bool {
        let ty = match self.ctx.any(element) {
            AnyElement::LocalVariable(v) => v.type_.get(),
            AnyElement::FormalParameter(p) => p.type_.get(),
            AnyElement::Field(f) => f.type_.get(),
            AnyElement::TopLevelVariable(t) => t.type_.get(),
            AnyElement::Getter(g) => g.return_type.get(),
            _ => None,
        };
        self.is_excluded_type(ty)
    }

    fn is_excluded_type(&self, ty: Option<TypeId>) -> bool {
        let Some(ty) = ty else {
            return false;
        };
        let TypeKind::Interface { element, .. } = *self.ctx.ty(ty) else {
            return false;
        };
        let iface = self.ctx.interface(element);
        let Some(name) = iface.name.map(|n| self.ctx.name_str(n)) else {
            return false;
        };
        let lib_uri = crate::flutter::library_uri_str(self.ctx, element.raw());
        match (lib_uri.as_deref(), name) {
            (Some("dart:core"), "Iterable") => true,
            (Some("dart:async"), "Future" | "FutureOr" | "Stream") => true,
            _ => false,
        }
    }

    fn record(&mut self, value: InlineValueItem, element: ElementId) {
        if self.is_excluded_element(element) {
            return;
        }
        let range = value.range();
        if let Some(existing) = self.values.get(&element)
            && existing.range().start.is_after_or_equal(range.start)
        {
            return;
        }
        self.values.insert(element, value);
    }
}

fn is_wildcard_variable(ctx: &Ctx<'_>, element: ElementId) -> bool {
    let name = ctx
        .element_data(element)
        .and_then(|d| d.name)
        .map(|n| ctx.name_str(n));
    name == Some("_")
        && matches!(
            element.tag(),
            Tag::LocalFunction
                | Tag::LocalVariable
                | Tag::Prefix
                | Tag::TypeParameter
                | Tag::FormalParameter
        )
        && ctx.features.is_enabled("wildcard-variables")
}

struct InlineValueVisitor<'u, 'c, 'a> {
    unit: &'u Unit<'c, 'a>,
    collector: InlineValueCollector<'c, 'a>,
    root_node: NodeId,
    current_execution_offset: u32,
    experimental_inline_values_properties: bool,
}

impl<'u, 'c, 'a> InlineValueVisitor<'u, 'c, 'a> {
    fn record_token_variable(&mut self, ast: &Ast, element: Option<ElementId>, name: TokenId) {
        let tok = ast.tokens.get(name);
        self.collector
            .record_variable_lookup(element, tok.offset, tok.length);
    }
}

impl GeneralizingAstVisitor for InlineValueVisitor<'_, '_, '_> {
    fn visit_block(&mut self, ast: &Ast, node: Id<Block>) {
        if self.current_execution_offset < ast.offset(node.raw())
            || self.current_execution_offset > ast.end(node.raw())
        {
            return;
        }
        self.visit_statement(ast, node.upcast());
    }

    fn visit_declared_identifier(&mut self, ast: &Ast, node: Id<DeclaredIdentifier>) {
        let element = self.unit.declared_element(node.raw());
        self.record_token_variable(ast, element, ast[node].name);
        self.visit_declaration(ast, node.upcast());
    }

    fn visit_declared_variable_pattern(&mut self, ast: &Ast, node: Id<DeclaredVariablePattern>) {
        let element = self.unit.declared_element(node.raw());
        self.record_token_variable(ast, element, ast[node].name);
        self.visit_variable_pattern(ast, node.upcast());
    }

    fn visit_formal_parameter(&mut self, ast: &Ast, node: Id<FormalParameter>) {
        let name = if let Some(r) = ast.cast::<RegularFormalParameter>(node.raw()) {
            ast[r].name
        } else if let Some(f) = ast.cast::<FieldFormalParameter>(node.raw()) {
            Some(ast[f].name)
        } else if let Some(s) = ast.cast::<SuperFormalParameter>(node.raw()) {
            Some(ast[s].name)
        } else {
            None
        };
        if let Some(name) = name {
            let element = self.unit.declared_element(node.raw());
            self.record_token_variable(ast, element, name);
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_function_expression(&mut self, ast: &Ast, node: Id<FunctionExpression>) {
        if node.raw() != self.root_node {
            return;
        }
        self.visit_expression(ast, node.upcast());
    }

    fn visit_generic_function_type(&mut self, _ast: &Ast, _node: Id<GenericFunctionType>) {
        // Don't descend into function types or we may generate values for their parameters.
    }

    fn visit_prefixed_identifier(&mut self, ast: &Ast, node: Id<PrefixedIdentifier>) {
        if self.experimental_inline_values_properties {
            let prefix_ty = self
                .unit
                .tables
                .static_type
                .get(ast[node].prefix.raw())
                .copied();
            if self.collector.is_excluded_type(prefix_ty) {
                return;
            }

            let is_target = ast
                .parent(node.raw())
                .and_then(|p| ast.cast::<PropertyAccess>(p))
                .and_then(|pa| property_access_real_target(ast, pa))
                .is_some_and(|t| t.raw() == node.raw());

            let element = self.unit.write_or_read_element(ast[node].identifier);
            let is_enum_getter = element.is_some_and(|e| {
                e.tag() == Tag::Getter
                    && self
                        .unit
                        .ctx
                        .element_data(e)
                        .and_then(|d| d.enclosing)
                        .is_some_and(|enc| enc.tag() == Tag::Enum)
            });

            if !is_target && !is_enum_getter {
                self.collector.record_expression(
                    element,
                    ast.offset(node.raw()),
                    ast.length(node.raw()),
                );
            }
        }
        self.visit_identifier(ast, node.upcast());
    }

    fn visit_property_access(&mut self, ast: &Ast, node: Id<PropertyAccess>) {
        if self.experimental_inline_values_properties
            && let Some(target) = ast[node].target
            && ast.is::<Identifier>(target.raw())
        {
            let target_ty = self.unit.tables.static_type.get(target.raw()).copied();
            if self.collector.is_excluded_type(target_ty) {
                return;
            }
            let element = self.unit.write_or_read_element(ast[node].property_name);
            self.collector.record_expression(
                element,
                ast.offset(node.raw()),
                ast.length(node.raw()),
            );
        }
        self.visit_expression(ast, node.upcast());
    }

    fn visit_simple_identifier(&mut self, ast: &Ast, node: Id<SimpleIdentifier>) {
        let parent = ast.parent(node.raw());
        let is_target = parent
            .and_then(|p| ast.cast::<PrefixedIdentifier>(p))
            .is_some_and(|pi| ast[pi].prefix.raw() == node.raw());
        let is_invocation = parent.is_some_and(|p| ast.is::<InvocationExpression>(p));

        if !is_target && !is_invocation {
            let element = self.unit.write_or_read_element(node);
            if let Some(el) = element {
                let matches_var_or_param = match el.tag() {
                    Tag::LocalVariable => self
                        .unit
                        .ctx
                        .element_data(el)
                        .and_then(|d| d.name)
                        .is_some(),
                    Tag::FormalParameter
                    | Tag::FieldFormalParameter
                    | Tag::SuperFormalParameter => true,
                    _ => false,
                };
                if matches_var_or_param {
                    self.collector.record_variable_lookup(
                        Some(el),
                        ast.offset(node.raw()),
                        ast.length(node.raw()),
                    );
                }
            }
        }
        self.visit_identifier(ast, node.upcast());
    }

    fn visit_switch_pattern_case(&mut self, ast: &Ast, node: Id<SwitchPatternCase>) {
        let end = ast
            .list_raw(ast[node].statements)
            .last()
            .map(|&last| ast.end(last))
            .unwrap_or_else(|| ast.end(node.raw()));
        if self.current_execution_offset < ast.offset(node.raw())
            || self.current_execution_offset > end
        {
            return;
        }
        self.visit_switch_member(ast, node.upcast());
    }

    fn visit_variable_declaration(&mut self, ast: &Ast, node: Id<VariableDeclaration>) {
        let element = self.unit.declared_element(node.raw());
        self.record_token_variable(ast, element, ast[node].name);
        self.visit_declaration(ast, node.upcast());
    }
}
