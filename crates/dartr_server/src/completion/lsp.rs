// Dart source: pkg/analysis_server/lib/src/lsp/completion_utils.dart
// Dart source: pkg/analysis_server/lib/src/lsp/mapping.dart (buildInsertText, relevanceToSortText)
// Dart source: pkg/analysis_server/lib/plugin/protocol/protocol_dart.dart (getParametersString)
// Dart source: pkg/analysis_server/lib/src/protocol_server.dart (getReturnTypeString)
// Dart source: pkg/analyzer_plugin/lib/src/utilities/documentation.dart (removeDartDocDelimiters)

//! The LSP completion items of the candidates (Dart `toLspCompletionItem`).

use dartr_element::display_string::{DisplayOptions, default_value_code, type_display_string_with};
use dartr_element::{Ctx, EId, ElemRef, ElementId, FormalParameterElement, Tag, TypeId, TypeKind};
#[allow(unused_imports)]
use dartr_typesystem::TypeExt;
use dartr_typesystem::member;
use serde_json::{Map, Value, json};

use super::candidate::{
    Candidate, Kind, SuggestionKind, display_name, is_enum_constant, utf16_len,
};
use super::elem;

/// Dart `type.getDisplayString()`.
pub fn type_display(ctx: &Ctx<'_>, ty: TypeId) -> String {
    type_display_string_with(ctx, ty, DisplayOptions::default())
}

/// The client capabilities that the mapping reads.
#[derive(Clone, Debug, Default)]
pub struct ItemCapabilities {
    pub snippets: bool,
    pub insert_replace: bool,
    pub deprecated_flag: bool,
    pub deprecated_tag: bool,
    pub label_details: bool,
    pub as_is_insert_mode: bool,
    pub item_kinds: Vec<i64>,
    /// `None` when the client sends no documentation formats.
    pub documentation_formats: Option<Vec<String>>,
    pub default_edit_range: bool,
    pub default_text_mode: bool,
    pub default_data: bool,
}

/// One parameter for the strings of the signature.
struct ParamInfo {
    name: String,
    kind: dartr_element::ParameterKind,
    ty: TypeId,
    default_code: Option<String>,
    has_required: bool,
    /// Dart `metadata.hasDeprecated`.
    has_deprecated: bool,
}

/// Dart `FormalParameterElement.displayName`: the name, or `<unnamed>`.
fn parameter_display_name(name: Option<&str>) -> String {
    match name {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => "<unnamed>".to_string(),
    }
}

fn element_params(ctx: &Ctx<'_>, element: ElemRef) -> Vec<ParamInfo> {
    member::formal_parameters(ctx, element)
        .into_iter()
        .map(|p| {
            let base = member::base_element(ctx, p);
            let fp = EId::<FormalParameterElement>::from_raw(base);
            ParamInfo {
                name: parameter_display_name(ctx.element_name(base)),
                kind: ctx.get(fp).kind,
                ty: member::type_(ctx, p),
                default_code: default_value_code(ctx, fp),
                has_required: elem::has_required(ctx, base),
                has_deprecated: elem::metadata_has(
                    ctx,
                    base,
                    dartr_resolver::element_metadata::flags::DEPRECATED,
                ),
            }
        })
        .collect()
}

fn function_type_params(ctx: &Ctx<'_>, ty: TypeId) -> Vec<ParamInfo> {
    let TypeKind::Function(f) = ctx.ty(ty) else {
        return Vec::new();
    };
    ctx.list(f.params)
        .iter()
        .map(|p| {
            let base = p.element.map(|e| member::base_element(ctx, e));
            let fp = base
                .filter(|b| b.cast::<FormalParameterElement>().is_some())
                .map(EId::<FormalParameterElement>::from_raw);
            ParamInfo {
                name: parameter_display_name(p.name.map(|n| ctx.name_str(n))),
                kind: p.kind,
                ty: p.ty,
                default_code: fp.and_then(|fp| default_value_code(ctx, fp)),
                has_required: base.is_some_and(|b| elem::has_required(ctx, b)),
                has_deprecated: base.is_some_and(|b| {
                    elem::metadata_has(ctx, b, dartr_resolver::element_metadata::flags::DEPRECATED)
                }),
            }
        })
        .collect()
}

