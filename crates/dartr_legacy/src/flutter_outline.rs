use dartr_ast::*;
use dartr_element::{
    AnyElement, ClassElement, ConstructorElement, Ctx, EId, ElementId, EnumElement, FieldElement,
    FormalParameterElement, FragmentFlags, InterfaceElement, ParameterKind, ResolutionTables, Tag,
    TypeId, TypeKind,
};
use dartr_format::text::{substring_utf16, utf16_len};
use dartr_format::{DartFormatter, SourceCode, Version};
use dartr_resolver::element_ext::is_enum_constant;
use dartr_resolver::element_metadata::{UnitAst, element_has, flags as meta_flags};
use dartr_resolver::error::support::corresponding_parameter;
use dartr_syntax::{LineInfo, TokenType};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::convert::location_from_line_info;
use crate::outline::compute_dart_outline;
use crate::protocol;

pub use dartr_server::flutter::{
    URI_ALIGNMENT, URI_BASIC, URI_CONTAINER, URI_EDGE_INSETS, URI_FRAMEWORK, URI_TEXT_STYLE,
    URI_WIDGETS_ICON, URI_WIDGETS_TEXT, constructor_of_creation, eval_string_literal,
    find_instance_creation_expression, is_exact_class, is_list_of_widgets_type,
    is_widget_interface, is_widget_type, library_uri_str, widget_presentation_text,
};

/// Computes the `FlutterOutline` for a resolved compilation unit,
/// delegating widget outline computation to [`dartr_server::flutter`].
pub fn compute_flutter_outline(
    ctx: &Ctx<'_>,
    ast: &Ast,
    tables: &ResolutionTables,
    unit: Id<CompilationUnit>,
    file: &str,
    content: &str,
    line_info: &LineInfo,
) -> protocol::FlutterOutline {
    let dart_outline = compute_dart_outline(file, ctx, ast, tables, line_info, unit, false);
    let mut flutter_dart_outline = convert_dart_outline(dart_outline);

    for outline in dartr_server::flutter::compute_widget_outlines(ctx, ast, tables, unit, content) {
        let mut opt = Some(outline);
        dartr_server::flutter::insert_into_dart_outline(&mut flutter_dart_outline, &mut opt);
    }

    to_protocol_flutter_outline(file, line_info, flutter_dart_outline)
}

fn convert_dart_outline(
    dart_outline: protocol::Outline,
) -> dartr_server::flutter::FlutterOutline<protocol::Element> {
    let children = dart_outline
        .children
        .map(|cs| cs.into_iter().map(convert_dart_outline).collect());
    dartr_server::flutter::FlutterOutline {
        kind: dartr_server::flutter::FlutterOutlineKind::DartElement,
        offset: dart_outline.offset as u32,
        length: dart_outline.length as u32,
        code_offset: dart_outline.code_offset as u32,
        code_length: dart_outline.code_length as u32,
        label: None,
        dart_element: Some(dart_outline.element),
        attributes: None,
        class_name: None,
        parent_association_label: None,
        variable_name: None,
        children,
    }
}

fn to_protocol_flutter_outline(
    file: &str,
    line_info: &LineInfo,
    outline: dartr_server::flutter::FlutterOutline<protocol::Element>,
) -> protocol::FlutterOutline {
    let kind = match outline.kind {
        dartr_server::flutter::FlutterOutlineKind::DartElement => {
            protocol::FlutterOutlineKind::DartElement
        }
        dartr_server::flutter::FlutterOutlineKind::Generic => protocol::FlutterOutlineKind::GENERIC,
        dartr_server::flutter::FlutterOutlineKind::NewInstance => {
            protocol::FlutterOutlineKind::NewInstance
        }
        dartr_server::flutter::FlutterOutlineKind::Invocation => {
            protocol::FlutterOutlineKind::INVOCATION
        }
        dartr_server::flutter::FlutterOutlineKind::Variable => {
            protocol::FlutterOutlineKind::VARIABLE
        }
        dartr_server::flutter::FlutterOutlineKind::Placeholder => {
            protocol::FlutterOutlineKind::PLACEHOLDER
        }
    };
    let attributes = outline.attributes.map(|attrs| {
        attrs
            .into_iter()
            .map(|a| protocol::FlutterOutlineAttribute {
                name: a.name,
                label: a.label,
                literal_value_boolean: a.literal_value_boolean,
                literal_value_integer: a.literal_value_integer,
                literal_value_string: a.literal_value_string,
                name_location: a
                    .name_range
                    .map(|(o, l)| location_from_line_info(file, line_info, o, l)),
                value_location: a
                    .value_range
                    .map(|(o, l)| location_from_line_info(file, line_info, o, l)),
            })
            .collect()
    });
    let children = outline.children.map(|cs| {
        cs.into_iter()
            .map(|c| to_protocol_flutter_outline(file, line_info, c))
            .collect()
    });
    protocol::FlutterOutline {
        kind,
        offset: outline.offset as i64,
        length: outline.length as i64,
        code_offset: outline.code_offset as i64,
        code_length: outline.code_length as i64,
        label: outline.label,
        dart_element: outline.dart_element,
        attributes,
        class_name: outline.class_name,
        parent_association_label: outline.parent_association_label,
        variable_name: outline.variable_name,
        children,
    }
}

// ---------------------------------------------------------------------------
// WidgetDescriptions (widget_descriptions.dart, class_description.dart, property.dart)
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct WidgetDescriptions {
    next_property_id: i64,
    properties: FxHashMap<i64, CachedProperty>,
}

#[derive(Clone)]
struct CachedProperty {
    file: String,
    content: String,
    version: (u32, u32),
    is_required: bool,
    remove_range: Option<(u32, u32)>,
    function_body_range: Option<(u32, u32)>,
    edit_action: PropertyEditAction,
}

