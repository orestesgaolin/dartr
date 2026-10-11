// Dart source: pkg/analysis_server/lib/src/services/refactoring/framework/refactoring_processor.dart
// Dart source: pkg/analysis_server/lib/src/services/refactoring/add_constructor_name.dart
// Dart source: pkg/analysis_server/lib/src/services/refactoring/add_import_prefix.dart
// Dart source: pkg/analysis_server/lib/src/services/refactoring/convert_all_formal_parameters_to_named.dart
// Dart source: pkg/analysis_server/lib/src/services/refactoring/convert_selected_formal_parameters_to_named.dart
// Dart source: pkg/analysis_server/lib/src/services/refactoring/move_selected_formal_parameters_left.dart
// Dart source: pkg/analysis_server/lib/src/services/refactoring/move_top_level_to_file.dart
// Dart source: pkg/analysis_server/lib/src/services/refactoring/remove_constructor_name.dart
// Dart source: pkg/analysis_server/lib/src/services/refactoring/remove_import_prefix.dart

//! Framework refactoring producers (`RefactoringProcessor.generators`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

use dartr_ast::*;
use dartr_element::{Ctx, ElementId, NoopSink, Tag};
use dartr_resolver::error::support;
use dartr_syntax::TokenId;
use dartr_typesystem::TypeExt;
use serde_json::{Value, json};

use super::naming::to_file_name;
use super::rename::{
    ChangeAcc, Engine, ResolvedUnitRefClone, Target, create_rename_for_target, source_references,
};
use crate::correction::change::SourceChange;
use crate::server::code_actions::commands;
use crate::server::{ResolvedUnitRef, SElem};
use crate::uri::{path_to_uri, uri_to_path};

#[derive(Debug)]
pub enum ComputeStatus {
    Success(SourceChange),
    Failure(Option<String>),
}

pub fn is_framework_refactor_command(command: &str) -> bool {
    matches!(
        command,
        commands::ADD_CONSTRUCTOR_NAME
            | commands::ADD_IMPORT_PREFIX
            | commands::CONVERT_ALL_FORMAL_PARAMETERS_TO_NAMED
            | commands::CONVERT_SELECTED_FORMAL_PARAMETERS_TO_NAMED
            | commands::MOVE_SELECTED_FORMAL_PARAMETERS_LEFT
            | commands::MOVE_TOP_LEVEL_TO_FILE
            | commands::REMOVE_CONSTRUCTOR_NAME
            | commands::REMOVE_IMPORT_PREFIX
    )
}

/// Computes available framework refactoring `CodeAction` JSON values in the exact order
/// of `RefactoringProcessor.generators`.
pub fn framework_refactor_actions(
    unit_ref: &ResolvedUnitRef,
    selection_offset: u32,
    selection_length: u32,
    include_experimental: bool,
    supports_file_creation: bool,
    interactive_forms_enabled: bool,
    should_include_kind: impl Fn(&str) -> bool,
) -> Vec<Value> {
    let mut actions = Vec::new();
    let generator_commands = [
        commands::ADD_CONSTRUCTOR_NAME,
        commands::ADD_IMPORT_PREFIX,
        commands::CONVERT_ALL_FORMAL_PARAMETERS_TO_NAMED,
        commands::CONVERT_SELECTED_FORMAL_PARAMETERS_TO_NAMED,
        commands::MOVE_SELECTED_FORMAL_PARAMETERS_LEFT,
        commands::MOVE_TOP_LEVEL_TO_FILE,
        commands::REMOVE_CONSTRUCTOR_NAME,
        commands::REMOVE_IMPORT_PREFIX,
    ];

    for &command in &generator_commands {
        let is_experimental = matches!(
            command,
            commands::CONVERT_ALL_FORMAL_PARAMETERS_TO_NAMED
                | commands::CONVERT_SELECTED_FORMAL_PARAMETERS_TO_NAMED
                | commands::MOVE_SELECTED_FORMAL_PARAMETERS_LEFT
        );
        if is_experimental && !include_experimental {
            continue;
        }
        let kind = producer_kind(command);
        if !should_include_kind(kind) {
            continue;
        }
        let Some((title, default_args, parameters)) = producer_availability(
            command,
            unit_ref,
            selection_offset,
            selection_length,
            supports_file_creation,
        ) else {
            continue;
        };

        let mut action = json!({
            "title": title,
            "kind": kind,
            "command": {
                "command": command,
                "title": title,
                "arguments": [{
                    "filePath": unit_ref.path(),
                    "selectionOffset": selection_offset,
                    "selectionLength": selection_length,
                    "arguments": default_args,
                }],
            },
        });
        if !parameters.is_empty() && !interactive_forms_enabled {
            action["data"] = json!({ "parameters": parameters });
        }
        actions.push(action);
    }

    actions
}

fn producer_kind(command: &str) -> &'static str {
    match command {
        commands::ADD_CONSTRUCTOR_NAME | commands::ADD_IMPORT_PREFIX => "refactor.add",
        commands::MOVE_TOP_LEVEL_TO_FILE => "refactor.move",
        commands::REMOVE_CONSTRUCTOR_NAME | commands::REMOVE_IMPORT_PREFIX => "refactor.remove",
        _ => "refactor",
    }
}

fn producer_availability(
    command: &str,
    unit_ref: &ResolvedUnitRef,
    selection_offset: u32,
    selection_length: u32,
    supports_file_creation: bool,
) -> Option<(String, Vec<Value>, Vec<Value>)> {
    match command {
        commands::ADD_CONSTRUCTOR_NAME => {
            let _ = find_constructor_at_selection(
                unit_ref,
                selection_offset,
                selection_length,
                false,
                true,
            )?;
            Some((
                "Add a name to the constructor".to_string(),
                Vec::new(),
                Vec::new(),
            ))
        }
        commands::ADD_IMPORT_PREFIX => {
            let _ = find_import_at_selection(
                unit_ref,
                selection_offset,
                selection_length,
                false,
                true,
                false,
            )?;
            Some((
                "Add a prefix to the import".to_string(),
                Vec::new(),
                Vec::new(),
            ))
        }
        commands::CONVERT_ALL_FORMAL_PARAMETERS_TO_NAMED => {
            let avail =
                analyze_signature_availability(unit_ref, selection_offset, selection_length)?;
            if !avail.has_positional_parameters_to_convert_to_named(unit_ref) {
                return None;
            }
            Some((
                "Convert all formal parameters to named".to_string(),
                Vec::new(),
                Vec::new(),
            ))
        }
        commands::CONVERT_SELECTED_FORMAL_PARAMETERS_TO_NAMED => {
            let avail =
                analyze_signature_availability(unit_ref, selection_offset, selection_length)?;
            if !avail.has_selected_formal_parameters_to_convert_to_named(unit_ref) {
                return None;
            }
            Some((
                "Convert selected formal parameter(s) to named".to_string(),
                Vec::new(),
                Vec::new(),
            ))
        }
        commands::MOVE_SELECTED_FORMAL_PARAMETERS_LEFT => {
            let avail =
                analyze_signature_availability(unit_ref, selection_offset, selection_length)?;
            if !avail.has_selected_formal_parameters_to_move_left() {
                return None;
            }
            Some((
                "Move selected formal parameter(s) left".to_string(),
                Vec::new(),
                Vec::new(),
            ))
        }
        commands::MOVE_TOP_LEVEL_TO_FILE => {
            if !supports_file_creation {
                return None;
            }
            let members = members_to_move(unit_ref, selection_offset, selection_length)?;
            let title = members.title();
            let default_uri = path_to_uri(&members.default_file_path());
            let param = json!({
                "parameterLabel": "Move to:",
                "parameterTitle": "Select a file to move to",
                "actionLabel": "Move",
                "defaultValue": default_uri,
                "kind": "saveUri",
                "filters": {
                    "Dart": ["dart"]
                }
            });
            Some((title, vec![json!(default_uri)], vec![param]))
        }
        commands::REMOVE_CONSTRUCTOR_NAME => {
            let _ = find_constructor_at_selection(
                unit_ref,
                selection_offset,
                selection_length,
                true,
                false,
            )?;
            Some((
                "Remove the name from the constructor".to_string(),
                Vec::new(),
                Vec::new(),
            ))
        }
        commands::REMOVE_IMPORT_PREFIX => {
            let _ = find_import_at_selection(
                unit_ref,
                selection_offset,
                selection_length,
                true,
                false,
                true,
            )?;
            Some((
                "Remove the prefix from the import".to_string(),
                Vec::new(),
                Vec::new(),
            ))
        }
        _ => None,
    }
}

pub fn producer_is_available(
    command: &str,
    unit_ref: &ResolvedUnitRef,
    selection_offset: u32,
    selection_length: u32,
    supports_file_creation: bool,
) -> bool {
    producer_availability(
        command,
        unit_ref,
        selection_offset,
        selection_length,
        supports_file_creation,
    )
    .is_some()
}

struct FormFieldDef {
    id: &'static str,
    description: &'static str,
    required: bool,
    default_value: Option<Value>,
    type_kind: &'static str,
    type_json: Value,
    validator: Option<fn(&Value) -> Option<String>>,
}

fn validate_ctor_name_value(value: &Value) -> Option<String> {
    let s = value.as_str()?;
    let status = super::naming::validate_constructor_name(s);
    if status.has_fatal_error() {
        Some(status.message().unwrap_or("invalid value").to_string())
    } else {
        None
    }
}

fn validate_import_prefix_value(value: &Value) -> Option<String> {
    let s = value.as_str()?;
    let status = super::naming::validate_import_prefix_name(s);
    if status.has_fatal_error() {
        Some(status.message().unwrap_or("invalid value").to_string())
    } else {
        None
    }
}