/// Dart `getParametersListString`.
fn parameters_list_string(ctx: &Ctx<'_>, mut parameters: Vec<ParamInfo>) -> String {
    let rank = |p: &ParamInfo| -> i64 {
        if p.kind.is_required_named() || p.has_required {
            0
        } else if !p.kind.is_named() {
            -1
        } else {
            1
        }
    };
    parameters = super::dart_sort_vec(parameters, |a, b| rank(a) - rank(b));
    let mut sb = String::new();
    let mut close = "";
    for p in &parameters {
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
        } else if p.has_deprecated {
            // Dart writes `@required ` for a deprecated parameter here.
            sb.push_str("@required ");
        }
        // Dart `appendToWithoutDelimiters`.
        sb.push_str(&type_display(ctx, p.ty));
        sb.push(' ');
        sb.push_str(&p.name);
        if let Some(code) = &p.default_code {
            sb.push_str(" = ");
            sb.push_str(code);
        }
    }
    sb.push_str(close);
    format!("({sb})")
}

/// Dart `getParametersString(element)`.
fn parameters_string(ctx: &Ctx<'_>, element: ElemRef) -> Option<String> {
    let base = member::base_element(ctx, element);
    if crate::element_locator::is_executable(base) {
        if base.tag() == Tag::Getter && member::formal_parameters(ctx, element).is_empty() {
            return None;
        }
        return Some(parameters_list_string(ctx, element_params(ctx, element)));
    }
    if let Some(alias) = base.cast::<dartr_element::TypeAliasElement>() {
        let aliased = ctx.get(alias).aliased_type.get()?;
        if matches!(ctx.ty(aliased), TypeKind::Function(_)) {
            return Some(parameters_list_string(
                ctx,
                function_type_params(ctx, aliased),
            ));
        }
    }
    None
}

/// Dart `getReturnTypeString(element)`.
fn return_type_string(ctx: &Ctx<'_>, element: ElemRef) -> Option<String> {
    let base = member::base_element(ctx, element);
    if crate::element_locator::is_executable(base) {
        if base.tag() == Tag::Setter {
            return None;
        }
        return Some(type_display(ctx, member::return_type(ctx, element)));
    }
    if base.is::<dartr_element::VariableElement>() {
        return Some(type_display(ctx, member::type_(ctx, element)));
    }
    if let Some(alias) = base.cast::<dartr_element::TypeAliasElement>() {
        let aliased = ctx.get(alias).aliased_type.get()?;
        if let TypeKind::Function(f) = ctx.ty(aliased) {
            return Some(type_display(ctx, f.ret));
        }
    }
    None
}

/// Dart `CompletionDetail`.
struct Detail {
    detail: String,
    truncated_params: String,
    truncated_signature: String,
    auto_import_uri: Option<String>,
}