#[derive(Clone)]
enum PropertyEditAction {
    None,
    ReplaceExpression {
        offset: u32,
        length: u32,
    },
    InsertNamedArgument {
        insert_offset: u32,
        needs_leading_comma: bool,
        parameter_name: String,
    },
    MaterializeNestedClass {
        parent_action: Box<PropertyEditAction>,
        class_name: String,
        parameter_name: String,
    },
    MaterializeVirtualContainer {
        parameter_name: String,
        virtual_container: VirtualContainerAction,
    },
    EdgeInsetsNested {
        side: EdgeInsetsSide,
        left_value: Option<f64>,
        top_value: Option<f64>,
        right_value: Option<f64>,
        bottom_value: Option<f64>,
        parent_remove_range: Option<(u32, u32)>,
        parent_edit_action: Box<PropertyEditAction>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EdgeInsetsSide {
    Left,
    Top,
    Right,
    Bottom,
}

#[derive(Clone)]
enum VirtualContainerAction {
    WrapWidget {
        widget_offset: u32,
        widget_end: u32,
    },
    ReplaceParent {
        parent_start: u32,
        parent_ctor_end: u32,
        existing_arg_name: String,
        existing_arg_offset: u32,
        existing_arg_end: u32,
    },
}

struct BuiltProperty {
    protocol_property: protocol::FlutterWidgetProperty,
    cached: CachedProperty,
    children: Vec<BuiltProperty>,
}

impl WidgetDescriptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn flush(&mut self) {
        self.properties.clear();
    }

    fn alloc_id(&mut self) -> i64 {
        let id = self.next_property_id;
        self.next_property_id += 1;
        id
    }

    #[allow(clippy::too_many_arguments)]
    pub fn get_description(
        &mut self,
        ctx: &Ctx<'_>,
        ast: &Ast,
        tables: &ResolutionTables,
        unit: Id<CompilationUnit>,
        file: &str,
        content: &str,
        version: (u32, u32),
        offset: i64,
    ) -> Option<protocol::FlutterGetWidgetDescriptionResult> {
        if offset < 0 {
            return None;
        }
        let node = node_covering_offset(ast, unit.raw(), offset as u32)?;
        let instance_creation = find_instance_creation_expression(ast, node)?;
        let ctor = constructor_of_creation(ctx, ast, tables, instance_creation)?;

        let class_alignment = find_class_in_ctx(ctx, "package:flutter/widgets.dart", "Alignment")
            .or_else(|| find_class_in_ctx(ctx, URI_ALIGNMENT, "Alignment"));
        let class_alignment_directional =
            find_class_in_ctx(ctx, "package:flutter/widgets.dart", "AlignmentDirectional")
                .or_else(|| find_class_in_ctx(ctx, URI_ALIGNMENT, "AlignmentDirectional"));
        let class_container = find_class_in_ctx(ctx, "package:flutter/widgets.dart", "Container")
            .or_else(|| find_class_in_ctx(ctx, URI_CONTAINER, "Container"));
        let class_edge_insets =
            find_class_in_ctx(ctx, "package:flutter/widgets.dart", "EdgeInsets")
                .or_else(|| find_class_in_ctx(ctx, URI_EDGE_INSETS, "EdgeInsets"));

        let mut computer = WidgetDescriptionComputer {
            descriptions: self,
            ctx,
            ast,
            tables,
            file,
            content,
            version,
            elements_being_processed: FxHashSet::default(),
            class_alignment,
            class_alignment_directional,
            class_container,
            class_edge_insets,
        };

        let mut properties = Vec::new();
        computer.add_properties(
            &mut properties,
            None,
            None,
            Some(instance_creation),
            Some(ctor),
            enclosing_function_body_range(ast, instance_creation.raw()),
        );
        computer.add_container_property(&mut properties, instance_creation);

        let protocol_properties = self.register_properties(properties);
        Some(protocol::FlutterGetWidgetDescriptionResult {
            properties: protocol_properties,
        })
    }

    fn register_properties(
        &mut self,
        properties: Vec<BuiltProperty>,
    ) -> Vec<protocol::FlutterWidgetProperty> {
        let mut out = Vec::with_capacity(properties.len());
        for mut prop in properties {
            let children = self.register_properties(prop.children);
            prop.protocol_property.children = Some(children);
            self.properties
                .insert(prop.protocol_property.id, prop.cached);
            out.push(prop.protocol_property);
        }
        out
    }

    pub fn set_property_value(
        &self,
        id: i64,
        value: Option<protocol::FlutterWidgetPropertyValue>,
    ) -> Result<protocol::FlutterSetWidgetPropertyValueResult, &'static str> {
        let Some(prop) = self.properties.get(&id) else {
            return Err("FLUTTER_SET_WIDGET_PROPERTY_VALUE_INVALID_ID");
        };

        if let Some(val) = value {
            let change = apply_property_value_change(prop, &val)?;
            Ok(protocol::FlutterSetWidgetPropertyValueResult { change })
        } else {
            if prop.is_required {
                return Err("FLUTTER_SET_WIDGET_PROPERTY_VALUE_IS_REQUIRED");
            }
            let mut file_edits = Vec::new();
            if let Some((begin, end)) = prop.remove_range
                && end > begin
            {
                file_edits.push(protocol::SourceFileEdit {
                    file: prop.file.clone(),
                    file_stamp: 0,
                    edits: vec![protocol::SourceEdit {
                        offset: begin as i64,
                        length: (end - begin) as i64,
                        replacement: String::new(),
                        id: None,
                        description: None,
                    }],
                });
            }
            Ok(protocol::FlutterSetWidgetPropertyValueResult {
                change: protocol::SourceChange {
                    message: String::new(),
                    edits: file_edits,
                    linked_edit_groups: Vec::new(),
                    selection: None,
                    selection_length: None,
                    id: None,
                },
            })
        }
    }
}

struct WidgetDescriptionComputer<'a, 'c> {
    descriptions: &'a mut WidgetDescriptions,
    ctx: &'a Ctx<'c>,
    ast: &'a Ast,
    tables: &'a ResolutionTables,
    file: &'a str,
    content: &'a str,
    version: (u32, u32),
    elements_being_processed: FxHashSet<ElementId>,
    class_alignment: Option<EId<ClassElement>>,
    class_alignment_directional: Option<EId<ClassElement>>,
    class_container: Option<EId<ClassElement>>,
    class_edge_insets: Option<EId<ClassElement>>,
}

impl<'a, 'c> WidgetDescriptionComputer<'a, 'c> {
    fn add_properties(
        &mut self,
        properties: &mut Vec<BuiltProperty>,
        parent_edit_action: Option<&PropertyEditAction>,
        class_description: Option<(EId<ClassElement>, EId<ConstructorElement>)>,
        instance_creation: Option<Id<InstanceCreationExpression>>,
        constructor_element: Option<EId<ConstructorElement>>,
        fn_body_range: Option<(u32, u32)>,
    ) {
        let ctor = constructor_element
            .or_else(|| {
                instance_creation
                    .and_then(|ic| constructor_of_creation(self.ctx, self.ast, self.tables, ic))
            })
            .or_else(|| class_description.map(|(_, c)| c));
        let Some(ctor) = ctor else {
            return;
        };
        let Some(enclosing_el) = self.ctx.get(ctor).enclosing else {
            return;
        };
        if !self.elements_being_processed.insert(enclosing_el) {
            return;
        }

        let mut existing_named = FxHashSet::default();
        if let Some(ic) = instance_creation {
            let args = self
                .ast
                .list(self.ast[self.ast[ic].argument_list].arguments);
            for &arg in args {
                let Some(param) =
                    corresponding_parameter(self.ctx, self.ast, self.tables, arg.raw())
                        .and_then(|e| e.cast::<FormalParameterElement>())
                else {
                    continue;
                };
                let value_expr = if let Some(named) = self.ast.cast::<NamedArgument>(arg) {
                    if let Some(pname) = self.ctx.get(param).name {
                        existing_named.insert(self.ctx.name_str(pname).to_string());
                    }
                    self.ast[named].argument_expression
                } else {
                    Id::<Expression>::from_raw(arg.raw())
                };
                self.add_property(
                    properties,
                    parent_edit_action,
                    param,
                    class_description,
                    Some(ic),
                    Some(arg),
                    Some(value_expr),
                    fn_body_range,
                );
            }
        }

        let formal_params = self.ctx.get(ctor).formal_params.clone();
        for param in formal_params {
            let pdata = self.ctx.get(param);
            if !pdata.kind.is_named() {
                continue;
            }
            let pname = pdata
                .name
                .map(|n| self.ctx.name_str(n).to_string())
                .unwrap_or_default();
            if existing_named.contains(&pname) {
                continue;
            }
            self.add_property(
                properties,
                parent_edit_action,
                param,
                class_description,
                instance_creation,
                None,
                None,
                fn_body_range,
            );
        }

        self.elements_being_processed.remove(&enclosing_el);
    }

