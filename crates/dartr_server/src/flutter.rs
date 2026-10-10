// Dart source: pkg/analysis_server/lib/src/flutter/flutter_outline_computer.dart
// Dart source: pkg/analyzer/lib/src/utilities/extensions/flutter.dart
// Dart source: pkg/analysis_server/lib/src/lsp/mapping.dart (toFlutterOutline, toFlutterOutlineAttribute)

//! Flutter outline computation and widget type/expression helpers, shared by
//! the LSP server (`dart/textDocument/publishFlutterOutline`) and the legacy
//! analysis server (`analysis.flutterOutline`).

use dartr_ast::to_source::to_source;
use dartr_ast::*;
use dartr_element::{
    ConstructorElement, Ctx, EId, ElementId, FormalParameterElement, InterfaceElement,
    ResolutionTables, Tag, TypeId, TypeKind,
};
use dartr_format::text::substring_utf16;
use dartr_resolver::error::support::corresponding_parameter;
use dartr_syntax::LineInfo;
use dartr_typesystem::TypeExt;
use serde_json::{Map, Value, json};

use crate::computer::outline::{Element, Outline, compute_outline};
use crate::features::element_json;
use crate::mapping::to_range;

pub const URI_FRAMEWORK: &str = "package:flutter/src/widgets/framework.dart";
pub const URI_CONTAINER: &str = "package:flutter/src/widgets/container.dart";
pub const URI_BASIC: &str = "package:flutter/src/widgets/basic.dart";
pub const URI_ALIGNMENT: &str = "package:flutter/src/painting/alignment.dart";
pub const URI_EDGE_INSETS: &str = "package:flutter/src/painting/edge_insets.dart";
pub const URI_TEXT_STYLE: &str = "package:flutter/src/painting/text_style.dart";
pub const URI_WIDGETS_ICON: &str = "package:flutter/src/widgets/icon.dart";
pub const URI_WIDGETS_TEXT: &str = "package:flutter/src/widgets/text.dart";

/// Dart `FlutterOutlineKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlutterOutlineKind {
    DartElement,
    Generic,
    NewInstance,
    Invocation,
    Variable,
    Placeholder,
}

impl FlutterOutlineKind {
    pub const fn name(self) -> &'static str {
        match self {
            FlutterOutlineKind::DartElement => "DART_ELEMENT",
            FlutterOutlineKind::Generic => "GENERIC",
            FlutterOutlineKind::NewInstance => "NEW_INSTANCE",
            FlutterOutlineKind::Invocation => "INVOCATION",
            FlutterOutlineKind::Variable => "VARIABLE",
            FlutterOutlineKind::Placeholder => "PLACEHOLDER",
        }
    }
}

/// Dart `FlutterOutlineAttribute`.
#[derive(Clone, Debug, PartialEq)]
pub struct FlutterOutlineAttribute {
    pub name: String,
    pub label: String,
    pub literal_value_boolean: Option<bool>,
    pub literal_value_integer: Option<i64>,
    pub literal_value_string: Option<String>,
    /// `(offset, length)`.
    pub name_range: Option<(u32, u32)>,
    /// `(offset, length)`.
    pub value_range: Option<(u32, u32)>,
}

/// Dart `FlutterOutline`, parameterized over the Dart element representation
/// `E` (`crate::computer::outline::Element` for LSP, `protocol::Element` for
/// the legacy protocol).
#[derive(Clone, Debug, PartialEq)]
pub struct FlutterOutline<E = Element> {
    pub kind: FlutterOutlineKind,
    pub offset: u32,
    pub length: u32,
    pub code_offset: u32,
    pub code_length: u32,
    pub label: Option<String>,
    pub dart_element: Option<E>,
    pub attributes: Option<Vec<FlutterOutlineAttribute>>,
    pub class_name: Option<String>,
    pub parent_association_label: Option<String>,
    pub variable_name: Option<String>,
    pub children: Option<Vec<FlutterOutline<E>>>,
}