/// Dart `_getCompletionDetail`.
fn completion_detail(
    ctx: &Ctx<'_>,
    c: &Candidate,
    supports_deprecated: bool,
    is_callable: bool,
    is_invocation: bool,
) -> Detail {
    let mut return_type: Option<String> = None;
    if let Kind::RecordField { field_type, .. } = &c.kind {
        return_type = Some(type_display(ctx, *field_type));
    }
    let mut element = c.element();
    let is_override = matches!(c.kind, Kind::Override { .. });
    let mut is_getter_override = false;
    let mut is_setter_override = false;
    if is_override {
        let base = element.map(|e| member::base_element(ctx, e));
        is_getter_override = base.is_some_and(|b| b.tag() == Tag::Getter);
        is_setter_override = base.is_some_and(|b| b.tag() == Tag::Setter);
    }
    if let Kind::NamedArgument { parameter, .. } = &c.kind {
        element = Some(*parameter);
    }
    let mut parameters: Option<String> = None;
    if let Some(e) = element {
        let base = member::base_element(ctx, e);
        parameters = parameters_string(ctx, e);
        let parameter_type = base
            .cast::<FormalParameterElement>()
            .map(|_| type_display(ctx, member::type_(ctx, e)));
        return_type = return_type_string(ctx, e).or(parameter_type);
        if return_type.is_none() && base.tag() == Tag::Setter {
            if let Some(p) = &parameters {
                // `^\((\S+)\s+\S+\)$`
                if let Some(inner) = p.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
                    let parts: Vec<&str> = inner.split_whitespace().collect();
                    if parts.len() == 2 && !inner.starts_with(char::is_whitespace) {
                        let mut it = inner.splitn(2, char::is_whitespace);
                        let first = it.next().unwrap_or("");
                        let rest = it.next().unwrap_or("").trim_start();
                        if !first.is_empty()
                            && !rest.is_empty()
                            && !rest.contains(char::is_whitespace)
                        {
                            return_type = Some(first.to_string());
                        }
                    }
                }
            }
            parameters = None;
        }
    } else if let Kind::FunctionCall { ty, .. } = &c.kind {
        parameters = Some(parameters_list_string(ctx, function_type_params(ctx, *ty)));
        if let TypeKind::Function(f) = ctx.ty(*ty) {
            return_type = Some(type_display(ctx, f.ret));
        }
    }
    let truncated_params = match parameters.as_deref() {
        None | Some("") => String::new(),
        Some("()") => "()".to_string(),
        Some(_) => "(…)".to_string(),
    };
    let full_signature = match (&parameters, &return_type) {
        (_, Some(r)) if is_getter_override => format!("{r} get"),
        (_, Some(r)) if is_setter_override => format!("set ({r})"),
        (None, r) => r.clone().unwrap_or_default(),
        (Some(p), None) => p.clone(),
        (Some(p), Some(r)) if r.is_empty() => p.clone(),
        (Some(p), Some(r)) => format!("{p} → {r}"),
    };
    let callable = (is_callable && is_invocation) || is_override;
    let truncated_signature = match (&parameters, &return_type) {
        (_, Some(r)) if is_getter_override => format!(" {r} get"),
        (_, Some(r)) if is_setter_override => format!(" set ({r})"),
        (None, Some(r)) => format!(" {r}"),
        (None, None) => String::new(),
        (Some(p), _) if p.is_empty() => String::new(),
        (Some(_), r) if r.as_deref().is_none_or(str::is_empty) => {
            if callable {
                truncated_params.clone()
            } else {
                format!(" {truncated_params}")
            }
        }
        (Some(_), r) => {
            let r = r.as_deref().unwrap_or("");
            if callable {
                format!("{truncated_params} → {r}")
            } else {
                format!(" {truncated_params} → {r}")
            }
        }
    };
    let mut detail = full_signature;
    if let Some(e) = element {
        if elem::is_deprecated(ctx, member::base_element(ctx, e)) && !supports_deprecated {
            detail = format!("{detail}\n\n(Deprecated)").trim().to_string();
        }
    }
    let auto_import_uri = c
        .import_data
        .as_ref()
        .filter(|d| d.is_not_imported && !d.library_uri.is_empty())
        .map(|d| d.library_uri.clone());
    Detail {
        detail,
        truncated_params,
        truncated_signature,
        auto_import_uri,
    }
}