    #[allow(clippy::too_many_arguments)]
    fn add_property(
        &mut self,
        properties: &mut Vec<BuiltProperty>,
        parent_edit_action: Option<&PropertyEditAction>,
        parameter: EId<FormalParameterElement>,
        class_description: Option<(EId<ClassElement>, EId<ConstructorElement>)>,
        instance_creation: Option<Id<InstanceCreationExpression>>,
        argument_expression: Option<Id<Argument>>,
        value_expression: Option<Id<Expression>>,
        fn_body_range: Option<(u32, u32)>,
    ) {
        let documentation = get_parameter_documentation(self.ctx, parameter);
        let value_expression_code = value_expression.map(|ve| {
            let off = self.ast.offset(ve) as usize;
            let end = self.ast.end(ve) as usize;
            substring_utf16(self.content, off, end).to_string()
        });

        let (value, is_safe_to_update) = if let Some(ve) = value_expression {
            let v = self.to_value(ve);
            let safe = v.is_some();
            (v, safe)
        } else {
            (None, true)
        };

        let pdata = self.ctx.get(parameter);
        let param_name = pdata
            .name
            .map(|n| self.ctx.name_str(n).to_string())
            .unwrap_or_default();
        let param_type = pdata.type_.get().unwrap_or(TypeId::INVALID);
        let is_required = pdata.kind == ParameterKind::Required;

        let remove_range = match (instance_creation, argument_expression) {
            (Some(ic), Some(arg)) => Some(compute_argument_remove_range(self.ast, ic, arg)),
            _ => None,
        };

        let edit_action = self.compute_edit_action(
            parent_edit_action,
            &param_name,
            class_description,
            instance_creation,
            value_expression,
        );

        let id = self.descriptions.alloc_id();
        let protocol_property = protocol::FlutterWidgetProperty {
            id,
            is_required,
            is_safe_to_update,
            name: param_name,
            documentation,
            editor: self.get_editor(param_type),
            expression: value_expression_code,
            value,
            children: None,
        };

        let mut built = BuiltProperty {
            protocol_property,
            cached: CachedProperty {
                file: self.file.to_string(),
                content: self.content.to_string(),
                version: self.version,
                is_required,
                remove_range,
                function_body_range: fn_body_range,
                edit_action: edit_action.clone(),
            },
            children: Vec::new(),
        };

        if let Some(class_edge_insets) = self.class_edge_insets
            && is_exact_edge_insets_geometry_type(self.ctx, param_type)
        {
            self.add_edge_insets_nested_properties(
                &mut built,
                class_edge_insets,
                value_expression,
                fn_body_range,
            );
        } else if let Some(ve) = value_expression
            && let Some(ve_ic) = self.ast.cast::<InstanceCreationExpression>(ve)
        {
            if let Some(&ty) = self.tables.static_type.get(ve.raw())
                && has_nested_properties(self.ctx, ty)
            {
                self.add_properties(
                    &mut built.children,
                    Some(&edit_action),
                    None,
                    Some(ve_ic),
                    None,
                    fn_body_range,
                );
            }
        } else if value_expression.is_none()
            && let TypeKind::Interface { element, .. } = *self.ctx.ty(param_type)
            && let Some(cd) = self.class_description_of(element)
        {
            self.add_properties(
                &mut built.children,
                Some(&edit_action),
                Some(cd),
                None,
                Some(cd.1),
                fn_body_range,
            );
        }

        properties.push(built);
    }

    fn compute_edit_action(
        &self,
        parent_edit_action: Option<&PropertyEditAction>,
        parameter_name: &str,
        class_description: Option<(EId<ClassElement>, EId<ConstructorElement>)>,
        instance_creation: Option<Id<InstanceCreationExpression>>,
        value_expression: Option<Id<Expression>>,
    ) -> PropertyEditAction {
        if let Some(ve) = value_expression {
            return PropertyEditAction::ReplaceExpression {
                offset: self.ast.offset(ve),
                length: self.ast.length(ve),
            };
        }
        if let Some(ic) = instance_creation {
            let (insert_offset, needs_leading_comma) =
                compute_named_arg_insertion(self.ast, ic, parameter_name);
            return PropertyEditAction::InsertNamedArgument {
                insert_offset,
                needs_leading_comma,
                parameter_name: parameter_name.to_string(),
            };
        }
        if let Some(parent_action) = parent_edit_action {
            if let PropertyEditAction::MaterializeVirtualContainer {
                virtual_container, ..
            } = parent_action
            {
                return PropertyEditAction::MaterializeVirtualContainer {
                    parameter_name: parameter_name.to_string(),
                    virtual_container: virtual_container.clone(),
                };
            }
            if let Some((cls, _)) = class_description {
                let class_name = self
                    .ctx
                    .get(cls)
                    .name
                    .map(|n| self.ctx.name_str(n).to_string())
                    .unwrap_or_default();
                return PropertyEditAction::MaterializeNestedClass {
                    parent_action: Box::new(parent_action.clone()),
                    class_name,
                    parameter_name: parameter_name.to_string(),
                };
            }
        }
        PropertyEditAction::None
    }