fn is_valid_file_uri(s: &str) -> bool {
    if s.len() < 5 || !s[..5].eq_ignore_ascii_case("file:") {
        return false;
    }
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b <= b' '
            || matches!(
                b,
                b'<' | b'>' | b'"' | b'`' | b'{' | b'}' | b'|' | b'\\' | b'^' | 0x7f
            )
        {
            return false;
        }
        if b == b'%' {
            if i + 2 >= bytes.len()
                || !bytes[i + 1].is_ascii_hexdigit()
                || !bytes[i + 2].is_ascii_hexdigit()
            {
                return false;
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    true
}

/// Resolves a `ParameterizedRefactoringProducer` command via `InteractiveForm`
/// (Dart `RefactorCommandResolver.handleRefactor`). Returns `None` if the
/// command is not a `ParameterizedRefactoringProducer`.
pub fn resolve_framework_refactor_command(
    command: &str,
    unit_ref: &ResolvedUnitRef,
    selection_offset: u32,
    selection_length: u32,
    supports_file_creation: bool,
    supported_input_types: &[String],
    params: &Value,
) -> Option<Value> {
    let fields: Vec<FormFieldDef> = match command {
        commands::ADD_CONSTRUCTOR_NAME => {
            let ctor_el = find_constructor_at_selection(
                unit_ref,
                selection_offset,
                selection_length,
                false,
                true,
            )?;
            let default_name = compute_default_constructor_name(&ctor_el);
            vec![FormFieldDef {
                id: "name",
                description: "Constructor Name",
                required: true,
                default_value: Some(Value::String(default_name)),
                type_kind: "string",
                type_json: json!({"kind": "string"}),
                validator: Some(validate_ctor_name_value),
            }]
        }
        commands::ADD_IMPORT_PREFIX => {
            let (_, import_id) = find_import_at_selection(
                unit_ref,
                selection_offset,
                selection_length,
                false,
                true,
                false,
            )?;
            let default_prefix = compute_default_import_prefix(unit_ref, import_id);
            vec![FormFieldDef {
                id: "name",
                description: "Import Prefix",
                required: true,
                default_value: Some(Value::String(default_prefix)),
                type_kind: "string",
                type_json: json!({"kind": "string"}),
                validator: Some(validate_import_prefix_value),
            }]
        }
        commands::MOVE_TOP_LEVEL_TO_FILE => {
            if !supports_file_creation {
                return None;
            }
            let members = members_to_move(unit_ref, selection_offset, selection_length)?;
            let default_uri = path_to_uri(&members.default_file_path());
            vec![FormFieldDef {
                id: "destinationUri",
                description: "Move to file",
                required: true,
                default_value: Some(Value::String(default_uri)),
                type_kind: "file",
                type_json: json!({"filters": ["dart"], "kind": "file", "type": 1}),
                validator: None,
            }]
        }
        _ => return None,
    };

    let is_supported = |field: &FormFieldDef| -> bool {
        supported_input_types.iter().any(|k| k == field.type_kind)
    };

    let mut answers_map: BTreeMap<&'static str, Value> = BTreeMap::new();
    let mut complete_ids: BTreeSet<&'static str> = BTreeSet::new();
    for field in &fields {
        if let Some(def) = &field.default_value {
            answers_map.insert(field.id, def.clone());
        }
        if !is_supported(field) {
            if field.default_value.is_none() {
                panic!(
                    "Cannot handle fields of type '{}' which are not supported by the client and do not have default values",
                    field.type_kind
                );
            }
            complete_ids.insert(field.id);
        }
    }

    let mut client_answers: Vec<Value> = Vec::new();
    let mut validation_errors: BTreeMap<&'static str, String> = BTreeMap::new();

    if let Some(form_answers) = params.get("formAnswers").and_then(Value::as_array) {
        let mut seen_ids = BTreeSet::new();
        for ans in form_answers {
            let id = ans.get("id").and_then(Value::as_str).unwrap_or("");
            if !seen_ids.insert(id) {
                panic!("Cannot process responses with duplicate IDs");
            }
            if !fields.iter().any(|f| f.id == id) {
                panic!("Cannot process response with unknown ID");
            }
            let val = ans.get("value").cloned().unwrap_or(Value::Null);
            client_answers.push(json!({"id": id, "value": val}));
        }

        validation_errors.clear();
        complete_ids.clear();

        for field in &fields {
            if !is_supported(field) {
                complete_ids.insert(field.id);
                continue;
            }
            let answer_val = form_answers
                .iter()
                .find(|a| a.get("id").and_then(Value::as_str) == Some(field.id))
                .and_then(|a| a.get("value"));
            let is_null = answer_val.is_none_or(Value::is_null);
            let err = if is_null && !field.required {
                None
            } else {
                let type_err = match field.type_kind {
                    "bool" => {
                        if answer_val.is_some_and(Value::is_boolean) {
                            None
                        } else {
                            Some("Must be a valid boolean".to_string())
                        }
                    }
                    "file" => {
                        if answer_val
                            .and_then(Value::as_str)
                            .is_some_and(is_valid_file_uri)
                        {
                            None
                        } else {
                            Some("Must be a valid file:// URI".to_string())
                        }
                    }
                    "number" => {
                        if answer_val.is_some_and(Value::is_number) {
                            None
                        } else {
                            Some("Must be a valid number".to_string())
                        }
                    }
                    "string" => {
                        if answer_val.is_some_and(Value::is_string) {
                            None
                        } else {
                            Some("Must be a valid string".to_string())
                        }
                    }
                    _ => None,
                };
                match type_err {
                    Some(e) => Some(e),
                    None => match (field.validator, answer_val) {
                        (Some(v), Some(val)) => v(val),
                        _ => None,
                    },
                }
            };
            if let Some(err_msg) = err {
                validation_errors.insert(field.id, err_msg);
            } else {
                answers_map.insert(field.id, answer_val.cloned().unwrap_or(Value::Null));
                complete_ids.insert(field.id);
            }
        }
    }

    let is_complete = fields.iter().all(|f| complete_ids.contains(f.id));
    let client_fields: Vec<Value> = if is_complete {
        Vec::new()
    } else {
        fields
            .iter()
            .filter(|f| is_supported(f))
            .map(|f| {
                let mut m = serde_json::Map::new();
                if let Some(def) = &f.default_value
                    && !def.is_null()
                {
                    m.insert("default".to_string(), def.clone());
                }
                m.insert("description".to_string(), json!(f.description));
                if let Some(err) = validation_errors.get(f.id) {
                    m.insert("error".to_string(), json!(err));
                }
                m.insert("id".to_string(), json!(f.id));
                m.insert("required".to_string(), json!(f.required));
                m.insert("type".to_string(), f.type_json.clone());
                Value::Object(m)
            })
            .collect()
    };

    let answers: Vec<Value> = fields
        .iter()
        .map(|f| answers_map.get(f.id).cloned().unwrap_or(Value::Null))
        .collect();

    let mut out = serde_json::Map::new();
    out.insert(
        "arguments".to_string(),
        json!([{
            "filePath": unit_ref.path(),
            "selectionOffset": selection_offset,
            "selectionLength": selection_length,
            "arguments": answers,
        }]),
    );
    out.insert("command".to_string(), json!(command));
    if let Some(data) = params.get("data").filter(|v| !v.is_null()) {
        out.insert("data".to_string(), data.clone());
    }
    if !client_answers.is_empty() {
        out.insert("formAnswers".to_string(), Value::Array(client_answers));
    }
    if !client_fields.is_empty() {
        out.insert("formFields".to_string(), Value::Array(client_fields));
    }
    if let Some(token) = params.get("workDoneToken").filter(|v| !v.is_null()) {
        out.insert("workDoneToken".to_string(), token.clone());
    }
    Some(Value::Object(out))
}

pub fn execute_framework_refactor(
    command: &str,
    unit_ref: &ResolvedUnitRef,
    selection_offset: u32,
    selection_length: u32,
    command_arguments: &[Value],
    engine: &mut Engine<'_>,
) -> ComputeStatus {
    match command {
        commands::ADD_CONSTRUCTOR_NAME => compute_add_constructor_name(
            unit_ref,
            selection_offset,
            selection_length,
            command_arguments,
            engine,
        ),
        commands::ADD_IMPORT_PREFIX => compute_add_import_prefix(
            unit_ref,
            selection_offset,
            selection_length,
            command_arguments,
            engine,
        ),
        commands::REMOVE_CONSTRUCTOR_NAME => {
            compute_remove_constructor_name(unit_ref, selection_offset, selection_length, engine)
        }
        commands::REMOVE_IMPORT_PREFIX => {
            compute_remove_import_prefix(unit_ref, selection_offset, selection_length, engine)
        }
        commands::CONVERT_ALL_FORMAL_PARAMETERS_TO_NAMED => {
            compute_convert_all_formal_parameters_to_named(
                unit_ref,
                selection_offset,
                selection_length,
                engine,
            )
        }
        commands::CONVERT_SELECTED_FORMAL_PARAMETERS_TO_NAMED => {
            compute_convert_selected_formal_parameters_to_named(
                unit_ref,
                selection_offset,
                selection_length,
                engine,
            )
        }
        commands::MOVE_SELECTED_FORMAL_PARAMETERS_LEFT => {
            compute_move_selected_formal_parameters_left(
                unit_ref,
                selection_offset,
                selection_length,
                engine,
            )
        }
        commands::MOVE_TOP_LEVEL_TO_FILE => compute_move_top_level_to_file(
            unit_ref,
            selection_offset,
            selection_length,
            command_arguments,
            engine,
        ),
        _ => ComputeStatus::Failure(None),
    }
}

// ============================================================================
// Selection helpers (`SelectionExtension` in `selection.dart`)
// ============================================================================

fn find_constructor_at_selection(
    unit_ref: &ResolvedUnitRef,
    offset: u32,
    length: u32,
    must_have_name: bool,
    must_not_have_name: bool,
) -> Option<SElem> {
    let sink = NoopSink;
    let ctx = unit_ref.ctx(&sink);
    let unit = unit_ref.unit();
    let ast = &unit.ast;
    let end = offset + length;
    let covering = ast.node_covering(unit.unit, offset, length)?;

    let meets_requirements = |has_name: bool| -> bool {
        if !has_name && must_have_name {
            return false;
        }
        if has_name && must_not_have_name {
            return false;
        }
        true
    };

    let make_selem = |id: ElementId| -> Option<SElem> {
        if id.tag() == Tag::Constructor {
            Some(SElem {
                lib: unit_ref.library.clone(),
                unit: unit_ref.index,
                id,
            })
        } else {
            None
        }
    };

    if let Some(cd) = ast.cast::<ConstructorDeclaration>(covering) {
        if !meets_requirements(ast[cd].name.is_some()) {
            return None;
        }
        let left = ast[cd]
            .type_name
            .map(|t| ast.offset(t))
            .or_else(|| ast[cd].factory_keyword.map(|t| ast.tokens.get(t).offset))
            .or_else(|| ast[cd].new_keyword.map(|t| ast.tokens.get(t).offset))
            .unwrap_or_else(|| ast.offset(cd));
        let right = ast[cd]
            .separator
            .map(|t| ast.tokens.get(t).offset)
            .unwrap_or_else(|| ast.offset(ast[cd].parameters));
        if left <= offset && end <= right {
            let el = support::declared_element(&ctx, &unit.tables, covering)?;
            return make_selem(el);
        }
        return None;
    }

    if ast.is::<SimpleIdentifier>(covering) || ast.is::<FormalParameterList>(covering) {
        if let Some(parent) = ast.parent(covering) {
            if let Some(cd) = ast.cast::<ConstructorDeclaration>(parent) {
                if !meets_requirements(ast[cd].name.is_some()) {
                    return None;
                }
                let el = support::declared_element(&ctx, &unit.tables, parent)?;
                return make_selem(el);
            }
            if let Some(cn) = ast.cast::<ConstructorName>(parent) {
                if !meets_requirements(ast[cn].name.is_some()) {
                    return None;
                }
                let el = resolve_constructor_name(&ctx, ast, &unit.tables, cn)?;
                return make_selem(el);
            }
        }
    }

    if let Some(pc) = ast.cast::<PrimaryConstructorDeclaration>(covering) {
        if !meets_requirements(ast[pc].constructor_name.is_some()) {
            return None;
        }
        let el = support::declared_element(&ctx, &unit.tables, covering)?;
        return make_selem(el);
    }

    if let Some(_pcn) = ast.cast::<PrimaryConstructorName>(covering) {
        if !meets_requirements(true) {
            return None;
        }
        if let Some(parent) = ast.parent(covering)
            && ast.is::<PrimaryConstructorDeclaration>(parent)
        {
            let el = support::declared_element(&ctx, &unit.tables, parent)?;
            return make_selem(el);
        }
    }

    if ast.is::<NamedType>(covering)
        && let Some(parent) = ast.parent(covering)
        && let Some(cn) = ast.cast::<ConstructorName>(parent)
    {
        if !meets_requirements(ast[cn].name.is_some()) {
            return None;
        }
        let el = resolve_constructor_name(&ctx, ast, &unit.tables, cn)?;
        return make_selem(el);
    }

    if let Some(cn) = ast.cast::<ConstructorName>(covering) {
        if !meets_requirements(ast[cn].name.is_some()) {
            return None;
        }
        let el = resolve_constructor_name(&ctx, ast, &unit.tables, cn)?;
        return make_selem(el);
    }

    None
}

fn resolve_constructor_name(
    ctx: &Ctx<'_>,
    ast: &Ast,
    tables: &dartr_element::ResolutionTables,
    cn: Id<ConstructorName>,
) -> Option<ElementId> {
    if let Some(el) = support::element_of(ctx, tables, cn)
        && el.tag() == Tag::Constructor
    {
        return Some(el);
    }
    if let Some(name) = ast[cn].name
        && let Some(el) = support::element_of(ctx, tables, name)
        && el.tag() == Tag::Constructor
    {
        return Some(el);
    }
    if let Some(parent) = ast.parent(cn)
        && let Some(el) = support::element_of(ctx, tables, parent)
        && el.tag() == Tag::Constructor
    {
        return Some(el);
    }
    None
}

fn find_import_at_selection(
    unit_ref: &ResolvedUnitRef,
    offset: u32,
    length: u32,
    must_have_prefix: bool,
    must_not_have_prefix: bool,
    must_not_be_deferred: bool,
) -> Option<(usize, Id<ImportDirective>)> {
    let unit = unit_ref.unit();
    let ast = &unit.ast;
    let mut cur = ast.node_covering(unit.unit, offset, length);
    let mut import_id = None;
    while let Some(n) = cur {
        if let Some(imp) = ast.cast::<ImportDirective>(n) {
            import_id = Some(imp);
            break;
        }
        cur = ast.parent(n);
    }
    let imp = import_id?;
    let has_prefix = ast[imp].prefix.is_some();
    let is_deferred = ast[imp].deferred_keyword.is_some();
    if must_have_prefix && !has_prefix {
        return None;
    }
    if must_not_have_prefix && has_prefix {
        return None;
    }
    if must_not_be_deferred && is_deferred {
        return None;
    }
    let mut import_index = 0;
    for &dir in ast.list_raw(ast[unit.unit].directives) {
        if let Some(d_imp) = ast.cast::<ImportDirective>(dir) {
            if d_imp == imp {
                return Some((import_index, imp));
            }
            import_index += 1;
        }
    }
    None
}

// ============================================================================
// Add / Remove Constructor Name & Import Prefix
// ============================================================================

fn compute_add_constructor_name(
    unit_ref: &ResolvedUnitRef,
    offset: u32,
    length: u32,
    command_arguments: &[Value],
    engine: &mut Engine<'_>,
) -> ComputeStatus {
    let Some(ctor_el) = find_constructor_at_selection(unit_ref, offset, length, false, true) else {
        return ComputeStatus::Failure(None);
    };
    let name = command_arguments
        .first()
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| compute_default_constructor_name(&ctor_el));

    let target = Target::Element(ctor_el);
    let Some(mut rename) = create_rename_for_target(unit_ref, target) else {
        return ComputeStatus::Failure(None);
    };
    rename.new_name = name;
    if engine.check_initial_conditions(&mut rename).has_error()
        || engine.check_new_name(&rename).has_error()
        || engine.check_final_conditions(&mut rename).has_error()
    {
        return ComputeStatus::Failure(None);
    }
    ComputeStatus::Success(engine.create_change(&mut rename))
}

fn compute_default_constructor_name(ctor_el: &SElem) -> String {
    ctor_el.with(|ctx| {
        let mut used = BTreeSet::new();
        if let Some(enclosing) = ctx.element_data(ctor_el.id).and_then(|d| d.enclosing)
            && let Some(iface) = enclosing.cast::<dartr_element::InterfaceElement>()
        {
            let data = ctx.interface(iface);
            for &c in &data.constructors {
                used.insert(ctx.element_name(c.raw()).unwrap_or("").to_string());
            }
            for &m in &data.methods {
                used.insert(ctx.element_name(m.raw()).unwrap_or("").to_string());
            }
            for &f in &data.fields {
                used.insert(ctx.element_name(f.raw()).unwrap_or("").to_string());
            }
        }
        let mut candidate = "name".to_string();
        let mut index = 1;
        while used.contains(&candidate) {
            candidate = format!("name{index}");
            index += 1;
        }
        candidate
    })
}

fn compute_remove_constructor_name(
    unit_ref: &ResolvedUnitRef,
    offset: u32,
    length: u32,
    engine: &mut Engine<'_>,
) -> ComputeStatus {
    let Some(ctor_el) = find_constructor_at_selection(unit_ref, offset, length, true, false) else {
        return ComputeStatus::Failure(None);
    };
    let already_has_unnamed = ctor_el.with(|ctx| {
        if let Some(enclosing) = ctx.element_data(ctor_el.id).and_then(|d| d.enclosing)
            && let Some(iface) = enclosing.cast::<dartr_element::InterfaceElement>()
        {
            for &c in &ctx.interface(iface).constructors {
                let cname = ctx.element_name(c.raw()).unwrap_or("new");
                if cname.is_empty() || cname == "new" {
                    return true;
                }
            }
        }
        false
    });
    if already_has_unnamed {
        return ComputeStatus::Failure(Some("There's already an unnamed constructor.".to_string()));
    }

    let target = Target::Element(ctor_el);
    let Some(mut rename) = create_rename_for_target(unit_ref, target) else {
        return ComputeStatus::Failure(None);
    };
    rename.new_name = String::new();
    let mut status = engine.check_initial_conditions(&mut rename);
    status.add_status(engine.check_new_name(&rename));
    status.add_status(engine.check_final_conditions(&mut rename));
    if status.has_error() {
        return ComputeStatus::Failure(status.message().map(str::to_string));
    }
    ComputeStatus::Success(engine.create_change(&mut rename))
}

fn compute_add_import_prefix(
    unit_ref: &ResolvedUnitRef,
    offset: u32,
    length: u32,
    command_arguments: &[Value],
    engine: &mut Engine<'_>,
) -> ComputeStatus {
    let Some((import_index, import_id)) =
        find_import_at_selection(unit_ref, offset, length, false, true, false)
    else {
        return ComputeStatus::Failure(None);
    };
    let prefix_name = command_arguments
        .first()
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| compute_default_import_prefix(unit_ref, import_id));

    let target = Target::Import {
        unit: ResolvedUnitRefClone::of(unit_ref),
        index: import_index,
    };
    let Some(mut rename) = create_rename_for_target(unit_ref, target) else {
        return ComputeStatus::Failure(None);
    };
    rename.new_name = prefix_name;
    if engine.check_initial_conditions(&mut rename).has_error()
        || engine.check_new_name(&rename).has_error()
        || engine.check_final_conditions(&mut rename).has_error()
    {
        return ComputeStatus::Failure(None);
    }
    ComputeStatus::Success(engine.create_change(&mut rename))
}

