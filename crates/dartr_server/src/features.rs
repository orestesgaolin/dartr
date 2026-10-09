// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_document_symbols.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_folding.dart
// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_selection_range.dart
// Dart source: pkg/analysis_server/lib/src/lsp/mapping.dart (toOutline, toElement, toElementName, toClosingLabel, toFoldingRangeKind)

//! The LSP results of the AST features: the computers in [`crate::computer`]
//! converted to LSP JSON.

use dartr_syntax::LineInfo;
use serde_json::{Map, Value, json};

use crate::capabilities::ClientCapabilities;
use crate::computer::closing_labels::compute_closing_labels;
use crate::computer::folding::{DartUnitFoldingComputer, FoldingKind};
use crate::computer::outline::{Element, Outline, compute_outline};
use crate::computer::selection_ranges::compute_selection_ranges;
use crate::mapping::{
    ElementKind, ErrorOr, element_kind_to_symbol_kind, read_position, to_offset, to_range,
};
use crate::server::ParsedFile;
use crate::uri::path_to_uri;

/// Dart `toElementName`.
fn element_name(e: &Element) -> String {
    if !e.name.is_empty() {
        return e.name.clone();
    }
    if e.kind == ElementKind::Extension {
        match &e.extended_type {
            Some(t) if !t.is_empty() => format!("extension on {t}"),
            _ => "<unnamed extension>".to_string(),
        }
    } else {
        "<unnamed>".to_string()
    }
}

/// Dart `toElement`.
fn element_json(line_info: &LineInfo, e: &Element) -> Value {
    let mut m = Map::new();
    if let Some((offset, length)) = e.location {
        m.insert("range".into(), to_range(line_info, offset, length));
    }
    m.insert("name".into(), json!(element_name(e)));
    m.insert("kind".into(), json!(e.kind.name()));
    if let Some(p) = &e.parameters {
        m.insert("parameters".into(), json!(p));
    }
    if let Some(p) = &e.type_parameters {
        m.insert("typeParameters".into(), json!(p));
    }
    if let Some(r) = &e.return_type {
        m.insert("returnType".into(), json!(r));
    }
    Value::Object(m)
}

/// Dart `toOutline`.
fn outline_json(line_info: &LineInfo, o: &Outline) -> Value {
    let mut m = Map::new();
    m.insert("element".into(), element_json(line_info, &o.element));
    m.insert("range".into(), to_range(line_info, o.offset, o.length));
    m.insert(
        "codeRange".into(),
        to_range(line_info, o.code_offset, o.code_length),
    );
    if let Some(children) = &o.children {
        m.insert(
            "children".into(),
            children
                .iter()
                .map(|c| outline_json(line_info, c))
                .collect(),
        );
    }
    Value::Object(m)
}

/// The `outline` of `dart/textDocument/publishOutline`.
pub fn outline(file: &ParsedFile) -> Value {
    let unit = &file.unit;
    outline_json(&unit.line_info, &compute_outline(&unit.ast, unit.unit))
}

/// The `labels` of `dart/textDocument/publishClosingLabels`.
pub fn closing_labels(file: &ParsedFile) -> Value {
    let unit = &file.unit;
    compute_closing_labels(&unit.ast, unit.unit, &unit.line_info)
        .iter()
        .map(|l| {
            json!({
                "range": to_range(&unit.line_info, l.offset, l.length),
                "label": l.label,
            })
        })
        .collect()
}

/// `textDocument/documentSymbol` (Dart `DocumentSymbolHandler._getSymbols`).
pub fn document_symbols(client: &ClientCapabilities, path: &str, file: &ParsedFile) -> Value {
    let unit = &file.unit;
    let line_info = &unit.line_info;
    let outline = compute_outline(&unit.ast, unit.unit);
    let kinds = client.document_symbol_kinds();
    if client.hierarchical_symbols() {
        fn as_document_symbol(kinds: &[i64], line_info: &LineInfo, o: &Outline) -> Value {
            let code_range = to_range(line_info, o.code_offset, o.code_length);
            let selection_range = match o.element.location {
                Some((offset, length)) => to_range(line_info, offset, length),
                None => code_range.clone(),
            };
            let mut m = Map::new();
            m.insert("name".into(), json!(element_name(&o.element)));
            if let Some(p) = &o.element.parameters {
                m.insert("detail".into(), json!(p));
            }
            m.insert(
                "kind".into(),
                json!(element_kind_to_symbol_kind(kinds, o.element.kind)),
            );
            m.insert("deprecated".into(), json!(o.element.is_deprecated));
            m.insert("range".into(), code_range);
            m.insert("selectionRange".into(), selection_range);
            if let Some(children) = &o.children {
                m.insert(
                    "children".into(),
                    children
                        .iter()
                        .map(|c| as_document_symbol(kinds, line_info, c))
                        .collect(),
                );
            }
            Value::Object(m)
        }
        match &outline.children {
            Some(children) => children
                .iter()
                .map(|c| as_document_symbol(&kinds, line_info, c))
                .collect(),
            None => Value::Null,
        }
    } else {
        let uri = path_to_uri(path);
        let mut all = Vec::new();
        fn add_symbol(
            all: &mut Vec<Value>,
            kinds: &[i64],
            uri: &str,
            line_info: &LineInfo,
            o: &Outline,
            parent_name: Option<&str>,
        ) {
            if let Some((offset, length)) = o.element.location {
                let mut m = Map::new();
                m.insert("name".into(), json!(element_name(&o.element)));
                m.insert(
                    "kind".into(),
                    json!(element_kind_to_symbol_kind(kinds, o.element.kind)),
                );
                m.insert("deprecated".into(), json!(o.element.is_deprecated));
                m.insert(
                    "location".into(),
                    json!({"uri": uri, "range": to_range(line_info, offset, length)}),
                );
                if let Some(p) = parent_name {
                    m.insert("containerName".into(), json!(p));
                }
                all.push(Value::Object(m));
            }
            for c in o.children.iter().flatten() {
                add_symbol(all, kinds, uri, line_info, c, Some(&o.element.name));
            }
        }
        for c in outline.children.iter().flatten() {
            add_symbol(&mut all, &kinds, &uri, line_info, c, None);
        }
        Value::Array(all)
    }
}

