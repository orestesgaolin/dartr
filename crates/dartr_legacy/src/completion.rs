// Dart source: pkg/analysis_server/lib/src/handler/legacy/completion_get_suggestions2.dart
// Dart source: pkg/analysis_server/lib/src/handler/legacy/completion_get_suggestion_details2.dart
// Dart source: pkg/analysis_server/lib/src/services/completion/dart/completion_utils.dart
// Dart source: pkg/analysis_server/lib/src/services/completion/yaml/yaml_completion_generator.dart
// Dart source: pkg/analysis_server/lib/src/services/completion/yaml/analysis_options_generator.dart
// Dart source: pkg/analysis_server/lib/src/services/completion/yaml/fix_data_generator.dart
// Dart source: pkg/analysis_server/lib/src/services/completion/yaml/pubspec_generator.dart

use std::collections::HashMap;

use dartr_ast::{self as ast, Ast};
use dartr_element::display_string::{DisplayOptions, default_value_code, type_display_string_with};
use dartr_element::{
    AnyElement, Ctx, EId, ElemRef, ElementId, FormalParameterElement, ParameterKind, Tag,
    TypeAliasElement, TypeId, TypeKind,
};
use dartr_project::fs;
use dartr_server::completion::candidate::{Candidate, Kind, SuggestionKind, display_name};
use dartr_server::completion::lsp::type_display;
use dartr_server::completion::{self as c, elem};
use dartr_server::hover::documentation;
use dartr_typesystem::{TypeExt, member};