fn compute_default_import_prefix(
    unit_ref: &ResolvedUnitRef,
    import_id: Id<ImportDirective>,
) -> String {
    let unit = unit_ref.unit();
    let ast = &unit.ast;
    let uri_node = ast[import_id].uri;
    let uri_start = ast.offset(uri_node) as usize;
    let uri_end = ast.end(uri_node) as usize;
    let raw_uri = &ast.tokens.source[uri_start..uri_end];
    let unquoted = raw_uri.trim_matches(|c| c == '\'' || c == '"');
    let base = unquoted
        .rsplit('/')
        .next()
        .unwrap_or(unquoted)
        .rsplit(':')
        .next()
        .unwrap_or(unquoted);
    let without_ext = base.strip_suffix(".dart").unwrap_or(base);
    let prefix = make_valid_prefix(without_ext);

    let mut used = BTreeSet::new();
    for &dir in ast.list_raw(ast[unit.unit].directives) {
        if let Some(imp) = ast.cast::<ImportDirective>(dir)
            && let Some(p_id) = ast[imp].prefix
        {
            used.insert(ast.tokens.lexeme(ast[p_id].token).to_string());
        }
    }
    if used.contains(&prefix) {
        for i in 1..1000 {
            let candidate = format!("{prefix}{i}");
            if !used.contains(&candidate) {
                return candidate;
            }
        }
    }
    prefix
}

fn make_valid_prefix(input: &str) -> String {
    let mut s = String::new();
    let mut prev_underscore = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' {
            if ch == '_' {
                if !prev_underscore {
                    s.push('_');
                    prev_underscore = true;
                }
            } else {
                s.push(ch.to_ascii_lowercase());
                prev_underscore = false;
            }
        } else if !prev_underscore {
            s.push('_');
            prev_underscore = true;
        }
    }
    let trimmed = s
        .trim_start_matches(|c: char| c.is_ascii_digit() || c == '_')
        .trim_end_matches('_');
    let is_keyword = (0..=255u8).any(|i| {
        let ty = dartr_syntax::TokenType(i);
        ty.is_keyword() && ty.lexeme() == trimmed
    });
    if trimmed.is_empty() || is_keyword {
        "prefix".to_string()
    } else {
        trimmed.to_string()
    }
}