/// Computes the `FlutterOutline` for a resolved compilation unit (LSP),
/// porting `FlutterOutlineComputer` from
/// `pkg/analysis_server/lib/src/flutter/flutter_outline_computer.dart`.
pub fn compute_flutter_outline(
    ctx: &Ctx<'_>,
    ast: &Ast,
    tables: &ResolutionTables,
    unit: Id<CompilationUnit>,
    content: &str,
) -> FlutterOutline<Element> {
    let dart_outline = compute_outline(ast, unit);
    let mut flutter_dart_outline = convert_dart_outline(dart_outline);

    for outline in compute_widget_outlines(ctx, ast, tables, unit, content) {
        let mut opt = Some(outline);
        insert_into_dart_outline(&mut flutter_dart_outline, &mut opt);
    }

    flutter_dart_outline
}

/// Computes the top-level widget `FlutterOutline` subtrees in `unit`, which
/// can be inserted into a Dart outline via [`insert_into_dart_outline`].
pub fn compute_widget_outlines<E>(
    ctx: &Ctx<'_>,
    ast: &Ast,
    tables: &ResolutionTables,
    unit: Id<CompilationUnit>,
    content: &str,
) -> Vec<FlutterOutline<E>> {
    let computer = FlutterOutlineComputer {
        ctx,
        ast,
        tables,
        content,
    };
    let mut builder = FlutterOutlineBuilder {
        computer: &computer,
        outlines: Vec::new(),
    };
    ast.accept(unit, &mut builder);
    builder.outlines
}

fn convert_dart_outline(dart_outline: Outline) -> FlutterOutline<Element> {
    let children = dart_outline
        .children
        .map(|cs| cs.into_iter().map(convert_dart_outline).collect());
    FlutterOutline {
        kind: FlutterOutlineKind::DartElement,
        offset: dart_outline.offset,
        length: dart_outline.length,
        code_offset: dart_outline.code_offset,
        code_length: dart_outline.code_length,
        label: None,
        dart_element: Some(dart_outline.element),
        attributes: None,
        class_name: None,
        parent_association_label: None,
        variable_name: None,
        children,
    }
}

/// Inserts `outline` into the first `DartElement` node in post-order (children
/// before parent) that strictly contains `outline`, matching `_depthFirstOrder`
/// in `FlutterOutlineComputer`.
pub fn insert_into_dart_outline<E>(
    parent: &mut FlutterOutline<E>,
    outline: &mut Option<FlutterOutline<E>>,
) -> bool {
    if parent.kind != FlutterOutlineKind::DartElement {
        return false;
    }
    if let Some(children) = &mut parent.children {
        for child in children.iter_mut() {
            if insert_into_dart_outline(child, outline) {
                return true;
            }
        }
    }
    let Some(o) = outline.as_ref() else {
        return false;
    };
    if parent.offset < o.offset && o.offset + o.length < parent.offset + parent.length {
        parent
            .children
            .get_or_insert_with(Vec::new)
            .push(outline.take().unwrap());
        return true;
    }
    false
}

/// Dart `toFlutterOutline`.
pub fn flutter_outline_json(line_info: &LineInfo, outline: &FlutterOutline<Element>) -> Value {
    let mut m = Map::new();
    m.insert("kind".into(), json!(outline.kind.name()));
    if let Some(label) = &outline.label {
        m.insert("label".into(), json!(label));
    }
    if let Some(class_name) = &outline.class_name {
        m.insert("className".into(), json!(class_name));
    }
    if let Some(variable_name) = &outline.variable_name {
        m.insert("variableName".into(), json!(variable_name));
    }
    if let Some(attributes) = &outline.attributes {
        m.insert(
            "attributes".into(),
            attributes
                .iter()
                .map(|a| flutter_outline_attribute_json(line_info, a))
                .collect(),
        );
    }
    if let Some(dart_element) = &outline.dart_element {
        m.insert("dartElement".into(), element_json(line_info, dart_element));
    }
    m.insert(
        "range".into(),
        to_range(line_info, outline.offset, outline.length),
    );
    m.insert(
        "codeRange".into(),
        to_range(line_info, outline.code_offset, outline.code_length),
    );
    if let Some(children) = &outline.children {
        m.insert(
            "children".into(),
            children
                .iter()
                .map(|c| flutter_outline_json(line_info, c))
                .collect(),
        );
    }
    Value::Object(m)
}

