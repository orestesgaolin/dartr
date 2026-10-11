// Dart source: pkg/analysis_server/lib/src/handler/legacy/edit_get_available_refactorings.dart
// Dart source: pkg/analysis_server/lib/src/handler/legacy/edit_get_refactoring.dart
// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/refactoring_manager.dart

//! Legacy `edit.getAvailableRefactorings` and `edit.getRefactoring` handlers,
//! backed by the unified refactoring engine in `dartr_server::refactoring`.

use dartr_element::Tag;
use dartr_server::refactoring::convert::is_method_to_getter_available_in_unit;
use dartr_server::refactoring::extract_local::ExtractLocal;
use dartr_server::refactoring::extract_method::{ExtractMethod, MethodParameter};
use dartr_server::refactoring::extract_widget::ExtractWidget;
use dartr_server::refactoring::inline_local::InlineLocal;
use dartr_server::refactoring::inline_method::InlineMethod;
use dartr_server::refactoring::move_file::MoveFile;
use dartr_server::refactoring::rename::{
    Engine, create_rename_for_target, element_to_rename_in_unit,
};
use dartr_server::refactoring::status::RefactoringStatus;
use serde_json::Value;

use crate::protocol;
use crate::search::{ResolvedUnitRef, SearchEngine};

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct RefactoringResponse {
    #[serde(rename = "initialProblems")]
    pub initial_problems: Vec<protocol::RefactoringProblem>,
    #[serde(rename = "optionsProblems")]
    pub options_problems: Vec<protocol::RefactoringProblem>,
    #[serde(rename = "finalProblems")]
    pub final_problems: Vec<protocol::RefactoringProblem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feedback: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<protocol::SourceChange>,
    #[serde(rename = "potentialEdits", skip_serializing_if = "Option::is_none")]
    pub potential_edits: Option<Vec<String>>,
}

pub fn get_available_refactorings(
    resolved: Option<&ResolvedUnitRef>,
    offset: i64,
    length: i64,
) -> protocol::EditGetAvailableRefactoringsResult {
    let mut kinds = Vec::new();
    if let Some(resolved) = resolved
        && let (Ok(off), Ok(len)) = (u32::try_from(offset), u32::try_from(length))
    {
        let unit = dartr_server::server::ResolvedUnitRef {
            library: resolved.library.clone(),
            index: resolved.index,
        };
        if ExtractLocal::is_available(&unit, off, len) {
            kinds.push(protocol::RefactoringKind::ExtractLocalVariable);
        }
        if ExtractMethod::is_available(&unit, off, len) {
            kinds.push(protocol::RefactoringKind::ExtractMethod);
        }
        if ExtractWidget::new(&unit, off, len).is_available() {
            kinds.push(protocol::RefactoringKind::ExtractWidget);
        }
        if is_method_to_getter_available_in_unit(&unit, off) {
            kinds.push(protocol::RefactoringKind::ConvertMethodToGetter);
        }
        if element_to_rename_in_unit(&unit, off)
            .and_then(|(target, _, _)| create_rename_for_target(&unit, target))
            .is_some()
        {
            kinds.push(protocol::RefactoringKind::RENAME);
        }
    }
    protocol::EditGetAvailableRefactoringsResult { kinds }
}