fn compute_remove_import_prefix(
    unit_ref: &ResolvedUnitRef,
    offset: u32,
    length: u32,
    engine: &mut Engine<'_>,
) -> ComputeStatus {
    let Some((import_index, _)) =
        find_import_at_selection(unit_ref, offset, length, true, false, true)
    else {
        return ComputeStatus::Failure(None);
    };
    let target = Target::Import {
        unit: ResolvedUnitRefClone::of(unit_ref),
        index: import_index,
    };
    let Some(mut rename) = create_rename_for_target(unit_ref, target) else {
        return ComputeStatus::Failure(None);
    };
    rename.new_name = String::new();
    if engine.check_initial_conditions(&mut rename).has_error()
        || engine.check_new_name(&rename).has_error()
        || engine.check_final_conditions(&mut rename).has_error()
    {
        return ComputeStatus::Failure(None);
    }
    ComputeStatus::Success(engine.create_change(&mut rename))
}

// ============================================================================
// Method Signature Refactorings (convert_all, convert_selected, move_left)
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FormalParamKind {
    RequiredPositional,
    OptionalPositional,
    RequiredNamed,
    OptionalNamed,
}

impl FormalParamKind {
    fn is_positional(self) -> bool {
        matches!(
            self,
            FormalParamKind::RequiredPositional | FormalParamKind::OptionalPositional
        )
    }

    fn is_named(self) -> bool {
        matches!(
            self,
            FormalParamKind::RequiredNamed | FormalParamKind::OptionalNamed
        )
    }
}

#[derive(Clone)]
struct ParamState {
    id: usize,
    kind: FormalParamKind,
    positional_index: Option<usize>,
    name: String,
    node_id: NodeId,
    is_selected: bool,
    is_field_formal: bool,
}

struct SignatureTarget {
    element: Option<SElem>,
    param_list_id: Id<FormalParameterList>,
    params: Vec<ParamState>,
}

impl SignatureTarget {
    fn has_positional_parameters_to_convert_to_named(&self, unit_ref: &ResolvedUnitRef) -> bool {
        let supports_private = self.supports_private_named_parameters(unit_ref);
        let mut has_positional = false;
        for p in &self.params {
            if p.kind.is_named() {
                continue;
            }
            has_positional = true;
            if p.name.starts_with('_') && (!supports_private || !p.is_field_formal) {
                return false;
            }
        }
        has_positional
    }

    fn has_selected_formal_parameters_to_convert_to_named(
        &self,
        unit_ref: &ResolvedUnitRef,
    ) -> bool {
        let selected: Vec<&ParamState> = self.params.iter().filter(|p| p.is_selected).collect();
        if selected.is_empty() {
            return false;
        }
        if selected
            .iter()
            .all(|p| p.kind == FormalParamKind::RequiredNamed)
        {
            return false;
        }
        for p in &self.params {
            if !p.is_selected && p.kind == FormalParamKind::OptionalPositional {
                return false;
            }
        }
        let supports_private = self.supports_private_named_parameters(unit_ref);
        for p in &selected {
            if p.name.is_empty() {
                return false;
            }
            if p.name.starts_with('_') && (!supports_private || !p.is_field_formal) {
                return false;
            }
        }
        true
    }

    fn has_selected_formal_parameters_to_move_left(&self) -> bool {
        let Some(first_selected_idx) = self.params.iter().position(|p| p.is_selected) else {
            return false;
        };
        if first_selected_idx < 1 {
            return false;
        }
        let first_selected = &self.params[first_selected_idx];
        let previous = &self.params[first_selected_idx - 1];
        if first_selected.kind == FormalParamKind::OptionalPositional
            && previous.kind != FormalParamKind::OptionalPositional
        {
            return false;
        }
        if first_selected.kind.is_named() && !previous.kind.is_named() {
            return false;
        }
        true
    }

    fn supports_private_named_parameters(&self, unit_ref: &ResolvedUnitRef) -> bool {
        let sink = NoopSink;
        let ctx = unit_ref.ctx(&sink);
        ctx.features.is_enabled("private-named-parameters")
    }
}

fn analyze_signature_availability(
    unit_ref: &ResolvedUnitRef,
    offset: u32,
    length: u32,
) -> Option<SignatureTarget> {
    let sink = NoopSink;
    let ctx = unit_ref.ctx(&sink);
    let unit = unit_ref.unit();
    let ast = &unit.ast;
    let end = offset + length;
    let covering = ast.node_covering(unit.unit, offset, length)?;

    let mut selected_param_ids: Vec<NodeId> = Vec::new();
    let mut decl_candidate: Option<NodeId> = None;
    let mut any_location = false;

    if let Some(fpl) = ast.cast::<FormalParameterList>(covering) {
        for &p_id in ast.list_raw(ast[fpl].parameters) {
            if offset <= ast.offset(p_id) && ast.end(p_id) <= end {
                selected_param_ids.push(p_id);
            }
        }
        if !selected_param_ids.is_empty() {
            decl_candidate = ast.parent(covering);
            any_location = true;
        }
    } else {
        // Check if covering is or is inside a FormalParameter whose parent is FormalParameterList
        let mut cur = Some(covering);
        while let Some(n) = cur {
            if ast.is::<FormalParameter>(n)
                && let Some(p) = ast.parent(n)
                && ast.is::<FormalParameterList>(p)
            {
                selected_param_ids.push(n);
                decl_candidate = ast.parent(p);
                any_location = true;
                break;
            }
            cur = ast.parent(n);
        }
    }

    if decl_candidate.is_none() {
        let mut cur = Some(covering);
        while let Some(n) = cur {
            if ast.is::<FunctionDeclaration>(n)
                || ast.is::<MethodDeclaration>(n)
                || ast.is::<ConstructorDeclaration>(n)
                || ast.is::<PrimaryConstructorDeclaration>(n)
            {
                decl_candidate = Some(n);
                break;
            }
            cur = ast.parent(n);
        }
    }

    let mut decl_node = decl_candidate?;
    if ast.is::<FunctionExpression>(decl_node)
        && let Some(p) = ast.parent(decl_node)
        && ast.is::<FunctionDeclaration>(p)
    {
        decl_node = p;
    }

    let (param_list_id, name_start, name_end) =
        if let Some(fd) = ast.cast::<FunctionDeclaration>(decl_node) {
            let fe = ast[fd].function_expression;
            let fpl = ast[fe].parameters?;
            let tok = ast.tokens.get(ast[fd].name);
            (fpl, tok.offset, tok.end())
        } else if let Some(md) = ast.cast::<MethodDeclaration>(decl_node) {
            let fpl = ast[md].parameters?;
            let tok = ast.tokens.get(ast[md].name);
            (fpl, tok.offset, tok.end())
        } else if let Some(cd) = ast.cast::<ConstructorDeclaration>(decl_node) {
            let fpl = ast[cd].parameters;
            let ns = ast[cd]
                .type_name
                .map(|t| ast.offset(t))
                .or_else(|| ast[cd].new_keyword.map(|t| ast.tokens.get(t).offset))
                .or_else(|| ast[cd].factory_keyword.map(|t| ast.tokens.get(t).offset))?;
            let ne = ast[cd]
                .name
                .map(|t| ast.tokens.get(t).end())
                .or_else(|| ast[cd].type_name.map(|t| ast.end(t)))
                .unwrap_or(ns);
            (fpl, ns, ne)
        } else if let Some(pc) = ast.cast::<PrimaryConstructorDeclaration>(decl_node) {
            let fpl = ast[pc].formal_parameters;
            let tok = ast.tokens.get(ast[pc].type_name);
            let ns = tok.offset;
            let ne = ast[pc]
                .constructor_name
                .map(|cn| ast.end(cn))
                .unwrap_or_else(|| tok.end());
            (fpl, ns, ne)
        } else {
            return None;
        };

    if !any_location && !(name_start <= offset && end <= name_end) {
        return None;
    }

    let mut params = Vec::new();
    let mut positional_index = 0;
    for (idx, &p_id) in ast
        .list_raw(ast[param_list_id].parameters)
        .iter()
        .enumerate()
    {
        let (name, kind, is_field_formal) = formal_param_info(ast, param_list_id, p_id)?;
        let pos_idx = if kind.is_positional() {
            let i = positional_index;
            positional_index += 1;
            Some(i)
        } else {
            None
        };
        params.push(ParamState {
            id: idx,
            kind,
            positional_index: pos_idx,
            name,
            node_id: p_id,
            is_selected: selected_param_ids.contains(&p_id),
            is_field_formal,
        });
    }

    let element = support::declared_element(&ctx, &unit.tables, decl_node).map(|id| SElem {
        lib: unit_ref.library.clone(),
        unit: unit_ref.index,
        id,
    });
    Some(SignatureTarget {
        element,
        param_list_id,
        params,
    })
}

fn formal_param_info(
    ast: &Ast,
    fpl_id: Id<FormalParameterList>,
    p_id: NodeId,
) -> Option<(String, FormalParamKind, bool)> {
    let fpl = &ast[fpl_id];
    let p_start = ast.offset(p_id);
    let p_end = ast.end(p_id);
    let in_delimiters = if let (Some(ld), Some(rd)) = (fpl.left_delimiter, fpl.right_delimiter) {
        p_start >= ast.tokens.get(ld).offset && p_end <= ast.tokens.get(rd).end()
    } else {
        false
    };
    let is_named_group = fpl
        .left_delimiter
        .is_some_and(|t| ast.tokens.lexeme(t) == "{")
        && in_delimiters;
    let is_opt_pos_group = fpl
        .left_delimiter
        .is_some_and(|t| ast.tokens.lexeme(t) == "[")
        && in_delimiters;

    let (name_tok, has_required, is_field) =
        if let Some(s) = ast.cast::<RegularFormalParameter>(p_id) {
            (ast[s].name?, ast[s].required_keyword.is_some(), false)
        } else if let Some(f) = ast.cast::<FieldFormalParameter>(p_id) {
            (ast[f].name, ast[f].required_keyword.is_some(), true)
        } else if let Some(sup) = ast.cast::<SuperFormalParameter>(p_id) {
            (ast[sup].name, ast[sup].required_keyword.is_some(), false)
        } else {
            return None;
        };

    let kind = if is_named_group {
        if has_required {
            FormalParamKind::RequiredNamed
        } else {
            FormalParamKind::OptionalNamed
        }
    } else if is_opt_pos_group {
        FormalParamKind::OptionalPositional
    } else {
        FormalParamKind::RequiredPositional
    };

    Some((ast.tokens.lexeme(name_tok).to_string(), kind, is_field))
}

