// Dart source: pkg/analysis_server/lib/src/lsp/source_edits.dart (applyAndConvertEditsToServer)

//! Incremental document changes (`textDocument/didChange`).

use dartr_syntax::LineInfo;
use serde_json::Value;

use crate::mapping::{ErrorOr, ResponseError, codes, read_position, to_offset};

/// The byte index of the UTF-16 offset [offset] in [content]. Offsets after
/// the end map to the end; an offset inside a surrogate pair maps to the
/// start of the character.
fn byte_index(content: &str, offset: u32) -> usize {
    let mut units = 0u32;
    for (i, c) in content.char_indices() {
        let next = units + c.len_utf16() as u32;
        if next > offset {
            return i;
        }
        units = next;
    }
    content.len()
}

/// Dart `applyAndConvertEditsToServer(failureIsCritical: true)`: applies
/// the `contentChanges` of `didChange` to [content].
pub fn apply_changes(content: &str, changes: &[Value]) -> ErrorOr<String> {
    let mut new_content = content.to_string();
    for change in changes {
        let text = change
            .get("text")
            .and_then(Value::as_str)
            .ok_or_else(|| ResponseError::new(codes::INVALID_PARAMS, "Invalid change"))?;
        match change.get("range") {
            Some(range) => {
                let start = range.get("start").and_then(read_position);
                let end = range.get("end").and_then(read_position);
                let (Some(start), Some(end)) = (start, end) else {
                    return Err(ResponseError::new(codes::INVALID_PARAMS, "Invalid range"));
                };
                let lines = LineInfo::from_content(&new_content);
                let start = to_offset(&lines, start.0, start.1, true)?;
                let end = to_offset(&lines, end.0, end.1, true)?;
                let (start, end) = (
                    byte_index(&new_content, start),
                    byte_index(&new_content, end),
                );
                if start > end {
                    return Err(ResponseError::new(
                        codes::CLIENT_SERVER_INCONSISTENT_STATE,
                        "Invalid range",
                    ));
                }
                new_content.replace_range(start..end, text);
            }
            None => new_content = text.to_string(),
        }
    }
    Ok(new_content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn incremental_and_full() {
        let changes = [
            json!({"range": {"start": {"line": 1, "character": 1}, "end": {"line": 1, "character": 2}}, "text": "XY"}),
            json!({"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}}, "text": "😀"}),
            json!({"range": {"start": {"line": 0, "character": 2}, "end": {"line": 0, "character": 3}}, "text": "-"}),
        ];
        assert_eq!(apply_changes("ab\ncde\n", &changes).unwrap(), "😀-b\ncXYe\n");
        assert_eq!(apply_changes("x", &[json!({"text": "y"})]).unwrap(), "y");
        let bad = [json!({"range": {"start": {"line": 5, "character": 0}, "end": {"line": 5, "character": 0}}, "text": ""})];
        assert_eq!(
            apply_changes("x", &bad).unwrap_err().code,
            codes::CLIENT_SERVER_INCONSISTENT_STATE
        );
    }
}