use crate::convert::convert_element;
use crate::protocol::{
    CompletionGetSuggestions2Result, CompletionSuggestion, CompletionSuggestionKind, Element,
    ElementKind, Location,
};

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Converts a `Candidate` to a legacy `protocol::CompletionSuggestion`,
/// following `candidateToCompletionSuggestion` in `completion_utils.dart`.
pub fn candidate_to_completion_suggestion(
    q: &c::Request<'_, '_>,
    mut candidate: Candidate,
    templates: &HashMap<String, String>,
) -> Option<CompletionSuggestion> {
    let ctx = q.ctx;
    if candidate.typed().is_some() {
        let data = c::overrides::typed_data(q, &candidate);
        if let Some(t) = candidate.typed_mut() {
            t.data = data;
        }
    }
    if let Kind::Override { .. } = &candidate.kind {
        let data = c::overrides::override_data(q, &candidate)?;
        let Kind::Override { element, .. } = &mut candidate.kind else {
            unreachable!()
        };
        let element = *element;
        let base = member::base_element(ctx, element);
        let completion = data.completion.clone();
        if completion.is_empty() {
            return None;
        }
        let (doc_complete, doc_summary) = get_documentation(
            ctx,
            element,
            candidate.referencing_interface().flatten(),
            templates,
        );
        let is_not_imported = if candidate.is_not_imported() {
            Some(true)
        } else {
            None
        };
        let library_uri = candidate
            .import_data
            .as_ref()
            .map(|d| d.library_uri.clone());
        return Some(CompletionSuggestion {
            kind: CompletionSuggestionKind::OVERRIDE,
            relevance: candidate.relevance as i64,
            completion: completion.clone(),
            display_text: Some(data.display_text),
            replacement_offset: None,
            replacement_length: None,
            selection_offset: data
                .selection_offset
                .map(|o| o as i64)
                .unwrap_or_else(|| utf16_len(&completion) as i64),
            selection_length: data.selection_length.unwrap_or(0) as i64,
            is_deprecated: elem::is_deprecated(ctx, base),
            is_potential: false,
            doc_summary,
            doc_complete,
            declaring_type: get_declaring_type(ctx, &candidate, element),
            default_argument_list_string: None,
            default_argument_list_text_ranges: None,
            element: Some(convert_elem_ref(ctx, element)),
            return_type: get_return_type(ctx, &candidate),
            parameter_names: None,
            parameter_types: None,
            required_parameter_count: None,
            has_named_parameters: None,
            parameter_name: None,
            parameter_type: None,
            library_uri,
            is_not_imported,
        });
    }

    let completion = candidate.completion(ctx);
    if completion.is_empty() {
        return None;
    }

    let is_not_imported = if candidate.is_importable() && candidate.is_not_imported() {
        Some(true)
    } else {
        None
    };
    let library_uri = if candidate.is_importable() {
        candidate
            .import_data
            .as_ref()
            .map(|d| d.library_uri.clone())
    } else {
        None
    };

    let mut kind = get_suggestion_kind(q, &candidate);
    let mut display_text: Option<String> = None;
    let mut selection_offset = utf16_len(&completion) as i64;
    let selection_length: i64 = 0;
    let mut replacement_length: Option<i64> = None;
    let mut is_deprecated = false;
    let mut doc_complete: Option<String> = None;
    let mut doc_summary: Option<String> = None;
    let mut declaring_type: Option<String> = None;
    let mut default_argument_list_string: Option<String> = None;
    let mut default_argument_list_text_ranges: Option<Vec<i64>> = None;
    let mut proto_element: Option<Element> = None;
    let return_type = get_return_type(ctx, &candidate);
    let mut parameter_names: Option<Vec<String>> = None;
    let mut parameter_types: Option<Vec<String>> = None;
    let mut required_parameter_count: Option<i64> = None;
    let mut has_named_parameters: Option<bool> = None;
    let mut parameter_name: Option<String> = None;
    let mut parameter_type: Option<String> = None;

    if let Some(typed) = candidate.typed() {
        if let Some(data) = &typed.data {
            display_text = Some(data.display_text.clone());
        }
    }

    match &candidate.kind {
        Kind::Closure { .. } => {
            if let Some(sd) = candidate.suggestion_data(ctx) {
                display_text = Some(sd.display_text);
                selection_offset = sd.selection_offset as i64;
            }
        }
        Kind::FunctionCall { ty, .. } => {
            let (p_names, p_types, req_count, has_named) =
                function_type_parameter_details(ctx, *ty);
            let ret_str = match ctx.ty(*ty) {
                TypeKind::Function(f) => type_display(ctx, f.ret),
                _ => "dynamic".to_string(),
            };
            let type_params_str = if p_types.is_empty() {
                None
            } else {
                Some(format!("<{}>", p_types.join(", ")))
            };
            proto_element = Some(Element {
                kind: ElementKind::METHOD,
                name: "call".to_string(),
                location: None,
                flags: 0,
                parameters: Some(format!("({})", p_names.join(","))),
                return_type: Some(ret_str),
                type_parameters: type_params_str,
                aliased_type: None,
                extended_type: None,
            });
            parameter_names = Some(p_names);
            parameter_types = Some(p_types);
            required_parameter_count = Some(req_count);
            has_named_parameters = Some(has_named);
        }
        Kind::Keyword {
            selection_offset: kw_sel,
            ..
        } => {
            selection_offset = *kw_sel as i64;
        }
        Kind::Label(name) => {
            proto_element = Some(create_local_suggestion_element(
                q.ast,
                q.path,
                q.target.containing_node,
                name,
            ));
        }
        Kind::NamedArgument {
            parameter,
            replacement_length: rep_len,
            ..
        } => {
            if let Some(sd) = candidate.suggestion_data(ctx) {
                selection_offset = sd.selection_offset as i64;
            }
            let base = member::base_element(ctx, *parameter);
            is_deprecated = elem::is_deprecated(ctx, base);
            parameter_name = Some(display_name(ctx, base));
            parameter_type = Some(type_display(ctx, member::type_(ctx, *parameter)));
            let (dc, ds) = get_documentation(ctx, *parameter, None, templates);
            doc_complete = dc;
            doc_summary = ds;
            proto_element = Some(convert_elem_ref(ctx, *parameter));
            replacement_length = rep_len.map(|l| l as i64);
        }
        Kind::RecordField { name, .. } => {
            if display_text.is_none() {
                display_text = Some(name.clone());
            }
        }
        Kind::RecordLiteralNamedField {
            name, field_type, ..
        } => {
            if let Some(sd) = candidate.suggestion_data(ctx) {
                selection_offset = sd.selection_offset as i64;
            }
            parameter_name = Some(name.clone());
            parameter_type = Some(type_display(ctx, *field_type));
        }
        Kind::SetState { .. } => {
            if let Some(sd) = candidate.suggestion_data(ctx) {
                selection_offset = sd.selection_offset as i64;
                if display_text.is_none() {
                    display_text = Some(sd.display_text);
                }
            }
        }
        Kind::Uri(_) => {}
        _ => {}
    }

    if let Some(element) = candidate.element() {
        let base = member::base_element(ctx, element);
        is_deprecated = elem::is_deprecated(ctx, base);
        declaring_type = get_declaring_type(ctx, &candidate, element);
        let (dc, ds) = get_documentation(
            ctx,
            element,
            candidate.referencing_interface().flatten(),
            templates,
        );
        doc_complete = dc;
        doc_summary = ds;
        if proto_element.is_none() {
            proto_element = Some(convert_elem_ref(ctx, element));
        }
        if dartr_server::element_locator::is_executable(base)
            && !matches!(base.tag(), Tag::Getter | Tag::Setter)
        {
            let params = member::formal_parameters(ctx, element);
            let mut p_names = Vec::with_capacity(params.len());
            let mut p_types = Vec::with_capacity(params.len());
            let mut req_count: i64 = 0;
            let mut has_named = false;
            for p in &params {
                let p_base = member::base_element(ctx, *p);
                p_names.push(display_name(ctx, p_base));
                p_types.push(type_display(ctx, member::type_(ctx, *p)));
                let p_kind = elem::parameter_kind(ctx, *p);
                if p_kind == ParameterKind::Required {
                    req_count += 1;
                }
                if p_kind.is_named() {
                    has_named = true;
                }
            }
            parameter_names = Some(p_names);
            parameter_types = Some(p_types);
            required_parameter_count = Some(req_count);
            has_named_parameters = Some(has_named);
            let default_args = compute_default_argument_list(ctx, &params, q.style.quote);
            default_argument_list_string = default_args.text;
            default_argument_list_text_ranges = default_args.ranges;
        }
    }

    if let Kind::Method { .. } = &candidate.kind {
        if q.target.is_functional_argument(q.ast, q.ctx, q.tables) {
            kind = CompletionSuggestionKind::IDENTIFIER;
        }
    }

    let relevance = if matches!(candidate.kind, Kind::RecordLiteralNamedField { .. }) {
        1000
    } else {
        candidate.relevance as i64
    };

    Some(CompletionSuggestion {
        kind,
        relevance,
        completion,
        display_text,
        replacement_offset: None,
        replacement_length,
        selection_offset,
        selection_length,
        is_deprecated,
        is_potential: false,
        doc_summary,
        doc_complete,
        declaring_type,
        default_argument_list_string,
        default_argument_list_text_ranges,
        element: proto_element,
        return_type,
        parameter_names,
        parameter_types,
        required_parameter_count,
        has_named_parameters,
        parameter_name,
        parameter_type,
        library_uri,
        is_not_imported,
    })
}

