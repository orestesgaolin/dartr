// Dart source: pkg/analysis_server/lib/src/services/correction/organize_imports.dart (ImportOrganizer)
// Dart source: pkg/analysis_server/lib/src/utilities/strings.dart (findCommonSuffix)

//! Dart `ImportOrganizer`: sorts the directives of a unit by kind and URI,
//! groups them with blank lines and removes unused imports.

use std::cmp::Ordering;

use dartr_ast::*;
use dartr_ast_builder::ignore_info::IgnoreInfo;
use dartr_diagnostics::Diagnostic;
use dartr_element::{Ctx, FId, LibraryFragment, ResolutionTables};
use dartr_resolver::element_metadata::{AnnotationRef, TargetKind, UnitAst, target_kinds};
use dartr_syntax::token::flags;
use dartr_syntax::{LineInfo, TokenId};

use super::change::SourceEdit;
use super::imports::{
    DirectiveSortKind, compare_directive_uri, dart_compare, directive_sort_priority,
};
use super::utils::Text;

/// Dart `findCommonSuffix` (UTF-16 code units).
pub fn find_common_suffix(a: &[u16], b: &[u16]) -> usize {
    let n = a.len().min(b.len());
    let mut i = 0;
    while i < n && a[a.len() - 1 - i] == b[b.len() - 1 - i] {
        i += 1;
    }
    i
}

struct DirectiveInfo {
    directive: NodeId,
    priority: u32,
    uri: String,
    offset: u32,
    end: u32,
    text: String,
}

impl DirectiveInfo {
    fn compare(&self, other: &DirectiveInfo) -> Ordering {
        if self.priority == other.priority {
            let c = compare_directive_uri(&self.uri, &other.uri);
            if c != Ordering::Equal {
                return c;
            }
            return dart_compare(&self.text, &other.text);
        }
        self.priority.cmp(&other.priority)
    }
}

/// The resolution of the unit (for the targets of annotations).
pub struct Resolution<'a> {
    pub ctx: &'a Ctx<'a>,
    pub tables: &'a ResolutionTables,
    pub fragment: FId<LibraryFragment>,
}

/// Dart `ImportOrganizer`.
pub struct ImportOrganizer<'a> {
    /// `None` for a parsed unit.
    pub resolution: Option<Resolution<'a>>,
    pub ast: &'a Ast,
    pub unit: Id<CompilationUnit>,
    pub line_info: &'a LineInfo,
    pub diagnostics: &'a [Diagnostic],
    pub remove_unused: bool,
}

