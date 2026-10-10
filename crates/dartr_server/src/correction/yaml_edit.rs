// Dart source: package:yaml_edit 2.2.4 lib/src/editor.dart (YamlEditor.update)
// Dart source: package:yaml_edit 2.2.4 lib/src/map_mutations.dart (updateInMap, _addToBlockMap, _addToFlowMap, _replaceInBlockMap, _replaceInFlowMap)
// Dart source: package:yaml_edit 2.2.4 lib/src/utils.dart (getContentSensitiveEnd, getMapInsertionIndex, getIndentation, getMapIndentation, getLineEnding)
// Dart source: package:yaml_edit 2.2.4 lib/src/strings.dart (yamlEncodeBlock, yamlEncodeFlow for plain strings and maps)
// Dart source: pkg/analysis_server_plugin/lib/src/correction/ignore_diagnostic.dart (IgnoreDiagnosticInAnalysisOptionsFile.compute)

//! The part of `package:yaml_edit` that the "ignore in
//! `analysis_options.yaml`" fix uses: setting a value at a path of nested
//! maps. Values are plain strings and maps of plain strings (the analyzer
//! codes and `ignore`).

use dartr_yaml::{CollectionStyle, NodeKind, Scalar, YamlNode};

use super::change_builder::ChangeBuilder;
use super::producer::ProducerContext;

/// A value to write: a plain string or a map.
#[derive(Clone, Debug)]
pub enum YValue {
    Str(String),
    Map(Vec<(String, YValue)>),
}

impl YValue {
    fn is_collection(&self) -> bool {
        matches!(self, YValue::Map(_))
    }

    fn is_empty(&self) -> bool {
        matches!(self, YValue::Map(m) if m.is_empty())
    }
}

/// A source edit of the YAML text (UTF-16 offsets).
#[derive(Clone, Debug)]
pub struct YamlSourceEdit {
    pub offset: usize,
    pub length: usize,
    pub replacement: String,
}

fn key_string(node: &YamlNode) -> String {
    match &node.kind {
        NodeKind::Scalar(Scalar::String(s)) => s.clone(),
        NodeKind::Scalar(Scalar::Null) => "null".into(),
        NodeKind::Scalar(Scalar::Bool(b)) => b.to_string(),
        NodeKind::Scalar(Scalar::Int(i)) => i.to_string(),
        NodeKind::Scalar(Scalar::Float(f)) => f.to_string(),
        _ => String::new(),
    }
}

/// Dart `getContentSensitiveEnd`.
fn content_sensitive_end(node: &YamlNode) -> usize {
    match &node.kind {
        NodeKind::List(items) if node.collection_style != CollectionStyle::Flow => items
            .last()
            .map(content_sensitive_end)
            .unwrap_or(node.span.end.offset),
        NodeKind::Map(entries) if node.collection_style != CollectionStyle::Flow => entries
            .last()
            .map(|(_, v)| content_sensitive_end(v))
            .unwrap_or(node.span.end.offset),
        _ => node.span.end.offset,
    }
}

/// Dart `getLineEnding`.
fn line_ending(yaml: &[u16]) -> &'static str {
    let mut unix = 0;
    let mut windows = 0;
    for (i, &c) in yaml.iter().enumerate() {
        if c == b'\n' as u16 {
            if i != 0 && yaml[i - 1] == b'\r' as u16 {
                windows += 1;
            } else {
                unix += 1;
            }
        }
    }
    if windows > unix { "\r\n" } else { "\n" }
}

fn last_index_of(yaml: &[u16], c: u8, start: usize) -> Option<usize> {
    let start = start.min(yaml.len().saturating_sub(1));
    (0..=start)
        .rev()
        .find(|&i| yaml.get(i) == Some(&(c as u16)))
}

fn index_of(yaml: &[u16], c: u8, start: usize) -> Option<usize> {
    (start..yaml.len()).find(|&i| yaml[i] == c as u16)
}

