// Dart source: pkg/analysis_server/lib/src/computer/computer_color.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_document_color.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_document_color_presentation.dart

//! `textDocument/documentColor` and `textDocument/colorPresentation`.

use std::sync::LazyLock;

use dartr_ast::*;
use dartr_constant::{DartObjectImpl, DeclaredVariables, GenericState};
use dartr_element::{
    ConstructorElement, Ctx, EId, ElementId, InterfaceElement, LibraryElement, NoopSink,
    ResolutionTables, Tag, TypeId, TypeKind, VariableElement,
};
use dartr_resolver::ast_ext::{
    ancestor_cascade_target, in_constant_context, instance_creation_is_const,
    property_access_is_cascaded,
};
use dartr_resolver::constant::evaluation::{ConstantEvaluationEngine, NodeRef};
use dartr_resolver::library_analyzer::ExternalUnitCache;
use dartr_typesystem::TypeExt;
use serde_json::{Map, Value, json};

use crate::correction::change_builder::ChangeBuilder;
use crate::correction::dart_edit::WriteType;
use crate::correction::producers::import_library::exported_element;
use crate::flutter::library_uri_str;
use crate::mapping::{self, ErrorOr};
use crate::server::ResolvedUnitRef;

const URI_FLUTTER_WIDGETS: &str = "package:flutter/widgets.dart";
const URI_FLUTTER_MATERIAL_COLORS: &str = "package:flutter/src/material/colors.dart";
const URI_FLUTTER_PAINTING_COLORS: &str = "package:flutter/src/painting/colors.dart";

/// Information about a color that is present in a document (0..=255 per component).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorInformation {
    pub alpha: i64,
    pub red: i64,
    pub green: i64,
    pub blue: i64,
}

/// Information about a specific known location of a [`ColorInformation`]
/// reference in a document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorReference {
    pub offset: u32,
    pub length: u32,
    pub color: ColorInformation,
}

static DECLARED_VARIABLES: LazyLock<DeclaredVariables> = LazyLock::new(DeclaredVariables::new);

/// Computes [`ColorReference`]s for `resolved`, porting `ColorComputer` from
/// `pkg/analysis_server/lib/src/computer/computer_color.dart`.
pub fn compute_colors(
    resolved: &ResolvedUnitRef,
    unit_sources: indexmap::IndexMap<
        std::sync::Arc<str>,
        (
            std::sync::Arc<str>,
            std::sync::Arc<dartr_ast_builder::ParsedUnit>,
        ),
    >,
) -> Vec<ColorReference> {
    let sink = NoopSink;
    let ctx = resolved.ctx(&sink);
    let unit = resolved.unit();
    let external = ExternalUnitCache::new(
        &resolved.library.world,
        &resolved.library.type_provider,
        dartr_resolver::options::AnalysisOptions::default(),
        unit_sources,
    );
    let engine = ConstantEvaluationEngine::new(
        &resolved.library.world,
        &resolved.library.type_provider,
        resolved.library.library.library,
        &DECLARED_VARIABLES,
        &resolved.library.library.units,
        Some(&external),
    );
    *engine.values.borrow_mut() = resolved.library.library.constants.clone();

    let mut computer = ColorComputer {
        ctx: &ctx,
        ast: &unit.ast,
        tables: &unit.tables,
        unit_index: resolved.index as u32,
        engine: &engine,
        colors: Vec::new(),
    };
    let mut visitor = ColorBuilder {
        computer: &mut computer,
    };
    unit.ast.accept(unit.unit, &mut visitor);
    computer.colors
}

/// Converts [`ColorReference`]s to the JSON response of `textDocument/documentColor`.
pub fn color_references_to_json(
    line_info: &dartr_syntax::LineInfo,
    references: &[ColorReference],
) -> Value {
    references
        .iter()
        .map(|reference| {
            json!({
                "range": mapping::to_range(line_info, reference.offset, reference.length),
                "color": {
                    "alpha": reference.color.alpha as f64 / 255.0,
                    "red": reference.color.red as f64 / 255.0,
                    "green": reference.color.green as f64 / 255.0,
                    "blue": reference.color.blue as f64 / 255.0,
                },
            })
        })
        .collect()
}