#[derive(Clone)]
struct FormalParamUpdate {
    id: usize,
    kind: FormalParamKind,
}

fn compute_convert_all_formal_parameters_to_named(
    unit_ref: &ResolvedUnitRef,
    offset: u32,
    length: u32,
    engine: &mut Engine<'_>,
) -> ComputeStatus {
    let Some(target) = analyze_signature_availability(unit_ref, offset, length) else {
        return ComputeStatus::Failure(None);
    };
    if !target.has_positional_parameters_to_convert_to_named(unit_ref) {
        return ComputeStatus::Failure(None);
    }
    let updates: Vec<FormalParamUpdate> = target
        .params
        .iter()
        .map(|p| FormalParamUpdate {
            id: p.id,
            kind: if p.kind.is_positional() {
                FormalParamKind::RequiredNamed
            } else {
                p.kind
            },
        })
        .collect();
    apply_signature_updates(unit_ref, &target, &updates, engine)
}

fn compute_convert_selected_formal_parameters_to_named(
    unit_ref: &ResolvedUnitRef,
    offset: u32,
    length: u32,
    engine: &mut Engine<'_>,
) -> ComputeStatus {
    let Some(target) = analyze_signature_availability(unit_ref, offset, length) else {
        return ComputeStatus::Failure(None);
    };
    if !target.has_selected_formal_parameters_to_convert_to_named(unit_ref) {
        return ComputeStatus::Failure(None);
    }
    let before_selected = target.params.iter().filter(|p| !p.is_selected);
    let selected = target.params.iter().filter(|p| p.is_selected);
    let mut updates = Vec::new();
    for p in before_selected.chain(selected) {
        let new_kind = if p.is_selected {
            FormalParamKind::RequiredNamed
        } else {
            p.kind
        };
        updates.push(FormalParamUpdate {
            id: p.id,
            kind: new_kind,
        });
    }
    apply_signature_updates(unit_ref, &target, &updates, engine)
}

fn compute_move_selected_formal_parameters_left(
    unit_ref: &ResolvedUnitRef,
    offset: u32,
    length: u32,
    engine: &mut Engine<'_>,
) -> ComputeStatus {
    let Some(target) = analyze_signature_availability(unit_ref, offset, length) else {
        return ComputeStatus::Failure(None);
    };
    if !target.has_selected_formal_parameters_to_move_left() {
        return ComputeStatus::Failure(None);
    }
    let Some(first_selected_idx) = target.params.iter().position(|p| p.is_selected) else {
        return ComputeStatus::Failure(None);
    };
    if first_selected_idx < 1 {
        return ComputeStatus::Failure(None);
    }
    let mut reordered = Vec::new();
    reordered.extend_from_slice(&target.params[..first_selected_idx - 1]);
    for p in &target.params[first_selected_idx..] {
        if p.is_selected {
            reordered.push(p.clone());
        }
    }
    reordered.push(target.params[first_selected_idx - 1].clone());
    for p in &target.params[first_selected_idx..] {
        if !p.is_selected {
            reordered.push(p.clone());
        }
    }
    let updates: Vec<FormalParamUpdate> = reordered
        .iter()
        .map(|p| FormalParamUpdate {
            id: p.id,
            kind: p.kind,
        })
        .collect();
    apply_signature_updates(unit_ref, &target, &updates, engine)
}

fn apply_signature_updates(
    unit_ref: &ResolvedUnitRef,
    target: &SignatureTarget,
    updates: &[FormalParamUpdate],
    engine: &mut Engine<'_>,
) -> ComputeStatus {
    let mut opt_pos_count = 0;
    let mut named_count = 0;
    for u in updates {
        match u.kind {
            FormalParamKind::RequiredPositional => {
                if opt_pos_count > 0 || named_count > 0 {
                    return ComputeStatus::Failure(None);
                }
            }
            FormalParamKind::OptionalPositional => {
                if named_count > 0 {
                    return ComputeStatus::Failure(None);
                }
                opt_pos_count += 1;
            }
            FormalParamKind::RequiredNamed | FormalParamKind::OptionalNamed => {
                if opt_pos_count > 0 {
                    return ComputeStatus::Failure(None);
                }
                named_count += 1;
            }
        }
    }

    let mut acc = ChangeAcc::default();

    let Some((off, len, repl)) = build_parameter_list_edit(unit_ref, target, updates) else {
        return ComputeStatus::Failure(None);
    };
    acc.add(unit_ref.path(), off, len, &repl);

    if let Some(ref selem) = target.element {
        let elements = match selem.id.tag() {
            Tag::Method | Tag::Constructor => {
                let (members, _) = engine.host.hierarchy_members_and_parameters(selem);
                if members.is_empty() {
                    vec![selem.clone()]
                } else {
                    members
                }
            }
            _ => vec![selem.clone()],
        };

        for el in elements {
            if el.id != selem.id {
                if let Some((path, o_off, o_len, o_repl)) =
                    build_override_parameter_list_edit(&el, target, updates, engine)
                {
                    acc.add(&path, o_off, o_len, &o_repl);
                }
            }

            let matches = engine.host.search_references(&el);
            for reference in source_references(&matches) {
                let Some(ref_unit) = engine
                    .host
                    .resolved_unit_in(&reference.path, Some(reference.context))
                else {
                    continue;
                };
                if let Some((a_off, a_len, a_repl)) =
                    build_invocation_arguments_edit(&ref_unit, reference.offset, target, updates)
                {
                    acc.add(&reference.path, a_off, a_len, &a_repl);
                }
            }
        }
    }

    ComputeStatus::Success(acc.into_change(String::new()))
}

fn build_parameter_list_edit(
    unit_ref: &ResolvedUnitRef,
    target: &SignatureTarget,
    updates: &[FormalParamUpdate],
) -> Option<(u32, u32, String)> {
    let unit = unit_ref.unit();
    let ast = &unit.ast;
    let content = &ast.tokens.source;
    let has_trailing_comma = has_param_list_trailing_comma(ast, target.param_list_id);

    let mut req_pos_writes = Vec::new();
    let mut opt_pos_writes = Vec::new();
    let mut named_writes = Vec::new();

    for u in updates {
        let state = target.params.get(u.id)?;
        let text = format_updated_param(ast, content, state.node_id, u.kind);
        match u.kind {
            FormalParamKind::RequiredPositional => req_pos_writes.push(text),
            FormalParamKind::OptionalPositional => opt_pos_writes.push(text),
            FormalParamKind::RequiredNamed | FormalParamKind::OptionalNamed => {
                named_writes.push(text)
            }
        }
    }

    let new_list_text = render_parameter_list(
        &req_pos_writes,
        &opt_pos_writes,
        &named_writes,
        has_trailing_comma,
    );
    Some((
        ast.offset(target.param_list_id),
        ast.length(target.param_list_id),
        new_list_text,
    ))
}

fn build_override_parameter_list_edit(
    el: &SElem,
    base_target: &SignatureTarget,
    updates: &[FormalParamUpdate],
    engine: &mut Engine<'_>,
) -> Option<(String, u32, u32, String)> {
    let (path, name_offset) = el.with(|ctx| {
        let first = ctx.element_data(el.id)?.first_fragment;
        let p = crate::navigation::fragment_path(ctx, first)?;
        let off = ctx.fragment_data(first)?.name_offset?;
        Some((p, off))
    })?;
    let resolved = engine.host.resolved_unit_in(&path, Some(el.lib.context))?;
    let unit = resolved.unit();
    let ast = &unit.ast;
    let content = &ast.tokens.source;
    let mut cur = ast.node_covering(unit.unit, name_offset, 0);
    let mut fpl_id = None;
    while let Some(n) = cur {
        if let Some(md) = ast.cast::<MethodDeclaration>(n) {
            fpl_id = ast[md].parameters;
            break;
        }
        if let Some(cd) = ast.cast::<ConstructorDeclaration>(n) {
            fpl_id = Some(ast[cd].parameters);
            break;
        }
        cur = ast.parent(n);
    }
    let fpl_id = fpl_id?;
    let has_trailing_comma = has_param_list_trailing_comma(ast, fpl_id);

    let mut positional_nodes = Vec::new();
    let mut named_nodes = BTreeMap::new();
    for &p_id in ast.list_raw(ast[fpl_id].parameters) {
        let (name, kind, _) = formal_param_info(ast, fpl_id, p_id)?;
        if kind.is_positional() {
            positional_nodes.push(p_id);
        } else {
            named_nodes.insert(name, p_id);
        }
    }

    let mut req_pos_writes = Vec::new();
    let mut opt_pos_writes = Vec::new();
    let mut named_writes = Vec::new();

    for u in updates {
        let base_state = base_target.params.get(u.id)?;
        let node_id = if let Some(pos_idx) = base_state.positional_index {
            *positional_nodes.get(pos_idx)?
        } else {
            let Some(n_id) = named_nodes.remove(&base_state.name) else {
                continue;
            };
            n_id
        };
        let text = format_updated_param(ast, content, node_id, u.kind);
        match u.kind {
            FormalParamKind::RequiredPositional => req_pos_writes.push(text),
            FormalParamKind::OptionalPositional => opt_pos_writes.push(text),
            FormalParamKind::RequiredNamed | FormalParamKind::OptionalNamed => {
                named_writes.push(text)
            }
        }
    }
    for (_, p_id) in named_nodes {
        let s = ast.offset(p_id) as usize;
        let e = ast.end(p_id) as usize;
        named_writes.push(content[s..e].to_string());
    }

    let new_list_text = render_parameter_list(
        &req_pos_writes,
        &opt_pos_writes,
        &named_writes,
        has_trailing_comma,
    );
    Some((path, ast.offset(fpl_id), ast.length(fpl_id), new_list_text))
}

fn has_param_list_trailing_comma(ast: &Ast, fpl_id: Id<FormalParameterList>) -> bool {
    let fpl = &ast[fpl_id];
    let params = ast.list_raw(fpl.parameters);
    let Some(&last_param) = params.last() else {
        return false;
    };
    let last_end = ast.end(last_param) as usize;
    let close_start = fpl
        .right_delimiter
        .map(|t| ast.tokens.get(t).offset as usize)
        .unwrap_or(ast.tokens.get(fpl.right_parenthesis).offset as usize);
    let content = &ast.tokens.source;
    if last_end <= close_start && close_start <= content.len() {
        content[last_end..close_start].trim().starts_with(',')
    } else {
        false
    }
}