/// Dart `_elementToCompletionItemKind`.
fn element_kinds(ctx: &Ctx<'_>, element: ElementId) -> &'static [i64] {
    // CompletionItemKind: Text=1, Method=2, Function=3, Constructor=4,
    // Field=5, Variable=6, Class=7, Module=9, Property=10, Enum=13,
    // Keyword=14, File=17, EnumMember=20, TypeParameter=25.
    match element.tag() {
        Tag::Class => &[7],
        Tag::Constructor => &[4],
        Tag::Enum => &[13],
        Tag::Extension => &[2],
        Tag::ExtensionType => &[7],
        Tag::Field => {
            if is_enum_constant(ctx, element) {
                &[20, 13]
            } else {
                &[5]
            }
        }
        Tag::LocalFunction | Tag::TopLevelFunction => &[3],
        Tag::Label => &[1],
        Tag::Library => &[9],
        Tag::LocalVariable
        | Tag::PatternVariable
        | Tag::BindPatternVariable
        | Tag::JoinPatternVariable => &[6],
        Tag::Method => &[2],
        Tag::Mixin => &[7],
        Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter => &[6],
        Tag::Prefix => &[6],
        Tag::Getter | Tag::Setter => &[10],
        Tag::TopLevelVariable => &[6],
        Tag::TypeAlias => &[7],
        Tag::TypeParameter => &[25, 6],
        Tag::GenericFunctionType => &[7],
        _ => &[],
    }
}

/// Dart `_candidateToCompletionItemKind`.
fn item_kind(ctx: &Ctx<'_>, caps: &ItemCapabilities, c: &Candidate, label: &str) -> Option<i64> {
    let supported = |k: &&i64| caps.item_kinds.contains(k);
    if let Some(e) = c.element() {
        return element_kinds(ctx, member::base_element(ctx, e))
            .iter()
            .find(supported)
            .copied();
    }
    let kinds: &[i64] = match &c.kind {
        Kind::Closure { .. } | Kind::FunctionCall { .. } => &[2],
        Kind::Identifier { .. }
        | Kind::NamedArgument { .. }
        | Kind::Name(_)
        | Kind::RecordField { .. }
        | Kind::RecordLiteralNamedField { .. } => &[6],
        Kind::Keyword { .. } => &[14],
        Kind::Label(_) => &[1],
        Kind::Uri(_) => {
            if !label.starts_with("dart:") {
                if label.ends_with(".dart") {
                    &[17, 9]
                } else {
                    &[19, 9]
                }
            } else {
                &[9]
            }
        }
        _ => &[],
    };
    kinds.iter().find(supported).copied()
}

/// Dart `_getDisplayText`.
fn display_text(ctx: &Ctx<'_>, c: &Candidate) -> String {
    if let Some(d) = c.suggestion_data(ctx) {
        if !matches!(c.kind, Kind::SetState { .. }) {
            return d.display_text;
        }
    }
    match &c.kind {
        Kind::Override { data, .. } => match data {
            Some(d) => d.display_text.clone(),
            None => c.completion(ctx),
        },
        Kind::SetState { typed, .. } => match &typed.data {
            Some(d) => d.display_text.clone(),
            None => c
                .suggestion_data(ctx)
                .map(|d| d.display_text)
                .unwrap_or_default(),
        },
        _ => match c.typed().and_then(|t| t.data.as_ref()) {
            Some(d) => d.display_text.clone(),
            None => c.completion(ctx),
        },
    }
}

/// Dart `computeCompletionDefaultArgumentList`: (text, ranges).
fn default_argument_list(params: &[ParamInfo]) -> (Option<String>, Vec<usize>) {
    let mut sb = String::new();
    let mut ranges = Vec::new();
    for p in params.iter().filter(|p| p.kind.is_required_positional()) {
        if !sb.is_empty() {
            sb.push_str(", ");
        }
        let offset = utf16_len(&sb);
        sb.push_str(&p.name);
        ranges.push(offset);
        ranges.push(utf16_len(&p.name));
    }
    for p in params.iter().filter(|p| p.kind.is_named()) {
        if p.has_required || p.kind.is_required_named() {
            if !sb.is_empty() {
                sb.push_str(", ");
            }
            sb.push_str(&p.name);
            sb.push_str(": ");
            let offset = utf16_len(&sb);
            sb.push_str(&p.name);
            ranges.push(offset);
            ranges.push(utf16_len(&p.name));
        }
    }
    ((!sb.is_empty()).then_some(sb), ranges)
}

/// Dart `SnippetBuilder.escapeSnippetPlainText` (`$` and `\\`).
fn escape_snippet(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if matches!(c, '\\' | '$') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Dart `SnippetBuilder.escapeSnippetVariableText` (`$`, `}` and `\\`).
fn escape_snippet_variable(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if matches!(c, '\\' | '$' | '}') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Dart `buildSnippetStringWithTabStops`.
fn snippet_with_tab_stops(text: &str, ranges: &[usize]) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    let is_final = ranges.len() == 2;
    let mut out = String::new();
    let mut offset = 0;
    let mut next = 1;
    for pair in ranges.chunks(2) {
        let (start, length) = (pair[0], pair[1]);
        if start + length > units.len() {
            continue;
        }
        out.push_str(&escape_snippet(&String::from_utf16_lossy(
            &units[offset..start],
        )));
        let content = String::from_utf16_lossy(&units[start..start + length]);
        let number = if is_final {
            0
        } else {
            next += 1;
            next - 1
        };
        if content.is_empty() {
            out.push_str(&format!("${number}"));
        } else {
            out.push_str(&format!(
                "${{{number}:{}}}",
                escape_snippet_variable(&content)
            ));
        }
        offset = start + length;
    }
    out.push_str(&escape_snippet(&String::from_utf16_lossy(&units[offset..])));
    out
}

/// Dart `buildInsertText`: (text, is snippet).
#[allow(clippy::too_many_arguments)]
fn build_insert_text(
    supports_snippets: bool,
    commit_characters_enabled: bool,
    mut complete_function_calls: bool,
    required_args: Option<&str>,
    required_ranges: &[usize],
    has_optional_parameters: bool,
    completion: &str,
    selection_offset: usize,
    selection_length: usize,
) -> (String, bool) {
    let mut text = completion.to_string();
    let mut snippet = false;
    if completion.contains('(') {
        complete_function_calls = false;
    }
    if supports_snippets {
        if !commit_characters_enabled && complete_function_calls {
            snippet = true;
            let suffix = if !required_ranges.is_empty() && required_args.is_some() {
                snippet_with_tab_stops(required_args.unwrap(), required_ranges)
            } else if has_optional_parameters {
                "$0".to_string()
            } else {
                String::new()
            };
            text = format!("{}({suffix})", escape_snippet(&text));
        } else if selection_offset != 0 && selection_offset != utf16_len(completion) {
            snippet = true;
            text = snippet_with_tab_stops(completion, &[selection_offset, selection_length]);
        }
    }
    (text, snippet)
}

/// Dart `removeDartDocDelimiters`.
pub fn remove_dart_doc_delimiters(s: &str) -> String {
    let mut s = s;
    if let Some(rest) = s.strip_prefix("/**") {
        s = rest;
    }
    if let Some(rest) = s.strip_suffix("*/") {
        s = rest;
    }
    let s = s.trim();
    let mut out = Vec::new();
    for line in s.split('\n') {
        let mut line = line.trim();
        if let Some(rest) = line.strip_prefix('*') {
            line = rest.strip_prefix(' ').unwrap_or(rest);
        } else if let Some(rest) = line.strip_prefix("///") {
            line = rest.strip_prefix(' ').unwrap_or(rest);
        }
        out.push(line);
    }
    out.join("\n")
}

/// Dart `getCleanElementDocumentation` with the `full` preference.
pub fn clean_element_documentation(
    ctx: &Ctx<'_>,
    element: ElementId,
    templates: &std::collections::HashMap<String, String>,
) -> Option<String> {
    let full = crate::hover::documentation(ctx, element, templates)?;
    Some(crate::hover::clean_dartdoc(&remove_dart_doc_delimiters(
        &full,
    )))
}

/// Dart `ElementLocation.forElement(element)?.encoding`.
pub fn element_location(ctx: &Ctx<'_>, element: ElementId) -> Option<String> {
    let library = elem::library_of(ctx, element)?;
    let library_uri = elem::library_uri(ctx, library);
    let enclosing = elem::enclosing(ctx, element)?;
    let lookup = |e: ElementId| -> Option<String> { member::lookup_name(ctx, ElemRef::Base(e)) };
    if enclosing.tag() == Tag::Library {
        let top = lookup(element)?;
        return Some(format!("{library_uri};{top}"));
    }
    let outer = elem::enclosing(ctx, enclosing)?;
    if outer.tag() == Tag::Library {
        let member_name = lookup(element)?;
        let top = lookup(enclosing)?;
        return Some(format!("{library_uri};{top};{member_name}"));
    }
    None
}

/// The inputs of [`to_item`] that are the same for every candidate.
pub struct ItemContext<'i> {
    pub caps: &'i ItemCapabilities,
    pub commit_characters_enabled: bool,
    pub complete_function_calls: bool,
    pub has_default_text_mode: bool,
    /// The path of the completion file (to make `file:` URIs relative).
    pub file_path: &'i str,
}

/// Dart `getCompletionDisplayUriString`.
fn display_uri(uri: &str, file_path: &str) -> String {
    if let Some(path) = uri.strip_prefix("file://") {
        let from = std::path::Path::new(file_path)
            .parent()
            .unwrap_or(std::path::Path::new("/"));
        return relative_path(std::path::Path::new(path), from);
    }
    uri.to_string()
}

fn relative_path(path: &std::path::Path, from: &std::path::Path) -> String {
    let a: Vec<_> = path.components().collect();
    let b: Vec<_> = from.components().collect();
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let mut parts: Vec<String> = Vec::new();
    for _ in common..b.len() {
        parts.push("..".to_string());
    }
    for c in &a[common..] {
        parts.push(c.as_os_str().to_string_lossy().to_string());
    }
    parts.join("/")
}

/// Dart `toLspCompletionItem`.
#[allow(clippy::too_many_arguments)]
pub fn to_item(
    ctx: &Ctx<'_>,
    ic: &ItemContext<'_>,
    c: &Candidate,
    edit_range: Value,
    insert_range: Value,
    has_default_edit_range: bool,
    resolution: Option<Value>,
    cleaned_doc: Option<String>,
    color_hex: Option<String>,
) -> Option<Value> {
    let caps = ic.caps;
    let element = c.element();
    let base = element.map(|e| member::base_element(ctx, e));
    let is_callable = base.is_some_and(|b| {
        matches!(
            b.tag(),
            Tag::Constructor | Tag::LocalFunction | Tag::TopLevelFunction | Tag::Method
        )
    }) || matches!(
        c.kind,
        Kind::FunctionCall {
            kind: SuggestionKind::Invocation,
            ..
        }
    );
    let is_invocation = matches!(c.executable_kind(), Some(SuggestionKind::Invocation))
        || matches!(c.kind, Kind::Closure { .. } | Kind::FunctionCall { .. });
    let complete_function_calls = ic.complete_function_calls && is_callable && is_invocation;
    let mut label = display_text(ctx, c);
    if label.is_empty() {
        return None;
    }
    // `completionFilterTextSplitPattern` = `=>|[\(]`.
    let filter_text = if !(label.starts_with("=>") || label.starts_with('(')) {
        let cut = match (label.find("=>"), label.find('(')) {
            (Some(a), Some(b)) => a.min(b),
            (Some(a), None) => a,
            (None, Some(b)) => b,
            (None, None) => label.len(),
        };
        label[..cut].trim().to_string()
    } else {
        label.clone()
    };
    if caps.label_details {
        label = filter_text.clone();
    }
    if matches!(c.kind, Kind::Override { .. }) && !label.starts_with("override ") {
        label = format!("override {label}");
    }
    if label.ends_with(',') {
        label.pop();
    }
    // Dart `server.getColorHexString(element)`: a color constant has the
    // kind `Color`.
    let kind = if color_hex.is_some() {
        Some(16)
    } else {
        item_kind(ctx, caps, c, &label)
    };
    let mut detail = completion_detail(
        ctx,
        c,
        caps.deprecated_flag || caps.deprecated_tag,
        is_callable,
        is_invocation,
    );
    if !caps.label_details
        && !matches!(
            c.kind,
            Kind::Closure { .. } | Kind::Override { .. } | Kind::SetState { .. }
        )
    {
        label.push_str(&detail.truncated_params);
    }
    let mut parameter_names: Option<Vec<String>> = None;
    let mut default_args: (Option<String>, Vec<usize>) = (None, Vec::new());
    if let (Some(e), Some(b)) = (element, base) {
        if crate::element_locator::is_executable(b) && !matches!(b.tag(), Tag::Getter | Tag::Setter)
        {
            let params = element_params(ctx, e);
            parameter_names = Some(params.iter().map(|p| p.name.clone()).collect());
            default_args = default_argument_list(&params);
        }
    }
    let completion = c.completion(ctx);
    let (selection_offset, selection_length) = match &c.kind {
        Kind::Keyword {
            selection_offset, ..
        } => (*selection_offset, 0),
        _ => {
            if let Some(d) = c.suggestion_data(ctx) {
                (d.selection_offset, 0)
            } else {
                let data = match &c.kind {
                    Kind::Override { data, .. } => data.as_ref(),
                    _ => c.typed().and_then(|t| t.data.as_ref()),
                };
                match data {
                    Some(d) if d.selection_offset.is_some() && d.selection_length.is_some() => {
                        (d.selection_offset.unwrap(), d.selection_length.unwrap())
                    }
                    _ => (utf16_len(&completion), 0),
                }
            }
        }
    };
    let (insert_text, is_snippet) = build_insert_text(
        caps.snippets,
        ic.commit_characters_enabled,
        complete_function_calls,
        default_args.0.as_deref(),
        &default_args.1,
        parameter_names.as_ref().is_some_and(|p| !p.is_empty()),
        &completion,
        selection_offset,
        selection_length,
    );
    let is_multiline = insert_text.contains('\n');
    let mut cleaned_doc = cleaned_doc;
    if let Some(doc) = &cleaned_doc {
        // `^_([\w ]{0,20})_$`
        if let Some(inner) = doc.strip_prefix('_').and_then(|d| d.strip_suffix('_')) {
            if inner.chars().count() <= 20
                && inner
                    .chars()
                    .all(|ch| ch.is_alphanumeric() || ch == '_' || ch == ' ')
            {
                detail.detail = inner.to_string();
                cleaned_doc = None;
            }
        }
    }
    // Dart appends the hex color to the documentation.
    if let Some(hex) = &color_hex {
        cleaned_doc = Some(
            format!("{}\n\n{hex}", cleaned_doc.unwrap_or_default())
                .trim()
                .to_string(),
        );
    }
    let is_deprecated = base.is_some_and(|b| elem::has_or_inherits_deprecated(ctx, b));
    let mut item = Map::new();
    item.insert("label".into(), json!(label));
    if let Some(k) = kind {
        item.insert("kind".into(), json!(k));
    }
    if caps.deprecated_tag && is_deprecated {
        item.insert("tags".into(), json!([1]));
    }
    if let Some(r) = resolution {
        item.insert("data".into(), r);
    }
    if !detail.detail.is_empty() {
        item.insert("detail".into(), json!(detail.detail));
    }
    if caps.label_details {
        let mut details = Map::new();
        if !detail.truncated_signature.is_empty() {
            details.insert("detail".into(), json!(detail.truncated_signature));
        }
        if let Some(uri) = &detail.auto_import_uri {
            details.insert("description".into(), json!(display_uri(uri, ic.file_path)));
        }
        if !details.is_empty() {
            item.insert("labelDetails".into(), Value::Object(details));
        }
    }
    if let Some(doc) = cleaned_doc {
        item.insert(
            "documentation".into(),
            crate::server::Server::markup_content_or_string(&caps.documentation_formats, doc),
        );
    }
    if caps.deprecated_flag && is_deprecated {
        item.insert("deprecated".into(), json!(true));
    }
    item.insert(
        "sortText".into(),
        json!((super::relevance::MAXIMUM_RELEVANCE_SORT - c.relevance as i64).to_string()),
    );
    if filter_text != label {
        item.insert("filterText".into(), json!(filter_text));
    }
    if is_snippet {
        item.insert("insertTextFormat".into(), json!(2));
    }
    if !ic.has_default_text_mode && caps.as_is_insert_mode && is_multiline {
        item.insert("insertTextMode".into(), json!(1));
    }
    if has_default_edit_range {
        if insert_text != label {
            item.insert("textEditText".into(), json!(insert_text));
        }
    } else if caps.insert_replace && insert_range != edit_range {
        item.insert(
            "textEdit".into(),
            json!({"insert": insert_range, "replace": edit_range, "newText": insert_text}),
        );
    } else {
        item.insert(
            "textEdit".into(),
            json!({"range": edit_range, "newText": insert_text}),
        );
    }
    Some(Value::Object(item))
}

/// The display name of an element for the resolve handler (`showName`).
pub fn show_name(ctx: &Ctx<'_>, element: ElementId) -> Option<String> {
    if let Some(enclosing) = elem::enclosing(ctx, element) {
        if enclosing.is::<dartr_element::InstanceElement>() {
            return ctx.element_name(enclosing).map(str::to_string);
        }
    }
    ctx.element_name(element)
        .map(|_| display_name(ctx, element))
}

/// Dart `DartType.isColor`: [ty] is the `Color` class of `dart:ui` or a
/// subtype of it.
pub fn is_color(ctx: &Ctx<'_>, ty: TypeId) -> bool {
    use dartr_typesystem::TypeExt;
    if !matches!(ctx.ty(ty), TypeKind::Interface { .. }) {
        return false;
    }
    let is_exact = |t: TypeId| {
        let Some(element) = ctx.interface_element(t) else {
            return false;
        };
        let element = element.raw();
        ctx.element_name(element) == Some("Color")
            && member::library(ctx, ElemRef::Base(element)).and_then(|l| ctx.element_name(l.raw()))
                == Some("dart.ui")
    };
    is_exact(ty) || ctx.all_supertypes(ty).into_iter().any(is_exact)
}

/// Dart `DartObjectImpl.getFieldFromHierarchy`.
fn field_from_hierarchy<'v>(
    value: &'v dartr_constant::DartObjectImpl,
    name: &str,
) -> Option<&'v dartr_constant::DartObjectImpl> {
    if let Some(f) = value.get_field(name) {
        return Some(f);
    }
    field_from_hierarchy(value.get_field("(super)")?, name)
}

/// Dart `getColorHexString` of a constant value (`ColorComputer
/// .getColorForObject`): `#RRGGBB`.
pub fn color_hex_string(ctx: &Ctx<'_>, value: &dartr_constant::DartObjectImpl) -> Option<String> {
    if value.is_null() || !is_color(ctx, value.ty) {
        return None;
    }
    let color = field_from_hierarchy(value, "color").unwrap_or(value);
    let channel = |name: &str| -> Option<u32> {
        let v = field_from_hierarchy(color, name)?.to_double_value()?;
        Some(((v * 255.0).round() as i64 & 0xff) as u32)
    };
    let _alpha = channel("a")?;
    let (r, g, b) = (channel("r")?, channel("g")?, channel("b")?);
    Some(format!("#{r:02X}{g:02X}{b:02X}"))
}