    fn add_container_property(
        &mut self,
        properties: &mut Vec<BuiltProperty>,
        widget_creation: Id<InstanceCreationExpression>,
    ) {
        let Some(ctor) = constructor_of_creation(self.ctx, self.ast, self.tables, widget_creation)
        else {
            return;
        };
        let Some(enc) = self
            .ctx
            .get(ctor)
            .enclosing
            .and_then(|e| e.cast::<InterfaceElement>())
        else {
            return;
        };
        if !is_widget_interface(self.ctx, enc) {
            return;
        }

        let fn_body_range = enclosing_function_body_range(self.ast, widget_creation.raw());
        let mut parent_creation: Option<Id<InstanceCreationExpression>> = None;
        if let Some(parent) = self.ast.parent(widget_creation)
            && let Some(child_arg) = self.ast.cast::<NamedArgument>(parent)
            && self.ast.tokens.lexeme(self.ast[child_arg].name) == "child"
            && let Some(arg_list) = self.ast.parent(child_arg)
            && self.ast.is::<ArgumentList>(arg_list)
            && let Some(arg_list_parent) = self.ast.parent(arg_list)
            && let Some(pc) = self.ast.cast::<InstanceCreationExpression>(arg_list_parent)
        {
            parent_creation = Some(pc);
        }

        if let Some(pc) = parent_creation
            && is_exactly_container_creation(self.ctx, self.tables, pc)
        {
            let id = self.descriptions.alloc_id();
            let mut container_prop = BuiltProperty {
                protocol_property: protocol::FlutterWidgetProperty {
                    id,
                    is_required: true,
                    is_safe_to_update: false,
                    name: "Container".to_string(),
                    documentation: None,
                    editor: None,
                    expression: None,
                    value: None,
                    children: None,
                },
                cached: CachedProperty {
                    file: self.file.to_string(),
                    content: self.content.to_string(),
                    version: self.version,
                    is_required: true,
                    remove_range: None,
                    function_body_range: fn_body_range,
                    edit_action: PropertyEditAction::None,
                },
                children: Vec::new(),
            };
            self.add_properties(
                &mut container_prop.children,
                None,
                None,
                Some(pc),
                None,
                fn_body_range,
            );
            container_prop
                .children
                .retain(|p| p.protocol_property.name != "child");
            properties.push(container_prop);
            return;
        }

        let Some(class_container) = self.class_container else {
            return;
        };
        let Some(container_desc) = self.class_description_of(class_container.upcast()) else {
            return;
        };

        let mut virtual_action = VirtualContainerAction::WrapWidget {
            widget_offset: self.ast.offset(widget_creation),
            widget_end: self.ast.end(widget_creation),
        };
        let container_id = self.descriptions.alloc_id();
        let parent_marker_action = PropertyEditAction::MaterializeVirtualContainer {
            parameter_name: String::new(),
            virtual_container: virtual_action.clone(),
        };
        let mut container_prop = BuiltProperty {
            protocol_property: protocol::FlutterWidgetProperty {
                id: container_id,
                is_required: true,
                is_safe_to_update: false,
                name: "Container".to_string(),
                documentation: None,
                editor: None,
                expression: None,
                value: None,
                children: None,
            },
            cached: CachedProperty {
                file: self.file.to_string(),
                content: self.content.to_string(),
                version: self.version,
                is_required: true,
                remove_range: None,
                function_body_range: fn_body_range,
                edit_action: parent_marker_action.clone(),
            },
            children: Vec::new(),
        };

        self.add_properties(
            &mut container_prop.children,
            Some(&parent_marker_action),
            Some(container_desc),
            None,
            Some(container_desc.1),
            fn_body_range,
        );

        if let Some(pc) = parent_creation {
            if is_exactly_align_creation(self.ctx, self.tables, pc)
                && find_named_arg(self.ast, pc, "widthFactor").is_none()
                && find_named_arg(self.ast, pc, "heightFactor").is_none()
            {
                self.replace_nested_container_property(
                    &mut container_prop,
                    &mut virtual_action,
                    pc,
                    "alignment",
                    fn_body_range,
                );
            }
            if is_exactly_padding_creation(self.ctx, self.tables, pc) {
                self.replace_nested_container_property(
                    &mut container_prop,
                    &mut virtual_action,
                    pc,
                    "padding",
                    fn_body_range,
                );
            }
        }

        // Update virtual_action in all children that materialize the virtual container
        for child in &mut container_prop.children {
            if let PropertyEditAction::MaterializeVirtualContainer {
                virtual_container, ..
            } = &mut child.cached.edit_action
            {
                *virtual_container = virtual_action.clone();
            }
        }

        container_prop
            .children
            .retain(|p| p.protocol_property.name != "child");
        properties.push(container_prop);
    }

    fn replace_nested_container_property(
        &mut self,
        container_prop: &mut BuiltProperty,
        virtual_action: &mut VirtualContainerAction,
        parent_creation: Id<InstanceCreationExpression>,
        name: &str,
        fn_body_range: Option<(u32, u32)>,
    ) {
        let Some(arg) = find_named_arg(self.ast, parent_creation, name) else {
            return;
        };
        let Some(param) = corresponding_parameter(self.ctx, self.ast, self.tables, arg.raw())
            .and_then(|e| e.cast::<FormalParameterElement>())
        else {
            return;
        };
        let mut replacements = Vec::new();
        self.add_property(
            &mut replacements,
            None,
            param,
            None,
            Some(parent_creation),
            Some(arg.upcast()),
            Some(self.ast[arg].argument_expression),
            fn_body_range,
        );
        if let Some(replacement) = replacements.into_iter().next() {
            for child in &mut container_prop.children {
                if child.protocol_property.name == name {
                    *child = replacement;
                    break;
                }
            }
            *virtual_action = VirtualContainerAction::ReplaceParent {
                parent_start: self.ast.offset(parent_creation),
                parent_ctor_end: self.ast.end(self.ast[parent_creation].constructor_name),
                existing_arg_name: self.ast.tokens.lexeme(self.ast[arg].name).to_string(),
                existing_arg_offset: self.ast.offset(arg),
                existing_arg_end: self.ast.end(arg),
            };
        }
    }