fn map_suggestion_kind(sk: SuggestionKind) -> CompletionSuggestionKind {
    match sk {
        SuggestionKind::Identifier => CompletionSuggestionKind::IDENTIFIER,
        SuggestionKind::Invocation => CompletionSuggestionKind::INVOCATION,
    }
}

fn get_suggestion_kind(_q: &c::Request<'_, '_>, candidate: &Candidate) -> CompletionSuggestionKind {
    match &candidate.kind {
        Kind::Class(_)
        | Kind::Enum(_)
        | Kind::EnumConstant { .. }
        | Kind::ExtensionType(_)
        | Kind::Field { .. }
        | Kind::FormalParameter { .. }
        | Kind::Getter { .. }
        | Kind::Identifier { .. }
        | Kind::ImportPrefix { .. }
        | Kind::Label(_)
        | Kind::LocalVariable { .. }
        | Kind::Mixin(_)
        | Kind::Name(_)
        | Kind::RecordField { .. }
        | Kind::Setter { .. }
        | Kind::StaticField(_)
        | Kind::SuperParameter(_)
        | Kind::TopLevelGetter(_)
        | Kind::TopLevelSetter(_)
        | Kind::TopLevelVariable(_)
        | Kind::TypeAlias(_)
        | Kind::TypeParameter(_) => CompletionSuggestionKind::IDENTIFIER,
        Kind::Closure { .. } | Kind::LoadLibrary { .. } => CompletionSuggestionKind::INVOCATION,
        Kind::Constructor {
            kind,
            is_tear_off,
            is_redirect,
            ..
        } => {
            if *is_tear_off || *is_redirect {
                CompletionSuggestionKind::IDENTIFIER
            } else {
                map_suggestion_kind(*kind)
            }
        }
        Kind::Extension { kind, .. }
        | Kind::FunctionCall { kind, .. }
        | Kind::LocalFunction { kind, .. }
        | Kind::Method { kind, .. }
        | Kind::SetState { kind, .. }
        | Kind::TopLevelFunction { kind, .. } => map_suggestion_kind(*kind),
        Kind::Keyword { .. } => CompletionSuggestionKind::KEYWORD,
        Kind::NamedArgument { .. } | Kind::RecordLiteralNamedField { .. } => {
            CompletionSuggestionKind::NamedArgument
        }
        Kind::Override { .. } => CompletionSuggestionKind::OVERRIDE,
        Kind::Uri(_) => CompletionSuggestionKind::IMPORT,
    }
}