struct ColorComputer<'a, 'c> {
    ctx: &'a Ctx<'c>,
    ast: &'a Ast,
    tables: &'a ResolutionTables,
    unit_index: u32,
    engine: &'a ConstantEvaluationEngine<'a>,
    colors: Vec<ColorReference>,
}

impl<'a, 'c> ColorComputer<'a, 'c> {
    fn is_color_expr(&self, expression: NodeId) -> bool {
        let Some(&ty) = self.tables.static_type.get(expression) else {
            return false;
        };
        is_color_type(self.ctx, ty)
    }

    /// Dart `ColorComputer.tryAddColor`.
    fn try_add_color(
        &mut self,
        expression: NodeId,
        target: Option<NodeId>,
        member_name: Option<&str>,
        index: Option<i64>,
    ) -> bool {
        if !self.is_color_expr(expression) {
            return false;
        }
        let target = target.unwrap_or(expression);
        let node_ref = NodeRef::new(self.unit_index, target);
        let eval_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if self.engine.expression_has_constant_error(node_ref) {
                return None;
            }
            self.engine.compute_expression_constant_value(node_ref)
        }));
        let Ok(Some(mut color_const)) = eval_result else {
            return false;
        };

        if let Some(member_name) = member_name {
            let Some(member_obj) = get_member(&color_const, member_name) else {
                return false;
            };
            color_const = member_obj;
        } else if let Some(index) = index {
            let Some(swatch_obj) = get_swatch_color(&color_const, index) else {
                return false;
            };
            color_const = swatch_obj;
        }

        let color = get_color_for_object(self.ctx, &color_const);
        self.try_record_color_information(expression, color)
    }

    /// Dart `ColorComputer.tryAddKnownColorConstructor`.
    fn try_add_known_color_constructor(
        &mut self,
        expression: NodeId,
        constructor: Option<EId<ConstructorElement>>,
        argument_list: Id<ArgumentList>,
    ) -> bool {
        if !self.is_color_expr(expression) {
            return false;
        }
        let class_element = constructor
            .and_then(|c| self.ctx.get(c).enclosing)
            .and_then(|e| e.cast::<InterfaceElement>());
        let class_name = class_element
            .and_then(|c| self.ctx.interface(c).name)
            .map(|n| self.ctx.name_str(n));
        let mut constructor_name = constructor
            .and_then(|c| self.ctx.get(c).name)
            .map(|n| self.ctx.name_str(n));
        if constructor_name == Some("new") || constructor_name == Some("") {
            constructor_name = None;
        }
        let args = self.ast.list(self.ast[argument_list].arguments);

        let color = if class_element.is_some_and(|c| is_dart_ui(self.ctx, c.raw()))
            && class_name == Some("Color")
        {
            self.get_dart_ui_color(constructor_name, args)
        } else if class_element.is_some_and(|c| is_flutter_painting(self.ctx, c.raw()))
            && class_name == Some("ColorSwatch")
        {
            self.get_flutter_swatch_color(constructor_name, args)
        } else if class_element.is_some_and(|c| is_flutter_material(self.ctx, c.raw()))
            && class_name == Some("MaterialAccentColor")
        {
            self.get_flutter_swatch_color(constructor_name, args)
        } else {
            None
        };

        self.try_record_color_information(expression, color)
    }

    fn get_dart_ui_color(
        &self,
        name: Option<&str>,
        args: &[Id<Argument>],
    ) -> Option<ColorInformation> {
        if name.is_none() && args.len() == 1 {
            let arg0 = self.ast.cast::<IntegerLiteral>(args[0])?;
            get_color_for_int(self.ast[arg0].value)
        } else if name == Some("from") {
            let mut alpha = None;
            let mut red = None;
            let mut green = None;
            let mut blue = None;
            for &arg in args {
                let Some(named) = self.ast.cast::<NamedArgument>(arg) else {
                    continue;
                };
                let expr = self.ast[named].argument_expression;
                let value = if let Some(d) = self.ast.cast::<DoubleLiteral>(expr) {
                    Some(self.ast[d].value)
                } else if let Some(i) = self.ast.cast::<IntegerLiteral>(expr) {
                    self.ast[i].value.map(|v| v as f64)
                } else {
                    None
                };
                match self.ast.tokens.lexeme(self.ast[named].name) {
                    "alpha" => alpha = value,
                    "red" => red = value,
                    "green" => green = value,
                    "blue" => blue = value,
                    _ => {}
                }
            }
            get_color_for_doubles(alpha, red, green, blue)
        } else if name == Some("fromARGB") && args.len() == 4 {
            let alpha = self
                .ast
                .cast::<IntegerLiteral>(args[0])
                .and_then(|i| self.ast[i].value)?;
            let red = self
                .ast
                .cast::<IntegerLiteral>(args[1])
                .and_then(|i| self.ast[i].value)?;
            let green = self
                .ast
                .cast::<IntegerLiteral>(args[2])
                .and_then(|i| self.ast[i].value)?;
            let blue = self
                .ast
                .cast::<IntegerLiteral>(args[3])
                .and_then(|i| self.ast[i].value)?;
            Some(ColorInformation {
                alpha,
                red,
                green,
                blue,
            })
        } else if name == Some("fromRGBO") && args.len() == 4 {
            let red = self
                .ast
                .cast::<IntegerLiteral>(args[0])
                .and_then(|i| self.ast[i].value)?;
            let green = self
                .ast
                .cast::<IntegerLiteral>(args[1])
                .and_then(|i| self.ast[i].value)?;
            let blue = self
                .ast
                .cast::<IntegerLiteral>(args[2])
                .and_then(|i| self.ast[i].value)?;
            let alpha = if let Some(i) = self.ast.cast::<IntegerLiteral>(args[3]) {
                self.ast[i].value.map(|v| v * 255)
            } else if let Some(d) = self.ast.cast::<DoubleLiteral>(args[3]) {
                Some((self.ast[d].value * 255.0) as i64)
            } else {
                None
            }?;
            Some(ColorInformation {
                alpha,
                red,
                green,
                blue,
            })
        } else {
            None
        }
    }

    fn get_flutter_swatch_color(
        &self,
        name: Option<&str>,
        args: &[Id<Argument>],
    ) -> Option<ColorInformation> {
        if name.is_none() && !args.is_empty() {
            let arg0 = self.ast.cast::<IntegerLiteral>(args[0])?;
            get_color_for_int(self.ast[arg0].value)
        } else {
            None
        }
    }

    fn try_record_color_information(
        &mut self,
        expression: NodeId,
        color: Option<ColorInformation>,
    ) -> bool {
        let Some(color) = color else {
            return false;
        };
        self.colors.push(ColorReference {
            offset: self.ast.offset(expression),
            length: self.ast.length(expression),
            color,
        });
        true
    }
}