    fn add_edge_insets_nested_properties(
        &mut self,
        property: &mut BuiltProperty,
        class_edge_insets: EId<ClassElement>,
        value_expression: Option<Id<Expression>>,
        fn_body_range: Option<(u32, u32)>,
    ) {
        let mut left_expr: Option<Id<Expression>> = None;
        let mut top_expr: Option<Id<Expression>> = None;
        let mut right_expr: Option<Id<Expression>> = None;
        let mut bottom_expr: Option<Id<Expression>> = None;

        if let Some(ve) = value_expression
            && let Some(ic) = self.ast.cast::<InstanceCreationExpression>(ve)
            && let Some(ctor) = constructor_of_creation(self.ctx, self.ast, self.tables, ic)
            && self.ctx.get(ctor).enclosing == Some(class_edge_insets.raw())
        {
            let ctor_name = self
                .ctx
                .get(ctor)
                .name
                .map(|n| self.ctx.name_str(n))
                .unwrap_or("new");
            let args = self
                .ast
                .list(self.ast[self.ast[ic].argument_list].arguments);
            let arg_expr = |idx: usize| -> Option<Id<Expression>> {
                let &a = args.get(idx)?;
                if let Some(named) = self.ast.cast::<NamedArgument>(a) {
                    Some(self.ast[named].argument_expression)
                } else {
                    Some(Id::<Expression>::from_raw(a.raw()))
                }
            };
            match ctor_name {
                "all" => {
                    let e = arg_expr(0);
                    left_expr = e;
                    top_expr = e;
                    right_expr = e;
                    bottom_expr = e;
                }
                "fromLTRB" => {
                    left_expr = arg_expr(0);
                    top_expr = arg_expr(1);
                    right_expr = arg_expr(2);
                    bottom_expr = arg_expr(3);
                }
                "only" => {
                    left_expr = find_named_arg(self.ast, ic, "left")
                        .map(|a| self.ast[a].argument_expression);
                    top_expr = find_named_arg(self.ast, ic, "top")
                        .map(|a| self.ast[a].argument_expression);
                    right_expr = find_named_arg(self.ast, ic, "right")
                        .map(|a| self.ast[a].argument_expression);
                    bottom_expr = find_named_arg(self.ast, ic, "bottom")
                        .map(|a| self.ast[a].argument_expression);
                }
                "symmetric" => {
                    let h = find_named_arg(self.ast, ic, "horizontal")
                        .map(|a| self.ast[a].argument_expression);
                    let v = find_named_arg(self.ast, ic, "vertical")
                        .map(|a| self.ast[a].argument_expression);
                    left_expr = h;
                    top_expr = v;
                    right_expr = h;
                    bottom_expr = v;
                }
                _ => {}
            }
        }

        let left_val = left_expr.and_then(|e| eval_double_literal(self.ast, e));
        let top_val = top_expr.and_then(|e| eval_double_literal(self.ast, e));
        let right_val = right_expr.and_then(|e| eval_double_literal(self.ast, e));
        let bottom_val = bottom_expr.and_then(|e| eval_double_literal(self.ast, e));

        let only_ctor = self
            .ctx
            .get(class_edge_insets)
            .constructors
            .iter()
            .copied()
            .find(|&c| {
                self.ctx
                    .get(c)
                    .name
                    .is_some_and(|n| self.ctx.name_str(n) == "only")
            });

        let sides = [
            ("left", EdgeInsetsSide::Left, left_expr, left_val),
            ("top", EdgeInsetsSide::Top, top_expr, top_val),
            ("right", EdgeInsetsSide::Right, right_expr, right_val),
            ("bottom", EdgeInsetsSide::Bottom, bottom_expr, bottom_val),
        ];

        for (name, side, expr, val) in sides {
            let param = only_ctor.and_then(|c| {
                self.ctx.get(c).formal_params.iter().copied().find(|&p| {
                    self.ctx
                        .get(p)
                        .name
                        .is_some_and(|n| self.ctx.name_str(n) == name)
                })
            });
            let documentation = param.and_then(|p| get_parameter_documentation(self.ctx, p));
            let expression = expr.map(|e| {
                let off = self.ast.offset(e) as usize;
                let end = self.ast.end(e) as usize;
                substring_utf16(self.content, off, end).to_string()
            });
            let value = val.map(|d| protocol::FlutterWidgetPropertyValue {
                bool_value: None,
                double_value: Some(d),
                int_value: None,
                string_value: None,
                enum_value: None,
                expression: None,
            });
            let id = self.descriptions.alloc_id();
            property.children.push(BuiltProperty {
                protocol_property: protocol::FlutterWidgetProperty {
                    id,
                    is_required: true,
                    is_safe_to_update: true,
                    name: name.to_string(),
                    documentation,
                    editor: Some(protocol::FlutterWidgetPropertyEditor {
                        kind: protocol::FlutterWidgetPropertyEditorKind::DOUBLE,
                        enum_items: None,
                    }),
                    expression,
                    value,
                    children: None,
                },
                cached: CachedProperty {
                    file: self.file.to_string(),
                    content: self.content.to_string(),
                    version: self.version,
                    is_required: true,
                    remove_range: None,
                    function_body_range: fn_body_range,
                    edit_action: PropertyEditAction::EdgeInsetsNested {
                        side,
                        left_value: left_val,
                        top_value: top_val,
                        right_value: right_val,
                        bottom_value: bottom_val,
                        parent_remove_range: property.cached.remove_range,
                        parent_edit_action: Box::new(property.cached.edit_action.clone()),
                    },
                },
                children: Vec::new(),
            });
        }
    }

    fn class_description_of(
        &self,
        element: EId<InterfaceElement>,
    ) -> Option<(EId<ClassElement>, EId<ConstructorElement>)> {
        let cls = element.raw().cast::<ClassElement>()?;
        if !is_opted_in_class(self.ctx, element) {
            return None;
        }
        let unnamed_ctor = self.ctx.get(cls).constructors.iter().copied().find(|&c| {
            self.ctx
                .get(c)
                .name
                .is_none_or(|n| matches!(self.ctx.name_str(n), "" | "new"))
        })?;
        let unit_ast = UnitAst {
            ast: self.ast,
            tables: self.tables,
        };
        for &p in &self.ctx.get(unnamed_ctor).formal_params {
            let pdata = self.ctx.get(p);
            if pdata.kind.is_required()
                || element_has(self.ctx, p.raw(), meta_flags::REQUIRED, Some(unit_ast))
            {
                return None;
            }
        }
        Some((cls, unnamed_ctor))
    }

    fn get_editor(&self, ty: TypeId) -> Option<protocol::FlutterWidgetPropertyEditor> {
        let TypeKind::Interface { element, .. } = *self.ctx.ty(ty) else {
            return None;
        };
        if element == self.ctx.tp.bool_element.get().upcast() {
            return Some(protocol::FlutterWidgetPropertyEditor {
                kind: protocol::FlutterWidgetPropertyEditorKind::BOOL,
                enum_items: None,
            });
        }
        if element == self.ctx.tp.double_element.get().upcast() {
            return Some(protocol::FlutterWidgetPropertyEditor {
                kind: protocol::FlutterWidgetPropertyEditorKind::DOUBLE,
                enum_items: None,
            });
        }
        if element == self.ctx.tp.int_element.get().upcast() {
            return Some(protocol::FlutterWidgetPropertyEditor {
                kind: protocol::FlutterWidgetPropertyEditorKind::INT,
                enum_items: None,
            });
        }
        if element == self.ctx.tp.string_element.get().upcast() {
            return Some(protocol::FlutterWidgetPropertyEditor {
                kind: protocol::FlutterWidgetPropertyEditorKind::STRING,
                enum_items: None,
            });
        }
        if let Some(enum_el) = element.raw().cast::<EnumElement>() {
            let items = self
                .ctx
                .get(enum_el)
                .fields
                .iter()
                .copied()
                .filter(|&f| is_field_static(self.ctx, f) && is_enum_constant(self.ctx, f.raw()))
                .filter_map(|f| to_enum_item(self.ctx, f))
                .collect();
            return Some(protocol::FlutterWidgetPropertyEditor {
                kind: protocol::FlutterWidgetPropertyEditorKind::ENUM,
                enum_items: Some(items),
            });
        }
        if is_exact_class(self.ctx, element, URI_ALIGNMENT, "AlignmentGeometry") {
            let mut items = Vec::new();
            if let Some(cls) = self.class_alignment {
                for &f in &self.ctx.get(cls).fields {
                    if is_field_static(self.ctx, f)
                        && let Some(item) = to_enum_item(self.ctx, f)
                    {
                        items.push(item);
                    }
                }
            }
            if let Some(cls) = self.class_alignment_directional {
                for &f in &self.ctx.get(cls).fields {
                    if is_field_static(self.ctx, f)
                        && let Some(item) = to_enum_item(self.ctx, f)
                    {
                        items.push(item);
                    }
                }
            }
            return Some(protocol::FlutterWidgetPropertyEditor {
                kind: protocol::FlutterWidgetPropertyEditorKind::EnumLike,
                enum_items: Some(items),
            });
        }
        None
    }