fn format_updated_param(
    ast: &Ast,
    content: &str,
    p_id: NodeId,
    new_kind: FormalParamKind,
) -> String {
    let default_clause = if let Some(s) = ast.cast::<RegularFormalParameter>(p_id) {
        ast[s].default_clause
    } else if let Some(f) = ast.cast::<FieldFormalParameter>(p_id) {
        ast[f].default_clause
    } else if let Some(sup) = ast.cast::<SuperFormalParameter>(p_id) {
        ast[sup].default_clause
    } else {
        None
    };
    let default_sep_start = default_clause.map(|dc| ast.offset(dc) as usize);

    let req_tok = if let Some(s) = ast.cast::<RegularFormalParameter>(p_id) {
        ast[s].required_keyword
    } else if let Some(f) = ast.cast::<FieldFormalParameter>(p_id) {
        ast[f].required_keyword
    } else if let Some(sup) = ast.cast::<SuperFormalParameter>(p_id) {
        ast[sup].required_keyword
    } else {
        None
    };

    let full_start = ast.offset(p_id) as usize;
    let full_end = ast.end(p_id) as usize;
    let without_default_end = default_sep_start.unwrap_or(full_end);

    match new_kind {
        FormalParamKind::RequiredNamed => {
            if req_tok.is_some() {
                content[full_start..without_default_end]
                    .trim_end()
                    .to_string()
            } else {
                let body = content[full_start..without_default_end].trim_end();
                format!("required {body}")
            }
        }
        FormalParamKind::RequiredPositional => {
            if let Some(rt_id) = req_tok {
                let rt = ast.tokens.get(rt_id);
                let before = &content[full_start..rt.offset as usize];
                let after = content[rt.end() as usize..without_default_end]
                    .trim_start()
                    .trim_end();
                if before.is_empty() {
                    after.to_string()
                } else {
                    format!("{before}{after}")
                }
            } else {
                content[full_start..without_default_end]
                    .trim_end()
                    .to_string()
            }
        }
        FormalParamKind::OptionalPositional | FormalParamKind::OptionalNamed => {
            if let Some(rt_id) = req_tok {
                let rt = ast.tokens.get(rt_id);
                let before = &content[full_start..rt.offset as usize];
                let after = content[rt.end() as usize..full_end].trim_start();
                if before.is_empty() {
                    after.to_string()
                } else {
                    format!("{before}{after}")
                }
            } else {
                content[full_start..full_end].to_string()
            }
        }
    }
}

fn render_parameter_list(
    req_pos: &[String],
    opt_pos: &[String],
    named: &[String],
    has_trailing_comma: bool,
) -> String {
    let mut out = String::from("(");
    let mut has_written = false;
    for p in req_pos {
        if has_written {
            out.push_str(", ");
        }
        out.push_str(p);
        has_written = true;
    }
    if !req_pos.is_empty() && opt_pos.is_empty() && named.is_empty() && has_trailing_comma {
        out.push(',');
    }

    let mut write_group = |params: &[String], left: &str, right: &str| {
        if !params.is_empty() {
            if has_written {
                out.push_str(", ");
                has_written = false;
            }
            out.push_str(left);
            for p in params {
                if has_written {
                    out.push_str(", ");
                }
                out.push_str(p);
                has_written = true;
            }
            if has_trailing_comma {
                out.push(',');
            }
            out.push_str(right);
        }
    };

    write_group(opt_pos, "[", "]");
    write_group(named, "{", "}");
    out.push(')');
    out
}

fn build_invocation_arguments_edit(
    unit_ref: &ResolvedUnitRef,
    ref_offset: u32,
    target: &SignatureTarget,
    updates: &[FormalParamUpdate],
) -> Option<(u32, u32, String)> {
    let unit = unit_ref.unit();
    let ast = &unit.ast;
    let content = &ast.tokens.source;
    let mut cur = ast.node_covering(unit.unit, ref_offset, 0);

    let mut arg_list_id: Option<Id<ArgumentList>> = None;
    while let Some(p) = cur {
        if let Some(mi) = ast.cast::<MethodInvocation>(p) {
            arg_list_id = Some(ast[mi].argument_list);
            break;
        }
        if let Some(ice) = ast.cast::<InstanceCreationExpression>(p) {
            arg_list_id = Some(ast[ice].argument_list);
            break;
        }
        if let Some(rci) = ast.cast::<RedirectingConstructorInvocation>(p) {
            arg_list_id = Some(ast[rci].argument_list);
            break;
        }
        if let Some(sci) = ast.cast::<SuperConstructorInvocation>(p) {
            arg_list_id = Some(ast[sci].argument_list);
            break;
        }
        cur = ast.parent(p);
    }
    let arg_list_id = arg_list_id?;
    let arg_list = &ast[arg_list_id];
    let args = ast.list_raw(arg_list.arguments);

    let mut pos_args = Vec::new();
    let mut named_args: Vec<(String, NodeId)> = Vec::new();
    for &arg_id in args {
        if let Some(ne) = ast.cast::<NamedArgument>(arg_id) {
            let name = ast.tokens.lexeme(ast[ne].name).to_string();
            named_args.push((name, ast[ne].argument_expression.raw()));
        } else {
            pos_args.push(arg_id);
        }
    }

    let has_trailing_comma = if let Some(&last_arg) = args.last() {
        let last_end = ast.end(last_arg) as usize;
        let close_start = ast.tokens.get(arg_list.right_parenthesis).offset as usize;
        last_end <= close_start
            && close_start <= content.len()
            && content[last_end..close_start].trim().starts_with(',')
    } else {
        false
    };

    let mut written_args = Vec::new();
    let mut used_named = BTreeSet::new();

    for u in updates {
        let state = target.params.get(u.id)?;
        let arg_expr = if let Some(pos_idx) = state.positional_index {
            pos_args.get(pos_idx).copied()
        } else {
            named_args
                .iter()
                .find(|(n, _)| *n == state.name)
                .map(|(_, e)| *e)
        };
        if state.positional_index.is_none() {
            used_named.insert(state.name.clone());
        }
        let Some(expr_id) = arg_expr else {
            continue;
        };
        let s = ast.offset(expr_id) as usize;
        let e = ast.end(expr_id) as usize;
        let expr_text = &content[s..e];
        match u.kind {
            FormalParamKind::RequiredPositional | FormalParamKind::OptionalPositional => {
                written_args.push(expr_text.to_string());
            }
            FormalParamKind::RequiredNamed | FormalParamKind::OptionalNamed => {
                written_args.push(format!("{}: {expr_text}", state.name));
            }
        }
    }

    for (name, expr_id) in &named_args {
        if !used_named.contains(name) {
            let s = ast.offset(*expr_id) as usize;
            let e = ast.end(*expr_id) as usize;
            let expr_text = &content[s..e];
            written_args.push(format!("{name}: {expr_text}"));
        }
    }

    let mut new_args_text = String::from("(");
    for (i, a) in written_args.iter().enumerate() {
        if i > 0 {
            new_args_text.push_str(", ");
        }
        new_args_text.push_str(a);
    }
    if !written_args.is_empty() && has_trailing_comma {
        new_args_text.push(',');
    }
    new_args_text.push(')');

    Some((
        ast.offset(arg_list_id),
        ast.length(arg_list_id),
        new_args_text,
    ))
}

// ============================================================================
// MoveTopLevelToFile (`move_top_level_to_file.dart`)
// ============================================================================

struct MemberToMove {
    decl_id: NodeId,
    name: Option<String>,
    var_count: usize,
}

struct MembersToMove {
    containing_file: String,
    members: Vec<MemberToMove>,
}

impl MembersToMove {
    fn default_file_name(&self) -> String {
        self.members
            .first()
            .and_then(|m| m.name.as_deref())
            .map(to_file_name)
            .unwrap_or_else(|| "newFile.dart".to_string())
    }

    fn default_file_path(&self) -> String {
        let parent = Path::new(&self.containing_file)
            .parent()
            .unwrap_or_else(|| Path::new(""));
        parent
            .join(self.default_file_name())
            .to_string_lossy()
            .to_string()
    }

    fn title(&self) -> String {
        let count: usize = self.members.iter().map(|m| m.var_count).sum();
        if count == 1
            && let Some(name) = self.members.first().and_then(|m| m.name.as_deref())
        {
            return format!("Move '{name}' to file");
        }
        format!("Move {count} declarations to file")
    }
}

