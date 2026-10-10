// Dart source: pkg/analysis_server/lib/src/computer/computer_signature.dart
// Dart source: pkg/analysis_server/lib/src/computer/computer_type_arguments_signature.dart
// Dart source: pkg/analysis_server/lib/src/lsp/mapping.dart (toSignatureHelp)

//! The signature of the invocation around an offset
//! (`textDocument/signatureHelp`): the parameters of the invoked function,
//! or the type parameters of the type around a type argument list.

use std::collections::HashMap;

use dartr_ast::*;
use dartr_element::display_string::{
    DisplayOptions, default_value_code, element_display_string_with,
    type_display_string_with, type_parameter_display_string,
};
use dartr_element::{Ctx, ElemRef, ElementId, EId, FormalParameterElement, Tag, TypeId, TypeKind};
use dartr_typesystem::member;

use crate::element_locator::Unit;

/// A parameter of a signature (Dart `FormalParameterElement`).
#[derive(Clone, Debug)]
pub struct SignatureParameter {
    pub name: String,
    pub kind: ParameterKind,
    pub ty: TypeId,
    /// Dart `defaultValueCode`.
    pub default_code: Option<String>,
    /// The base element, when the parameter has one.
    pub element: Option<ElementId>,
}

/// Dart `SignatureInformation`.
#[derive(Debug)]
pub struct SignatureInformation {
    pub name: String,
    pub parameters: Vec<SignatureParameter>,
    /// The offset of the argument list (its `(`).
    pub argument_list_offset: u32,
    pub dartdoc: Option<String>,
    pub active_parameter_index: Option<usize>,
}

/// The parameters of [element] (a member keeps its substituted types).
fn element_parameters(ctx: &Ctx<'_>, element: ElemRef) -> Vec<SignatureParameter> {
    member::formal_parameters(ctx, element)
        .into_iter()
        .map(|p| {
            let base = member::base_element(ctx, p);
            let parameter = EId::<FormalParameterElement>::from_raw(base);
            SignatureParameter {
                name: ctx
                    .element_data(base)
                    .and_then(|d| d.name)
                    .map(|n| ctx.name_str(n).to_string())
                    .unwrap_or_default(),
                kind: ctx.get(parameter).kind,
                ty: member::type_(ctx, p),
                default_code: default_value_code(ctx, parameter),
                element: Some(base),
            }
        })
        .collect()
}

/// The parameters of a function type (Dart `FunctionType.formalParameters`).
fn function_type_parameters(ctx: &Ctx<'_>, ty: TypeId) -> Option<Vec<SignatureParameter>> {
    let TypeKind::Function(f) = ctx.ty(ty) else {
        return None;
    };
    Some(
        ctx.list(f.params)
            .iter()
            .map(|p| {
                let base = p.element.map(|e| member::base_element(ctx, e));
                SignatureParameter {
                    // Dart `displayName` of a parameter without a name.
                    name: p
                        .name
                        .map(|n| ctx.name_str(n).to_string())
                        .filter(|n| !n.is_empty())
                        .unwrap_or_else(|| "<unnamed>".to_string()),
                    kind: p.kind,
                    ty: p.ty,
                    default_code: base
                        .filter(|e| e.cast::<FormalParameterElement>().is_some())
                        .and_then(|e| default_value_code(ctx, EId::from_raw(e))),
                    element: base,
                }
            })
            .collect(),
    )
}