    fn to_value(
        &self,
        value_expression: Id<Expression>,
    ) -> Option<protocol::FlutterWidgetPropertyValue> {
        if let Some(b) = self.ast.cast::<BooleanLiteral>(value_expression) {
            return Some(protocol::FlutterWidgetPropertyValue {
                bool_value: Some(self.ast[b].value),
                double_value: None,
                int_value: None,
                string_value: None,
                enum_value: None,
                expression: None,
            });
        }
        if let Some(d) = self.ast.cast::<DoubleLiteral>(value_expression) {
            return Some(protocol::FlutterWidgetPropertyValue {
                bool_value: None,
                double_value: Some(self.ast[d].value),
                int_value: None,
                string_value: None,
                enum_value: None,
                expression: None,
            });
        }
        if self.ast.is::<Identifier>(value_expression) {
            let elem_ref = self
                .tables
                .element
                .get(value_expression.raw())
                .copied()
                .or_else(|| {
                    let p = self.ast.cast::<PrefixedIdentifier>(value_expression)?;
                    self.tables
                        .element
                        .get(self.ast[p].identifier.raw())
                        .copied()
                });
            if let Some(er) = elem_ref {
                let el = dartr_typesystem::member::base_element(self.ctx, er);
                let field_opt = if el.tag() == Tag::Getter {
                    if let AnyElement::Getter(g) = self.ctx.any(el) {
                        g.variable
                            .get()
                            .and_then(|v| v.raw().cast::<FieldElement>())
                    } else {
                        None
                    }
                } else {
                    el.cast::<FieldElement>()
                };
                if let Some(field) = field_opt
                    && is_field_static(self.ctx, field)
                    && let Some(enc) = self
                        .ctx
                        .get(field)
                        .enclosing
                        .and_then(|e| e.cast::<InterfaceElement>())
                {
                    let is_enum_const = is_enum_constant(self.ctx, field.raw());
                    if (is_enum_const
                        || is_exact_class(self.ctx, enc, URI_ALIGNMENT, "Alignment")
                        || is_exact_class(self.ctx, enc, URI_ALIGNMENT, "AlignmentDirectional"))
                        && let Some(item) = to_enum_item(self.ctx, field)
                    {
                        return Some(protocol::FlutterWidgetPropertyValue {
                            bool_value: None,
                            double_value: None,
                            int_value: None,
                            string_value: None,
                            enum_value: Some(item),
                            expression: None,
                        });
                    }
                }
            }
        }
        if let Some(i) = self.ast.cast::<IntegerLiteral>(value_expression) {
            return Some(protocol::FlutterWidgetPropertyValue {
                bool_value: None,
                double_value: None,
                int_value: self.ast[i].value,
                string_value: None,
                enum_value: None,
                expression: None,
            });
        }
        if let Some(s) = self.ast.cast::<SimpleStringLiteral>(value_expression) {
            return Some(protocol::FlutterWidgetPropertyValue {
                bool_value: None,
                double_value: None,
                int_value: None,
                string_value: Some(self.ast[s].value.to_string()),
                enum_value: None,
                expression: None,
            });
        }
        None
    }
}

fn find_class_in_ctx(
    ctx: &Ctx<'_>,
    library_uri: &str,
    class_name: &str,
) -> Option<EId<ClassElement>> {
    let &lib_id = ctx.world.libraries.get(library_uri)?;
    let lib = ctx.get(lib_id);
    if let Some(ns) = lib.export_namespace.try_get() {
        for (&n, &el) in &ns.defined_names {
            if ctx.name_str(n) == class_name
                && let Some(cls) = el.cast::<ClassElement>()
            {
                return Some(cls);
            }
        }
    }
    lib.classes.iter().copied().find(|&c| {
        ctx.get(c)
            .name
            .is_some_and(|n| ctx.name_str(n) == class_name)
    })
}

fn is_field_static(ctx: &Ctx<'_>, field: EId<FieldElement>) -> bool {
    let frag = ctx.get(field).first_fragment();
    ctx.fragment(frag)
        .flags
        .has(FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC)
}

fn to_enum_item(
    ctx: &Ctx<'_>,
    field: EId<FieldElement>,
) -> Option<protocol::FlutterWidgetPropertyValueEnumItem> {
    let fdata = ctx.get(field);
    let enc = fdata.enclosing?.cast::<InterfaceElement>()?;
    let library_uri = library_uri_str(ctx, enc.raw())?;
    let class_name = ctx
        .interface(enc)
        .name
        .map(|n| ctx.name_str(n).to_string())?;
    let name = fdata.name.map(|n| ctx.name_str(n).to_string())?;
    let documentation = get_field_documentation(ctx, field);
    Some(protocol::FlutterWidgetPropertyValueEnumItem {
        library_uri,
        class_name,
        name,
        documentation,
    })
}

fn get_field_documentation(ctx: &Ctx<'_>, field: EId<FieldElement>) -> Option<String> {
    let frag = ctx.get(field).first_fragment();
    let raw = ctx.fragment(frag).documentation_comment.as_deref()?;
    get_dart_doc_plain_text(raw)
}

fn get_parameter_documentation(
    ctx: &Ctx<'_>,
    param: EId<FormalParameterElement>,
) -> Option<String> {
    if param.raw().tag() == Tag::FieldFormalParameter {
        let field = ctx.get(param).field.get()?;
        return get_field_documentation(ctx, field);
    }
    None
}

fn get_dart_doc_plain_text(raw_text: &str) -> Option<String> {
    let mut text = raw_text;
    let mut is_block = false;
    if let Some(stripped) = text.strip_prefix("/**") {
        is_block = true;
        text = stripped;
        if let Some(s2) = text.strip_suffix("*/") {
            text = s2;
        }
    }
    text = text.trim();
    let mut result = String::new();
    for (idx, mut line) in text.split('\n').enumerate() {
        line = line.trim();
        if is_block && let Some(s) = line.strip_prefix('*') {
            line = s.strip_prefix(' ').unwrap_or(s);
        } else if !is_block && let Some(s) = line.strip_prefix("///") {
            line = s.strip_prefix(' ').unwrap_or(s);
        }
        if idx > 0 && !result.is_empty() {
            result.push('\n');
        }
        result.push_str(line);
    }
    Some(result)
}

fn is_exact_edge_insets_geometry_type(ctx: &Ctx<'_>, ty: TypeId) -> bool {
    let TypeKind::Interface { element, .. } = *ctx.ty(ty) else {
        return false;
    };
    is_exact_class(ctx, element, URI_EDGE_INSETS, "EdgeInsetsGeometry")
}

fn is_opted_in_class(ctx: &Ctx<'_>, element: EId<InterfaceElement>) -> bool {
    is_exact_class(ctx, element, URI_CONTAINER, "Container")
        || is_exact_class(ctx, element, URI_TEXT_STYLE, "TextStyle")
}

fn has_nested_properties(ctx: &Ctx<'_>, ty: TypeId) -> bool {
    let TypeKind::Interface { element, .. } = *ctx.ty(ty) else {
        return false;
    };
    is_opted_in_class(ctx, element)
}