impl ImportOrganizer<'_> {
    /// Dart `organize`: the edits (at most one).
    pub fn organize(&self) -> Vec<SourceEdit> {
        let initial = Text::new(&self.ast.tokens.source);
        let code = self.organize_code(&initial);
        let mut edits = Vec::new();
        if code != initial.units {
            let suffix = find_common_suffix(&initial.units, &code);
            edits.push(SourceEdit {
                offset: 0,
                length: (initial.units.len() - suffix) as u32,
                replacement: String::from_utf16_lossy(&code[..code.len() - suffix]),
                id: 0,
            });
        }
        edits
    }

    fn has_unresolved_identifier_error(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.code.is_unresolved_identifier)
    }

    /// Dart `_isUnusedImport`.
    fn is_unused_import(&self, directive: NodeId) -> bool {
        let uri = directive_uri_node(self.ast, directive);
        let Some(uri) = uri else { return false };
        let offset = self.ast.offset(uri);
        self.diagnostics.iter().any(|d| {
            matches!(
                d.code.unique_name,
                "duplicate_import" | "unused_import" | "unnecessary_import"
            ) && d.offset as u32 == offset
        })
    }

    /// Dart `_isUnusedShowName`.
    fn is_unused_show_name(&self, name: Id<SimpleIdentifier>) -> bool {
        let offset = self.ast.offset(name);
        self.diagnostics
            .iter()
            .any(|d| d.code.unique_name == "unused_shown_name" && d.offset as u32 == offset)
    }

    /// Dart `_isLibraryTargetAnnotation`.
    fn is_library_target_annotation(&self, annotation: Id<Annotation>) -> bool {
        let Some(r) = &self.resolution else {
            return false;
        };
        let unit = UnitAst {
            ast: self.ast,
            tables: r.tables,
        };
        let a = AnnotationRef::of_node(r.ctx, unit, annotation, r.fragment);
        target_kinds(r.ctx, &a).is_some_and(|k| k.contains(&TargetKind::Library))
    }

    /// Dart `_organizeDirectives` on [text] (the code of the unit, maybe
    /// with edits after the directives): the new code.
    pub fn organize_code(&self, text: &Text) -> Vec<u16> {
        let ast = self.ast;
        let line_info = self.line_info;
        let code = &text.units;
        let mut has_library_directive = false;
        let mut directives: Vec<DirectiveInfo> = Vec::new();
        let all: Vec<NodeId> = ast.list_raw(ast[self.unit].directives).to_vec();
        for &directive in &all {
            if ast.is::<LibraryDirective>(directive) {
                has_library_directive = true;
            }
            let kind = if ast.is::<ImportDirective>(directive) {
                DirectiveSortKind::Import
            } else if ast.is::<ExportDirective>(directive) {
                DirectiveSortKind::Export
            } else if ast.is::<PartDirective>(directive) {
                DirectiveSortKind::Part
            } else {
                continue;
            };
            let uri_content = directive_uri_node(ast, directive)
                .and_then(|u| dartr_resolver::element_metadata::string_value(ast, u))
                .unwrap_or_default();
            let priority = directive_sort_priority(&uri_content, kind);
            let mut offset = ast.offset(directive);
            let mut end = ast.end(directive);
            let is_pseudo_library_directive =
                !has_library_directive && all.first() == Some(&directive);
            let mut library_end: Option<u32> = None;
            let mut last_library_annotation: Option<Id<Annotation>> = None;
            if is_pseudo_library_directive {
                let metadata = directive_metadata(ast, directive);
                last_library_annotation = metadata
                    .iter()
                    .take_while(|a| self.is_library_target_annotation(**a))
                    .last()
                    .copied();
                library_end = last_library_annotation.map(|a| ast.end(a)).or_else(|| {
                    super::utils::documentation_comment(ast, directive).map(|c| ast.end(c))
                });
                if let Some(e) = library_end {
                    let mut e = line_info.get_offset_of_line_after(e).unwrap_or(e);
                    let next_line = line_info.get_offset_of_line_after(e).unwrap_or(e);
                    if text.slice(e, next_line).trim().is_empty() {
                        e = next_line;
                    }
                    library_end = Some(e);
                }
            }
            let leading_token = if last_library_annotation.is_none() {
                Some(ast.begin_token(directive))
            } else {
                None
            };
            let leading_comment = leading_token
                .and_then(|t| self.get_leading_comment(t, is_pseudo_library_directive));
            let trailing_comment = self.get_trailing_comment(directive);
            if let Some(c) = leading_comment {
                let comment_offset = ast.tokens.get(c).offset;
                offset = match library_end {
                    Some(e) => e.max(comment_offset),
                    None => comment_offset,
                };
            }
            if let Some(c) = trailing_comment {
                end = ast.tokens.get(c).end();
            }
            offset = library_end.unwrap_or(offset);
            let text_of = text.slice(offset, end);
            directives.push(DirectiveInfo {
                directive,
                priority,
                uri: uri_content,
                offset,
                end,
                text: text_of,
            });
        }
        if directives.is_empty() {
            return code.clone();
        }
        let first_directive_offset = directives.first().unwrap().offset;
        let last_directive_end = directives.last().unwrap().end;
        // Dart `List.sort` is not stable, but equal elements have equal text.
        directives.sort_by(|a, b| a.compare(b));
        let eol = if String::from_utf16_lossy(code).contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let has_unresolved = self.has_unresolved_identifier_error();
        let mut sb = String::new();
        let mut current_priority: Option<u32> = None;
        let mut previous_text = String::new();
        let mut show_combinators: Vec<(NodeId, Vec<Id<SimpleIdentifier>>)> = Vec::new();
        for info in &directives {
            if !has_unresolved {
                let directive = info.directive;
                if self.remove_unused && self.is_unused_import(directive)
                    || (self.remove_unused && previous_text == info.text)
                {
                    continue;
                }
                if let Some(i) = ast.cast::<ImportDirective>(directive) {
                    let combinators = ast.list_raw(ast[i].combinators);
                    if !combinators.is_empty() {
                        let mut list = Vec::new();
                        for &c in combinators {
                            if let Some(show) = ast.cast::<ShowCombinator>(c) {
                                for &name in ast.list(ast[show].shown_names) {
                                    if self.is_unused_show_name(name) {
                                        list.push(name);
                                    }
                                }
                            }
                        }
                        show_combinators.push((directive, list));
                    }
                }
            }
            if current_priority != Some(info.priority) {
                if current_priority.is_some() {
                    sb.push_str(eol);
                }
                current_priority = Some(info.priority);
            }
            let mut text_of = info.text.clone();
            if let Some((_, list)) = show_combinators.iter().find(|(d, _)| *d == info.directive) {
                let show_offset = text_of.find("show").unwrap_or(0);
                for name in list {
                    let name = ast.tokens.lexeme(ast[*name].token);
                    if text_of.contains(&format!("{name},")) {
                        text_of =
                            replace_first_from(&text_of, &format!("{name}, "), "", show_offset);
                    } else if text_of.contains(&format!(", {name}")) {
                        text_of =
                            replace_first_from(&text_of, &format!(", {name}"), "", show_offset);
                    }
                }
            }
            sb.push_str(&text_of);
            sb.push_str(eol);
            previous_text = text_of;
        }
        let directives_code = sb.trim_end().to_string();
        let mut out: Vec<u16> = code[..first_directive_offset as usize].to_vec();
        out.extend(directives_code.encode_utf16());
        out.extend(&code[(last_directive_end as usize).min(code.len())..]);
        out
    }

    /// Dart `getLeadingComment`.
    fn get_leading_comment(
        &self,
        begin_token: TokenId,
        is_pseudo_library_directive: bool,
    ) -> Option<TokenId> {
        let ast = self.ast;
        let tokens = &ast.tokens;
        let line_info = self.line_info;
        let first = tokens.get(begin_token).preceding_comments.get()?;
        let mut first_comment = Some(first);
        let mut comment = Some(first);
        let mut next_comment = tokens.get(first).next.get();
        while is_pseudo_library_directive && comment.is_some() && next_comment.is_some() {
            let (c, n) = (comment.unwrap(), next_comment.unwrap());
            if line_info.line_number_difference(tokens.get(c).offset, tokens.get(n).offset) > 1 {
                first_comment = Some(n);
            }
            comment = Some(n);
            next_comment = tokens.get(n).next.get();
        }
        if let Some(fc) = first_comment {
            if tokens.get(fc).flags & flags::LANGUAGE_VERSION != 0 {
                first_comment = tokens.get(fc).next.get();
            }
        }
        let unit_begin = ast[self.unit].begin_token;
        if let Some(fc) = first_comment {
            if tokens.get(unit_begin).preceding_comments.get() == Some(fc) {
                return if IgnoreInfo::is_ignore_comment(tokens.lexeme(fc)) {
                    Some(fc)
                } else {
                    None
                };
            }
        }
        let mut comment = first_comment;
        if is_pseudo_library_directive {
            if let Some(c) = comment {
                if line_info
                    .line_number_difference(tokens.get(begin_token).offset, tokens.get(c).offset)
                    == -1
                {
                    return Some(c);
                } else {
                    return None;
                }
            }
        }
        let previous_end = tokens.get(tokens.previous(begin_token)).end();
        while let Some(c) = comment {
            if !line_info.on_same_line(previous_end, tokens.get(c).offset) {
                break;
            }
            comment = tokens.get(c).next.get();
        }
        comment
    }

    /// Dart `getTrailingComment`.
    fn get_trailing_comment(&self, directive: NodeId) -> Option<TokenId> {
        let ast = self.ast;
        let tokens = &ast.tokens;
        let next = tokens.next(ast.end_token(directive));
        let end = ast.end(directive);
        let mut comment = tokens.get(next).preceding_comments.get();
        while let Some(c) = comment {
            if self.line_info.on_same_line(tokens.get(c).offset, end) {
                return Some(c);
            }
            comment = tokens.get(c).next.get();
        }
        None
    }
}

