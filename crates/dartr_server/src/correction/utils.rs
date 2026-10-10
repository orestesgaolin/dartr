// Dart source: pkg/analysis_server_plugin/lib/edit/correction_utils.dart (CorrectionUtils: getLinePrefix, getLineContentStart, getLineContentEnd, endOfLine, getLinesRange, findNode)
// Dart source: pkg/analysis_server_plugin/lib/edit/range_factory.dart (RangeFactory)
// Dart source: pkg/analyzer/lib/src/dart/ast/ast.dart (AnnotatedNode.firstTokenAfterCommentAndMetadata)

//! The text and range helpers of the correction producers (Dart
//! `CorrectionUtils` and `RangeFactory`). Offsets are UTF-16.

use dartr_ast::*;
use dartr_syntax::{LineInfo, TokenId};

/// A source range (Dart `SourceRange`): offset and length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    pub offset: u32,
    pub length: u32,
}

impl Range {
    pub fn new(offset: u32, length: u32) -> Range {
        Range { offset, length }
    }

    pub fn end(&self) -> u32 {
        self.offset + self.length
    }

    /// Dart `SourceRange.contains`.
    pub fn contains(&self, offset: u32) -> bool {
        self.offset <= offset && offset < self.end()
    }
}

/// The source text of a unit with UTF-16 indexing.
pub struct Text {
    pub units: Vec<u16>,
}

impl Text {
    pub fn new(content: &str) -> Text {
        Text {
            units: content.encode_utf16().collect(),
        }
    }

    pub fn len(&self) -> u32 {
        self.units.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    /// Dart `substring(start, end)`.
    pub fn slice(&self, start: u32, end: u32) -> String {
        let start = (start as usize).min(self.units.len());
        let end = (end as usize).clamp(start, self.units.len());
        String::from_utf16_lossy(&self.units[start..end])
    }

    /// Dart `codeUnitAt`.
    pub fn at(&self, index: u32) -> u16 {
        self.units.get(index as usize).copied().unwrap_or(0)
    }
}

/// Dart `isSpace` (space or tab).
pub fn is_space(c: u16) -> bool {
    c == b' ' as u16 || c == b'\t' as u16
}

/// Dart `isEOL`.
pub fn is_eol(c: u16) -> bool {
    c == 0x0D || c == 0x0A
}

/// Dart `isWhitespace` (space, tab or EOL).
pub fn is_whitespace(c: u16) -> bool {
    is_space(c) || is_eol(c)
}

/// Dart `CorrectionUtils`.
pub struct CorrectionUtils<'a> {
    pub ast: &'a Ast,
    pub text: Text,
    pub line_info: &'a LineInfo,
    pub end_of_line: String,
}

impl<'a> CorrectionUtils<'a> {
    pub fn new(ast: &'a Ast, line_info: &'a LineInfo) -> Self {
        let content: &str = &ast.tokens.source;
        let end_of_line = super::change_builder::end_of_line(content)
            .unwrap_or("\n")
            .to_string();
        CorrectionUtils {
            ast,
            text: Text::new(content),
            line_info,
            end_of_line,
        }
    }

    /// Dart `getText(offset, length)`.
    pub fn get_text(&self, offset: u32, length: u32) -> String {
        self.text.slice(offset, offset + length)
    }

    /// Dart `getNodeText`.
    pub fn get_node_text(&self, node: impl Into<NodeId>) -> String {
        let node = node.into();
        self.get_text(self.ast.offset(node), self.ast.length(node))
    }

    /// Dart `getRangeText`.
    pub fn get_range_text(&self, range: Range) -> String {
        self.get_text(range.offset, range.length)
    }

    /// Dart `getLineContentStart`: skips spaces and tabs on the left.
    pub fn get_line_content_start(&self, index: u32) -> u32 {
        let mut index = index;
        while index > 0 {
            if !is_space(self.text.at(index - 1)) {
                break;
            }
            index -= 1;
        }
        index
    }

