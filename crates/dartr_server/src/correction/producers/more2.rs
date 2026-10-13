// Dart source: pkg/analysis_server/lib/src/services/correction/dart/add_return_null.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_comment.dart (RemoveComment, _RemoveIgnoreComment)
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_ignored_diagnostic.dart
// Dart source: pkg/analyzer/lib/src/utilities/extensions/ast.dart (commentTokenCovering)
// Dart source: pkg/analyzer/lib/src/ignore_comments/ignore_info.dart (ignoredElements: the names)

//! Fixes for `body_might_complete_normally_nullable` and the unnecessary
//! ignore comments.

use dartr_ast::*;
use dartr_syntax::TokenId;

use super::super::change_builder::ChangeBuilder;
use super::super::fix_kind::FixKind;
use super::super::generated::fix_kinds as k;
use super::super::producer::*;
use super::simple::producer;

producer!(
    AddReturnNull,
    k::ADD_RETURN_NULL,
    Some(&k::ADD_RETURN_NULL_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let node = c.node;
        let block = if let Some(b) = ast.cast::<Block>(node) {
            b
        } else {
            let body = if let Some(f) = ast.cast::<FunctionDeclaration>(node) {
                ast[ast[f].function_expression].body
            } else if let Some(m) = ast.cast::<MethodDeclaration>(node) {
                ast[m].body
            } else {
                return;
            };
            let Some(b) = ast.cast::<BlockFunctionBody>(body) else {
                return;
            };
            ast[b].block
        };
        let eol = c.eol().to_string();
        let indent = c.utils.one_indent();
        let statements = ast.list(ast[block].statements);
        let (position, text) = match statements.last() {
            None => {
                let prefix = c.utils.get_line_prefix(ast.offset(block));
                (
                    ast.offset(block) + 1,
                    format!("{eol}{prefix}{indent}return null;{eol}{prefix}"),
                )
            }
            Some(&last) => {
                let prefix = c.utils.get_node_prefix(last.raw());
                (ast.end(last), format!("{eol}{prefix}return null;"))
            }
        };
        builder.add_dart_file_edit(c.path, |b| b.add_simple_insertion(position, &text));
    }
);

/// Dart `AstNode.commentTokenCovering(offset)`: the comment token of the
/// unit that covers [offset].
fn comment_covering(c: &ProducerContext<'_>, offset: u32) -> Option<TokenId> {
    let tokens = &c.ast.tokens;
    let mut t = c.ast.begin_token(c.unit);
    loop {
        let mut comment = tokens.get(t).preceding_comments;
        while comment.is_some() && tokens.get(comment).is_comment() {
            let token = tokens.get(comment);
            if token.offset <= offset && offset < token.end() {
                return Some(comment);
            }
            let next = tokens.next(comment);
            if next == comment {
                break;
            }
            comment = next;
        }
        if tokens.get(t).is_eof() {
            return None;
        }
        let next = tokens.next(t);
        if next.is_none() || next == t {
            return None;
        }
        t = next;
    }
}

/// Dart `RemoveComment.ignore` (`_RemoveIgnoreComment`).
pub struct RemoveIgnoreComment;

impl CorrectionProducer for RemoveIgnoreComment {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::REMOVE_UNNECESSARY_IGNORE_COMMENT)
    }

    fn multi_fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::REMOVE_UNNECESSARY_IGNORE_COMMENT_MULTI)
    }

    fn applicability(&self) -> Applicability {
        Applicability::Automatically
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let Some(d) = c.diagnostic else { return };
        let Some(comment) = comment_covering(c, d.offset as u32) else {
            return;
        };
        let offset = c.token_offset(comment);
        let comment_end = c.token_end(comment);
        let mut start = c.utils.get_line_content_start(offset);
        let mut end = c.utils.get_line_content_end(comment_end);
        let next_line = c.utils.get_line_next(comment_end);
        if next_line != end {
            start = offset;
        } else if start != c.utils.get_line_this(offset) {
            end -= c.eol().len() as u32;
        }
        builder.add_dart_file_edit(c.path, |b| b.add_deletion(start, end - start));
    }
}

/// The diagnostic names of an ignore comment with their offsets in the
/// comment (Dart `CommentToken.ignoredElements`, the names only).
fn ignored_names(lexeme: &str) -> Vec<(usize, String)> {
    let Some(colon) = lexeme
        .find("ignore:")
        .map(|i| i + "ignore:".len())
        .or_else(|| {
            lexeme
                .find("ignore_for_file:")
                .map(|i| i + "ignore_for_file:".len())
        })
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut index = colon;
    for part in lexeme[colon..].split(',') {
        let trimmed = part.trim_start();
        let start = index + (part.len() - trimmed.len());
        let name: String = trimmed
            .chars()
            .take_while(|ch| {
                ch.is_alphanumeric() || *ch == '_' || *ch == '.' || *ch == '=' || *ch == '-'
            })
            .collect();
        if !name.is_empty() && !name.contains('=') {
            out.push((start, name));
        }
        index += part.len() + 1;
    }
    out
}

/// Dart `RemoveIgnoredDiagnostic`.
pub struct RemoveIgnoredDiagnostic {
    name: String,
}

impl RemoveIgnoredDiagnostic {
    pub fn new() -> Self {
        RemoveIgnoredDiagnostic {
            name: String::new(),
        }
    }
}