/// Dart `DartUnitSignatureComputer.compute`.
pub fn compute_signature(
    unit: &Unit<'_, '_>,
    root: Id<CompilationUnit>,
    offset: u32,
    templates: &HashMap<String, String>,
) -> Option<SignatureInformation> {
    let ast = unit.ast;
    let ctx = unit.ctx;
    let node = ast.node_covering(root.raw(), offset, 0)?;
    let (argument_list, argument) = find_argument_and_list(ast, node)?;
    let parent = ast.parent(argument_list.raw())?;
    let element_ref = |n: NodeId| unit.tables.element.get(n).copied();
    let mut name = None;
    let mut element: Option<ElemRef> = None;
    let mut parameters = None;
    if let Some(m) = ast.cast::<MethodInvocation>(parent) {
        let method_name = ast[m].method_name;
        name = Some(ast.tokens.lexeme(ast[method_name].token).to_string());
        element = element_ref(method_name.raw())
            .or_else(|| unit.locate(parent).map(ElemRef::Base));
        parameters = element
            .filter(|e| is_function_typed(member::base_element(ctx, *e)))
            .map(|e| element_parameters(ctx, e));
    } else if let Some(i) = ast.cast::<InstanceCreationExpression>(parent) {
        let constructor_name = ast[i].constructor_name;
        let named_type = ast[constructor_name].type_;
        let mut n = qualified_name(ast, named_type);
        if let Some(c) = ast[constructor_name].name {
            n = format!("{n}.{}", ast.tokens.lexeme(ast[c].token));
        }
        name = Some(n);
        element = element_ref(constructor_name.raw())
            .or_else(|| unit.locate(parent).map(ElemRef::Base));
        parameters = element
            .filter(|e| is_function_typed(member::base_element(ctx, *e)))
            .map(|e| element_parameters(ctx, e));
    } else if let Some(f) = ast.cast::<FunctionExpressionInvocation>(parent) {
        let function = ast[f].function.raw();
        if ast.is::<Identifier>(function) {
            name = Some(identifier_name(ast, function));
            let static_type = unit.tables.static_type.get(function).copied();
            if let Some(t) = static_type.filter(|t| matches!(ctx.ty(*t), TypeKind::Function(_))) {
                element = identifier_element(unit, function);
                parameters = function_type_parameters(ctx, t);
            } else if let Some(e) = element_ref(parent)
                .filter(|e| is_executable(member::base_element(ctx, *e)))
            {
                element = Some(e);
                parameters = Some(element_parameters(ctx, e));
            }
        }
    }
    let (name, element, parameters) = (name?, element?, parameters?);

    // The active parameter.
    let mut active = None;
    let corresponding = argument.and_then(|a| corresponding_parameter(unit, a));
    if let Some(p) = corresponding {
        // Dart `parameters.indexOf(correspondingParameter)`: a substituted
        // parameter is a new element in Dart, never in the list (-1).
        active = match p {
            ElemRef::Base(base) if !invocation_parameters_substituted(unit, argument_list.raw()) => {
                parameters.iter().position(|q| q.element == Some(base))
            }
            _ => None,
        };
        if active.is_none() {
            active = Some(usize::MAX);
        }
    } else if !argument.is_some_and(|a| ast.is::<NamedArgument>(a)) {
        let arguments = ast.list_raw(ast[argument_list].arguments);
        let mut skip = arguments
            .iter()
            .filter(|a| !ast.is::<NamedArgument>(**a))
            .take_while(|a| ast.end(**a) < offset)
            .count();
        for (i, p) in parameters.iter().enumerate() {
            if p.kind.is_positional() {
                if skip == 0 {
                    active = Some(i);
                    break;
                }
                skip -= 1;
            }
        }
    }
    let base = member::base_element(ctx, element);
    let dartdoc = crate::hover::documentation(ctx, base, templates);
    Some(SignatureInformation {
        name,
        parameters,
        argument_list_offset: ast.offset(argument_list.raw()),
        dartdoc,
        active_parameter_index: active,
    })
}

/// Whether the parameters of the invocation of [argument_list] are
/// substituted in Dart (new elements, never equal to the parameters of the
/// declaration): the invoked element is a member (a generic receiver or
/// class), or the invocation has inferred type arguments.
pub fn invocation_parameters_substituted(unit: &Unit<'_, '_>, argument_list: NodeId) -> bool {
    let ast = unit.ast;
    let Some(invocation) = ast.parent(argument_list) else {
        return false;
    };
    let ctx = unit.ctx;
    let has_type_arguments = unit
        .tables
        .type_arg_types
        .get(invocation)
        .is_some_and(|l| !ctx.list(*l).is_empty());
    let element = if let Some(m) = ast.cast::<MethodInvocation>(invocation) {
        unit.tables.element.get(ast[m].method_name.raw()).copied()
    } else if let Some(i) = ast.cast::<InstanceCreationExpression>(invocation) {
        unit.tables.element.get(ast[i].constructor_name.raw()).copied()
    } else {
        unit.tables.element.get(invocation).copied()
    };
    has_type_arguments || matches!(element, Some(ElemRef::Member(_)))
}