fn is_exactly_container_creation(
    ctx: &Ctx<'_>,
    tables: &ResolutionTables,
    node: Id<InstanceCreationExpression>,
) -> bool {
    let Some(&ty) = tables.static_type.get(node.raw()) else {
        return false;
    };
    let TypeKind::Interface { element, .. } = *ctx.ty(ty) else {
        return false;
    };
    is_exact_class(ctx, element, URI_CONTAINER, "Container")
}

fn is_exactly_align_creation(
    ctx: &Ctx<'_>,
    tables: &ResolutionTables,
    node: Id<InstanceCreationExpression>,
) -> bool {
    let Some(&ty) = tables.static_type.get(node.raw()) else {
        return false;
    };
    let TypeKind::Interface { element, .. } = *ctx.ty(ty) else {
        return false;
    };
    is_exact_class(ctx, element, URI_BASIC, "Align")
}

fn is_exactly_padding_creation(
    ctx: &Ctx<'_>,
    tables: &ResolutionTables,
    node: Id<InstanceCreationExpression>,
) -> bool {
    let Some(&ty) = tables.static_type.get(node.raw()) else {
        return false;
    };
    let TypeKind::Interface { element, .. } = *ctx.ty(ty) else {
        return false;
    };
    is_exact_class(ctx, element, URI_BASIC, "Padding")
}

fn find_named_arg(
    ast: &Ast,
    creation: Id<InstanceCreationExpression>,
    name: &str,
) -> Option<Id<NamedArgument>> {
    for &arg in ast.list(ast[ast[creation].argument_list].arguments) {
        if let Some(named) = ast.cast::<NamedArgument>(arg)
            && ast.tokens.lexeme(ast[named].name) == name
        {
            return Some(named);
        }
    }
    None
}

fn eval_double_literal(ast: &Ast, expr: Id<Expression>) -> Option<f64> {
    if let Some(d) = ast.cast::<DoubleLiteral>(expr) {
        return Some(ast[d].value);
    }
    if let Some(i) = ast.cast::<IntegerLiteral>(expr) {
        return ast[i].value.map(|v| v as f64);
    }
    None
}

fn to_double_code(value: Option<f64>) -> String {
    let Some(v) = value else {
        return "0".to_string();
    };
    let code = format!("{v:.1}");
    if let Some(stripped) = code.strip_suffix(".0") {
        stripped.to_string()
    } else {
        code
    }
}

fn to_primitive_value_code(val: &protocol::FlutterWidgetPropertyValue) -> Option<String> {
    if let Some(b) = val.bool_value {
        return Some(b.to_string());
    }
    if let Some(d) = val.double_value {
        return Some(to_double_code(Some(d)));
    }
    if let Some(i) = val.int_value {
        return Some(i.to_string());
    }
    if let Some(s) = &val.string_value {
        let escaped = s.replace('\'', "\\'");
        return Some(format!("'{escaped}'"));
    }
    None
}

fn value_to_code(val: &protocol::FlutterWidgetPropertyValue) -> Result<String, &'static str> {
    if let Some(expr) = &val.expression {
        return Ok(expr.clone());
    }
    if let Some(enum_val) = &val.enum_value {
        return Ok(format!("{}.{}", enum_val.class_name, enum_val.name));
    }
    to_primitive_value_code(val).ok_or("FLUTTER_SET_WIDGET_PROPERTY_VALUE_INVALID_EXPRESSION")
}

fn apply_property_value_change(
    prop: &CachedProperty,
    val: &protocol::FlutterWidgetPropertyValue,
) -> Result<protocol::SourceChange, &'static str> {
    if let PropertyEditAction::EdgeInsetsNested {
        side,
        mut left_value,
        mut top_value,
        mut right_value,
        mut bottom_value,
        parent_remove_range,
        parent_edit_action,
    } = prop.edit_action.clone()
    {
        let Some(double_val) = val.double_value else {
            return Ok(protocol::SourceChange {
                message: String::new(),
                edits: Vec::new(),
                linked_edit_groups: Vec::new(),
                selection: None,
                selection_length: None,
                id: None,
            });
        };
        match side {
            EdgeInsetsSide::Left => left_value = Some(double_val),
            EdgeInsetsSide::Top => top_value = Some(double_val),
            EdgeInsetsSide::Right => right_value = Some(double_val),
            EdgeInsetsSide::Bottom => bottom_value = Some(double_val),
        }
        let left_code = to_double_code(left_value);
        let top_code = to_double_code(top_value);
        let right_code = to_double_code(right_value);
        let bottom_code = to_double_code(bottom_value);

        if left_code == "0" && top_code == "0" && right_code == "0" && bottom_code == "0" {
            let mut file_edits = Vec::new();
            if let Some((begin, end)) = parent_remove_range
                && end > begin
            {
                file_edits.push(protocol::SourceFileEdit {
                    file: prop.file.clone(),
                    file_stamp: 0,
                    edits: vec![protocol::SourceEdit {
                        offset: begin as i64,
                        length: (end - begin) as i64,
                        replacement: String::new(),
                        id: None,
                        description: None,
                    }],
                });
            }
            return Ok(protocol::SourceChange {
                message: String::new(),
                edits: file_edits,
                linked_edit_groups: Vec::new(),
                selection: None,
                selection_length: None,
                id: None,
            });
        }

        let code = if left_code == right_code && top_code == bottom_code {
            if left_code == top_code {
                format!("EdgeInsets.all({left_code})")
            } else {
                let mut parts = Vec::new();
                if left_code != "0" {
                    parts.push(format!("horizontal: {left_code}"));
                }
                if top_code != "0" {
                    parts.push(format!("vertical: {top_code}"));
                }
                format!("EdgeInsets.symmetric({})", parts.join(", "))
            }
        } else {
            let mut parts = Vec::new();
            if left_code != "0" {
                parts.push(format!("left: {left_code}"));
            }
            if top_code != "0" {
                parts.push(format!("top: {top_code}"));
            }
            if right_code != "0" {
                parts.push(format!("right: {right_code}"));
            }
            if bottom_code != "0" {
                parts.push(format!("bottom: {bottom_code}"));
            }
            format!("EdgeInsets.only({})", parts.join(", "))
        };

        let raw_edits = build_edits_for_action(&parent_edit_action, &code);
        return finalize_formatted_change(prop, raw_edits);
    }

    let code = value_to_code(val)?;
    let raw_edits = build_edits_for_action(&prop.edit_action, &code);
    finalize_formatted_change(prop, raw_edits)
}