fn members_to_move(unit_ref: &ResolvedUnitRef, offset: u32, length: u32) -> Option<MembersToMove> {
    let unit = unit_ref.unit();
    let ast = &unit.ast;
    let end = offset + length;
    let covering = ast.node_covering(unit.unit, offset, length)?;

    let token_in_selection = |tok: TokenId| -> bool {
        let t = ast.tokens.get(tok);
        offset >= t.offset && end <= t.end()
    };

    let top_decls = ast.list_raw(ast[unit.unit].declarations);
    let mut selected_decls: Vec<NodeId> = Vec::new();

    if covering == unit.unit.raw() {
        for &d_id in top_decls {
            if offset <= ast.offset(d_id) && ast.end(d_id) <= end {
                selected_decls.push(d_id);
            }
        }
        if selected_decls.is_empty() {
            return None;
        }
    } else if let Some(vd) = ast.cast::<VariableDeclaration>(covering) {
        if !token_in_selection(ast[vd].name) {
            return None;
        }
        for &d_id in top_decls {
            if let Some(tlv) = ast.cast::<TopLevelVariableDeclaration>(d_id) {
                let vars = ast.list_raw(ast[ast[tlv].variables].variables);
                if vars.len() == 1 && vars[0] == covering {
                    selected_decls.push(d_id);
                    break;
                }
            }
        }
        if selected_decls.is_empty() {
            return None;
        }
    } else if top_decls.contains(&covering) {
        selected_decls.push(covering);
    } else if let Some(parent) = ast.parent(covering)
        && top_decls.contains(&parent)
    {
        selected_decls.push(parent);
    } else {
        return None;
    }

    let multiple = selected_decls.len() > 1;
    let valid_selection = |tok: TokenId| -> bool { multiple || token_in_selection(tok) };

    let mut members = Vec::new();
    for &d_id in &selected_decls {
        let (name, var_count) = if let Some(c) = ast.cast::<ClassDeclaration>(d_id) {
            let tok = support::class_name_token(ast, ast[c].name_part);
            if !valid_selection(tok) {
                return None;
            }
            (Some(ast.tokens.lexeme(tok).to_string()), 1)
        } else if let Some(e) = ast.cast::<EnumDeclaration>(d_id) {
            let tok = support::class_name_token(ast, ast[e].name_part);
            if !valid_selection(tok) {
                return None;
            }
            (Some(ast.tokens.lexeme(tok).to_string()), 1)
        } else if let Some(ext) = ast.cast::<ExtensionDeclaration>(d_id) {
            if let Some(tok) = ast[ext].name {
                if !valid_selection(tok) {
                    return None;
                }
                (Some(ast.tokens.lexeme(tok).to_string()), 1)
            } else if multiple {
                (None, 1)
            } else {
                return None;
            }
        } else if let Some(et) = ast.cast::<ExtensionTypeDeclaration>(d_id) {
            let tok = support::class_name_token(ast, ast[et].name_part);
            if !valid_selection(tok) {
                return None;
            }
            (Some(ast.tokens.lexeme(tok).to_string()), 1)
        } else if let Some(f) = ast.cast::<FunctionDeclaration>(d_id) {
            let tok = ast[f].name;
            if !valid_selection(tok) {
                return None;
            }
            (Some(ast.tokens.lexeme(tok).to_string()), 1)
        } else if let Some(m) = ast.cast::<MixinDeclaration>(d_id) {
            let tok = ast[m].name;
            if !valid_selection(tok) {
                return None;
            }
            (Some(ast.tokens.lexeme(tok).to_string()), 1)
        } else if let Some(tlv) = ast.cast::<TopLevelVariableDeclaration>(d_id) {
            let vars = ast.list_raw(ast[ast[tlv].variables].variables);
            let name = if vars.len() == 1
                && let Some(vd) = ast.cast::<VariableDeclaration>(vars[0])
            {
                Some(ast.tokens.lexeme(ast[vd].name).to_string())
            } else {
                None
            };
            (name, vars.len())
        } else if let Some(fta) = ast.cast::<FunctionTypeAlias>(d_id) {
            let tok = ast[fta].name;
            if !valid_selection(tok) {
                return None;
            }
            (Some(ast.tokens.lexeme(tok).to_string()), 1)
        } else if let Some(gta) = ast.cast::<GenericTypeAlias>(d_id) {
            let tok = ast[gta].name;
            if !valid_selection(tok) {
                return None;
            }
            (Some(ast.tokens.lexeme(tok).to_string()), 1)
        } else if let Some(cta) = ast.cast::<ClassTypeAlias>(d_id) {
            let tok = ast[cta].name;
            if !valid_selection(tok) {
                return None;
            }
            (Some(ast.tokens.lexeme(tok).to_string()), 1)
        } else {
            return None;
        };

        members.push(MemberToMove {
            decl_id: d_id,
            name,
            var_count,
        });
    }

    if members.is_empty() {
        return None;
    }

    Some(MembersToMove {
        containing_file: unit_ref.path().to_string(),
        members,
    })
}

fn compute_move_top_level_to_file(
    unit_ref: &ResolvedUnitRef,
    offset: u32,
    length: u32,
    command_arguments: &[Value],
    engine: &mut Engine<'_>,
) -> ComputeStatus {
    let Some(members) = members_to_move(unit_ref, offset, length) else {
        return ComputeStatus::Failure(None);
    };
    let dest_path = if let Some(uri_str) = command_arguments.first().and_then(Value::as_str) {
        uri_to_path(uri_str).unwrap_or_else(|_| uri_str.to_string())
    } else {
        members.default_file_path()
    };

    let dest_import_uri = engine
        .host
        .collection()
        .and_then(|c| {
            c.context_for(&dest_path)
                .or_else(|| c.context_for(&members.containing_file))
                .or_else(|| c.contexts.first())
        })
        .and_then(|ctx| ctx.packages.path_to_package_uri(&dest_path))
        .unwrap_or_else(|| path_to_uri(&dest_path));

    let unit = unit_ref.unit();
    let ast = &unit.ast;
    let content = &ast.tokens.source;
    let line_info = unit_ref.line_info();
    let source_unit_uri = unit.uri.to_string();

    let first_decl = members.members.first().unwrap().decl_id;
    let last_decl = members.members.last().unwrap().decl_id;
    let extract_start = ast.offset(first_decl) as usize;
    let extract_end = member_group_end_offset(ast, content, line_info, last_decl);
    let delete_start = member_group_start_offset(ast, content, line_info, first_decl);
    let moved_text = content[extract_start..extract_end].to_string();

    let dest_existing = engine
        .host
        .resolved_unit_in(&dest_path, None)
        .map(|u| u.unit().ast.tokens.source.clone())
        .or_else(|| std::fs::read_to_string(&dest_path).ok().map(Arc::from));

    let eol = if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };

    let (insert_offset, insert_leading_newline) = if let Some(ref existing) = dest_existing {
        (existing.encode_utf16().count() as u32, true)
    } else {
        (0, false)
    };

    let analyzed = analyze_move_imports(
        unit_ref,
        &members.containing_file,
        extract_start as u32,
        extract_end as u32,
    );

    // Search references in other libraries to moving declarations.
    let source_lib_path = unit_ref.library.inputs[0].path.to_string();
    let mut other_lib_prefixes: indexmap::IndexMap<String, indexmap::IndexSet<String>> =
        indexmap::IndexMap::new();
    for &decl_el in &analyzed.moving_declarations {
        let selem = SElem {
            lib: unit_ref.library.clone(),
            unit: unit_ref.index,
            id: decl_el,
        };
        let matches = engine.host.search_references(&selem);
        for m in matches {
            let Some(resolved_m) = engine.host.resolved_unit_in(&m.path, Some(m.context)) else {
                continue;
            };
            let lib_path = resolved_m.library.inputs[0].path.to_string();
            if lib_path == source_lib_path {
                continue;
            }
            let pfx = prefix_used_at_offset(&resolved_m, m.offset);
            other_lib_prefixes.entry(lib_path).or_default().insert(pfx);
        }
    }

    let title = members.title();
    let mut built_change = None;
    engine.host.with_change_workspace(&mut |ws| {
        let mut cb = crate::correction::change_builder::ChangeBuilder::new(ws, Some(eol));
        let populate_dest = |b: &mut crate::correction::change_builder::FileEditBuilder<'_, '_>| {
            b.add_insertion(insert_offset, |eb| {
                if insert_leading_newline {
                    eb.newline();
                }
                eb.write(&moved_text);
            });
            if analyzed.has_moving_ref_to_staying {
                b.import_library(&source_unit_uri, None, None, false);
            }
            for (el_name, imp) in &analyzed.moving_external_imports {
                if let Some(ref uri) = imp.library_uri
                    && uri != "dart:core"
                {
                    let has_show = imp.combinators.iter().any(|(is_show, _)| *is_show);
                    b.import_library(uri, imp.prefix.as_deref(), el_name.as_deref(), has_show);
                }
            }
        };
        if dest_existing.is_some() {
            cb.add_dart_file_edit(&dest_path, populate_dest);
        } else {
            cb.add_new_dart_file_edit(&dest_path, &dest_import_uri, unit_ref, populate_dest);
        }
        cb.add_dart_file_edit(&members.containing_file, |b| {
            if analyzed.has_staying_ref_to_moving {
                b.import_library(&dest_import_uri, None, None, false);
            }
            b.add_deletion(delete_start as u32, (extract_end - delete_start) as u32);
        });
        for (lib_path, prefixes) in &other_lib_prefixes {
            cb.add_dart_file_edit(lib_path, |b| {
                for pfx in prefixes {
                    let pfx_opt = if pfx.is_empty() {
                        None
                    } else {
                        Some(pfx.as_str())
                    };
                    b.import_library(&dest_import_uri, pfx_opt, None, false);
                }
            });
        }
        let mut sc = cb.source_change();
        sc.message = title.clone();
        built_change = Some(sc);
    });

    match built_change {
        Some(sc) => ComputeStatus::Success(sc),
        None => ComputeStatus::Failure(None),
    }
}

struct MoveImportAnalysis {
    moving_declarations: indexmap::IndexSet<ElementId>,
    has_moving_ref_to_staying: bool,
    has_staying_ref_to_moving: bool,
    moving_external_imports: Vec<(Option<String>, crate::correction::imports::ExistingImport)>,
}