#[allow(clippy::too_many_arguments)]
pub fn get_refactoring(
    resolved: Option<ResolvedUnitRef>,
    search_engine: Option<SearchEngine<'_>>,
    kind: protocol::RefactoringKind,
    file: &str,
    offset: i64,
    length: i64,
    validate_only: bool,
    options: Option<&Value>,
) -> std::result::Result<RefactoringResponse, (&'static str, String)> {
    let Some(resolved) = resolved else {
        return Ok(fatal_initial_problem("Unable to create a refactoring"));
    };
    let Ok(off) = u32::try_from(offset.max(0)) else {
        return Ok(fatal_initial_problem("Unable to create a refactoring"));
    };
    let len = u32::try_from(length.max(0)).unwrap_or(0);

    let unit = dartr_server::server::ResolvedUnitRef {
        library: resolved.library.clone(),
        index: resolved.index,
    };

    match kind {
        protocol::RefactoringKind::RENAME => {
            compute_rename_refactoring(&unit, search_engine, off, validate_only, options)
        }
        protocol::RefactoringKind::ExtractLocalVariable => compute_extract_local_refactoring(
            &unit,
            search_engine,
            off,
            len,
            validate_only,
            options,
        ),
        protocol::RefactoringKind::InlineLocalVariable => {
            compute_inline_local_refactoring(&unit, search_engine, off, validate_only)
        }
        protocol::RefactoringKind::ConvertMethodToGetter => {
            compute_convert(&unit, search_engine, off, validate_only, true)
        }
        protocol::RefactoringKind::ConvertGetterToMethod => {
            compute_convert(&unit, search_engine, off, validate_only, false)
        }
        protocol::RefactoringKind::ExtractMethod => compute_extract_method_refactoring(
            &unit,
            search_engine,
            off,
            len,
            validate_only,
            options,
        ),
        protocol::RefactoringKind::ExtractWidget => {
            compute_extract_widget_refactoring(&unit, off, len, validate_only, options)
        }
        protocol::RefactoringKind::InlineMethod => {
            compute_inline_method_refactoring(&unit, search_engine, off, validate_only, options)
        }
        protocol::RefactoringKind::MoveFile => {
            compute_move_file_refactoring(search_engine, file, validate_only, options)
        }
    }
}

fn empty_response() -> RefactoringResponse {
    RefactoringResponse {
        initial_problems: Vec::new(),
        options_problems: Vec::new(),
        final_problems: Vec::new(),
        feedback: None,
        change: None,
        potential_edits: None,
    }
}

fn fatal_initial_problem(message: impl Into<String>) -> RefactoringResponse {
    RefactoringResponse {
        initial_problems: vec![protocol::RefactoringProblem {
            severity: protocol::RefactoringProblemSeverity::FATAL,
            message: message.into(),
            location: None,
        }],
        ..empty_response()
    }
}

fn compute_rename_refactoring(
    unit: &dartr_server::server::ResolvedUnitRef,
    search_engine: Option<SearchEngine<'_>>,
    offset: u32,
    validate_only: bool,
    options: Option<&Value>,
) -> std::result::Result<RefactoringResponse, (&'static str, String)> {
    let Some(mut search_engine) = search_engine else {
        return Ok(fatal_initial_problem("Unable to create a refactoring"));
    };
    let mut engine = Engine {
        host: &mut search_engine,
    };
    let Some((target, name_offset, name_length)) = engine.element_to_rename(unit, offset) else {
        return Ok(fatal_initial_problem("Unable to create a refactoring"));
    };
    let Some(mut rename) = engine.create_rename(unit, target) else {
        return Ok(fatal_initial_problem("Unable to create a refactoring"));
    };
    let mut response = empty_response();
    let initial = engine.check_initial_conditions(&mut rename);
    response.initial_problems = crate::refactoring_host::problems(&initial);
    if initial.has_fatal_error() {
        return Ok(response);
    }
    response.feedback = serde_json::to_value(protocol::RENAMEFeedback {
        offset: if name_offset == u32::MAX {
            -1
        } else {
            name_offset as i64
        },
        length: name_length as i64,
        element_kind_name: engine.element_kind_name(&rename),
        old_name: rename.old_name.clone(),
    })
    .ok();
    let Some(new_name) = options
        .and_then(|o| o.get("newName"))
        .and_then(Value::as_str)
    else {
        return Ok(response);
    };
    rename.new_name = new_name.to_string();
    let options_status = engine.check_new_name(&rename);
    response.options_problems = crate::refactoring_host::problems(&options_status);
    if options_status.has_fatal_error() || validate_only {
        return Ok(response);
    }
    let final_status = engine.check_final_conditions(&mut rename);
    response.final_problems = crate::refactoring_host::problems(&final_status);
    if final_status.has_fatal_error() {
        return Ok(response);
    }
    let change = engine.create_change(&mut rename);
    response.change = Some(crate::refactoring_host::change(&change));
    Ok(response)
}