    /// Dart `getLineContentEnd`: skips whitespace and a single EOL on the
    /// right.
    pub fn get_line_content_end(&self, index: u32) -> u32 {
        let length = self.text.len();
        let mut index = index;
        while index < length {
            let c = self.text.at(index);
            if !is_whitespace(c) || is_eol(c) {
                break;
            }
            index += 1;
        }
        if index < length && self.text.at(index) == 0x0D {
            index += 1;
        }
        if index < length && self.text.at(index) == 0x0A {
            index += 1;
        }
        index
    }

    /// Dart `getLineNext`.
    pub fn get_line_next(&self, index: u32) -> u32 {
        let length = self.text.len();
        let mut index = index;
        while index < length {
            let c = self.text.at(index);
            if c == 0x0D || c == 0x0A {
                break;
            }
            index += 1;
        }
        if index < length && self.text.at(index) == 0x0D {
            index += 1;
        }
        if index < length && self.text.at(index) == 0x0A {
            index += 1;
        }
        index
    }

    /// Dart `getLinePrefix`.
    pub fn get_line_prefix(&self, index: u32) -> String {
        let line_start = self.get_line_this(index);
        let length = self.text.len();
        let mut end = line_start;
        while end < length {
            let c = self.text.at(end);
            if c == 0x0D || c == 0x0A || !is_whitespace(c) {
                break;
            }
            end += 1;
        }
        self.text.slice(line_start, end)
    }

    /// Dart `getLineThis`.
    pub fn get_line_this(&self, index: u32) -> u32 {
        let mut index = index;
        while index > 0 {
            let c = self.text.at(index - 1);
            if c == 0x0D || c == 0x0A {
                break;
            }
            index -= 1;
        }
        index
    }

    /// Dart `getPrefix`.
    pub fn get_prefix(&self, end_index: u32) -> String {
        let start = self.get_line_content_start(end_index);
        self.text.slice(start, end_index)
    }

    /// Dart `getNodePrefix`.
    pub fn get_node_prefix(&self, node: NodeId) -> String {
        let offset = self.ast.offset(node);
        if self.ast.is::<FunctionExpression>(node) {
            return self.get_line_prefix(offset);
        }
        self.get_prefix(offset)
    }

    /// Dart `getLinesRange`.
    pub fn get_lines_range(&self, range: Range, skip_leading_empty_lines: bool) -> Range {
        let start_offset = range.offset;
        let mut start_line_offset = self.get_line_content_start(start_offset);
        if skip_leading_empty_lines {
            start_line_offset = self.skip_empty_lines_left(start_line_offset);
        }
        let end_offset = range.end();
        let mut after_end_line_offset = end_offset;
        let line = self.line_info.get_location(start_line_offset).line_number - 1;
        let line_start = self.line_info.line_starts[line as usize];
        if line_start == start_line_offset {
            after_end_line_offset = self.get_line_content_end(end_offset);
        }
        Range::new(start_line_offset, after_end_line_offset - start_line_offset)
    }

    /// Dart `_skipEmptyLinesLeft`.
    fn skip_empty_lines_left(&self, index: u32) -> u32 {
        let mut start_of_target_line = index;
        let mut index = index;
        while index > 0 {
            let c = self.text.at(index - 1);
            if !is_whitespace(c) {
                return start_of_target_line;
            }
            if c == 0x0A {
                start_of_target_line = index;
            }
            index -= 1;
        }
        0
    }

    /// Dart `findNode`.
    pub fn find_node(&self, unit: NodeId, offset: u32) -> Option<NodeId> {
        self.ast.node_covering(unit, offset, 0)
    }

    /// Dart `getIndent(level)`.
    pub fn get_indent(&self, level: usize) -> String {
        "  ".repeat(level)
    }

    /// Dart `oneIndent`.
    pub fn one_indent(&self) -> &'static str {
        "  "
    }
}