fn analyze_move_imports(
    unit_ref: &ResolvedUnitRef,
    source_path: &str,
    range_start: u32,
    range_end: u32,
) -> MoveImportAnalysis {
    let sink = NoopSink;
    let ctx = unit_ref.ctx(&sink);
    let library = unit_ref.library.library.library;
    let existing_imps = crate::correction::imports::existing_imports(&ctx, library);

    let mut moving_declarations = indexmap::IndexSet::new();
    let mut staying_declarations = indexmap::IndexSet::new();

    for (u_idx, u) in unit_ref.library.library.units.iter().enumerate() {
        let u_ref = ResolvedUnitRef {
            library: unit_ref.library.clone(),
            index: u_idx,
        };
        let u_ctx = u_ref.ctx(&sink);
        let u_path = unit_ref.library.inputs[u_idx].path.as_ref();
        let ast = &u.ast;
        for &d_id in ast.list_raw(ast[u.unit].declarations) {
            let d_off = ast.offset(d_id);
            let is_moving = u_path == source_path && d_off >= range_start && d_off < range_end;
            let mut record_decl = |el: Option<ElementId>| {
                if let Some(el) = el {
                    if is_moving {
                        moving_declarations.insert(el);
                    } else {
                        staying_declarations.insert(el);
                    }
                }
            };
            if let Some(tlv) = ast.cast::<TopLevelVariableDeclaration>(d_id) {
                for &v_id in ast.list_raw(ast[ast[tlv].variables].variables) {
                    record_decl(support::declared_element(&u_ctx, &u.tables, v_id));
                }
            } else {
                let el = support::declared_element(&u_ctx, &u.tables, d_id);
                record_decl(el);
                if let Some(ext_el) = el.and_then(|e| e.cast::<dartr_element::ExtensionElement>()) {
                    let ext_data = u_ctx.get(ext_el);
                    for &g in &ext_data.getters {
                        if !crate::completion::elem::is_static(&u_ctx, g.raw())
                            && crate::completion::elem::is_origin_declaration(&u_ctx, g.raw())
                        {
                            record_decl(Some(g.raw()));
                        }
                    }
                    for &s in &ext_data.setters {
                        if !crate::completion::elem::is_static(&u_ctx, s.raw())
                            && crate::completion::elem::is_origin_declaration(&u_ctx, s.raw())
                        {
                            record_decl(Some(s.raw()));
                        }
                    }
                    for &f in &ext_data.fields {
                        if !crate::completion::elem::is_static(&u_ctx, f.raw())
                            && crate::completion::elem::is_origin_declaration(&u_ctx, f.raw())
                        {
                            record_decl(Some(f.raw()));
                        }
                    }
                    for &m in &ext_data.methods {
                        if !crate::completion::elem::is_static(&u_ctx, m.raw()) {
                            record_decl(Some(m.raw()));
                        }
                    }
                }
            }
        }
    }

    let mut moving_references: indexmap::IndexMap<ElementId, Vec<usize>> =
        indexmap::IndexMap::new();
    let mut staying_references: indexmap::IndexMap<ElementId, Vec<usize>> =
        indexmap::IndexMap::new();

    for (u_idx, u) in unit_ref.library.library.units.iter().enumerate() {
        let u_ref = ResolvedUnitRef {
            library: unit_ref.library.clone(),
            index: u_idx,
        };
        let u_ctx = u_ref.ctx(&sink);
        let u_path = unit_ref.library.inputs[u_idx].path.as_ref();
        let mut visitor = MoveRefVisitor {
            ctx: &u_ctx,
            tables: &u.tables,
            library,
            existing_imps: &existing_imps,
            in_source_unit: u_path == source_path,
            range_start,
            range_end,
            moving_references: &mut moving_references,
            staying_references: &mut staying_references,
        };
        u.ast.accept(u.unit, &mut visitor);
    }

    for el in &moving_declarations {
        moving_references.shift_remove(el);
    }
    for el in &staying_declarations {
        staying_references.shift_remove(el);
    }

    let has_moving_ref_to_staying = staying_declarations
        .iter()
        .any(|d| moving_references.contains_key(d));
    let has_staying_ref_to_moving = moving_declarations
        .iter()
        .any(|d| staying_references.contains_key(d));

    let mut moving_external_imports = Vec::new();
    for (&el, imp_indices) in &moving_references {
        let el_name = ctx.element_name(el).map(str::to_string);
        for &idx in imp_indices {
            moving_external_imports.push((el_name.clone(), existing_imps[idx].clone()));
        }
    }

    MoveImportAnalysis {
        moving_declarations,
        has_moving_ref_to_staying,
        has_staying_ref_to_moving,
        moving_external_imports,
    }
}

struct MoveRefVisitor<'a, 'c> {
    ctx: &'a Ctx<'c>,
    tables: &'a dartr_element::ResolutionTables,
    library: dartr_element::EId<dartr_element::LibraryElement>,
    existing_imps: &'a [crate::correction::imports::ExistingImport],
    in_source_unit: bool,
    range_start: u32,
    range_end: u32,
    moving_references: &'a mut indexmap::IndexMap<ElementId, Vec<usize>>,
    staying_references: &'a mut indexmap::IndexMap<ElementId, Vec<usize>>,
}

impl MoveRefVisitor<'_, '_> {
    fn record_ref(&mut self, raw_el: Option<ElementId>, ref_offset: u32, prefix: Option<&str>) {
        let Some(raw_el) = raw_el else { return };
        let mut el =
            dartr_typesystem::member::base_element(self.ctx, dartr_element::ElemRef::Base(raw_el));
        if el.tag() == Tag::Constructor {
            if let Some(enc) = self.ctx.element_data(el).and_then(|d| d.enclosing) {
                el = enc;
            }
        }
        if let Some(enc) = self.ctx.element_data(el).and_then(|d| d.enclosing)
            && enc.tag() == Tag::Extension
            && !crate::completion::elem::is_static(self.ctx, el)
        {
            el = enc;
        }
        if matches!(el.tag(), Tag::Getter | Tag::Setter)
            && !crate::completion::elem::is_origin_declaration(self.ctx, el)
            && let Some(var_el) =
                dartr_resolver::element_metadata::accessor_variable_any(self.ctx, el)
        {
            el = var_el;
        }
        if el.tag() == Tag::Prefix {
            return;
        }
        let is_top_level = self
            .ctx
            .element_data(el)
            .and_then(|d| d.enclosing)
            .is_some_and(|enc| enc.tag() == Tag::Library);
        if !is_top_level {
            return;
        }

        let target_lib = support::library_of(self.ctx, el);
        let pfx = prefix.unwrap_or("");
        let mut matched_imp = None;
        if let Some(t_lib) = target_lib
            && t_lib != self.library
        {
            for (idx, imp) in self.existing_imps.iter().enumerate() {
                if imp.prefix.as_deref().unwrap_or("") != pfx {
                    continue;
                }
                if imp.library == Some(t_lib) {
                    matched_imp = Some(idx);
                    break;
                }
                if let Some(imp_lib) = imp.library
                    && let Some(ns) = self.ctx.get(imp_lib).export_namespace.try_get()
                    && ns.defined_names.values().any(|&e| e == el)
                {
                    matched_imp = Some(idx);
                    break;
                }
            }
        }

        let is_moving =
            self.in_source_unit && ref_offset >= self.range_start && ref_offset < self.range_end;
        let map = if is_moving {
            &mut *self.moving_references
        } else {
            &mut *self.staying_references
        };
        let list = map.entry(el).or_default();
        if let Some(idx) = matched_imp
            && !list.contains(&idx)
        {
            list.push(idx);
        }
    }
}

impl AstVisitor for MoveRefVisitor<'_, '_> {
    fn visit_named_type(&mut self, ast: &Ast, node: Id<NamedType>) {
        let el = support::element_of(self.ctx, self.tables, node.raw());
        let pfx = ast[node]
            .import_prefix
            .map(|ip| ast.tokens.lexeme(ast[ip].name));
        self.record_ref(el, ast.offset(node.raw()), pfx);
        ast.visit_children(node, self);
    }

    fn visit_simple_identifier(&mut self, ast: &Ast, node: Id<SimpleIdentifier>) {
        if support::in_declaration_context(ast, node) {
            return;
        }
        let el = support::write_or_read_element(self.ctx, ast, self.tables, node)
            .or_else(|| support::read_element(self.ctx, self.tables, node))
            .or_else(|| support::element_of(self.ctx, self.tables, node.raw()));
        let mut pfx = None;
        if let Some(parent) = ast.parent(node.raw()) {
            if let Some(pi) = ast.cast::<PrefixedIdentifier>(parent)
                && ast[pi].identifier == node
                && support::element_of(self.ctx, self.tables, ast[pi].prefix.raw())
                    .is_some_and(|e| e.tag() == Tag::Prefix)
            {
                pfx = Some(ast.tokens.lexeme(ast[ast[pi].prefix].token));
            } else if let Some(mi) = ast.cast::<MethodInvocation>(parent)
                && ast[mi].method_name == node
                && let Some(t) = ast[mi].target
                && let Some(tsi) = ast.cast::<SimpleIdentifier>(t.raw())
                && support::element_of(self.ctx, self.tables, tsi.raw())
                    .is_some_and(|e| e.tag() == Tag::Prefix)
            {
                pfx = Some(ast.tokens.lexeme(ast[tsi].token));
            }
        }
        self.record_ref(el, ast.offset(node.raw()), pfx);
    }
}

fn prefix_used_at_offset(resolved: &ResolvedUnitRef, offset: u32) -> String {
    let sink = NoopSink;
    let ctx = resolved.ctx(&sink);
    let unit = resolved.unit();
    let ast = &unit.ast;
    let mut cur = ast.node_covering(unit.unit, offset, 0);
    while let Some(n) = cur {
        if let Some(nt) = ast.cast::<NamedType>(n) {
            if let Some(ip) = ast[nt].import_prefix {
                return ast.tokens.lexeme(ast[ip].name).to_string();
            }
            return String::new();
        }
        if let Some(eo) = ast.cast::<ExtensionOverride>(n) {
            if let Some(ip) = ast[eo].import_prefix {
                return ast.tokens.lexeme(ast[ip].name).to_string();
            }
            return String::new();
        }
        if let Some(si) = ast.cast::<SimpleIdentifier>(n) {
            if let Some(parent) = ast.parent(n) {
                if let Some(pi) = ast.cast::<PrefixedIdentifier>(parent)
                    && ast[pi].identifier == si
                    && support::element_of(&ctx, &unit.tables, ast[pi].prefix.raw())
                        .is_some_and(|e| e.tag() == Tag::Prefix)
                {
                    return ast.tokens.lexeme(ast[ast[pi].prefix].token).to_string();
                }
                if let Some(mi) = ast.cast::<MethodInvocation>(parent)
                    && ast[mi].method_name == si
                    && let Some(t) = ast[mi].target
                    && let Some(tsi) = ast.cast::<SimpleIdentifier>(t.raw())
                    && support::element_of(&ctx, &unit.tables, tsi.raw())
                        .is_some_and(|e| e.tag() == Tag::Prefix)
                {
                    return ast.tokens.lexeme(ast[tsi].token).to_string();
                }
            }
        }
        cur = ast.parent(n);
    }
    String::new()
}

fn member_group_start_offset(
    ast: &Ast,
    content: &str,
    line_info: &dartr_syntax::LineInfo,
    first_decl: NodeId,
) -> usize {
    let start = ast.offset(first_decl) as usize;
    let start_line = line_info
        .get_location(start as u32)
        .line_number
        .saturating_sub(1) as usize;
    if start_line >= 1 {
        let prev_line_start = line_info.get_offset_of_line(start_line - 1).unwrap_or(0) as usize;
        let cur_line_start = line_info.get_offset_of_line(start_line).unwrap_or(0) as usize;
        if prev_line_start <= cur_line_start
            && cur_line_start <= content.len()
            && content[prev_line_start..cur_line_start].trim().is_empty()
        {
            return prev_line_start;
        }
    }
    start
}

fn member_group_end_offset(
    ast: &Ast,
    content: &str,
    line_info: &dartr_syntax::LineInfo,
    last_decl: NodeId,
) -> usize {
    let end = ast.end(last_decl) as usize;
    let rest = &content[end..];
    if rest.trim().is_empty() {
        return content.len();
    }
    let end_line = line_info
        .get_location(end as u32)
        .line_number
        .saturating_sub(1) as usize;
    let next_line = line_info
        .get_offset_of_line(end_line + 1)
        .map(|o| o as usize)
        .unwrap_or(content.len());
    if next_line > end && next_line <= content.len() && content[end..next_line].trim().is_empty() {
        return next_line;
    }
    end
}