/// Dart `getMapIndentation`.
fn map_indentation(yaml: &[u16], map: &YamlNode) -> usize {
    if map.collection_style == CollectionStyle::Flow {
        return 0;
    }
    let NodeKind::Map(entries) = &map.kind else {
        return 0;
    };
    let Some((last_key, _)) = entries.last() else {
        return 0;
    };
    let last_span_offset = last_key.span.start.offset;
    let last_new_line = last_index_of(yaml, b'\n', last_span_offset);
    let last_question = last_index_of(yaml, b'?', last_span_offset);
    match (last_question, last_new_line) {
        (None, None) => last_span_offset,
        (None, Some(nl)) => last_span_offset - nl - 1,
        (Some(q), None) => q,
        (Some(q), Some(nl)) if q > nl => q - nl - 1,
        (_, Some(nl)) => last_span_offset - nl - 1,
    }
}

/// Dart `getIndentation` (maps only; dartr does not edit lists).
fn indentation(yaml: &[u16], root: &YamlNode) -> usize {
    let mut indentation = 2;
    if let NodeKind::Map(entries) = &root.kind {
        if root.collection_style != CollectionStyle::Flow {
            for (_, child) in entries {
                let indent = match &child.kind {
                    NodeKind::Map(e) if !e.is_empty() => map_indentation(yaml, child),
                    _ => 0,
                };
                if indent != 0 {
                    indentation = indent;
                }
            }
        }
    }
    indentation
}

/// Dart `getMapInsertionIndex`.
fn map_insertion_index(map: &YamlNode, new_key: &str) -> usize {
    let NodeKind::Map(entries) = &map.kind else {
        return 0;
    };
    let keys: Vec<String> = entries.iter().map(|(k, _)| key_string(k)).collect();
    for i in 1..keys.len() {
        if super::imports::dart_compare(&keys[i], &keys[i - 1]).is_lt() {
            return keys.len();
        }
    }
    keys.iter()
        .position(|k| super::imports::dart_compare(k, new_key).is_gt())
        .unwrap_or(keys.len())
}