/// Dart `Argument.correspondingParameter`.
fn corresponding_parameter(unit: &Unit<'_, '_>, argument: NodeId) -> Option<ElemRef> {
    let ast = unit.ast;
    if let Some(n) = ast.cast::<NamedArgument>(argument) {
        return unit
            .tables
            .param_element
            .get(n.raw())
            .or_else(|| unit.tables.param_element.get(ast[n].argument_expression.raw()))
            .copied();
    }
    unit.tables.param_element.get(argument).copied()
}

fn identifier_name(ast: &Ast, identifier: NodeId) -> String {
    if let Some(s) = ast.cast::<SimpleIdentifier>(identifier) {
        return ast.tokens.lexeme(ast[s].token).to_string();
    }
    if let Some(p) = ast.cast::<PrefixedIdentifier>(identifier) {
        // Dart `PrefixedIdentifier.name`: `prefix.identifier`.
        let prefix = ast.tokens.lexeme(ast[ast[p].prefix].token);
        let id = ast.tokens.lexeme(ast[ast[p].identifier].token);
        return format!("{prefix}.{id}");
    }
    String::new()
}

/// Dart `Identifier.element` (of a prefixed identifier: its identifier).
fn identifier_element(unit: &Unit<'_, '_>, identifier: NodeId) -> Option<ElemRef> {
    let ast = unit.ast;
    unit.tables.element.get(identifier).copied().or_else(|| {
        let p = ast.cast::<PrefixedIdentifier>(identifier)?;
        unit.tables.element.get(ast[p].identifier.raw()).copied()
    })
}

/// Dart `NamedType.qualifiedName`.
fn qualified_name(ast: &Ast, named_type: Id<NamedType>) -> String {
    let name = ast.tokens.lexeme(ast[named_type].name);
    match ast[named_type].import_prefix {
        Some(p) => format!("{}.{name}", ast.tokens.lexeme(ast[p].name)),
        None => name.to_string(),
    }
}

fn is_executable(e: ElementId) -> bool {
    crate::element_locator::is_executable(e)
}

/// Dart `element is FunctionTypedElement`.
fn is_function_typed(e: ElementId) -> bool {
    is_executable(e)
}

/// Dart `_findArgumentAndList`.
fn find_argument_and_list(ast: &Ast, node: NodeId) -> Option<(Id<ArgumentList>, Option<NodeId>)> {
    let mut node = Some(node);
    while let Some(n) = node {
        if ast.is::<FunctionExpression>(n) {
            return None;
        }
        if let Some(list) = ast.cast::<ArgumentList>(n) {
            return Some((list, None));
        }
        if let Some(list) = ast.parent(n).and_then(|p| ast.cast::<ArgumentList>(p)) {
            return Some((list, Some(n)));
        }
        node = ast.parent(n);
    }
    None
}

/// Dart `getParamLabel`: `required int a = 1`.
fn parameter_label(ctx: &Ctx<'_>, p: &SignatureParameter) -> String {
    let default = p.default_code.as_ref().map(|c| format!(" = {c}")).unwrap_or_default();
    let prefix = if p.kind.is_required_named() { "required " } else { "" };
    let ty = type_display_string_with(ctx, p.ty, DisplayOptions::default());
    format!("{prefix}{ty} {}{default}", p.name)
}

/// Dart `toSignatureHelp`.
pub fn to_signature_help(
    ctx: &Ctx<'_>,
    signature: &SignatureInformation,
    documentation: impl Fn(String) -> serde_json::Value,
    null_active_parameter: bool,
) -> serde_json::Value {
    let ps = &signature.parameters;
    let labels = |filter: &dyn Fn(&SignatureParameter) -> bool| -> Vec<String> {
        ps.iter().filter(|p| filter(p)).map(|p| parameter_label(ctx, p)).collect()
    };
    let required = labels(&|p| p.kind.is_required_positional());
    let optional = labels(&|p| p.kind.is_optional_positional());
    let named = labels(&|p| p.kind.is_named());
    let mut groups = Vec::new();
    if !required.is_empty() {
        groups.push(required.join(", "));
    }
    if !optional.is_empty() {
        groups.push(format!("[{}]", optional.join(", ")));
    }
    if !named.is_empty() {
        groups.push(format!("{{{}}}", named.join(", ")));
    }
    let label = format!("{}({})", signature.name, groups.join(", "));
    let mut info = serde_json::Map::new();
    info.insert("label".into(), label.into());
    if let Some(doc) = signature.dartdoc.as_deref().map(crate::hover::clean_dartdoc) {
        info.insert("documentation".into(), documentation(doc));
    }
    info.insert(
        "parameters".into(),
        ps.iter()
            .map(|p| serde_json::json!({"label": parameter_label(ctx, p)}))
            .collect::<Vec<_>>()
            .into(),
    );
    let active = match signature.active_parameter_index {
        // Dart sends `indexOf` = -1 as is.
        Some(usize::MAX) => serde_json::json!(-1),
        Some(i) => serde_json::json!(i),
        None if null_active_parameter => serde_json::Value::Null,
        None => serde_json::json!(ps.len()),
    };
    let mut help = serde_json::Map::new();
    help.insert("signatures".into(), serde_json::Value::Array(vec![info.into()]));
    help.insert("activeSignature".into(), serde_json::json!(0));
    if !active.is_null() {
        help.insert("activeParameter".into(), active);
    }
    help.into()
}