/// Dart `RangeFactory` (`range`): ranges of nodes and tokens.
pub struct RangeFactory<'a> {
    pub ast: &'a Ast,
}

impl<'a> RangeFactory<'a> {
    pub fn new(ast: &'a Ast) -> Self {
        RangeFactory { ast }
    }

    fn token_offset(&self, t: TokenId) -> u32 {
        self.ast.tokens.get(t).offset
    }

    fn token_end(&self, t: TokenId) -> u32 {
        self.ast.tokens.get(t).end()
    }

    /// Dart `range.node`.
    pub fn node(&self, node: impl Into<NodeId>) -> Range {
        let node = node.into();
        Range::new(self.ast.offset(node), self.ast.length(node))
    }

    /// Dart `range.token`.
    pub fn token(&self, t: TokenId) -> Range {
        let token = self.ast.tokens.get(t);
        Range::new(token.offset, token.length)
    }

    /// Dart `range.startEnd` of offsets.
    pub fn start_offset_end_offset(&self, start: u32, end: u32) -> Range {
        Range::new(start, end.saturating_sub(start))
    }

    /// Dart `range.startStart` (nodes).
    pub fn start_start(&self, a: impl Into<NodeId>, b: impl Into<NodeId>) -> Range {
        let start = self.ast.offset(a.into());
        self.start_offset_end_offset(start, self.ast.offset(b.into()))
    }

    /// Dart `range.startEnd` (nodes).
    pub fn start_end(&self, a: impl Into<NodeId>, b: impl Into<NodeId>) -> Range {
        let start = self.ast.offset(a.into());
        self.start_offset_end_offset(start, self.ast.end(b.into()))
    }

    /// Dart `range.endEnd` (nodes).
    pub fn end_end(&self, a: impl Into<NodeId>, b: impl Into<NodeId>) -> Range {
        let start = self.ast.end(a.into());
        self.start_offset_end_offset(start, self.ast.end(b.into()))
    }

    /// Dart `range.endStart` (nodes).
    pub fn end_start(&self, a: impl Into<NodeId>, b: impl Into<NodeId>) -> Range {
        let start = self.ast.end(a.into());
        self.start_offset_end_offset(start, self.ast.offset(b.into()))
    }

    /// Token variants.
    pub fn token_start_end(&self, a: TokenId, b: TokenId) -> Range {
        self.start_offset_end_offset(self.token_offset(a), self.token_end(b))
    }

    pub fn token_start_start(&self, a: TokenId, b: TokenId) -> Range {
        self.start_offset_end_offset(self.token_offset(a), self.token_offset(b))
    }

    pub fn token_end_end(&self, a: TokenId, b: TokenId) -> Range {
        self.start_offset_end_offset(self.token_end(a), self.token_end(b))
    }

    pub fn token_end_start(&self, a: TokenId, b: TokenId) -> Range {
        self.start_offset_end_offset(self.token_end(a), self.token_offset(b))
    }

    /// Dart `range.startOffsetEndOffset`/`range.startEnd` mixing a token
    /// start and a node end.
    pub fn token_start_node_end(&self, a: TokenId, b: impl Into<NodeId>) -> Range {
        self.start_offset_end_offset(self.token_offset(a), self.ast.end(b.into()))
    }

    pub fn node_start_token_end(&self, a: impl Into<NodeId>, b: TokenId) -> Range {
        self.start_offset_end_offset(self.ast.offset(a.into()), self.token_end(b))
    }

    pub fn node_start_token_start(&self, a: impl Into<NodeId>, b: TokenId) -> Range {
        self.start_offset_end_offset(self.ast.offset(a.into()), self.token_offset(b))
    }

    pub fn token_start_node_start(&self, a: TokenId, b: impl Into<NodeId>) -> Range {
        self.start_offset_end_offset(self.token_offset(a), self.ast.offset(b.into()))
    }