/// Dart `toFlutterOutlineAttribute`.
fn flutter_outline_attribute_json(
    line_info: &LineInfo,
    attribute: &FlutterOutlineAttribute,
) -> Value {
    let mut m = Map::new();
    m.insert("name".into(), json!(attribute.name));
    m.insert("label".into(), json!(attribute.label));
    if let Some((offset, length)) = attribute.value_range {
        m.insert("valueRange".into(), to_range(line_info, offset, length));
    }
    Value::Object(m)
}

struct FlutterOutlineComputer<'a, 'c> {
    ctx: &'a Ctx<'c>,
    ast: &'a Ast,
    tables: &'a ResolutionTables,
    content: &'a str,
}

impl<'a, 'c> FlutterOutlineComputer<'a, 'c> {
    fn create_outline<E>(
        &self,
        node: Id<Expression>,
        with_generic: bool,
    ) -> Option<FlutterOutline<E>> {
        let &ty = self.tables.static_type.get(node.raw())?;
        let TypeKind::Interface { element: iface, .. } = *self.ctx.ty(ty) else {
            return None;
        };
        if !is_widget_interface(self.ctx, iface) {
            return None;
        }
        let class_name = self
            .ctx
            .interface(iface)
            .name
            .map(|n| self.ctx.name_str(n).to_string())
            .unwrap_or_default();

        if let Some(creation) = self.ast.cast::<InstanceCreationExpression>(node) {
            let mut attributes = Vec::new();
            let mut children = Vec::new();
            let args = self
                .ast
                .list(self.ast[self.ast[creation].argument_list].arguments);
            for &argument in args {
                let (parent_association_label, children_expression) =
                    if let Some(named) = self.ast.cast::<NamedArgument>(argument) {
                        (
                            Some(self.ast.tokens.lexeme(self.ast[named].name).to_string()),
                            self.ast[named].argument_expression,
                        )
                    } else {
                        (None, Id::<Expression>::from_raw(argument.raw()))
                    };

                let arg_type = self
                    .tables
                    .static_type
                    .get(children_expression.raw())
                    .copied();
                let is_widget_arg = arg_type.is_some_and(|t| is_widget_type(self.ctx, t));
                let is_widget_list_arg =
                    arg_type.is_some_and(|t| is_list_of_widgets_type(self.ctx, t));

                if is_widget_arg
                    && let Some(cond) = self.ast.cast::<ConditionalExpression>(children_expression)
                {
                    self.add_children_from(&mut children, self.ast[cond].then_expression.raw());
                    self.add_children_from(&mut children, self.ast[cond].else_expression.raw());
                } else if is_widget_arg {
                    if let Some(mut child) = self.create_outline(children_expression, true) {
                        child.parent_association_label = parent_association_label;
                        children.push(child);
                    }
                } else if is_widget_list_arg {
                    if let Some(list_lit) = self.ast.cast::<ListLiteral>(children_expression) {
                        for &el in self.ast.list(self.ast[list_lit].elements) {
                            self.add_children_from(&mut children, el.raw());
                        }
                    }
                } else {
                    let mut visitor = FlutterOutlineBuilder {
                        computer: self,
                        outlines: Vec::new(),
                    };
                    self.ast.accept(argument, &mut visitor);
                    if !visitor.outlines.is_empty() {
                        children.extend(visitor.outlines);
                    } else {
                        let parameter = corresponding_parameter(
                            self.ctx,
                            self.ast,
                            self.tables,
                            argument.raw(),
                        )
                        .and_then(|e| e.cast::<FormalParameterElement>());
                        self.add_attribute(&mut attributes, argument, parameter);
                    }
                }
            }

            let offset = self.ast.offset(node);
            let length = self.ast.length(node);
            return Some(FlutterOutline {
                kind: FlutterOutlineKind::NewInstance,
                offset,
                length,
                code_offset: offset,
                code_length: length,
                label: None,
                dart_element: None,
                attributes: Some(attributes),
                class_name: Some(class_name),
                parent_association_label: None,
                variable_name: None,
                children: Some(children),
            });
        }

        if with_generic {
            let mut kind = FlutterOutlineKind::Generic;
            let mut variable_name = None;
            if let Some(ident) = self.ast.cast::<SimpleIdentifier>(node) {
                kind = FlutterOutlineKind::Variable;
                variable_name = Some(self.ast.tokens.lexeme(self.ast[ident].token).to_string());
            }
            let label = if kind == FlutterOutlineKind::Generic {
                Some(self.get_short_label(node.raw()))
            } else {
                None
            };
            let offset = self.ast.offset(node);
            let length = self.ast.length(node);
            return Some(FlutterOutline {
                kind,
                offset,
                length,
                code_offset: offset,
                code_length: length,
                label,
                dart_element: None,
                attributes: None,
                class_name: Some(class_name),
                parent_association_label: None,
                variable_name,
                children: None,
            });
        }

        None
    }