/// Dart `String.replaceFirst(from, to, startIndex)`.
fn replace_first_from(s: &str, from: &str, to: &str, start: usize) -> String {
    match s[start.min(s.len())..].find(from) {
        Some(i) => {
            let i = start + i;
            format!("{}{}{}", &s[..i], to, &s[i + from.len()..])
        }
        None => s.to_string(),
    }
}

fn directive_uri_node(ast: &Ast, directive: NodeId) -> Option<NodeId> {
    if let Some(d) = ast.cast::<ImportDirective>(directive) {
        Some(ast[d].uri.raw())
    } else if let Some(d) = ast.cast::<ExportDirective>(directive) {
        Some(ast[d].uri.raw())
    } else {
        ast.cast::<PartDirective>(directive)
            .map(|d| ast[d].uri.raw())
    }
}

fn directive_metadata(ast: &Ast, directive: NodeId) -> Vec<Id<Annotation>> {
    if let Some(d) = ast.cast::<ImportDirective>(directive) {
        ast.list(ast[d].metadata).to_vec()
    } else if let Some(d) = ast.cast::<ExportDirective>(directive) {
        ast.list(ast[d].metadata).to_vec()
    } else if let Some(d) = ast.cast::<PartDirective>(directive) {
        ast.list(ast[d].metadata).to_vec()
    } else {
        Vec::new()
    }
}