/// Dart `toFoldingRangeKind`.
fn folding_range_kind(kind: FoldingKind) -> Option<&'static str> {
    match kind {
        FoldingKind::Comment | FoldingKind::DocumentationComment | FoldingKind::FileHeader => {
            Some("comment")
        }
        FoldingKind::Directives => Some("imports"),
        _ => None,
    }
}

/// `textDocument/foldingRange` (Dart `FoldingHandler.handle`).
pub fn folding_ranges(client: &ClientCapabilities, file: &ParsedFile) -> Value {
    let unit = &file.unit;
    let line_info = &unit.line_info;
    let line_only = client.line_folding_only();
    let mut regions = DartUnitFoldingComputer::new(&unit.ast, unit.unit, line_info).compute();
    regions.sort_by_key(|r| r.offset);

    struct Range {
        start_line: u32,
        start_character: u32,
        end_line: u32,
        end_character: u32,
        kind: Option<&'static str>,
    }
    let mut ranges: Vec<Range> = regions
        .iter()
        .map(|r| {
            let start = line_info.get_location(r.offset);
            let end = line_info.get_location(r.offset + r.length);
            Range {
                start_line: start.line_number - 1,
                start_character: start.column_number - 1,
                end_line: end.line_number - 1,
                end_character: end.column_number - 1,
                kind: folding_range_kind(r.kind),
            }
        })
        .collect();

    // Dart `_compensateForLineFolding`.
    if line_only {
        let start_lines: std::collections::HashSet<u32> =
            ranges.iter().map(|r| r.start_line).collect();
        let mut i = 0;
        while i < ranges.len() {
            let range = &ranges[i];
            if !start_lines.contains(&range.end_line) {
                i += 1;
                continue;
            }
            let mut new_end_line = range.end_line as i64;
            while start_lines.contains(&(new_end_line as u32))
                && new_end_line >= range.start_line as i64
            {
                new_end_line -= 1;
            }
            if new_end_line <= range.start_line as i64 {
                ranges.remove(i);
            } else {
                ranges[i].end_line = new_end_line as u32;
                i += 1;
            }
        }
    }

    ranges
        .iter()
        .map(|r| {
            let mut m = Map::new();
            m.insert("startLine".into(), json!(r.start_line));
            if !line_only {
                m.insert("startCharacter".into(), json!(r.start_character));
            }
            m.insert("endLine".into(), json!(r.end_line));
            if !line_only {
                m.insert("endCharacter".into(), json!(r.end_character));
            }
            if let Some(k) = r.kind {
                m.insert("kind".into(), json!(k));
            }
            Value::Object(m)
        })
        .collect()
}

/// `textDocument/selectionRange` (Dart `SelectionRangeHandler.handle`).
pub fn selection_ranges(file: &ParsedFile, positions: &[Value]) -> ErrorOr<Value> {
    let unit = &file.unit;
    let line_info = &unit.line_info;
    let mut offsets = Vec::new();
    for p in positions {
        let (line, character) = read_position(p).unwrap_or((0, 0));
        offsets.push(to_offset(line_info, line, character, false)?);
    }
    let mut result = Vec::new();
    for offset in offsets {
        let ranges = compute_selection_ranges(&unit.ast, unit.unit, offset);
        let mut last: Option<Value> = None;
        for r in ranges.iter().rev() {
            let mut m = Map::new();
            m.insert("range".into(), to_range(line_info, r.offset, r.length));
            if let Some(parent) = last.take() {
                m.insert("parent".into(), parent);
            }
            last = Some(Value::Object(m));
        }
        result.push(last.unwrap_or_else(|| json!({"range": to_range(line_info, offset, 0)})));
    }
    Ok(Value::Array(result))
}