impl CorrectionProducer for RemoveIgnoredDiagnostic {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::REMOVE_IGNORED_DIAGNOSTIC)
    }

    fn multi_fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::REMOVE_IGNORED_DIAGNOSTIC_MULTI)
    }

    fn fix_arguments(&self) -> Vec<String> {
        vec![self.name.clone()]
    }

    fn applicability(&self) -> Applicability {
        Applicability::Automatically
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let Some(d) = c.diagnostic else { return };
        let diagnostic_offset = d.offset as u32;
        let Some(comment) = comment_covering(c, diagnostic_offset) else {
            return;
        };
        let comment_offset = c.token_offset(comment);
        let text: Vec<u16> = c.lexeme(comment).encode_utf16().collect();
        let in_comment = (diagnostic_offset - comment_offset) as usize;
        let mut range: Option<(u32, u32)> = None;
        for (index, (start, name)) in ignored_names(c.lexeme(comment)).into_iter().enumerate() {
            let element_offset = comment_offset + start as u32;
            if element_offset != diagnostic_offset {
                continue;
            }
            self.name = name.clone();
            let name_len = name.encode_utf16().count() as u32;
            if index != 0 {
                let mut offset = in_comment as i64;
                while offset > -1 {
                    if text[offset as usize] == b',' as u16 {
                        let back = (in_comment as i64 - offset) as u32;
                        range = Some((diagnostic_offset - back, name_len + back));
                        break;
                    }
                    offset -= 1;
                }
            } else {
                let Some(mut chars) = text[in_comment..].iter().position(|&ch| ch == b',' as u16)
                else {
                    return;
                };
                chars += 1;
                if text.get(in_comment + chars) == Some(&(b' ' as u16)) {
                    chars += 1;
                }
                range = Some((element_offset, chars as u32));
            }
        }
        if let Some((offset, length)) = range {
            builder.add_dart_file_edit(c.path, |b| b.add_deletion(offset, length));
        }
    }
}

/// Dart `CreateFile` (`pkg/analysis_server/lib/src/services/correction/dart/create_file.dart`).
#[derive(Default)]
pub struct CreateFile {
    file_name: String,
}

impl CreateFile {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CorrectionProducer for CreateFile {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::CREATE_FILE)
    }

    fn fix_arguments(&self) -> Vec<String> {
        vec![self.file_name.clone()]
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        if !ast.is::<SimpleStringLiteral>(c.node) {
            return;
        }
        let Some(parent) = ast.parent(c.node) else {
            return;
        };
        let Some(fragment) = c
            .tables
            .declared_fragment
            .get(c.unit.raw())
            .and_then(|f| f.cast::<dartr_element::LibraryFragment>())
        else {
            return;
        };
        let data = c.ctx.fragment(fragment);
        let directives = ast.list_raw(ast[c.unit].directives);
        if ast.is::<ImportDirective>(parent) || ast.is::<ExportDirective>(parent) {
            let uri = if ast.is::<ImportDirective>(parent) {
                let Some(index) = directives
                    .iter()
                    .filter(|d| ast.is::<ImportDirective>(**d))
                    .position(|d| *d == parent)
                else {
                    return;
                };
                data.library_imports
                    .iter()
                    .filter(|i| !i.is_synthetic)
                    .nth(index)
                    .map(|i| &i.directive.uri)
            } else {
                let Some(index) = directives
                    .iter()
                    .filter(|d| ast.is::<ExportDirective>(**d))
                    .position(|d| *d == parent)
                else {
                    return;
                };
                data.library_exports.get(index).map(|e| &e.directive.uri)
            };
            let Some(
                dartr_element::DirectiveUri::Source { source, .. }
                | dartr_element::DirectiveUri::Library { source, .. },
            ) = uri
            else {
                return;
            };
            let full_name = &*source.path;
            if full_name.starts_with('/') && full_name.ends_with(".dart") {
                builder.add_new_dart_file_edit(full_name, &source.uri, c.resolved, |b| {
                    b.add_simple_insertion(0, "// TODO Implement this library.");
                });
                self.file_name = full_name
                    .rsplit('/')
                    .next()
                    .unwrap_or(full_name)
                    .to_string();
            }
        } else if ast.is::<PartDirective>(parent) {
            let Some(index) = directives
                .iter()
                .filter(|d| ast.is::<PartDirective>(**d))
                .position(|d| *d == parent)
            else {
                return;
            };
            let Some(part) = data.parts.get(index) else {
                return;
            };
            let source = match &part.directive.uri {
                dartr_element::DirectiveUri::Source { source, .. } => source,
                dartr_element::DirectiveUri::Unit {
                    library_fragment, ..
                } => &c.ctx.fragment(*library_fragment).source,
                _ => return,
            };
            let full_name = &*source.path;
            if !full_name.starts_with('/') || !full_name.ends_with(".dart") {
                return;
            }
            let library_path = &*data.source.path;
            let from = match full_name.rfind('/') {
                Some(i) => &full_name[..i],
                None => ".",
            };
            let to: Vec<&str> = library_path.split('/').filter(|s| !s.is_empty()).collect();
            let from_parts: Vec<&str> = from.split('/').filter(|s| !s.is_empty()).collect();
            let common = to
                .iter()
                .zip(&from_parts)
                .take_while(|(a, b)| a == b)
                .count();
            let mut parts: Vec<&str> = vec![".."; from_parts.len() - common];
            parts.extend(&to[common..]);
            let relative = if parts.is_empty() {
                ".".to_string()
            } else {
                parts.join("/")
            };
            builder.add_new_dart_file_edit(full_name, &source.uri, c.resolved, |b| {
                let eol = b.eol();
                b.add_simple_insertion(0, &format!("part of '{relative}';{eol}{eol}"));
            });
            self.file_name = full_name
                .rsplit('/')
                .next()
                .unwrap_or(full_name)
                .to_string();
        }
    }
}