/// Dart `yamlEncodeFlow`.
fn encode_flow(value: &YValue) -> String {
    match value {
        YValue::Str(s) => s.clone(),
        YValue::Map(entries) => {
            let parts: Vec<String> = entries
                .iter()
                .map(|(k, v)| format!("{k}: {}", encode_flow(v)))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
    }
}

/// Dart `yamlEncodeBlock`.
fn encode_block(value: &YValue, indentation: usize, line_ending: &str) -> String {
    match value {
        YValue::Str(s) => s.clone(),
        YValue::Map(entries) => {
            if entries.is_empty() {
                return format!("{}{{}}", " ".repeat(indentation));
            }
            entries
                .iter()
                .map(|(k, v)| {
                    let formatted_key = format!("{}{k}", " ".repeat(indentation));
                    let formatted_value = encode_block(v, indentation + 2, line_ending);
                    if v.is_collection() && !v.is_empty() {
                        format!("{formatted_key}:{line_ending}{formatted_value}")
                    } else {
                        format!("{formatted_key}: {formatted_value}")
                    }
                })
                .collect::<Vec<_>>()
                .join(line_ending)
        }
    }
}

/// Dart `YamlEditor.update(path, value)`: the edit, `None` when the path
/// cannot be updated by this port.
pub fn update(
    text: &str,
    root: &YamlNode,
    path: &[&str],
    value: &YValue,
) -> Option<YamlSourceEdit> {
    let yaml: Vec<u16> = text.encode_utf16().collect();
    let le = line_ending(&yaml);
    if path.is_empty() {
        let start = root.span.start.offset;
        let end = content_sensitive_end(root);
        return Some(YamlSourceEdit {
            offset: start,
            length: end.saturating_sub(start),
            replacement: encode_block(value, 0, le),
        });
    }
    let mut parent = root;
    for key in &path[..path.len() - 1] {
        let NodeKind::Map(entries) = &parent.kind else {
            return None;
        };
        parent = &entries.iter().find(|(k, _)| key_string(k) == *key)?.1;
    }
    let key = *path.last().unwrap();
    let NodeKind::Map(entries) = &parent.kind else {
        return None;
    };
    let existing = entries.iter().find(|(k, _)| key_string(k) == key);
    let flow = parent.collection_style == CollectionStyle::Flow;
    match (existing, flow) {
        (None, false) => {
            // Dart `_addToBlockMap`.
            let map_indent = map_indentation(&yaml, parent);
            let new_indentation = map_indent + indentation(&yaml, root);
            let mut formatted = " ".repeat(map_indent);
            let mut offset = parent.span.end.offset;
            let insertion_index = map_insertion_index(parent, key);
            if !entries.is_empty() {
                if insertion_index == entries.len() {
                    let last_end = content_sensitive_end(&entries.last().unwrap().1);
                    match index_of(&yaml, b'\n', last_end) {
                        Some(nl) => offset = nl + 1,
                        None => formatted = format!("{le}{formatted}"),
                    }
                } else {
                    let key_start = entries[insertion_index].0.span.start.offset;
                    offset = last_index_of(&yaml, b'\n', key_start)
                        .map(|i| i + 1)
                        .unwrap_or(0);
                }
            }
            let value_string = encode_block(value, new_indentation, le);
            if value.is_collection() && !value.is_empty() {
                formatted.push_str(&format!("{key}:{le}{value_string}{le}"));
            } else {
                formatted.push_str(&format!("{key}: {value_string}{le}"));
            }
            Some(YamlSourceEdit {
                offset,
                length: 0,
                replacement: formatted,
            })
        }
        (None, true) => {
            // Dart `_addToFlowMap`.
            let value_string = encode_flow(value);
            if entries.is_empty() {
                return Some(YamlSourceEdit {
                    offset: parent.span.end.offset - 1,
                    length: 0,
                    replacement: format!("{key}: {value_string}"),
                });
            }
            let index = map_insertion_index(parent, key);
            if index == entries.len() {
                return Some(YamlSourceEdit {
                    offset: parent.span.end.offset - 1,
                    length: 0,
                    replacement: format!(", {key}: {value_string}"),
                });
            }
            Some(YamlSourceEdit {
                offset: entries[index].0.span.start.offset,
                length: 0,
                replacement: format!("{key}: {value_string}, "),
            })
        }
        (Some((key_node, value_node)), false) => {
            // Dart `_replaceInBlockMap`.
            let new_indentation = map_indentation(&yaml, parent) + indentation(&yaml, root);
            let mut value_string = encode_block(value, new_indentation, le);
            if value.is_collection() && !value.is_empty() {
                value_string = format!("{le}{value_string}");
            }
            if !value_string.starts_with(le) {
                value_string = format!(" {value_string}");
            }
            let start = key_node.span.end.offset + 1;
            let mut end = content_sensitive_end(value_node);
            if end < start {
                end = start;
            }
            Some(YamlSourceEdit {
                offset: start,
                length: end - start,
                replacement: value_string,
            })
        }
        (Some((_, value_node)), true) => Some(YamlSourceEdit {
            offset: value_node.span.start.offset,
            length: value_node.span.length(),
            replacement: encode_flow(value),
        }),
    }
}

/// Dart `IgnoreDiagnosticInAnalysisOptionsFile.compute` (after the
/// unignorable check).
pub fn ignore_in_analysis_options(
    c: &ProducerContext<'_>,
    code: &str,
    builder: &mut ChangeBuilder<'_>,
) {
    let Some(file) = c.options.file.clone() else {
        return;
    };
    let Some(content) = builder.workspace.content(&file) else {
        return;
    };
    let result = dartr_yaml::load_yaml_node_with_options(&content, false);
    if !result.errors.is_empty() {
        return;
    }
    let Some(options) = result.node else {
        return;
    };
    let ignore = || YValue::Map(vec![(code.to_string(), YValue::Str("ignore".into()))]);
    let (path, value): (Vec<&str>, YValue) = match &options.kind {
        NodeKind::Map(entries) => {
            let analyzer = entries
                .iter()
                .find(|(k, _)| key_string(k) == "analyzer")
                .map(|(_, v)| v);
            let has_errors = match analyzer.map(|a| &a.kind) {
                Some(NodeKind::Map(e)) => e.iter().any(|(k, _)| key_string(k) == "errors"),
                _ => false,
            };
            if !has_errors {
                (
                    vec!["analyzer"],
                    YValue::Map(vec![("errors".into(), ignore())]),
                )
            } else {
                (
                    vec!["analyzer", "errors", code],
                    YValue::Str("ignore".into()),
                )
            }
        }
        _ => (
            vec![],
            YValue::Map(vec![(
                "analyzer".into(),
                YValue::Map(vec![("errors".into(), ignore())]),
            )]),
        ),
    };
    let Some(edit) = update(&content, &options, &path, &value) else {
        return;
    };
    builder.add_generic_file_edit(&file, |b| {
        b.add_simple_insertion(edit.offset as u32, &edit.replacement);
    });
}