/// Dart `DartTypeArgumentsSignatureComputer.compute`: the signature help and
/// the offset of the type argument list.
pub fn compute_type_arguments_signature(
    unit: &Unit<'_, '_>,
    root: Id<CompilationUnit>,
    offset: u32,
    templates: &HashMap<String, String>,
    documentation: impl Fn(String) -> serde_json::Value,
    null_active_parameter: bool,
) -> Option<(serde_json::Value, u32)> {
    let ast = unit.ast;
    let ctx = unit.ctx;
    let node = ast.node_covering(root.raw(), offset, 0)?;
    let mut current = Some(node);
    let list = loop {
        let n = current?;
        if let Some(l) = ast.cast::<TypeArgumentList>(n) {
            break l;
        }
        if ast.is::<FunctionExpression>(n) {
            return None;
        }
        current = ast.parent(n);
    };
    let parent = ast.parent(list.raw())?;
    let element = if ast.is::<NamedType>(parent) {
        unit.tables.element.get(parent).map(|&e| member::base_element(ctx, e))
    } else if let Some(m) = ast.cast::<MethodInvocation>(parent) {
        unit.locate(ast[m].method_name.raw())
    } else {
        None
    }?;
    let type_parameters = type_parameters_of(ctx, element)?;
    if type_parameters.is_empty() {
        return None;
    }
    let label = element_display_string_with(ctx, element, DisplayOptions::default());
    let doc = crate::hover::documentation(ctx, element, templates);
    let parameters: Vec<serde_json::Value> = type_parameters
        .iter()
        .map(|&t| serde_json::json!({"label": type_parameter_display_string(ctx, t)}))
        .collect();
    let mut info = serde_json::Map::new();
    info.insert("label".into(), label.into());
    if let Some(doc) = doc.as_deref().map(crate::hover::clean_dartdoc) {
        info.insert("documentation".into(), documentation(doc));
    }
    let count = parameters.len();
    info.insert("parameters".into(), parameters.into());
    let mut help = serde_json::Map::new();
    help.insert("signatures".into(), serde_json::Value::Array(vec![info.into()]));
    help.insert("activeSignature".into(), serde_json::json!(0));
    if !null_active_parameter {
        help.insert("activeParameter".into(), serde_json::json!(count));
    }
    Some((help.into(), ast.offset(list.raw())))
}

/// Dart `TypeParameterizedElement.typeParameters`, `None` when the element
/// is not type parameterized.
fn type_parameters_of(
    ctx: &Ctx<'_>,
    element: ElementId,
) -> Option<Vec<EId<dartr_element::TypeParameterElement>>> {
    match element.tag() {
        Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType | Tag::Extension => {
            let instance = element.cast::<dartr_element::InstanceElement>()?;
            Some(ctx.instance(instance).type_params.to_vec())
        }
        Tag::TypeAlias => {
            let alias = element.cast::<dartr_element::TypeAliasElement>()?;
            Some(ctx.get(alias).type_params.to_vec())
        }
        Tag::Method | Tag::TopLevelFunction | Tag::LocalFunction => {
            let executable = element.cast::<dartr_element::ExecutableElement>()?;
            Some(ctx.executable(executable).type_params.to_vec())
        }
        _ => None,
    }
}