fn compute_extract_local_refactoring(
    unit: &dartr_server::server::ResolvedUnitRef,
    search_engine: Option<SearchEngine<'_>>,
    offset: u32,
    length: u32,
    validate_only: bool,
    options: Option<&Value>,
) -> std::result::Result<RefactoringResponse, (&'static str, String)> {
    let Some(mut search_engine) = search_engine else {
        return Ok(fatal_initial_problem("Unable to create a refactoring"));
    };
    let mut refactoring = ExtractLocal::new(unit, offset, length);
    let mut response = empty_response();
    let initial = refactoring.check_initial_conditions();
    response.initial_problems = crate::refactoring_host::problems(&initial);
    if initial.has_fatal_error() {
        return Ok(response);
    }
    response.feedback = serde_json::to_value(protocol::ExtractLocalVariableFeedback {
        covering_expression_offsets: Some(
            refactoring
                .covering_expression_offsets
                .iter()
                .map(|&v| v as i64)
                .collect(),
        ),
        covering_expression_lengths: Some(
            refactoring
                .covering_expression_lengths
                .iter()
                .map(|&v| v as i64)
                .collect(),
        ),
        names: refactoring.names.clone(),
        offsets: refactoring.offsets.iter().map(|&v| v as i64).collect(),
        lengths: refactoring.lengths.iter().map(|&v| v as i64).collect(),
    })
    .ok();

    let Some(opts) = options else {
        return Ok(response);
    };
    if let Some(name) = opts.get("name").and_then(Value::as_str) {
        refactoring.name = name.to_string();
    }
    if let Some(extract_all) = opts.get("extractAll").and_then(Value::as_bool) {
        refactoring.extract_all = extract_all;
    }
    let options_status = refactoring.check_name();
    response.options_problems = crate::refactoring_host::problems(&options_status);
    if options_status.has_fatal_error() || validate_only {
        return Ok(response);
    }
    let final_status = refactoring.check_final_conditions();
    response.final_problems = crate::refactoring_host::problems(&final_status);
    if final_status.has_fatal_error() {
        return Ok(response);
    }
    let change = refactoring.create_change(&mut search_engine);
    response.change = Some(crate::refactoring_host::change(&change));
    Ok(response)
}

fn compute_inline_local_refactoring(
    unit: &dartr_server::server::ResolvedUnitRef,
    search_engine: Option<SearchEngine<'_>>,
    offset: u32,
    validate_only: bool,
) -> std::result::Result<RefactoringResponse, (&'static str, String)> {
    let Some(mut search_engine) = search_engine else {
        return Ok(fatal_initial_problem("Unable to create a refactoring"));
    };
    let mut refactoring = InlineLocal::new(unit, offset);
    let mut response = empty_response();
    let initial = refactoring.check_initial_conditions();
    response.initial_problems = crate::refactoring_host::problems(&initial);
    if initial.has_fatal_error() {
        return Ok(response);
    }
    response.feedback = serde_json::to_value(protocol::InlineLocalVariableFeedback {
        name: refactoring.variable_name.clone().unwrap_or_default(),
        occurrences: refactoring.reference_count as i64,
    })
    .ok();
    if validate_only {
        return Ok(response);
    }
    let final_status = refactoring.check_final_conditions();
    response.final_problems = crate::refactoring_host::problems(&final_status);
    if final_status.has_fatal_error() {
        return Ok(response);
    }
    let change = refactoring.create_change(&mut search_engine);
    response.change = Some(crate::refactoring_host::change(&change));
    Ok(response)
}

