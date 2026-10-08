// Dart source: pkg/analyzer/lib/src/ignore_comments/ignore_info.dart
use crate::LinterContext;
use dartr_diagnostics::Diagnostic;
use dartr_syntax::TokenId;
use dartr_syntax::token::flags;
use indexmap::IndexMap;

#[derive(Default)]
pub struct IgnoreInfo {
    ignored_for_file: Vec<String>,
    ignored_on_line: IndexMap<u32, Vec<String>>,
}
impl IgnoreInfo {
    pub fn for_dart(ctx: &LinterContext<'_>) -> Self {
        let mut result = Self::default();
        let utf16: Vec<_> = ctx.source.encode_utf16().collect();
        for index in 0..ctx.ast.tokens.len() {
            let id = TokenId(index as u32);
            let token = ctx.ast.tokens.get(id);
            if token.flags & flags::COMMENT == 0 {
                continue;
            }
            let text = ctx.ast.tokens.lexeme(id);
            let Some(rest) = text.strip_prefix("//") else {
                continue;
            };
            let inline = rest
                .trim_start_matches('/')
                .trim_start_matches(' ')
                .strip_prefix("ignore:");
            let file = rest
                .trim_start_matches(' ')
                .strip_prefix("ignore_for_file:");
            if let Some(rest) = inline {
                let location = ctx.parsed.line_info.get_location(token.offset);
                let start =
                    ctx.parsed.line_info.line_starts[location.line_number as usize - 1] as usize;
                let before = String::from_utf16_lossy(&utf16[start..token.offset as usize]);
                let line = location.line_number + u32::from(before.trim().is_empty());
                result
                    .ignored_on_line
                    .entry(line)
                    .or_default()
                    .extend(parse_ignored_elements(rest));
            } else if let Some(rest) = file {
                result.ignored_for_file.extend(parse_ignored_elements(rest));
            }
        }
        result
    }
    pub fn ignored(&self, ctx: &LinterContext<'_>, diagnostic: &Diagnostic) -> bool {
        let line = ctx
            .parsed
            .line_info
            .get_location(diagnostic.offset as u32)
            .line_number;
        self.ignored_for_file
            .iter()
            .chain(self.ignored_on_line.get(&line).into_iter().flatten())
            .any(|name| {
                name == diagnostic.code.name
                    || name == diagnostic.code.unique_name
                    || name == "type=lint"
            })
    }
}
fn parse_ignored_elements(mut text: &str) -> Vec<String> {
    let mut names = vec![];
    loop {
        text = text.trim_start();
        let length = text
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .count();
        if length == 0 || !text.as_bytes()[0].is_ascii_alphabetic() && text.as_bytes()[0] != b'_' {
            break;
        }
        let word = text[..length].to_ascii_lowercase();
        text = &text[length..];
        let name = if word == "type" {
            text = text.trim_start();
            let Some(rest) = text.strip_prefix('=') else {
                break;
            };
            text = rest.trim_start();
            let length = text
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .count();
            if length == 0 {
                break;
            }
            let value = format!("type={}", text[..length].to_ascii_lowercase());
            text = &text[length..];
            value
        } else if text.starts_with('/') {
            // Plugin-qualified diagnostic names never match built-in lint codes.
            text = &text[1..];
            let length = text
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .count();
            if length == 0 {
                break;
            }
            let value = format!("{word}/{}", &text[..length]);
            text = &text[length..];
            value
        } else {
            word
        };
        if !text.is_empty() && !text.starts_with(' ') && !text.starts_with(',') {
            break;
        }
        names.push(name);
        text = text.trim_start();
        let Some(rest) = text.strip_prefix(',') else {
            break;
        };
        text = rest;
    }
    names
}