/// Follows `_getDeclaringType` in `completion_utils.dart`.
fn get_declaring_type(ctx: &Ctx<'_>, _candidate: &Candidate, element: ElemRef) -> Option<String> {
    let base = member::base_element(ctx, element);
    if matches!(
        base.tag(),
        Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter
    ) {
        return None;
    }
    let enclosing = elem::enclosing(ctx, base)?;
    if matches!(
        enclosing.tag(),
        Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType
    ) {
        return Some(display_name(ctx, enclosing));
    }
    None
}

/// Follows `_getReturnType` in `completion_utils.dart`.
fn get_return_type(ctx: &Ctx<'_>, candidate: &Candidate) -> Option<String> {
    if let Kind::RecordField { field_type, .. } = &candidate.kind {
        return Some(type_display(ctx, *field_type));
    }
    if let Kind::FunctionCall { ty, .. } = &candidate.kind {
        if let TypeKind::Function(f) = ctx.ty(*ty) {
            return Some(type_display(ctx, f.ret));
        }
    }
    let element = candidate.element()?;
    let base = member::base_element(ctx, element);
    match base.tag() {
        Tag::Getter | Tag::Method | Tag::TopLevelFunction | Tag::LocalFunction => {
            Some(type_display(ctx, member::return_type(ctx, element)))
        }
        Tag::Setter => {
            let params = member::formal_parameters(ctx, element);
            let first = params.first()?;
            Some(type_display(ctx, member::type_(ctx, *first)))
        }
        Tag::Field
        | Tag::TopLevelVariable
        | Tag::LocalVariable
        | Tag::PatternVariable
        | Tag::BindPatternVariable
        | Tag::JoinPatternVariable
        | Tag::FormalParameter
        | Tag::FieldFormalParameter
        | Tag::SuperFormalParameter => Some(type_display(ctx, member::type_(ctx, element))),
        Tag::TypeAlias => {
            let ta = base.cast::<TypeAliasElement>()?;
            let aliased = ctx.get(ta).aliased_type.get()?;
            if let TypeKind::Function(f) = ctx.ty(aliased) {
                Some(type_display(ctx, f.ret))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Converts an `ElemRef` to a `protocol::Element`, preserving member type substitution
/// on parameters and returnType (matching `convertElement` in `protocol_dart.dart`).
pub fn convert_elem_ref(ctx: &Ctx<'_>, elem_ref: ElemRef) -> Element {
    let base = member::base_element(ctx, elem_ref);
    let mut el = convert_element(ctx, base, None);
    if matches!(elem_ref, ElemRef::Member(_)) {
        if dartr_server::element_locator::is_executable(base) {
            if !(base.tag() == Tag::Getter && member::formal_parameters(ctx, elem_ref).is_empty()) {
                el.parameters = Some(substituted_parameters_string(ctx, elem_ref));
            }
            if base.tag() != Tag::Setter {
                el.return_type = Some(type_display(ctx, member::return_type(ctx, elem_ref)));
            }
        } else if base.is::<dartr_element::VariableElement>() {
            el.return_type = Some(type_display(ctx, member::type_(ctx, elem_ref)));
        }
    }
    if base.tag() == Tag::Constructor {
        // In protocol_dart.dart, convertElement sets returnType from constructor's enclosing Element displayName.
        if let AnyElement::Constructor(ctor) = ctx.any(base)
            && let Some(enc) = ctor.enclosing
        {
            el.return_type = Some(display_name(ctx, enc));
        }
    }
    el
}

fn substituted_parameters_string(ctx: &Ctx<'_>, element: ElemRef) -> String {
    struct Entry {
        kind: ParameterKind,
        has_req: bool,
        ty: TypeId,
        name: String,
        default_code: Option<String>,
    }
    let mut entries: Vec<Entry> = member::formal_parameters(ctx, element)
        .into_iter()
        .map(|p| {
            let base = member::base_element(ctx, p);
            let fp = EId::<FormalParameterElement>::from_raw(base);
            let name = ctx
                .element_name(base)
                .filter(|s| !s.is_empty())
                .unwrap_or("<unnamed>")
                .to_string();
            Entry {
                kind: ctx.get(fp).kind,
                has_req: elem::has_required(ctx, base),
                ty: member::type_(ctx, p),
                name,
                default_code: default_value_code(ctx, fp),
            }
        })
        .collect();
    let rank = |p: &Entry| -> i64 {
        if p.kind.is_required_named() || p.has_req {
            0
        } else if !p.kind.is_named() {
            -1
        } else {
            1
        }
    };
    entries = c::dart_sort_vec(entries, |a, b| rank(a) - rank(b));
    let mut sb = String::new();
    let mut close = "";
    for p in &entries {
        if !sb.is_empty() {
            sb.push_str(", ");
        }
        if close.is_empty() {
            if p.kind.is_named() {
                sb.push('{');
                close = "}";
            } else if p.kind.is_optional_positional() {
                sb.push('[');
                close = "]";
            }
        }
        if p.kind.is_required_named() {
            sb.push_str("required ");
        } else if p.has_req {
            sb.push_str("@required ");
        }
        sb.push_str(&type_display(ctx, p.ty));
        if !p.name.is_empty() && p.name != "<unnamed>" {
            sb.push(' ');
            sb.push_str(&p.name);
        }
        if let Some(code) = &p.default_code {
            sb.push_str(" = ");
            sb.push_str(code);
        }
    }
    sb.push_str(close);
    format!("({sb})")
}

fn function_type_parameter_details(
    ctx: &Ctx<'_>,
    ty: TypeId,
) -> (Vec<String>, Vec<String>, i64, bool) {
    let TypeKind::Function(f) = ctx.ty(ty) else {
        return (Vec::new(), Vec::new(), 0, false);
    };
    let params = ctx.list(f.params);
    let mut names = Vec::with_capacity(params.len());
    let mut types = Vec::with_capacity(params.len());
    let mut req_count = 0i64;
    let mut has_named = false;
    for p in params {
        names.push(
            p.name
                .map(|n| ctx.name_str(n).to_string())
                .unwrap_or_default(),
        );
        types.push(type_display(ctx, p.ty));
        if p.kind == ParameterKind::Required {
            req_count += 1;
        }
        if p.kind.is_named() {
            has_named = true;
        }
    }
    (names, types, req_count, has_named)
}

struct DefaultArgList {
    text: Option<String>,
    ranges: Option<Vec<i64>>,
}

/// Follows `computeDefaultArgumentList` in `completion_utils.dart`.
fn compute_default_argument_list(ctx: &Ctx<'_>, params: &[ElemRef], quote: char) -> DefaultArgList {
    let mut sb = String::new();
    let mut ranges = Vec::new();
    for param in params {
        let base = member::base_element(ctx, *param);
        let kind = elem::parameter_kind(ctx, *param);
        if kind == ParameterKind::Required {
            if !sb.is_empty() {
                sb.push_str(", ");
            }
            let name = display_name(ctx, base);
            let start = utf16_len(&sb) as i64;
            sb.push_str(&name);
            let len = utf16_len(&name) as i64;
            ranges.push(start);
            ranges.push(len);
        } else if kind.is_named() {
            let has_req = elem::has_required(ctx, base) || kind.is_required_named();
            if has_req {
                if !sb.is_empty() {
                    sb.push_str(", ");
                }
                let name = display_name(ctx, base);
                sb.push_str(&name);
                sb.push_str(": ");
                let ty = member::type_(ctx, *param);
                if let Some((val_text, val_sel)) = default_argument_value_for_type(ctx, ty, quote) {
                    let offset = utf16_len(&sb) as i64;
                    sb.push_str(&val_text);
                    if let Some(sel) = val_sel {
                        ranges.push(offset + sel as i64);
                        ranges.push(0);
                    }
                } else {
                    let start = utf16_len(&sb) as i64;
                    sb.push_str(&name);
                    let len = utf16_len(&name) as i64;
                    ranges.push(start);
                    ranges.push(len);
                }
            }
        }
    }
    if sb.is_empty() {
        DefaultArgList {
            text: None,
            ranges: None,
        }
    } else {
        DefaultArgList {
            text: Some(sb),
            ranges: if ranges.is_empty() {
                None
            } else {
                Some(ranges)
            },
        }
    }
}

fn default_argument_value_for_type(
    ctx: &Ctx<'_>,
    ty: TypeId,
    quote: char,
) -> Option<(String, Option<usize>)> {
    match ctx.ty(ty) {
        TypeKind::Interface { element, .. } => {
            let e = element.raw();
            if ctx.is_element(e, "dart.core", "List") {
                Some(("[]".to_string(), Some(1)))
            } else if ctx.is_element(e, "dart.core", "Map") {
                Some(("{}".to_string(), Some(1)))
            } else if ctx.is_element(e, "dart.core", "String") {
                Some((format!("{quote}{quote}"), Some(1)))
            } else {
                None
            }
        }
        TypeKind::Function(f) => {
            let params: Vec<String> = ctx
                .list(f.params)
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let name = p
                        .name
                        .map(|n| ctx.name_str(n).to_string())
                        .unwrap_or_else(|| format!("p{}", i + 1));
                    let prefix = if matches!(ctx.ty(p.ty), TypeKind::Dynamic) {
                        String::new()
                    } else {
                        format!(
                            "{} ",
                            type_display_string_with(ctx, p.ty, DisplayOptions::default())
                        )
                    };
                    format!("{prefix}{name}")
                })
                .collect();
            let text = format!("({}) {{  }}", params.join(", "));
            let len = utf16_len(&text);
            Some((text, Some(len - 2)))
        }
        _ => None,
    }
}

/// Follows `_getDocumentation` in `completion_utils.dart` and `DartDocumentationComputer.compute`.
fn get_documentation(
    ctx: &Ctx<'_>,
    element: ElemRef,
    _referencing_interface: Option<ElementId>,
    templates: &HashMap<String, String>,
) -> (Option<String>, Option<String>) {
    let base = member::base_element(ctx, element);
    let target = if matches!(
        base.tag(),
        Tag::FieldFormalParameter | Tag::SuperFormalParameter
    ) && let AnyElement::FormalParameter(fp) = ctx.any(base)
        && let Some(field) = fp.field.get()
    {
        field.raw()
    } else {
        base
    };
    let Some(full) = documentation(ctx, target, templates) else {
        return (None, None);
    };
    let summary = extract_doc_summary(&full);
    (Some(full), summary)
}

fn extract_doc_summary(full: &str) -> Option<String> {
    if full.is_empty() {
        return Some(String::new());
    }
    let lines: Vec<&str> = full.split('\n').collect();
    let mut count = 0;
    for (i, line) in lines.iter().enumerate() {
        if line.trim().is_empty() {
            if count > 0 {
                return Some(lines[..i].join("\n"));
            }
        } else {
            count += 1;
        }
    }
    Some(full.to_string())
}

/// Follows `_createLocalSuggestionElement` in `completion_utils.dart`.
fn create_local_suggestion_element(
    ast: &Ast,
    path: &str,
    containing_node: ast::NodeId,
    label_name: &str,
) -> Element {
    let mut offset = 0i64;
    let mut length = 0i64;
    let mut cur = Some(containing_node);
    'outer: while let Some(n) = cur {
        if let Some(labeled) = ast.cast::<ast::LabeledStatement>(n) {
            for &lbl in ast.list(ast[labeled].labels) {
                let tok = ast[lbl].name;
                if ast.tokens.lexeme(tok) == label_name {
                    let t = ast.tokens.get(tok);
                    offset = t.offset as i64;
                    length = t.length as i64;
                    break 'outer;
                }
            }
        } else if let Some(sw) = ast.cast::<ast::SwitchStatement>(n) {
            for &member_id in ast.list(ast[sw].members) {
                let labels = if let Some(c) = ast.cast::<ast::SwitchCase>(member_id.raw()) {
                    ast[c].labels
                } else if let Some(d) = ast.cast::<ast::SwitchDefault>(member_id.raw()) {
                    ast[d].labels
                } else if let Some(pc) = ast.cast::<ast::SwitchPatternCase>(member_id.raw()) {
                    ast[pc].labels
                } else {
                    continue;
                };
                for &lbl in ast.list(labels) {
                    let tok = ast[lbl].name;
                    if ast.tokens.lexeme(tok) == label_name {
                        let t = ast.tokens.get(tok);
                        offset = t.offset as i64;
                        length = t.length as i64;
                        break 'outer;
                    }
                }
            }
        }
        cur = ast.parent(n);
    }
    Element {
        kind: ElementKind::LABEL,
        name: label_name.to_string(),
        location: Some(Location {
            file: path.to_string(),
            offset,
            length,
            start_line: 0,
            start_column: 0,
            end_line: Some(0),
            end_column: Some(0),
        }),
        flags: 0,
        parameters: None,
        return_type: None,
        type_parameters: None,
        aliased_type: None,
        extended_type: None,
    }
}

// ---------------------------------------------------------------------------
// YAML Completion (analysis_options.yaml, fix_data.yaml, pubspec.yaml)
// ---------------------------------------------------------------------------

/// Computes YAML completions for `analysis_options.yaml`, `fix_data.yaml`, and `pubspec.yaml`,
/// following `YamlCompletionGenerator.getSuggestions`.
pub fn compute_yaml_suggestions(file_path: &str, offset: i64) -> CompletionGetSuggestions2Result {
    let Some(kind) = c::yaml::YamlFileKind::from_path(file_path) else {
        return CompletionGetSuggestions2Result {
            replacement_offset: offset,
            replacement_length: 0,
            suggestions: Vec::new(),
            is_incomplete: false,
        };
    };

    let Some(content) = fs::read_string(file_path) else {
        return CompletionGetSuggestions2Result {
            replacement_offset: 0,
            replacement_length: 0,
            suggestions: Vec::new(),
            is_incomplete: false,
        };
    };

    let results = c::yaml::compute_yaml_suggestions_with_content(
        kind,
        file_path,
        &content,
        offset.max(0) as usize,
    );
    let suggestions = results
        .suggestions
        .into_iter()
        .map(|s| CompletionSuggestion {
            kind: match s.kind {
                c::yaml::YamlSuggestionKind::Identifier => CompletionSuggestionKind::IDENTIFIER,
                c::yaml::YamlSuggestionKind::PackageName => CompletionSuggestionKind::PackageName,
            },
            relevance: s.relevance,
            completion: s.completion,
            display_text: None,
            replacement_offset: None,
            replacement_length: None,
            selection_offset: s.selection_offset as i64,
            selection_length: s.selection_length as i64,
            is_deprecated: false,
            is_potential: false,
            doc_summary: None,
            doc_complete: s.doc_complete,
            declaring_type: None,
            default_argument_list_string: None,
            default_argument_list_text_ranges: None,
            element: None,
            return_type: None,
            parameter_names: None,
            parameter_types: None,
            required_parameter_count: None,
            has_named_parameters: None,
            parameter_name: None,
            parameter_type: None,
            library_uri: None,
            is_not_imported: None,
        })
        .collect();

    CompletionGetSuggestions2Result {
        replacement_offset: results.replacement_offset as i64,
        replacement_length: results.replacement_length as i64,
        suggestions,
        is_incomplete: false,
    }
}