    pub fn node_end_token_end(&self, a: impl Into<NodeId>, b: TokenId) -> Range {
        self.start_offset_end_offset(self.ast.end(a.into()), self.token_end(b))
    }

    pub fn token_end_node_end(&self, a: TokenId, b: impl Into<NodeId>) -> Range {
        self.start_offset_end_offset(self.token_end(a), self.ast.end(b.into()))
    }

    /// Dart `range.nodeInList`: the range to delete [item] of [list]
    /// (with a separating comma).
    pub fn node_in_list(&self, list: &[NodeId], item: NodeId) -> Range {
        let ast = self.ast;
        let Some(index) = list.iter().position(|n| *n == item) else {
            return self.node(item);
        };
        if list.len() == 1 {
            let next = ast.tokens.next(ast.end_token(item));
            if ast.tokens.ty(next) == dartr_syntax::TokenType::COMMA {
                return self.node_start_token_end(item, next);
            }
            return self.node(item);
        }
        if index == list.len() - 1 {
            // Delete the preceding comma.
            return self.end_end(list[index - 1], item);
        }
        self.start_start(item, list[index + 1])
    }

    /// Dart `range.deletionRange`: the node with the whitespace after it,
    /// up to the next token.
    pub fn deletion_range(&self, node: impl Into<NodeId>) -> Range {
        let node = node.into();
        let next = self.ast.tokens.next(self.ast.end_token(node));
        let next = self.comment_or_token(next);
        self.start_offset_end_offset(self.ast.offset(node), self.token_offset(next))
    }

    /// The first comment before [t], or [t].
    pub fn comment_or_token(&self, t: TokenId) -> TokenId {
        let c = self.ast.tokens.get(t).preceding_comments;
        if c.is_some() { c } else { t }
    }
}

/// Dart `AnnotatedNode.firstTokenAfterCommentAndMetadata` for the nodes
/// that have it; the begin token otherwise.
pub fn first_token_after_comment_and_metadata(ast: &Ast, node: NodeId) -> TokenId {
    macro_rules! dispatch {
        ($($t:ident),*) => {
            $(if let Some(n) = ast.cast::<$t>(node) {
                return ast[n].first_token_after_comment_and_metadata(ast);
            })*
        };
    }
    dispatch!(
        ClassDeclaration,
        ClassTypeAlias,
        ConstructorDeclaration,
        DeclaredIdentifier,
        EnumConstantDeclaration,
        EnumDeclaration,
        ExportDirective,
        ExtensionDeclaration,
        ExtensionTypeDeclaration,
        FieldDeclaration,
        FieldFormalParameter,
        FunctionDeclaration,
        FunctionTypeAlias,
        GenericTypeAlias,
        ImportDirective,
        LibraryDirective,
        MethodDeclaration,
        MixinDeclaration,
        PartDirective,
        PartOfDirective,
        PatternVariableDeclaration,
        PrimaryConstructorBody,
        RegularFormalParameter,
        SuperFormalParameter,
        TopLevelVariableDeclaration,
        TypeParameter,
        VariableDeclaration,
        VariableDeclarationList
    );
    ast.begin_token(node)
}

/// Dart `AnnotatedNode.documentationComment` for declarations and
/// directives.
pub fn documentation_comment(ast: &Ast, node: NodeId) -> Option<NodeId> {
    macro_rules! dispatch {
        ($($t:ident),*) => {
            $(if let Some(n) = ast.cast::<$t>(node) {
                return ast[n].documentation_comment.map(|c| c.raw());
            })*
        };
    }
    dispatch!(
        ClassDeclaration,
        ClassTypeAlias,
        ConstructorDeclaration,
        EnumConstantDeclaration,
        EnumDeclaration,
        ExportDirective,
        ExtensionDeclaration,
        ExtensionTypeDeclaration,
        FieldDeclaration,
        FunctionDeclaration,
        FunctionTypeAlias,
        GenericTypeAlias,
        ImportDirective,
        LibraryDirective,
        MethodDeclaration,
        MixinDeclaration,
        PartDirective,
        PartOfDirective,
        TopLevelVariableDeclaration
    );
    None
}