    fn add_children_from<E>(&self, children: &mut Vec<FlutterOutline<E>>, element: NodeId) {
        if let Some(cond) = self.ast.cast::<ConditionalExpression>(element) {
            self.add_children_from(children, self.ast[cond].then_expression.raw());
            self.add_children_from(children, self.ast[cond].else_expression.raw());
        } else if let Some(expr) = self.ast.cast::<Expression>(element) {
            if let Some(child) = self.create_outline(expr, true) {
                children.push(child);
            }
        } else if let Some(if_el) = self.ast.cast::<IfElement>(element) {
            self.add_children_from(children, self.ast[if_el].then_element.raw());
            if let Some(else_el) = self.ast[if_el].else_element {
                self.add_children_from(children, else_el.raw());
            }
        } else if let Some(for_el) = self.ast.cast::<ForElement>(element) {
            self.add_children_from(children, self.ast[for_el].body.raw());
        } else if self.ast.is::<SpreadElement>(element) {
            // Ignored.
        } else if let Some(null_aware) = self.ast.cast::<NullAwareElement>(element) {
            self.add_children_from(children, self.ast[null_aware].value.raw());
        }
    }

    fn add_attribute(
        &self,
        attributes: &mut Vec<FlutterOutlineAttribute>,
        argument: Id<Argument>,
        parameter: Option<EId<FormalParameterElement>>,
    ) {
        let Some(parameter) = parameter else {
            return;
        };

        let (name_range, value_argument) =
            if let Some(named) = self.ast.cast::<NamedArgument>(argument) {
                let name_tok = self.ast.tokens.get(self.ast[named].name);
                (
                    Some((name_tok.offset, name_tok.end() - name_tok.offset)),
                    self.ast[named].argument_expression,
                )
            } else {
                (None, Id::<Expression>::from_raw(argument.raw()))
            };

        let value_offset = self.ast.offset(value_argument);
        let value_length = self.ast.length(value_argument);
        let value_range = Some((value_offset, value_length));

        let name = self
            .ctx
            .get(parameter)
            .name
            .map(|n| self.ctx.name_str(n).to_string())
            .unwrap_or_default();

        let mut label = substring_utf16(
            self.content,
            value_offset as usize,
            (value_offset + value_length) as usize,
        )
        .to_string();
        if label.contains('\n') {
            label = "…".to_string();
        }

        let mut literal_value_boolean = None;
        let mut literal_value_integer = None;
        let mut literal_value_string = None;

        if let Some(b) = self.ast.cast::<BooleanLiteral>(value_argument) {
            literal_value_boolean = Some(self.ast[b].value);
        } else if let Some(i) = self.ast.cast::<IntegerLiteral>(value_argument) {
            literal_value_integer = self.ast[i].value;
        } else if self.ast.is::<StringLiteral>(value_argument) {
            literal_value_string = eval_string_literal(self.ast, value_argument.raw());
        } else if let Some(fn_expr) = self.ast.cast::<FunctionExpression>(value_argument) {
            let has_parameters = self.ast[fn_expr]
                .parameters
                .is_some_and(|p| !self.ast.list(self.ast[p].parameters).is_empty());
            if self
                .ast
                .is::<ExpressionFunctionBody>(self.ast[fn_expr].body)
            {
                label = if has_parameters {
                    "(…) => …".to_string()
                } else {
                    "() => …".to_string()
                };
            } else {
                label = if has_parameters {
                    "(…) { … }".to_string()
                } else {
                    "() { … }".to_string()
                };
            }
        } else if self.ast.is::<ListLiteral>(value_argument) {
            label = "[…]".to_string();
        } else if self.ast.is::<SetOrMapLiteral>(value_argument) {
            label = "{…}".to_string();
        }

        attributes.push(FlutterOutlineAttribute {
            name,
            label,
            literal_value_boolean,
            literal_value_integer,
            literal_value_string,
            name_range,
            value_range,
        });
    }

