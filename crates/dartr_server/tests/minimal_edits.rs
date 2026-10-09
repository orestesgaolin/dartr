//! The minimal edits of the formatting requests
//! (`source_edits::generate_minimal_edits`, Dart `_MinimalEditComputer`).
//!
//! - `fixtures/minimal_edits/*.json` were recorded from `dart language-server`
//!   3.13.3 (`generate.py`): the unformatted source, the output of
//!   `dart format` for it, and the edits of `textDocument/formatting` or
//!   `textDocument/rangeFormatting`. dartr must compute the same edits from
//!   the same two sources.
//! - Handmade pairs cover the cases that the formatter does not produce:
//!   token substitutions and the fallbacks for unexpected token changes.

use std::path::Path;

use dartr_server::mapping::codes;
use dartr_server::source_edits::{
    generate_edits_for_formatting, generate_minimal_edits, range_offsets,
};
use dartr_syntax::{LineInfo, ScannerConfiguration};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn edits(unformatted: &str, formatted: &str, range: Option<&Value>) -> Vec<Value> {
    let line_info = LineInfo::from_content(unformatted);
    let range = range.map(|r| range_offsets(&line_info, r).unwrap());
    generate_minimal_edits(
        unformatted,
        &line_info,
        formatted,
        ScannerConfiguration::default(),
        range,
    )
}

/// Applies LSP edits (positions in UTF-16 code units, all relative to the
/// original text) to [text].
fn apply(text: &str, edits: &[Value]) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    let line_info = LineInfo::from_content(text);
    let offset = |p: &Value| {
        line_info.line_starts[p["line"].as_u64().unwrap() as usize]
            + p["character"].as_u64().unwrap() as u32
    };
    let mut sorted: Vec<&Value> = edits.iter().collect();
    sorted.sort_by_key(|e| std::cmp::Reverse(offset(&e["range"]["start"])));
    let mut out = units;
    for e in sorted {
        let start = offset(&e["range"]["start"]) as usize;
        let end = offset(&e["range"]["end"]) as usize;
        let new: Vec<u16> = e["newText"].as_str().unwrap().encode_utf16().collect();
        out.splice(start..end, new);
    }
    String::from_utf16(&out).unwrap()
}

#[test]
fn edits_match_dart_language_server() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/minimal_edits");
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    names.sort();
    assert!(names.len() >= 9, "fixtures missing in {}", dir.display());
    for path in names {
        let case: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let unformatted = case["unformatted"].as_str().unwrap();
        let formatted = case["formatted"].as_str().unwrap();
        let range = case.get("range");
        let actual = edits(unformatted, formatted, range);
        let name = path.file_stem().unwrap().to_string_lossy();
        assert_eq!(Value::Array(actual.clone()), case["edits"], "{name}");
        if range.is_none() {
            assert_eq!(apply(unformatted, &actual), formatted, "{name}");
        }
    }
}

#[test]
fn index_token_substitution() {
    // `[ ]` (two tokens) becomes `[]` (one `INDEX` token): only the space
    // is removed.
    let e = edits("var x = [ ];\n", "var x = [];\n", None);
    assert_eq!(
        e,
        [
            json!({"range": {"start": {"line": 0, "character": 9}, "end": {"line": 0, "character": 10}}, "newText": ""})
        ]
    );
    // And the other way.
    let e = edits("var x = [];\n", "var x = [ ];\n", None);
    assert_eq!(
        e,
        [
            json!({"range": {"start": {"line": 0, "character": 9}, "end": {"line": 0, "character": 9}}, "newText": " "})
        ]
    );
}

#[test]
fn shift_token_substitution() {
    // `>>` split into `>` `>` (as in nested type arguments).
    let e = edits(
        "var x = <List<int>>[];\n",
        "var x = <List<int> >[];\n",
        None,
    );
    assert_eq!(
        apply("var x = <List<int>>[];\n", &e),
        "var x = <List<int> >[];\n"
    );
    assert_eq!(e.len(), 1, "{e:?}");
}

#[test]
fn unexpected_token_change_falls_back() {
    // A full format replaces the whole document.
    let e = edits("var a = 1;\n", "var b = 1;\n", None);
    assert_eq!(
        e,
        [
            json!({"range": {"start": {"line": 0, "character": 0}, "end": {"line": 1, "character": 0}}, "newText": "var b = 1;\n"})
        ]
    );
    // A range format changes nothing.
    let range = json!({"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 10}});
    assert_eq!(
        edits("var a = 1;\n", "var b = 1;\n", Some(&range)),
        Vec::<Value>::new()
    );
    // An added token that is not a comma or semicolon at the end.
    let e = edits("f() {}\n", "f() {}\nvar x;\n", None);
    assert_eq!(e.len(), 1);
    assert_eq!(e[0]["newText"], json!("f() {}\nvar x;\n"));
}

#[test]
fn unchanged_source_is_null_and_bad_range_is_an_error() {
    let content = "var a = 1;\n";
    let line_info = LineInfo::from_content(content);
    let config = ScannerConfiguration::default();
    assert_eq!(
        generate_edits_for_formatting(content, &line_info, content, config, None).unwrap(),
        Value::Null
    );
    // The range is checked only when there are changes (like Dart).
    let bad = json!({"start": {"line": 200, "character": 0}, "end": {"line": 400, "character": 0}});
    assert_eq!(
        generate_edits_for_formatting(content, &line_info, content, config, Some(&bad)).unwrap(),
        Value::Null
    );
    let e = generate_edits_for_formatting(content, &line_info, "var a  = 1;\n", config, Some(&bad))
        .unwrap_err();
    assert_eq!(
        e.to_json(),
        json!({"code": codes::INVALID_FILE_LINE_COL, "message": "Invalid line number", "data": "200"})
    );
}