// Dart source: pkg/analysis_server/lib/src/utilities/extensions/range_factory.dart (nodeWithComments, trailingComment, _leadingComment)

impl RangeFactory<'_> {
    fn on_same_line(&self, line_info: &LineInfo, a: u32, b: u32) -> bool {
        line_info.on_same_line(a, b)
    }

    /// Dart `nodeWithComments`.
    pub fn node_with_comments(&self, line_info: &LineInfo, node: NodeId) -> Range {
        let ast = self.ast;
        let begin = ast.begin_token(node);
        let root = ast.root(node);
        let is_first_item = begin == ast.begin_token(root);
        let leading = if is_first_item {
            begin
        } else {
            self.leading_comment(line_info, begin)
        };
        let (trailing, _) = self.trailing_comment(line_info, ast.end_token(node), false);
        self.token_start_end(leading, trailing)
    }

    /// Dart `trailingComment`: the token and whether a comma is included.
    pub fn trailing_comment(
        &self,
        line_info: &LineInfo,
        token: TokenId,
        return_comma: bool,
    ) -> (TokenId, bool) {
        let tokens = &self.ast.tokens;
        let mut last = token;
        let mut next = tokens.next(last);
        let includes_comma = tokens.ty(next) == dartr_syntax::TokenType::COMMA
            && self.should_include_comments_after_comma(line_info, next);
        if includes_comma {
            last = next;
            next = tokens.next(last);
        }
        let mut comment = tokens.get(next).preceding_comments.get();
        if comment.is_none()
            && includes_comma
            && !self.on_same_line(line_info, tokens.get(token).offset, tokens.get(last).offset)
        {
            comment = tokens.get(last).preceding_comments.get();
            last = token;
        }
        if let Some(mut c) = comment {
            let last_offset = tokens.get(last).offset;
            if self.on_same_line(line_info, tokens.get(c).offset, last_offset) {
                let mut n = tokens.get(c).next.get();
                while let Some(x) = n {
                    if !self.on_same_line(line_info, tokens.get(x).offset, last_offset) {
                        break;
                    }
                    c = x;
                    n = tokens.get(x).next.get();
                }
                return (c, includes_comma);
            }
        }
        (if return_comma { last } else { token }, false)
    }

    /// Dart `_leadingComment`.
    pub fn leading_comment(&self, line_info: &LineInfo, token: TokenId) -> TokenId {
        let tokens = &self.ast.tokens;
        let previous = tokens.previous(token);
        let preceding = tokens.get(token).preceding_comments.get();
        if previous.is_none() || tokens.get(previous).is_eof() {
            return preceding.unwrap_or(token);
        }
        let mut comment = preceding;
        if !self.on_same_line(
            line_info,
            tokens.get(token).offset,
            tokens.get(previous).offset,
        ) {
            while let Some(c) = comment {
                if !self.on_same_line(line_info, tokens.get(previous).offset, tokens.get(c).offset)
                {
                    break;
                }
                comment = tokens.get(c).next.get();
            }
        }
        comment.unwrap_or(token)
    }

    /// Dart `_shouldIncludeCommentsAfterComma`.
    fn should_include_comments_after_comma(&self, line_info: &LineInfo, comma: TokenId) -> bool {
        use dartr_syntax::TokenType;
        let tokens = &self.ast.tokens;
        let after = tokens.next(comma);
        let ty = tokens.ty(after);
        if matches!(
            ty,
            TokenType::CLOSE_CURLY_BRACKET
                | TokenType::CLOSE_PAREN
                | TokenType::CLOSE_SQUARE_BRACKET
        ) {
            return true;
        }
        !self.on_same_line(
            line_info,
            tokens.get(comma).offset,
            tokens.get(after).offset,
        )
    }
}