    fn get_short_label(&self, node: NodeId) -> String {
        if let Some(mi) = self.ast.cast::<MethodInvocation>(node) {
            let mut buf = String::new();
            if let Some(target) = self.ast[mi].target {
                buf.push_str(&self.get_short_label(target.raw()));
                buf.push('.');
            }
            buf.push_str(
                self.ast
                    .tokens
                    .lexeme(self.ast[self.ast[mi].method_name].token),
            );
            let args = self
                .ast
                .list(self.ast[self.ast[mi].argument_list].arguments);
            if args.is_empty() {
                buf.push_str("()");
            } else {
                buf.push_str("(…)");
            }
            return buf;
        }
        to_source(self.ast, node)
    }
}

struct FlutterOutlineBuilder<'a, 'b, 'c, E> {
    computer: &'a FlutterOutlineComputer<'b, 'c>,
    outlines: Vec<FlutterOutline<E>>,
}

impl<E> AstVisitor for FlutterOutlineBuilder<'_, '_, '_, E> {
    fn visit_node(&mut self, ast: &Ast, node: NodeId) {
        if let Some(expr) = ast.cast::<Expression>(node) {
            if let Some(outline) = self.computer.create_outline(expr, false) {
                self.outlines.push(outline);
            } else {
                ast.visit_children(node, self);
            }
        } else {
            ast.visit_children(node, self);
        }
    }
}

pub fn eval_string_literal(ast: &Ast, node: NodeId) -> Option<String> {
    if let Some(s) = ast.cast::<SimpleStringLiteral>(node) {
        return Some(ast[s].value.to_string());
    }
    if let Some(adj) = ast.cast::<AdjacentStrings>(node) {
        let mut out = String::new();
        for &part in ast.list(ast[adj].strings) {
            out.push_str(&eval_string_literal(ast, part.raw())?);
        }
        return Some(out);
    }
    None
}

fn elide_to(s: &str, limit: usize) -> String {
    let chars: Vec<u16> = s.encode_utf16().collect();
    if chars.len() > limit {
        let head_len = limit / 2 - 1;
        let tail_len = limit - head_len - 3;
        let head = String::from_utf16_lossy(&chars[..head_len]);
        let tail = String::from_utf16_lossy(&chars[chars.len() - tail_len..]);
        format!("{head}...{tail}")
    } else {
        s.to_string()
    }
}

/// Returns `node.widgetPresentationText` if `node.isWidgetCreation` is true,
/// following `InstanceCreationExpressionExtension.widgetPresentationText` in
/// `pkg/analyzer/lib/src/utilities/extensions/flutter.dart`.
pub fn widget_presentation_text(
    ctx: &Ctx<'_>,
    ast: &Ast,
    tables: &ResolutionTables,
    node: Id<InstanceCreationExpression>,
) -> Option<String> {
    let ctor = constructor_of_creation(ctx, ast, tables, node)?;
    let enc = ctx.get(ctor).enclosing?.cast::<InterfaceElement>()?;
    if !is_widget_interface(ctx, enc) {
        return None;
    }
    let args = ast.list(ast[ast[node].argument_list].arguments);
    if is_exact_class(ctx, enc, URI_WIDGETS_ICON, "Icon") {
        if let Some(&first) = args.first() {
            let arg = elide_to(&to_source(ast, first.raw()), 32);
            return Some(format!("Icon({arg})"));
        }
        return Some("Icon".to_string());
    }
    if is_exact_class(ctx, enc, URI_WIDGETS_TEXT, "Text") {
        if let Some(&first) = args.first() {
            let arg = elide_to(&to_source(ast, first.raw()), 32);
            return Some(format!("Text({arg})"));
        }
        return Some("Text".to_string());
    }
    Some(
        ctx.interface(enc)
            .name
            .map(|n| ctx.name_str(n).to_string())
            .unwrap_or_else(|| "<unknown>".to_string()),
    )
}