fn compute_convert(
    unit: &dartr_server::server::ResolvedUnitRef,
    search_engine: Option<SearchEngine<'_>>,
    offset: u32,
    validate_only: bool,
    to_getter: bool,
) -> std::result::Result<RefactoringResponse, (&'static str, String)> {
    let Some(mut search_engine) = search_engine else {
        return Ok(fatal_initial_problem("Unable to create a refactoring"));
    };
    let mut engine = Engine {
        host: &mut search_engine,
    };
    let element = engine.element_at(unit, offset).filter(|e| {
        if to_getter {
            dartr_server::element_locator::is_executable(e.id)
        } else {
            e.id.tag() == Tag::Getter
        }
    });
    let Some(element) = element else {
        return Ok(fatal_initial_problem("Unable to create a refactoring"));
    };
    let mut response = empty_response();
    let initial = if to_getter {
        engine.method_to_getter_initial(&element)
    } else {
        engine.getter_to_method_initial(&element)
    };
    response.initial_problems = crate::refactoring_host::problems(&initial);
    if initial.has_fatal_error() || validate_only {
        return Ok(response);
    }
    let final_status = if to_getter {
        RefactoringStatus::new()
    } else {
        engine.getter_to_method_final(&element)
    };
    response.final_problems = crate::refactoring_host::problems(&final_status);
    if final_status.has_fatal_error() {
        return Ok(response);
    }
    let change = if to_getter {
        engine.method_to_getter_change(&element)
    } else {
        engine.getter_to_method_change(&element)
    };
    response.change = Some(crate::refactoring_host::change(&change));
    Ok(response)
}

fn compute_extract_method_refactoring(
    unit: &dartr_server::server::ResolvedUnitRef,
    search_engine: Option<SearchEngine<'_>>,
    offset: u32,
    length: u32,
    validate_only: bool,
    options: Option<&Value>,
) -> std::result::Result<RefactoringResponse, (&'static str, String)> {
    let Some(mut search_engine) = search_engine else {
        return Ok(fatal_initial_problem("Unable to create a refactoring"));
    };
    let mut refactoring = ExtractMethod::new(unit, offset, length);
    let mut response = empty_response();
    let initial = refactoring.check_initial_conditions();
    response.initial_problems = crate::refactoring_host::problems(&initial);
    if initial.has_fatal_error() {
        return Ok(response);
    }
    let parameters = refactoring
        .parameters
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "kind": p.kind,
                "type": p.r#type,
                "name": p.name,
                "parameters": p.parameters,
            })
        })
        .collect::<Vec<_>>();
    response.feedback = Some(serde_json::json!({
        "offset": refactoring.selection_offset as i64,
        "length": refactoring.selection_length as i64,
        "returnType": refactoring.return_type,
        "names": refactoring.names,
        "canCreateGetter": refactoring.can_create_getter,
        "parameters": parameters,
        "offsets": refactoring.offsets.iter().map(|&v| v as i64).collect::<Vec<_>>(),
        "lengths": refactoring.lengths.iter().map(|&v| v as i64).collect::<Vec<_>>(),
    }));

    let Some(opts) = options else {
        return Ok(response);
    };
    if let Some(return_type) = opts.get("returnType").and_then(Value::as_str) {
        refactoring.return_type = return_type.to_string();
    }
    if let Some(create_getter) = opts.get("createGetter").and_then(Value::as_bool) {
        refactoring.create_getter = create_getter;
    }
    if let Some(name) = opts.get("name").and_then(Value::as_str) {
        refactoring.name = name.to_string();
    }
    if let Some(extract_all) = opts.get("extractAll").and_then(Value::as_bool) {
        refactoring.extract_all = extract_all;
    }
    if let Some(params) = opts.get("parameters").and_then(Value::as_array) {
        refactoring.parameters = params
            .iter()
            .filter_map(|p| {
                let kind = match p.get("kind").and_then(Value::as_str)? {
                    "POSITIONAL" => "POSITIONAL",
                    "NAMED" => "NAMED",
                    _ => "REQUIRED",
                };
                Some(MethodParameter {
                    id: p
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    kind,
                    r#type: p.get("type")?.as_str()?.to_string(),
                    name: p.get("name")?.as_str()?.to_string(),
                    parameters: p
                        .get("parameters")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                })
            })
            .collect();
    }

    let options_status = refactoring.check_name();
    response.options_problems = crate::refactoring_host::problems(&options_status);
    if options_status.has_fatal_error() || validate_only {
        return Ok(response);
    }
    let final_status = refactoring.check_final_conditions();
    response.final_problems = crate::refactoring_host::problems(&final_status);
    if final_status.has_fatal_error() {
        return Ok(response);
    }
    let change = refactoring.create_change(&mut search_engine);
    response.change = Some(crate::refactoring_host::change(&change));
    Ok(response)
}