struct ColorBuilder<'a, 'b, 'c> {
    computer: &'a mut ColorComputer<'b, 'c>,
}

impl AstVisitor for ColorBuilder<'_, '_, '_> {
    fn visit_dot_shorthand_constructor_invocation(
        &mut self,
        ast: &Ast,
        node: Id<DotShorthandConstructorInvocation>,
    ) {
        if !self.computer.try_add_color(node.raw(), None, None, None) {
            let ctor = self
                .computer
                .tables
                .element
                .get(ast[node].constructor_name.raw())
                .or_else(|| self.computer.tables.element.get(node.raw()))
                .copied()
                .map(|r| dartr_typesystem::member::base_element(self.computer.ctx, r))
                .and_then(|e| e.cast::<ConstructorElement>());
            self.computer.try_add_known_color_constructor(
                node.raw(),
                ctor,
                ast[node].argument_list,
            );
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_index_expression(&mut self, ast: &Ast, node: Id<IndexExpression>) {
        if let Some(index) = ast.cast::<IntegerLiteral>(ast[node].index)
            && let Some(index_value) = ast[index].value
        {
            let real_target = index_expression_real_target(ast, node).map(|e| e.raw());
            if self
                .computer
                .try_add_color(node.raw(), real_target, None, Some(index_value))
            {
                return;
            }
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_instance_creation_expression(
        &mut self,
        ast: &Ast,
        node: Id<InstanceCreationExpression>,
    ) {
        if !self.computer.try_add_color(node.raw(), None, None, None) {
            let ctor = crate::flutter::constructor_of_creation(
                self.computer.ctx,
                ast,
                self.computer.tables,
                node,
            );
            self.computer.try_add_known_color_constructor(
                node.raw(),
                ctor,
                ast[node].argument_list,
            );
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_prefixed_identifier(&mut self, ast: &Ast, node: Id<PrefixedIdentifier>) {
        if self.computer.try_add_color(node.raw(), None, None, None) {
            return;
        }
        let member_name = ast.tokens.lexeme(ast[ast[node].identifier].token);
        if self.computer.try_add_color(
            node.raw(),
            Some(ast[node].prefix.raw()),
            Some(member_name),
            None,
        ) {
            return;
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_property_access(&mut self, ast: &Ast, node: Id<PropertyAccess>) {
        let real_target = property_access_real_target(ast, node).map(|e| e.raw());
        let member_name = ast.tokens.lexeme(ast[ast[node].property_name].token);
        if self
            .computer
            .try_add_color(node.raw(), real_target, Some(member_name), None)
        {
            return;
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_simple_identifier(&mut self, ast: &Ast, node: Id<SimpleIdentifier>) {
        self.computer.try_add_color(node.raw(), None, None, None);
        self.visit_node(ast, node.raw());
    }
}

fn index_expression_real_target(ast: &Ast, node: Id<IndexExpression>) -> Option<Id<Expression>> {
    let is_cascaded = ast[node]
        .period
        .is_some_and(|p| matches!(ast.tokens.lexeme(p), ".." | "?.."));
    if is_cascaded {
        ancestor_cascade_target(ast, node.raw())
    } else {
        ast[node].target
    }
}

fn property_access_real_target(ast: &Ast, node: Id<PropertyAccess>) -> Option<Id<Expression>> {
    if property_access_is_cascaded(ast, node) {
        ancestor_cascade_target(ast, node.raw())
    } else {
        ast[node].target
    }
}

fn get_field_from_hierarchy(obj: &DartObjectImpl, field_name: &str) -> Option<DartObjectImpl> {
    if let Some(val) = obj.get_field(field_name) {
        return Some(val.clone());
    }
    let super_obj = obj.get_field(GenericState::SUPERCLASS_FIELD)?;
    get_field_from_hierarchy(&super_obj, field_name)
}

fn get_member(target: &DartObjectImpl, member_name: &str) -> Option<DartObjectImpl> {
    if let Some(color) = get_field_from_hierarchy(target, member_name) {
        return Some(color);
    }
    if let Some(rest) = member_name.strip_prefix("shade")
        && let Ok(shade_number) = rest.parse::<i64>()
    {
        return get_swatch_color(target, shade_number);
    }
    None
}

fn get_swatch_color(target: &DartObjectImpl, swatch_value: i64) -> Option<DartObjectImpl> {
    let swatch_obj = get_field_from_hierarchy(target, "_swatch")?;
    let swatch = swatch_obj.to_map_value()?;
    for (k, v) in swatch.iter() {
        if k.to_int_value() == Some(swatch_value) {
            return Some(v.clone());
        }
    }
    None
}

fn get_color_for_object(ctx: &Ctx<'_>, color: &DartObjectImpl) -> Option<ColorInformation> {
    if color.is_null() || !is_color_type(ctx, color.ty) {
        return None;
    }
    let inner = get_field_from_hierarchy(color, "color");
    let color = inner.as_ref().unwrap_or(color);
    let alpha = get_field_from_hierarchy(color, "a")?.to_double_value();
    let red = get_field_from_hierarchy(color, "r")?.to_double_value();
    let green = get_field_from_hierarchy(color, "g")?.to_double_value();
    let blue = get_field_from_hierarchy(color, "b")?.to_double_value();
    get_color_for_doubles(alpha, red, green, blue)
}

fn get_color_for_doubles(
    alpha: Option<f64>,
    red: Option<f64>,
    green: Option<f64>,
    blue: Option<f64>,
) -> Option<ColorInformation> {
    Some(ColorInformation {
        alpha: ((alpha? * 255.0).round() as i64) & 0xff,
        red: ((red? * 255.0).round() as i64) & 0xff,
        green: ((green? * 255.0).round() as i64) & 0xff,
        blue: ((blue? * 255.0).round() as i64) & 0xff,
    })
}

fn get_color_for_int(value: Option<i64>) -> Option<ColorInformation> {
    let value = value?;
    Some(ColorInformation {
        alpha: (value >> 24) & 0xff,
        red: (value >> 16) & 0xff,
        green: (value >> 8) & 0xff,
        blue: value & 0xff,
    })
}

pub fn is_color_type(ctx: &Ctx<'_>, ty: TypeId) -> bool {
    let TypeKind::Interface { element, .. } = *ctx.ty(ty) else {
        return false;
    };
    is_color_interface(ctx, element)
}

pub fn is_color_interface(ctx: &Ctx<'_>, element: EId<InterfaceElement>) -> bool {
    if element.raw().tag() != Tag::Class {
        return false;
    }
    if is_exact_color_class(ctx, element) {
        return true;
    }
    for &sup_ty in ctx.all_supertypes(ctx.interface_this_type(element)).iter() {
        if let TypeKind::Interface { element: sup, .. } = *ctx.ty(sup_ty)
            && is_exact_color_class(ctx, sup)
        {
            return true;
        }
    }
    false
}

fn is_exact_color_class(ctx: &Ctx<'_>, element: EId<InterfaceElement>) -> bool {
    if element.raw().tag() != Tag::Class {
        return false;
    }
    let iface = ctx.interface(element);
    let Some(el_name) = iface.name else {
        return false;
    };
    if ctx.name_str(el_name) != "Color" {
        return false;
    }
    is_dart_ui(ctx, element.raw())
}

fn is_dart_ui(ctx: &Ctx<'_>, element: ElementId) -> bool {
    let Some(lib_id) = ctx.element_data(element).and_then(|d| d.library) else {
        return false;
    };
    let lib = ctx.get(lib_id);
    if lib.name.is_some_and(|n| ctx.name_str(n) == "dart.ui") {
        return true;
    }
    let first_frag = lib.first_fragment();
    ctx.fragment(first_frag).source.uri.as_ref() == "dart:ui"
}

fn is_flutter_material(ctx: &Ctx<'_>, element: ElementId) -> bool {
    library_uri_str(ctx, element).as_deref() == Some(URI_FLUTTER_MATERIAL_COLORS)
}

fn is_flutter_painting(ctx: &Ctx<'_>, element: ElementId) -> bool {
    library_uri_str(ctx, element).as_deref() == Some(URI_FLUTTER_PAINTING_COLORS)
}

// ---------------------------------------------------------------------------
// textDocument/colorPresentation
// ---------------------------------------------------------------------------

pub fn find_library_by_uri(ctx: &Ctx<'_>, uri: &str) -> Option<EId<LibraryElement>> {
    ctx.library_by_uri(uri)
}

fn round_double(value: f64) -> String {
    let s = format!("{value:.3}");
    let s = s.trim_end_matches('0');
    let s = s.trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

fn format_dart_num(v: &Value, f: f64) -> String {
    if let Some(i) = v.as_i64() {
        i.to_string()
    } else if let Some(u) = v.as_u64() {
        u.to_string()
    } else if f.fract() == 0.0 {
        format!("{f:.1}")
    } else {
        format!("{f}")
    }
}

/// Dart `DocumentColorPresentationHandler._willRequireConstKeyword`.
fn will_require_const_keyword(
    ctx: &Ctx<'_>,
    ast: &Ast,
    tables: &ResolutionTables,
    node: Option<NodeId>,
) -> bool {
    let Some(node) = node else {
        return false;
    };
    if !Expression::test(ast.kind(node)) {
        return false;
    }
    if in_constant_context(ast, node) {
        return false;
    }
    if let Some(ic) = ast.cast::<InstanceCreationExpression>(node) {
        return instance_creation_is_const(ast, ic);
    }
    if ast.is::<SimpleIdentifier>(node) {
        let target_node = match ast.parent(node) {
            Some(parent) if ast.is::<PrefixedIdentifier>(parent) => parent,
            _ => node,
        };
        let Some(&elem_ref) = tables.element.get(target_node) else {
            return false;
        };
        let element = dartr_typesystem::member::base_element(ctx, elem_ref);
        let var_el = if matches!(element.tag(), Tag::Getter | Tag::Setter) {
            ctx.property_accessor(element.cast().unwrap())
                .variable
                .get()
                .map(|v| v.raw())
        } else if element.is::<VariableElement>() {
            Some(element)
        } else {
            None
        };
        if let Some(var_el) = var_el {
            return dartr_resolver::element_ext::is_const(ctx, var_el);
        }
    }
    false
}

#[allow(clippy::too_many_arguments)]
fn create_color_presentation(
    workspace: &mut dyn crate::correction::change_builder::ChangeWorkspace,
    path: &str,
    line_info: &dartr_syntax::LineInfo,
    edit_start: u32,
    edit_length: u32,
    written_type: Option<TypeId>,
    label_prefix: &str,
    invocation_prefix: &str,
    invocation_string: &str,
    include_const_keyword: bool,
) -> Option<Value> {
    let mut builder = ChangeBuilder::new(workspace, None);
    if !builder.add_dart_file_edit(path, |builder| {
        builder.add_replacement(edit_start, edit_length, |builder| {
            if include_const_keyword {
                builder.write("const ");
            }
            if let Some(ty) = written_type {
                builder.write_type(Some(ty), &WriteType::default());
            }
            builder.write(invocation_prefix);
            builder.write(invocation_string);
        });
    }) {
        return None;
    }

    let change = builder.source_change();
    let edits_for_file: Vec<_> = change
        .edits
        .into_iter()
        .filter(|e| e.file == path)
        .flat_map(|e| e.edits)
        .collect();
    let main_edit = edits_for_file.iter().find(|e| e.offset == edit_start)?;
    let other_edits: Vec<Value> = edits_for_file
        .iter()
        .filter(|e| e.offset != edit_start)
        .map(|e| {
            json!({
                "range": mapping::to_range(line_info, e.offset, e.length),
                "newText": e.replacement,
            })
        })
        .collect();

    let mut m = Map::new();
    m.insert(
        "label".into(),
        json!(format!("{label_prefix}{invocation_string}")),
    );
    m.insert(
        "textEdit".into(),
        json!({
            "range": mapping::to_range(line_info, main_edit.offset, main_edit.length),
            "newText": main_edit.replacement,
        }),
    );
    if !other_edits.is_empty() {
        m.insert("additionalTextEdits".into(), Value::Array(other_edits));
    }
    Some(Value::Object(m))
}

/// Computes the JSON response of `textDocument/colorPresentation`.
pub fn compute_color_presentations(
    server: &mut crate::server::Server,
    params: &Value,
    path: &str,
) -> ErrorOr<Value> {
    let resolved = server.require_resolved_unit(path)?;
    if !server.is_analyzed(path) {
        return Ok(json!([]));
    }

    let color_obj = params.get("color").ok_or_else(|| {
        mapping::ResponseError::new(mapping::codes::INVALID_PARAMS, "Invalid params")
    })?;
    let alpha_val = color_obj.get("alpha").ok_or_else(|| {
        mapping::ResponseError::new(mapping::codes::INVALID_PARAMS, "Invalid params")
    })?;
    let alpha_double = alpha_val.as_f64().ok_or_else(|| {
        mapping::ResponseError::new(mapping::codes::INVALID_PARAMS, "Invalid params")
    })?;
    let red_double = color_obj
        .get("red")
        .and_then(Value::as_f64)
        .ok_or_else(|| {
            mapping::ResponseError::new(mapping::codes::INVALID_PARAMS, "Invalid params")
        })?;
    let green_double = color_obj
        .get("green")
        .and_then(Value::as_f64)
        .ok_or_else(|| {
            mapping::ResponseError::new(mapping::codes::INVALID_PARAMS, "Invalid params")
        })?;
    let blue_double = color_obj
        .get("blue")
        .and_then(Value::as_f64)
        .ok_or_else(|| {
            mapping::ResponseError::new(mapping::codes::INVALID_PARAMS, "Invalid params")
        })?;
    let opacity_str = format_dart_num(alpha_val, alpha_double);

    let alpha_rounded = round_double(alpha_double);
    let red_rounded = round_double(red_double);
    let green_rounded = round_double(green_double);
    let blue_rounded = round_double(blue_double);

    let alpha = (alpha_double * 255.0) as i64;
    let red = (red_double * 255.0) as i64;
    let green = (green_double * 255.0) as i64;
    let blue = (blue_double * 255.0) as i64;

    let range_obj = params.get("range").ok_or_else(|| {
        mapping::ResponseError::new(mapping::codes::INVALID_PARAMS, "Invalid params")
    })?;
    let (start_line, start_char) = range_obj
        .get("start")
        .and_then(mapping::read_position)
        .ok_or_else(|| {
            mapping::ResponseError::new(mapping::codes::INVALID_PARAMS, "Invalid params")
        })?;
    let (end_line, end_char) = range_obj
        .get("end")
        .and_then(mapping::read_position)
        .ok_or_else(|| {
            mapping::ResponseError::new(mapping::codes::INVALID_PARAMS, "Invalid params")
        })?;

    let edit_start = mapping::to_offset(resolved.line_info(), start_line, start_char, false)?;
    let edit_end = mapping::to_offset(resolved.line_info(), end_line, end_char, false)?;
    let edit_length = edit_end.saturating_sub(edit_start);

    let has_widgets = {
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        find_library_by_uri(&ctx, URI_FLUTTER_WIDGETS).is_some()
    };
    let resolved = if !has_widgets {
        server.link_library_in(resolved.library.context, URI_FLUTTER_WIDGETS);
        server.require_resolved_unit(path)?
    } else {
        resolved
    };

    let (written_type, requires_const_keyword, is_dot_shorthand, line_info) = {
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let Some(widgets_lib) = find_library_by_uri(&ctx, URI_FLUTTER_WIDGETS) else {
            return Ok(json!([]));
        };
        let color_el = exported_element(&ctx, widgets_lib, "Color")
            .or_else(|| {
                find_library_by_uri(&ctx, "dart:ui")
                    .and_then(|ui_lib| exported_element(&ctx, ui_lib, "Color"))
            })
            .and_then(|e| e.cast::<InterfaceElement>());
        let Some(color_el) = color_el else {
            return Ok(json!([]));
        };
        let color_type = ctx.interface_this_type(color_el);
        let unit = resolved.unit();
        let node = unit.ast.node_covering(unit.unit, edit_start, 0);
        let requires_const_keyword =
            will_require_const_keyword(&ctx, &unit.ast, &unit.tables, node);
        let is_dot_shorthand =
            node.is_some_and(|n| unit.ast.is::<DotShorthandConstructorInvocation>(n));
        let written_type = if is_dot_shorthand {
            None
        } else {
            Some(color_type)
        };
        (
            written_type,
            requires_const_keyword,
            is_dot_shorthand,
            resolved.line_info().clone(),
        )
    };

    let color_value =
        ((alpha & 0xff) << 24) | ((red & 0xff) << 16) | ((green & 0xff) << 8) | (blue & 0xff);
    let color_value_hex = format!("0x{color_value:08X}");

    let mut presentations = Vec::new();
    let specs = [
        ("", format!(".fromARGB({alpha}, {red}, {green}, {blue})")),
        (
            "",
            format!(".fromRGBO({red}, {green}, {blue}, {opacity_str})"),
        ),
        (
            "",
            format!(
                ".from(alpha: {alpha_rounded}, red: {red_rounded}, green: {green_rounded}, blue: {blue_rounded})"
            ),
        ),
        (
            if is_dot_shorthand { ".new" } else { "" },
            format!("({color_value_hex})"),
        ),
    ];

    for (invocation_prefix, invocation_string) in &specs {
        if let Some(p) = server
            .with_change_workspace(|workspace| {
                create_color_presentation(
                    workspace,
                    path,
                    &line_info,
                    edit_start,
                    edit_length,
                    written_type,
                    "Color",
                    invocation_prefix,
                    invocation_string,
                    requires_const_keyword,
                )
            })
            .flatten()
        {
            presentations.push(p);
        }
    }

    Ok(Value::Array(presentations))
}

impl crate::server::Server {
    /// Handles `textDocument/documentColor` (Dart `DocumentColorHandler.handle`).
    pub(crate) fn document_color(&mut self, params: &Value) -> ErrorOr<Value> {
        if !crate::server::is_dart_document(params) {
            return Ok(json!([]));
        }
        let path = self.path_of_doc(params)?;
        let resolved = self.require_resolved_unit(&path)?;
        let unit_sources = self.unit_sources(resolved.library.context);
        let references = compute_colors(&resolved, unit_sources);
        Ok(color_references_to_json(resolved.line_info(), &references))
    }

    /// Handles `textDocument/colorPresentation` (Dart `ColorPresentationHandler.handle`).
    pub(crate) fn color_presentation(&mut self, params: &Value) -> ErrorOr<Value> {
        if !crate::server::is_dart_document(params) {
            return Ok(json!([]));
        }
        let path = self.path_of_doc(params)?;
        compute_color_presentations(self, params, &path)
    }
}