pub fn is_widget_type(ctx: &Ctx<'_>, ty: TypeId) -> bool {
    let TypeKind::Interface { element, .. } = *ctx.ty(ty) else {
        return false;
    };
    is_widget_interface(ctx, element)
}

pub fn is_list_of_widgets_type(ctx: &Ctx<'_>, ty: TypeId) -> bool {
    let TypeKind::Interface { element, args, .. } = *ctx.ty(ty) else {
        return false;
    };
    if element != ctx.tp.list_element.get().upcast() {
        return false;
    }
    let type_args = ctx.list(args);
    type_args
        .first()
        .is_some_and(|&arg| is_widget_type(ctx, arg))
}

pub fn is_widget_interface(ctx: &Ctx<'_>, element: EId<InterfaceElement>) -> bool {
    if element.raw().tag() != Tag::Class {
        return false;
    }
    if is_exact_class(ctx, element, URI_FRAMEWORK, "Widget") {
        return true;
    }
    for &sup_ty in ctx.all_supertypes(ctx.interface_this_type(element)).iter() {
        if let TypeKind::Interface { element: sup, .. } = *ctx.ty(sup_ty)
            && is_exact_class(ctx, sup, URI_FRAMEWORK, "Widget")
        {
            return true;
        }
    }
    false
}

pub fn is_exact_class(
    ctx: &Ctx<'_>,
    element: EId<InterfaceElement>,
    uri: &str,
    name: &str,
) -> bool {
    if element.raw().tag() != Tag::Class {
        return false;
    }
    let iface = ctx.interface(element);
    let Some(el_name) = iface.name else {
        return false;
    };
    if ctx.name_str(el_name) != name {
        return false;
    }
    library_uri_str(ctx, element.raw()).as_deref() == Some(uri)
}

pub fn library_uri_str(ctx: &Ctx<'_>, element: ElementId) -> Option<String> {
    let lib_id = ctx.element_data(element)?.library?;
    let first_frag = ctx.get(lib_id).first_fragment();
    Some(ctx.fragment(first_frag).source.uri.to_string())
}

pub fn constructor_of_creation(
    ctx: &Ctx<'_>,
    ast: &Ast,
    tables: &ResolutionTables,
    creation: Id<InstanceCreationExpression>,
) -> Option<EId<ConstructorElement>> {
    let ctor_name = ast[creation].constructor_name;
    let elem_ref = tables
        .element
        .get(ctor_name.raw())
        .or_else(|| tables.element.get(creation.raw()))
        .copied()?;
    let el = dartr_typesystem::member::base_element(ctx, elem_ref);
    el.cast::<ConstructorElement>()
}

pub fn find_instance_creation_expression(
    ast: &Ast,
    mut node: NodeId,
) -> Option<Id<InstanceCreationExpression>> {
    if ast.is::<ImportPrefixReference>(node) {
        node = ast.parent(node)?;
    }
    if ast.is::<SimpleIdentifier>(node) {
        node = ast.parent(node)?;
    }
    if ast.is::<PrefixedIdentifier>(node) {
        node = ast.parent(node)?;
    }
    if ast.is::<NamedType>(node) {
        node = ast.parent(node)?;
    }
    if ast.is::<ConstructorName>(node) {
        node = ast.parent(node)?;
    }
    ast.cast::<InstanceCreationExpression>(node)
}