fn compute_extract_widget_refactoring(
    unit: &dartr_server::server::ResolvedUnitRef,
    offset: u32,
    length: u32,
    validate_only: bool,
    options: Option<&Value>,
) -> std::result::Result<RefactoringResponse, (&'static str, String)> {
    let mut refactoring = ExtractWidget::new(unit, offset, length);
    let mut response = empty_response();
    let initial = refactoring.check_initial_conditions();
    response.initial_problems = crate::refactoring_host::problems(&initial);
    if initial.has_fatal_error() {
        return Ok(response);
    }
    response.feedback = Some(serde_json::json!({}));

    let Some(opts) = options else {
        return Ok(response);
    };
    if let Some(name) = opts.get("name").and_then(Value::as_str) {
        refactoring.name = name.to_string();
    }
    let options_status = refactoring.check_name();
    response.options_problems = crate::refactoring_host::problems(&options_status);
    if options_status.has_fatal_error() || validate_only {
        return Ok(response);
    }
    let final_status = refactoring.check_final_conditions();
    response.final_problems = crate::refactoring_host::problems(&final_status);
    if final_status.has_fatal_error() {
        return Ok(response);
    }
    let change = refactoring.create_change();
    response.change = Some(crate::refactoring_host::change(&change));
    Ok(response)
}

fn compute_inline_method_refactoring(
    unit: &dartr_server::server::ResolvedUnitRef,
    search_engine: Option<SearchEngine<'_>>,
    offset: u32,
    validate_only: bool,
    options: Option<&Value>,
) -> std::result::Result<RefactoringResponse, (&'static str, String)> {
    let Some(mut search_engine) = search_engine else {
        return Ok(fatal_initial_problem("Unable to create a refactoring"));
    };
    let mut refactoring = InlineMethod::new(unit, offset);
    let mut response = empty_response();
    let initial = refactoring.check_initial_conditions(&mut search_engine);
    response.initial_problems = crate::refactoring_host::problems(&initial);
    if initial.has_fatal_error() {
        return Ok(response);
    }
    response.feedback = serde_json::to_value(protocol::InlineMethodFeedback {
        class_name: refactoring.class_name.clone(),
        method_name: refactoring.method_name.clone().unwrap_or_default(),
        is_declaration: refactoring.is_declaration,
    })
    .ok();

    let Some(opts) = options else {
        return Ok(response);
    };
    if let Some(delete_source) = opts.get("deleteSource").and_then(Value::as_bool) {
        refactoring.delete_source = delete_source;
    }
    if let Some(inline_all) = opts.get("inlineAll").and_then(Value::as_bool) {
        refactoring.inline_all = inline_all;
    }
    if validate_only {
        return Ok(response);
    }
    let final_status = refactoring.check_final_conditions(&mut search_engine);
    response.final_problems = crate::refactoring_host::problems(&final_status);
    if final_status.has_fatal_error() {
        return Ok(response);
    }
    let change = refactoring.create_change();
    response.change = Some(crate::refactoring_host::change(&change));
    Ok(response)
}

fn compute_move_file_refactoring(
    search_engine: Option<SearchEngine<'_>>,
    file: &str,
    validate_only: bool,
    options: Option<&Value>,
) -> std::result::Result<RefactoringResponse, (&'static str, String)> {
    let Some(mut search_engine) = search_engine else {
        return Ok(fatal_initial_problem("Unable to create a refactoring"));
    };
    let Some(new_file) = options
        .and_then(|o| o.get("newFile"))
        .and_then(Value::as_str)
    else {
        return Ok(empty_response());
    };
    let refactoring = MoveFile::new(file, new_file);
    let mut response = empty_response();
    let initial = refactoring.check_initial_conditions();
    response.initial_problems = crate::refactoring_host::problems(&initial);
    if initial.has_fatal_error() || validate_only {
        return Ok(response);
    }
    let final_status = refactoring.check_final_conditions(&mut search_engine);
    response.final_problems = crate::refactoring_host::problems(&final_status);
    if final_status.has_fatal_error() {
        return Ok(response);
    }
    let change = refactoring.create_change(&mut search_engine);
    response.change = Some(crate::refactoring_host::change(&change));
    Ok(response)
}