fn build_edits_for_action(
    action: &PropertyEditAction,
    value_code: &str,
) -> Vec<(u32, u32, String)> {
    match action {
        PropertyEditAction::None => Vec::new(),
        PropertyEditAction::ReplaceExpression { offset, length } => {
            vec![(*offset, *length, value_code.to_string())]
        }
        PropertyEditAction::InsertNamedArgument {
            insert_offset,
            needs_leading_comma,
            parameter_name,
        } => {
            let prefix = if *needs_leading_comma { ", " } else { "" };
            let text = format!("{prefix}{parameter_name}: {value_code}, ");
            vec![(*insert_offset, 0, text)]
        }
        PropertyEditAction::MaterializeNestedClass {
            parent_action,
            class_name,
            parameter_name,
        } => {
            let nested = format!("{class_name}({parameter_name}: {value_code}, )");
            build_edits_for_action(parent_action, &nested)
        }
        PropertyEditAction::MaterializeVirtualContainer {
            parameter_name,
            virtual_container,
        } => match virtual_container {
            VirtualContainerAction::WrapWidget {
                widget_offset,
                widget_end,
            } => vec![
                (*widget_end, 0, ",)".to_string()),
                (
                    *widget_offset,
                    0,
                    format!("Container({parameter_name}: {value_code}, child: "),
                ),
            ],
            VirtualContainerAction::ReplaceParent {
                parent_start,
                parent_ctor_end,
                existing_arg_name,
                existing_arg_offset,
                existing_arg_end,
            } => {
                let (param_offset, text) = if existing_arg_name.as_str() > parameter_name.as_str() {
                    (
                        *existing_arg_offset,
                        format!("{parameter_name}: {value_code}, "),
                    )
                } else {
                    (
                        *existing_arg_end,
                        format!(", {parameter_name}: {value_code}"),
                    )
                };
                if param_offset >= *parent_ctor_end {
                    vec![
                        (param_offset, 0, text),
                        (
                            *parent_start,
                            *parent_ctor_end - *parent_start,
                            "Container".to_string(),
                        ),
                    ]
                } else {
                    vec![
                        (
                            *parent_start,
                            *parent_ctor_end - *parent_start,
                            "Container".to_string(),
                        ),
                        (param_offset, 0, text),
                    ]
                }
            }
        },
        PropertyEditAction::EdgeInsetsNested { .. } => Vec::new(),
    }
}

fn finalize_formatted_change(
    prop: &CachedProperty,
    mut raw_edits: Vec<(u32, u32, String)>,
) -> Result<protocol::SourceChange, &'static str> {
    if raw_edits.is_empty() {
        return Ok(protocol::SourceChange {
            message: String::new(),
            edits: Vec::new(),
            linked_edit_groups: Vec::new(),
            selection: None,
            selection_length: None,
            id: None,
        });
    }

    // Sort edits in descending offset order as ChangeBuilder does
    raw_edits.sort_by_key(|b| std::cmp::Reverse(b.0));

    let mut new_content = prop.content.clone();
    let mut new_range_offset = prop
        .function_body_range
        .map(|(o, _)| o as isize)
        .unwrap_or(0);
    let mut new_range_length = prop
        .function_body_range
        .map(|(_, l)| l as isize)
        .unwrap_or(0);

    for (offset, length, replacement) in &raw_edits {
        let off = *offset as usize;
        let len = *length as usize;
        let before = substring_utf16(&new_content, 0, off);
        let after = substring_utf16(&new_content, off + len, usize::MAX);
        new_content = format!("{before}{replacement}{after}");

        let repl_len = utf16_len(replacement) as isize;
        let length_delta = repl_len - (len as isize);
        if (*offset as isize) < new_range_offset {
            new_range_offset += length_delta;
        } else if (*offset as isize) < new_range_offset + new_range_length {
            new_range_length += length_delta;
        }
    }

    let mut final_edits: Vec<protocol::SourceEdit> = raw_edits
        .iter()
        .map(|(offset, length, replacement)| protocol::SourceEdit {
            offset: *offset as i64,
            length: *length as i64,
            replacement: replacement.clone(),
            id: None,
            description: None,
        })
        .collect();

    if let Some((body_offset, body_length)) = prop.function_body_range
        && new_range_offset >= 0
        && new_range_length >= 0
        && let Ok(source) = SourceCode::new(
            new_content.clone(),
            None,
            true,
            Some(new_range_offset as usize),
            Some(new_range_length as usize),
        )
    {
        let formatter = DartFormatter::new(Version::new(prop.version.0, prop.version.1));
        if let Ok(formatted) = formatter.format_source(&source)
            && formatted.text != new_content
        {
            final_edits = vec![protocol::SourceEdit {
                offset: body_offset as i64,
                length: body_length as i64,
                replacement: formatted.selected_text().to_string(),
                id: None,
                description: None,
            }];
        }
    }

    Ok(protocol::SourceChange {
        message: String::new(),
        edits: vec![protocol::SourceFileEdit {
            file: prop.file.clone(),
            file_stamp: 0,
            edits: final_edits,
        }],
        linked_edit_groups: Vec::new(),
        selection: None,
        selection_length: None,
        id: None,
    })
}

fn compute_argument_remove_range(
    ast: &Ast,
    creation: Id<InstanceCreationExpression>,
    arg: Id<Argument>,
) -> (u32, u32) {
    let arg_list = ast[creation].argument_list;
    let args = ast.list(ast[arg_list].arguments);
    let begin_offset = ast.offset(arg);
    let idx = args.iter().position(|&a| a == arg).unwrap_or(0);
    let end_offset = if idx + 1 < args.len() {
        ast.offset(args[idx + 1])
    } else {
        ast.tokens.offset(ast[arg_list].right_parenthesis)
    };
    (begin_offset, end_offset)
}

fn compute_named_arg_insertion(
    ast: &Ast,
    creation: Id<InstanceCreationExpression>,
    parameter_name: &str,
) -> (u32, bool) {
    let arg_list = ast[creation].argument_list;
    let args = ast.list(ast[arg_list].arguments);
    for &arg in args {
        if let Some(named) = ast.cast::<NamedArgument>(arg) {
            let arg_name = ast.tokens.lexeme(ast[named].name);
            if arg_name > parameter_name || arg_name == "child" || arg_name == "children" {
                return (ast.offset(arg), false);
            }
        }
    }
    let right_paren = ast[arg_list].right_parenthesis;
    let insert_offset = ast.tokens.offset(right_paren);
    let prev = ast.tokens.get(right_paren).previous.get();
    let needs_leading_comma = prev.is_some_and(|p| {
        ast.tokens.ty(p) != TokenType::COMMA && p != ast[arg_list].left_parenthesis
    });
    (insert_offset, needs_leading_comma)
}

fn enclosing_function_body_range(ast: &Ast, mut node: NodeId) -> Option<(u32, u32)> {
    loop {
        if ast.is::<FunctionBody>(node) {
            return Some((ast.offset(node), ast.length(node)));
        }
        node = ast.parent(node)?;
    }
}

fn node_covering_offset(ast: &Ast, root: NodeId, offset: u32) -> Option<NodeId> {
    struct Finder {
        offset: u32,
        best: Option<NodeId>,
    }
    impl AstVisitor for Finder {
        fn visit_node(&mut self, ast: &Ast, node: NodeId) {
            let start = ast.offset(node);
            let end = ast.end(node);
            if start <= self.offset && self.offset <= end {
                self.best = Some(node);
                ast.visit_children(node, self);
            }
        }
    }
    let mut finder = Finder { offset, best: None };
    finder.visit_node(ast, root);
    finder.best
}
