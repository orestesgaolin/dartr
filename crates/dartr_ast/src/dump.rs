// Dart source: tools/oracle/bin/oracle.dart (nodeJson, tokenJson)

//! The `ast` dump: each node as `{"t": <class name without Impl>, "o":
//! offset, "e": end, "c": [child entities]}`, each token as `{"k": type
//! name, "o": offset, "l": length, "x": lexeme, "syn": true (if synthetic)}`.
//! The output is byte for byte what the oracle (`dart jsonEncode`) writes.

use std::fmt::Write;

use dartr_syntax::TokenId;

use crate::arena::{Ast, Entity, NodeId};

/// Writes [s] as a JSON string like Dart `jsonEncode`.
pub fn write_json_string(out: &mut String, s: &str) {
    out.push('"');
    let bytes = s.as_bytes();
    let mut start = 0;
    for (i, &b) in bytes.iter().enumerate() {
        if b >= 0x20 && b != b'"' && b != b'\\' {
            continue;
        }
        out.push_str(&s[start..i]);
        start = i + 1;
        match b {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            8 => out.push_str("\\b"),
            9 => out.push_str("\\t"),
            10 => out.push_str("\\n"),
            12 => out.push_str("\\f"),
            13 => out.push_str("\\r"),
            _ => {
                let _ = write!(out, "\\u{:04x}", b);
            }
        }
    }
    out.push_str(&s[start..]);
    out.push('"');
}

/// Oracle `tokenJson` (without comments).
pub fn write_token_json(out: &mut String, ast: &Ast, id: TokenId) {
    let t = ast.tokens.get(id);
    out.push_str("{\"k\":\"");
    out.push_str(t.ty.name());
    let _ = write!(out, "\",\"o\":{},\"l\":{},\"x\":", t.offset, t.length);
    write_json_string(out, ast.tokens.lexeme(id));
    if t.is_synthetic() {
        out.push_str(",\"syn\":true");
    }
    out.push('}');
}

/// Oracle `nodeJson`.
pub fn write_node_json(out: &mut String, ast: &Ast, id: NodeId) {
    out.push_str("{\"t\":\"");
    out.push_str(ast.kind(id).name());
    let _ = write!(out, "\",\"o\":{},\"e\":{},\"c\":[", ast.offset(id), ast.end(id));
    for (i, e) in ast.child_entities(id).into_iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        match e {
            Entity::Node(n) => write_node_json(out, ast, n),
            Entity::Token(t) => write_token_json(out, ast, t),
        }
    }
    out.push_str("]}");
}

/// The `ast` dump of [id] as a string.
pub fn node_json(ast: &Ast, id: impl Into<NodeId>) -> String {
    let mut out = String::new();
    write_node_json(&mut out, ast, id.into());
    out
}
