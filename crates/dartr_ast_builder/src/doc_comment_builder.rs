// Dart source: pkg/analyzer/lib/src/fasta/doc_comment_builder.dart

//! Builds the `Comment` node of a documentation comment: comment
//! references, Markdown code blocks, doc directives and doc imports (Dart
//! `DocCommentBuilder`).
//!
//! # Port notes
//!
//! - Dart strings are UTF-16; the comment text is handled as UTF-16 code
//!   units (`Vec<u16>`) so that all indexes and offsets are the Dart ones.
//! - A comment reference is scanned into a separate token arena (Dart
//!   `scanString`) and rewritten there; then its tokens (from the first
//!   token up to the EOF token) are copied into the token arena of the unit
//!   with the offsets shifted by the offset of the reference, like Dart
//!   shifts the offsets of the scanned tokens.
//! - A doc import (`@docImport`) is scanned and parsed into its own
//!   [`Ast`] ([`DocImport::ast`]). Dart scans it with
//!   `DocImportStringScanner`, which maps the token offsets (`tokenStart`)
//!   into the unit; here the scanner adds the same offset to `tokenStart`
//!   (`dartr_syntax::scan_with_token_start_delta`), so tokens and error
//!   tokens made from `tokenStart` are in the unit, and error offsets that
//!   come from `stringOffset` stay offsets in the synthetic `import ...`
//!   text, like in Dart.

use dartr_ast::doc_comment::{
    BlockDocDirective, CodeBlockType, DocDirective, DocDirectiveArgument,
    DocDirectiveNamedArgument, DocDirectivePositionalArgument, DocDirectiveTag, DocDirectiveType,
    DocImport, MdCodeBlock, MdCodeBlockLine, SimpleDocDirective,
};
use dartr_ast::{
    Ast, Comment, CommentReference, Id, ImportDirective, PrefixedIdentifier, PropertyAccess,
    SimpleIdentifier,
};
use dartr_diagnostics::{LocatableDiagnostic, diag};
use dartr_parser::Parser;
use dartr_parser::experimental_features::ExperimentalFeatures;
use dartr_parser::parser_impl::synthetic_previous_token;
use dartr_parser::token_stream_rewriter::TokenStreamRewriter;
use dartr_parser::util::{is_letter, is_letter_or_digit, is_whitespace, optional};
use dartr_syntax::token::flags;
use dartr_syntax::{Token, TokenId, TokenType, Tokens, scan_string, scan_with_token_start_delta};

use crate::ast_builder::AstBuilder;
use crate::error_converter::FastaErrorReporter;
use crate::parse::LibraryLanguageVersion;

const LEFT_BRACKET: u16 = 0x5B; // `[`
const RIGHT_BRACKET: u16 = 0x5D; // `]`
const COLON: u16 = 0x3A; // `:`
const BACKTICK: u16 = 0x60; // '`'
const PERIOD: u16 = 0x2E; // `.`
const LEFT_PAREN: u16 = 0x28; // `(`
const STAR: u16 = 0x2A; // `*`
const SPACE: u16 = 0x20; // ` `
const NEWLINE: u16 = 0x0A; // `\n`

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn ws(c: u16) -> bool {
    is_whitespace(c as i32)
}

/// Dart `String.indexOf(pattern, start)` on code units; -1 if not found.
fn index_of(s: &[u16], pattern: &[u16], start: usize) -> i64 {
    let mut i = start;
    while i + pattern.len() <= s.len() {
        if &s[i..i + pattern.len()] == pattern {
            return i as i64;
        }
        i += 1;
    }
    -1
}

/// Dart `String.startsWith(pattern, index)` on code units.
fn starts_with(s: &[u16], pattern: &str, index: usize) -> bool {
    // The patterns are ASCII.
    let p = pattern.as_bytes();
    index + p.len() <= s.len()
        && s[index..index + p.len()]
            .iter()
            .zip(p)
            .all(|(&a, &b)| a == b as u16)
}

fn string_of(s: &[u16]) -> String {
    String::from_utf16_lossy(s)
}

/// Given a comment reference without a closing `]`, search for a possible
/// place where `]` should be (Dart `_findCommentReferenceEnd`).
fn find_comment_reference_end(comment: &[u16], mut index: usize, end: usize) -> usize {
    // Find the end of the identifier if there is one.
    if index >= end || !is_letter(comment[index] as i32) {
        return index;
    }
    while index < end && is_letter_or_digit(comment[index] as i32) {
        index += 1;
    }

    // Check for a trailing `.`.
    if index >= end || comment[index] != PERIOD {
        return index;
    }
    index += 1;

    // Find end of the identifier after the `.`.
    if index >= end || !is_letter(comment[index] as i32) {
        return index;
    }
    index += 1;
    while index < end && is_letter_or_digit(comment[index] as i32) {
        index += 1;
    }
    index
}

/// Returns whether bracketed text, ending at [right_index], appears to be a
/// Markdown link (Dart `_isLinkText`).
fn is_link_text(comment: &[u16], right_index: usize, can_be_link_reference: bool) -> bool {
    let length = comment.len();
    let mut index = right_index + 1;
    if index >= length {
        return false;
    }
    let mut ch = comment[index];
    if ch == LEFT_PAREN {
        return true;
    }
    if can_be_link_reference && ch == COLON {
        return true;
    }
    while ws(ch) {
        index += 1;
        if index >= length {
            return false;
        }
        ch = comment[index];
    }
    ch == LEFT_BRACKET
}

fn is_right_curly_brace(character: u16) -> bool {
    character == 0x7D
}

fn is_equal_sign(character: u16) -> bool {
    character == 0x3D
}

/// Reads past any opening whitespace in [content], returning the index
/// after the last whitespace character (Dart `_readWhitespace`).
fn read_whitespace(content: &[u16], mut index: usize) -> usize {
    let length = content.len();
    if index >= length {
        return index;
    }
    while ws(content[index]) {
        index += 1;
        if index >= length {
            return index;
        }
    }
    index
}

/// A line of a doc comment: the offset in the unit and the content.
type LineInfo = (i64, Vec<u16>);

/// A class which temporarily stores data for a documentation [`Comment`],
/// which is ultimately built with [`DocCommentBuilder::build`].
pub struct DocCommentBuilder<'a> {
    ast: &'a mut Ast,
    diagnostic_reporter: &'a mut FastaErrorReporter,
    uri: &'a str,
    feature_set: ExperimentalFeatures,
    language_version: LibraryLanguageVersion,
    references: Vec<Id<CommentReference>>,
    code_blocks: Vec<MdCodeBlock>,
    doc_imports: Vec<DocImport>,
    doc_directives: Vec<DocDirective>,
    has_nodoc: bool,
    start_token: TokenId,
    character_sequence: CharacterSequence,
    block_doc_directive_builder_stack: Vec<BlockDocDirectiveBuilder>,
}

impl<'a> DocCommentBuilder<'a> {
    pub fn new(
        ast: &'a mut Ast,
        diagnostic_reporter: &'a mut FastaErrorReporter,
        uri: &'a str,
        feature_set: ExperimentalFeatures,
        language_version: LibraryLanguageVersion,
        start_token: TokenId,
    ) -> Self {
        let character_sequence = CharacterSequence::new(&ast.tokens, start_token);
        DocCommentBuilder {
            ast,
            diagnostic_reporter,
            uri,
            feature_set,
            language_version,
            references: Vec::new(),
            code_blocks: Vec::new(),
            doc_imports: Vec::new(),
            doc_directives: Vec::new(),
            has_nodoc: false,
            start_token,
            character_sequence,
            block_doc_directive_builder_stack: Vec::new(),
        }
    }

    /// Dart `build`.
    pub fn build(mut self) -> Id<Comment> {
        self.parse_doc_comment();
        let tokens = &self.ast.tokens;
        let mut list = vec![self.start_token];
        if tokens.lexeme(self.start_token).starts_with("///") {
            let mut token = tokens.next(self.start_token);
            while token.is_some() {
                if tokens.lexeme(token).starts_with("///") {
                    list.push(token);
                }
                token = tokens.next(token);
            }
        }
        let tokens = self.ast.new_token_list(list);
        let references = self.ast.new_list(self.references);
        self.ast.add(Comment {
            references,
            tokens,
            code_blocks: self.code_blocks,
            doc_imports: self.doc_imports,
            doc_directives: self.doc_directives,
            has_nodoc: self.has_nodoc,
        })
    }

    fn next_line(&mut self) -> Option<LineInfo> {
        self.character_sequence.next(&self.ast.tokens)
    }

    /// The current offset in the compilation unit (Dart
    /// `_characterSequence._offset`).
    fn seq_offset(&self) -> i64 {
        self.character_sequence.offset()
    }

    fn report(&mut self, d: LocatableDiagnostic, offset: i64, length: i64) {
        self.diagnostic_reporter
            .report(d.at_offset(offset as usize, length as usize));
    }

    /// Parses a closing tag for a block doc directive, matching it with it's
    /// opening tag on the block directive stack, if one can be found (Dart
    /// `_endBlockDocDirectiveTag`).
    fn end_block_doc_directive_tag(&mut self, parser: &mut DirectiveParser, ty: DocDirectiveType) {
        let closing_tag = parser.directive(ty, self);
        let stack_len = self.block_doc_directive_builder_stack.len();
        for i in (0..stack_len).rev() {
            if self.block_doc_directive_builder_stack[i].matches(&closing_tag) {
                let mut builder = self.block_doc_directive_builder_stack.remove(i);
                builder.closing_tag = Some(closing_tag);
                // First add this block directive to it's parent on the
                // stack, then add the directives which it contained, as
                // their offsets are greater than this one's.
                let built = builder.build();
                let higher = &mut self.block_doc_directive_builder_stack[i - 1];
                higher.push(built);
                higher
                    .inner_doc_directives
                    .extend(builder.inner_doc_directives);

                // Remove each opening tag with no associated closing tag
                // from the stack.
                while self.block_doc_directive_builder_stack.len() > i {
                    let builder = self.block_doc_directive_builder_stack.remove(i);
                    // It would not be useful to create a synthetic closing
                    // tag; just use `None`.
                    if let Some(opening_tag) = &builder.opening_tag {
                        let (offset, end) = (opening_tag.offset as i64, opening_tag.end as i64);
                        let name = opening_tag.ty.opposing_name().unwrap();
                        self.report(
                            diag::doc_directive_missing_closing_tag(name),
                            offset,
                            end - offset,
                        );
                    }
                    let built = builder.build();
                    let higher = &mut self.block_doc_directive_builder_stack[i - 1];
                    higher.push(built);
                    higher
                        .inner_doc_directives
                        .extend(builder.inner_doc_directives);
                }
                return;
            }
        }

        // No matching opening tag was found.
        let (offset, end) = (closing_tag.offset as i64, closing_tag.end as i64);
        self.report(
            diag::doc_directive_missing_opening_tag(closing_tag.ty.name()),
            offset,
            end - offset,
        );
        self.push_doc_directive(DocDirective::Simple(SimpleDocDirective {
            tag: closing_tag,
        }));
    }

    /// Determines if [content] can represent a fenced codeblock delimiter
    /// (starts with optional whitespace, then at least three backticks)
    /// (Dart `_fencedCodeBlockDelimiter`). Returns the index of the three
    /// backticks, or -1.
    fn fenced_code_block_delimiter(content: &[u16], minimum_tick_count: usize) -> i64 {
        if content.is_empty() {
            return -1;
        }
        let index = read_whitespace(content, 0);
        let length = content.len();
        if index + 3 > length {
            return -1;
        }
        // Dart: `content.substring(index, index + 3) == '`' * minimumTickCount`
        // (never true when `minimumTickCount` is not 3).
        if minimum_tick_count == 3 && content[index..index + 3].iter().all(|&c| c == BACKTICK) {
            index as i64
        } else {
            -1
        }
    }

    /// Dart `_parseBlockDocDirectiveTag`.
    fn parse_block_doc_directive_tag(
        &mut self,
        parser: &mut DirectiveParser,
        ty: DocDirectiveType,
    ) {
        let opening = parser.directive(ty, self);
        self.block_doc_directive_builder_stack
            .push(BlockDocDirectiveBuilder::new(Some(opening)));
    }

    /// Parses a documentation comment (Dart `_parseDocComment`).
    ///
    /// All parsed data is added to the fields on this builder.
    #[allow(clippy::if_same_then_else)]
    fn parse_doc_comment(&mut self) {
        // Track whether the previous line is empty, in order to correctly
        // parse an indented code block.
        let mut is_previous_line_empty = true;
        let mut line_info = self.next_line();
        self.block_doc_directive_builder_stack
            .push(BlockDocDirectiveBuilder::new(None));
        while let Some((offset, content)) = line_info {
            let whitespace_end_index = read_whitespace(&content, 0);
            if is_previous_line_empty && whitespace_end_index >= 4 {
                line_info = self.parse_indented_code_block(&content);
                if let Some((_, c)) = &line_info {
                    is_previous_line_empty = c.is_empty();
                }
                continue;
            }

            if self.parse_fenced_code_block(&content) {
                is_previous_line_empty = false;
            } else if self.parse_doc_directive_tag(whitespace_end_index, &content) {
                is_previous_line_empty = false;
            } else if self.parse_doc_import(whitespace_end_index, &content) {
                is_previous_line_empty = false;
            } else if self.parse_nodoc(whitespace_end_index, &content) {
                is_previous_line_empty = false;
            } else {
                self.parse_references(offset, &content);
                is_previous_line_empty = content.is_empty();
            }
            line_info = self.next_line();
        }

        // Resolve any unclosed block directives.
        while self.block_doc_directive_builder_stack.len() > 1 {
            let builder = self.block_doc_directive_builder_stack.pop().unwrap();
            if let Some(opening_tag) = &builder.opening_tag {
                let (offset, end) = (opening_tag.offset as i64, opening_tag.end as i64);
                let name = opening_tag.ty.opposing_name().unwrap();
                self.report(
                    diag::doc_directive_missing_closing_tag(name),
                    offset,
                    end - offset,
                );
            }
            self.push_block_doc_directive_and_inner_directives(builder);
        }

        // Move all directives from the top block doc directive builder to
        // the comment.
        let builder = self.block_doc_directive_builder_stack.pop().unwrap();
        debug_assert!(builder.opening_tag.is_none());
        self.doc_directives.extend(builder.inner_doc_directives);
    }

    /// Dart `_parseDocDirectiveTag`.
    fn parse_doc_directive_tag(&mut self, mut index: usize, content: &[u16]) -> bool {
        const OPENING_LENGTH: usize = 2;
        if !starts_with(content, "{@", index) {
            return false;
        }

        let seq_offset = self.seq_offset();
        let start_offset = seq_offset + index as i64;
        index += OPENING_LENGTH;

        let length = content.len();
        if index >= length {
            return false;
        }
        let name_index = index;
        loop {
            let character = content[index];
            if ws(character) || is_right_curly_brace(character) {
                break;
            }
            index += 1;
            if index >= length {
                break;
            }
        }

        let name_end = index;
        index = read_whitespace(content, index);

        let name = string_of(&content[name_index..name_end]);

        let mut parser = DirectiveParser {
            offset: start_offset,
            content_offset: seq_offset,
            name_offset: seq_offset + name_index as i64,
            name_end: seq_offset + name_end as i64,
            content: content.to_vec(),
            length: content.len(),
            index,
            end: None,
        };

        use DocDirectiveType as T;
        match name.as_str() {
            "animation" => self.simple_directive(&mut parser, T::Animation),
            "canonicalFor" => self.simple_directive(&mut parser, T::CanonicalFor),
            "category" => self.simple_directive(&mut parser, T::Category),
            "end-inject-html" => self.end_block_doc_directive_tag(&mut parser, T::EndInjectHtml),
            "end-tool" => self.end_block_doc_directive_tag(&mut parser, T::EndTool),
            "endtemplate" => self.end_block_doc_directive_tag(&mut parser, T::EndTemplate),
            "example" => self.simple_directive(&mut parser, T::Example),
            "inject-html" => self.parse_block_doc_directive_tag(&mut parser, T::InjectHtml),
            "macro" => self.simple_directive(&mut parser, T::Macro),
            "subCategory" => self.simple_directive(&mut parser, T::SubCategory),
            "template" => self.parse_block_doc_directive_tag(&mut parser, T::Template),
            "tool" => self.parse_block_doc_directive_tag(&mut parser, T::Tool),
            "youtube" => self.simple_directive(&mut parser, T::Youtube),
            _ => {
                self.report(
                    diag::doc_directive_unknown(&name),
                    seq_offset + name_index as i64,
                    (name_end - name_index) as i64,
                );
                return false;
            }
        }
        true
    }

    /// `_pushDocDirective(parser.simpleDirective(type))`.
    fn simple_directive(&mut self, parser: &mut DirectiveParser, ty: DocDirectiveType) {
        let tag = parser.directive(ty, self);
        self.push_doc_directive(DocDirective::Simple(SimpleDocDirective { tag }));
    }

    /// Tries to parse a doc import at the beginning of a line of a doc
    /// comment, returning whether this was successful (Dart
    /// `_parseDocImport`).
    ///
    /// A doc import begins with `@docImport ` and then can contain any other
    /// legal syntax that a regular Dart import can contain.
    fn parse_doc_import(&mut self, index: usize, content: &[u16]) -> bool {
        const DOC_IMPORT_LENGTH: usize = "@docImport ".len();
        const IMPORT_LENGTH: i64 = "import ".len() as i64;
        if !starts_with(content, "@docImport ", index) {
            return false;
        }

        let index = read_whitespace(content, index + DOC_IMPORT_LENGTH);
        let mut synthetic_import = String::from("import ");
        synthetic_import.push_str(&string_of(&content[index..]));

        // TODO(srawlins): Handle multiple lines.
        // The source map has one entry: offset 0 of the doc import is this
        // offset in the unit.
        let offset_in_unit = self.seq_offset() + (index as i64 - IMPORT_LENGTH);

        let configuration = self.feature_set.build_scanner_configuration();
        // Dart `DocImportStringScanner(...).tokenize()`: the offsets of the
        // tokens made with `tokenStart` are offsets in the unit.
        let result =
            scan_with_token_start_delta(&synthetic_import, Some(configuration), offset_in_unit);
        let tokens = result.tokens;

        let doc_import_listener = AstBuilder::new(
            self.uri.to_string(),
            true,
            self.feature_set,
            self.language_version,
            None,
        );
        let mut parser = Parser::new(doc_import_listener, tokens, true, self.feature_set);
        parser.parse_unit(result.first);
        let (tokens, mut doc_import_listener) = parser.into_parts();
        doc_import_listener.ast.tokens = tokens;

        // The Dart builder reports to the same reporter.
        let diagnostics = std::mem::take(
            &mut doc_import_listener
                .diagnostic_reporter
                .diagnostic_reporter
                .diagnostics,
        );
        for d in diagnostics {
            self.diagnostic_reporter.report_error(d);
        }

        let Some(&directive) = doc_import_listener.directives.first() else {
            return false;
        };

        if let Some(import) = doc_import_listener.ast.cast::<ImportDirective>(directive) {
            let offset = self.seq_offset() as u32;
            self.doc_imports.push(DocImport {
                offset,
                ast: Box::new(std::mem::take(&mut doc_import_listener.ast)),
                import,
            });
            return true;
        }

        false
    }

    /// Parses a fenced code block, starting with [content] (Dart
    /// `_parseFencedCodeBlock`).
    ///
    /// When this method returns, the character sequence is positioned at
    /// the closing delimiter line (`next()` must be called to move to the
    /// next line).
    fn parse_fenced_code_block(&mut self, content: &[u16]) -> bool {
        let index = Self::fenced_code_block_delimiter(content, 3);
        if index == -1 {
            return false;
        }
        let mut index = index as usize;
        let mut tick_count = 0;
        let length = content.len();
        while content[index] == BACKTICK {
            tick_count += 1;
            index += 1;
            if index >= length {
                break;
            }
        }

        let info_string = if index == length {
            None
        } else {
            info_string_parse(&content[index..])
        };
        let mut fenced_code_block_lines = vec![MdCodeBlockLine {
            offset: self.seq_offset() as u32,
            length: content.len() as u32,
        }];

        let mut line_info = self.next_line();
        while let Some((offset, content)) = line_info {
            fenced_code_block_lines.push(MdCodeBlockLine {
                offset: offset as u32,
                length: content.len() as u32,
            });
            if Self::fenced_code_block_delimiter(&content, tick_count) > -1 {
                // End the fenced code block.
                break;
            }
            line_info = self.next_line();
        }

        self.code_blocks.push(MdCodeBlock {
            info_string,
            lines: fenced_code_block_lines,
            ty: CodeBlockType::Fenced,
        });
        true
    }

    /// Dart `_parseIndentedCodeBlock`.
    fn parse_indented_code_block(&mut self, content: &[u16]) -> Option<LineInfo> {
        let mut code_block_lines = vec![MdCodeBlockLine {
            offset: self.seq_offset() as u32,
            length: content.len() as u32,
        }];

        let mut line_info = self.next_line();
        while let Some((offset, content)) = &line_info {
            let whitespace_end_index = read_whitespace(content, 0);
            if whitespace_end_index >= 4 {
                code_block_lines.push(MdCodeBlockLine {
                    offset: *offset as u32,
                    length: content.len() as u32,
                });
            } else {
                // End the code block.
                self.code_blocks.push(MdCodeBlock {
                    info_string: None,
                    lines: code_block_lines,
                    ty: CodeBlockType::Indented,
                });
                return line_info;
            }

            line_info = self.next_line();
        }

        // The indented code block ends the comment.
        self.code_blocks.push(MdCodeBlock {
            info_string: None,
            lines: code_block_lines,
            ty: CodeBlockType::Indented,
        });
        line_info
    }

    /// Tries to parse a `@nodoc` doc directive at the beginning of a line of
    /// a doc comment, returning whether this was successful (Dart
    /// `_parseNodoc`).
    fn parse_nodoc(&mut self, index: usize, content: &[u16]) -> bool {
        const NODOC_LENGTH: usize = "@nodoc".len();
        if !starts_with(content, "@nodoc", index) {
            return false;
        }
        if content.len() == index + NODOC_LENGTH || content[index + NODOC_LENGTH] == SPACE {
            self.has_nodoc = true;
            return true;
        }
        false
    }

    /// Parses the [source] text, found at [offset] in a single comment
    /// reference (Dart `_parseOneCommentReference`). Returns `None` if the
    /// text could not be parsed as a comment reference.
    fn parse_one_comment_reference(
        &mut self,
        source: &str,
        offset: i64,
        mut is_synthetic: bool,
    ) -> Option<Id<CommentReference>> {
        let result = scan_string(source, None, false, None);
        if result.has_errors {
            return None;
        }
        let mut tk = result.tokens;
        let mut token = result.first;
        let mut begin = token;
        let mut new_keyword = None;
        if optional(&tk, "new", token) {
            new_keyword = Some(token);
            token = tk.next(token);
        }
        let mut first_token = None;
        let mut first_period = None;
        let mut second_token = None;
        let mut second_period = None;
        if tk.get(token).is_identifier() && optional(&tk, ".", tk.next(token)) {
            second_token = Some(token);
            second_period = Some(tk.next(token));
            let sp = tk.next(token);
            if tk.get(tk.next(sp)).is_identifier() && optional(&tk, ".", tk.next(tk.next(sp))) {
                first_token = second_token;
                first_period = second_period;
                let st = tk.next(sp);
                second_token = Some(st);
                second_period = Some(tk.next(st));
            }
            let sp = second_period.unwrap();
            let identifier = tk.next(sp);
            if tk.ty(identifier).is_keyword() && optional(&tk, "new", identifier) {
                // Treat `new` after `.` is as an identifier so that it can
                // represent an unnamed constructor. This support is separate
                // from the constructor-tearoffs feature.
                let replacement = tk.push_string_like(TokenType::IDENTIFIER, identifier);
                TokenStreamRewriter::new(&mut tk).replace_token_following(sp, replacement);
            }
            token = tk.next(sp);
        }
        if tk.get(token).is_eof() {
            // Recovery: Insert a synthetic identifier for code completion.
            let previous = match second_period.or(new_keyword) {
                Some(t) => t,
                None => synthetic_previous_token(&mut tk, token),
            };
            token = TokenStreamRewriter::new(&mut tk).insert_synthetic_identifier(previous, "");
            is_synthetic = true;
            if begin == tk.next(token) {
                begin = token;
            }
        }
        let mut operator_keyword = None;
        if optional(&tk, "operator", token) {
            operator_keyword = Some(token);
            token = tk.next(token);
        }
        let identifier_or_operator = if tk.ty(token).is_user_definable_operator() {
            if !tk.get(tk.next(token)).is_eof() {
                return None;
            }
            token
        } else {
            token = operator_keyword.unwrap_or(token);
            // `this`, `null`, `true` and `false` are not supported (see the
            // TODO in the Dart code).
            if !tk.get(tk.next(token)).is_eof() || !tk.get(token).is_identifier() {
                return None;
            }
            token
        };
        Some(self.parse_one_comment_reference_rest(
            &tk,
            begin,
            offset,
            new_keyword,
            first_token,
            first_period,
            second_token,
            second_period,
            identifier_or_operator,
            is_synthetic,
        ))
    }

    /// Parses the parameters into a [`CommentReference`] (Dart
    /// `_parseOneCommentReferenceRest`). The tokens of the reference (in
    /// [tokens], from [begin] up to the EOF token) are copied into the
    /// unit's token arena with the offsets shifted by [reference_offset].
    #[allow(clippy::too_many_arguments)]
    fn parse_one_comment_reference_rest(
        &mut self,
        tokens: &Tokens,
        begin: TokenId,
        reference_offset: i64,
        new_keyword: Option<TokenId>,
        first_token: Option<TokenId>,
        first_period: Option<TokenId>,
        second_token: Option<TokenId>,
        second_period: Option<TokenId>,
        identifier_or_operator: TokenId,
        is_synthetic: bool,
    ) -> Id<CommentReference> {
        // Adjust the token offsets to match the enclosing comment token.
        let byte_offset = self.ast.tokens.byte_offset(self.start_token);
        let mut map: Vec<(TokenId, TokenId)> = Vec::new();
        let mut token = begin;
        let mut previous: Option<TokenId> = None;
        loop {
            let src = tokens.get(token);
            let synthetic = src.flags & flags::SYNTHETIC != 0;
            let offset = (src.offset as i64 + reference_offset) as u32;
            let copy = if src.flags & flags::FIXED_LEXEME != 0 {
                let mut t = Token::fixed(src.ty, offset, byte_offset, synthetic);
                t.length = src.length;
                self.ast.tokens.push(t)
            } else {
                let mut t = Token::fixed(src.ty, offset, byte_offset, false);
                t.flags = flags::FIXED_LEXEME | if synthetic { flags::SYNTHETIC } else { 0 };
                t.length = src.length;
                self.ast.tokens.push_with_lexeme(t, tokens.lexeme(token))
            };
            if let Some(p) = previous {
                self.ast.tokens.set_next(p, copy);
            }
            previous = Some(copy);
            map.push((token, copy));
            token = tokens.next(token);
            if tokens.get(token).is_eof() {
                break;
            }
        }
        let m = |t: TokenId| -> TokenId {
            map.iter()
                .find(|(s, _)| *s == t)
                .map(|(_, c)| *c)
                .expect("comment reference token not in the copied chain")
        };

        let identifier = self.ast.add(SimpleIdentifier {
            token: m(identifier_or_operator),
        });
        let new_keyword = new_keyword.map(m);
        if let Some(first_token) = first_token {
            let prefix = self.ast.add(SimpleIdentifier {
                token: m(first_token),
            });
            let second = self.ast.add(SimpleIdentifier {
                token: m(second_token.unwrap()),
            });
            let target = self.ast.add(PrefixedIdentifier {
                prefix,
                period: m(first_period.unwrap()),
                identifier: second,
            });
            let expression = self.ast.add(PropertyAccess {
                target: Some(target.upcast()),
                operator: m(second_period.unwrap()),
                property_name: identifier,
            });
            self.ast.add(CommentReference {
                new_keyword,
                expression: expression.upcast(),
                is_synthetic,
            })
        } else if let Some(second_token) = second_token {
            let prefix = self.ast.add(SimpleIdentifier {
                token: m(second_token),
            });
            let expression = self.ast.add(PrefixedIdentifier {
                prefix,
                period: m(second_period.unwrap()),
                identifier,
            });
            self.ast.add(CommentReference {
                new_keyword,
                expression: expression.upcast(),
                is_synthetic,
            })
        } else {
            self.ast.add(CommentReference {
                new_keyword,
                expression: identifier.upcast(),
                is_synthetic,
            })
        }
    }

    /// Parses the comment references in [content] which starts at [offset]
    /// (Dart `_parseReferences`).
    fn parse_references(&mut self, offset: i64, content: &[u16]) {
        let mut index = 0usize;
        let end = content.len();
        let mut seen_only_whitespace = true;
        while index < end {
            let ch = content[index];
            if ch == LEFT_BRACKET {
                index += 1;
                if index < end && content[index] == COLON {
                    // Skip old-style code block, e.g. `/// Text [:int:]`.
                    let found = index_of(content, &[COLON, RIGHT_BRACKET], index + 1) + 1;
                    if found == 0 || found as usize > end {
                        break;
                    }
                    index = found as usize;
                } else {
                    let reference_start = index;
                    let found = index_of(content, &[RIGHT_BRACKET], index);
                    let mut is_synthetic = false;
                    if found == -1 || found as usize >= end {
                        // Recovery: terminating ']' is not typed yet.
                        index = find_comment_reference_end(content, reference_start, end);
                        is_synthetic = true;
                    } else {
                        index = found as usize;
                    }
                    if is_link_text(content, index, seen_only_whitespace) {
                        // TODO(brianwilkerson): Handle the case where there's
                        // a library URI in the link text.
                    } else {
                        let source = string_of(&content[reference_start..index]);
                        if let Some(reference) = self.parse_one_comment_reference(
                            &source,
                            offset + reference_start as i64,
                            is_synthetic,
                        ) {
                            self.references.push(reference);
                        }
                    }
                }
                seen_only_whitespace = false;
            } else if ch == BACKTICK {
                // Skip inline code block if there is both starting '`' and
                // ending '`'.
                let end_code_block = index_of(content, &[BACKTICK], index + 1);
                if end_code_block != -1 && (end_code_block as usize) < end {
                    index = end_code_block as usize;
                }
                seen_only_whitespace = false;
            } else if !ws(ch) {
                seen_only_whitespace = false;
            }
            index += 1;
        }
    }

    /// Dart `_pushBlockDocDirectiveAndInnerDirectives`.
    fn push_block_doc_directive_and_inner_directives(&mut self, builder: BlockDocDirectiveBuilder) {
        let built = builder.build();
        self.push_doc_directive(built);
        for doc_directive in builder.inner_doc_directives {
            self.push_doc_directive(doc_directive);
        }
    }

    /// Push [doc_directive] onto the current block doc directive (Dart
    /// `_pushDocDirective`).
    fn push_doc_directive(&mut self, doc_directive: DocDirective) {
        self.block_doc_directive_builder_stack
            .last_mut()
            .unwrap()
            .push(doc_directive);
    }
}

/// A builder for a [`BlockDocDirective`], which keeps track of various
/// information until the closing tag is found, or the directive is closed
/// as part of recovery (Dart `_BlockDocDirectiveBuilder`). The bottom of the
/// stack is a placeholder with no opening tag.
struct BlockDocDirectiveBuilder {
    opening_tag: Option<DocDirectiveTag>,
    closing_tag: Option<DocDirectiveTag>,
    inner_doc_directives: Vec<DocDirective>,
}

impl BlockDocDirectiveBuilder {
    fn new(opening_tag: Option<DocDirectiveTag>) -> Self {
        BlockDocDirectiveBuilder {
            opening_tag,
            closing_tag: None,
            inner_doc_directives: Vec::new(),
        }
    }

    /// Dart `build` (the inner directives stay in the builder).
    fn build(&self) -> DocDirective {
        let opening_tag = self
            .opening_tag
            .clone()
            .expect("Attempting to build a block doc directive with no opening tag.");
        DocDirective::Block(BlockDocDirective {
            opening_tag,
            closing_tag: self.closing_tag.clone(),
        })
    }

    /// Whether this doc directive's opening tag is the opposing tag for
    /// [tag] (Dart `matches`).
    fn matches(&self, tag: &DocDirectiveTag) -> bool {
        match &self.opening_tag {
            Some(opening_tag) => opening_tag.ty.opposing_name() == Some(tag.ty.name()),
            None => false,
        }
    }

    fn push(&mut self, doc_directive: DocDirective) {
        self.inner_doc_directives.push(doc_directive);
    }
}

/// An abstraction of the character sequences in either a single-line doc
/// comment (a series of tokens) or a multi-line doc comment (one token)
/// (Dart `_CharacterSequence`).
enum CharacterSequence {
    /// Dart `_CharacterSequenceFromSingleLineComment`.
    SingleLine { token: TokenId, offset: i64 },
    /// Dart `_CharacterSequenceFromMultiLineComment`.
    MultiLine {
        lexeme: Vec<u16>,
        token_offset: i64,
        offset: i64,
        end: i64,
    },
}

impl CharacterSequence {
    fn new(tokens: &Tokens, token: TokenId) -> Self {
        if tokens.lexeme(token).starts_with("///") {
            CharacterSequence::SingleLine { token, offset: -1 }
        } else {
            CharacterSequence::MultiLine {
                lexeme: utf16(tokens.lexeme(token)),
                token_offset: tokens.offset(token) as i64,
                offset: -1,
                end: -1,
            }
        }
    }

    /// The current offset in the compilation unit (Dart `_offset`).
    fn offset(&self) -> i64 {
        match self {
            CharacterSequence::SingleLine { offset, .. } => *offset,
            CharacterSequence::MultiLine { offset, .. } => *offset,
        }
    }

    /// Moves the current position of the doc comment to the next line
    /// (Dart `next`). Returns the offset in the unit and the content of the
    /// line.
    fn next(&mut self, tokens: &Tokens) -> Option<LineInfo> {
        match self {
            CharacterSequence::SingleLine { token, offset } => {
                const THREE_SLASHES_LENGTH: i64 = 3;
                if *offset == -1 {
                    *offset = tokens.offset(*token) as i64;
                } else {
                    // The sequence of single-line doc comment tokens can
                    // contain non-doc comment tokens as well, starting with
                    // `//` (but not `///`) or `/*`.
                    loop {
                        let next_token = tokens.next(*token);
                        if next_token.is_none() {
                            return None;
                        }
                        *token = next_token;
                        *offset = tokens.offset(next_token) as i64;
                        if tokens.lexeme(*token).starts_with("///") {
                            break;
                        }
                    }
                }
                *offset += THREE_SLASHES_LENGTH;
                let lexeme = utf16(tokens.lexeme(*token));
                Some((*offset, lexeme[3..].to_vec()))
            }
            CharacterSequence::MultiLine {
                lexeme,
                token_offset,
                offset,
                end,
            } => {
                let token_offset = *token_offset;
                let len = lexeme.len() as i64;
                if *offset == -1 {
                    *offset = token_offset;
                    let mut end_index = index_of(lexeme, &[NEWLINE], 0);
                    if end_index == -1 {
                        end_index = len;
                    }
                    *end = token_offset + end_index;
                    let index_in_lexeme = (*offset - token_offset) as usize;
                    return Some((
                        *offset,
                        lexeme[index_in_lexeme..end_index as usize].to_vec(),
                    ));
                }

                *offset = *end + 1;
                if *offset - token_offset >= len {
                    return None;
                }
                while ws(lexeme[(*offset - token_offset) as usize]) {
                    *offset += 1;
                    if *offset - token_offset >= len {
                        return None;
                    }
                }

                let mut end_index = index_of(lexeme, &[NEWLINE], (*offset - token_offset) as usize);
                if end_index == -1 {
                    end_index = len;
                }
                *end = token_offset + end_index;

                const STAR_SPACE_LENGTH: i64 = 2;
                const STAR_LENGTH: i64 = 1;
                let i = (*offset - token_offset) as usize;
                if lexeme[i] == STAR && i + 1 < lexeme.len() && lexeme[i + 1] == SPACE {
                    *offset += STAR_SPACE_LENGTH;
                } else if *end == *offset + 1 && lexeme[i] == STAR {
                    *offset += STAR_LENGTH;
                }

                let start = (*offset - token_offset) as usize;
                assert!(
                    start <= end_index as usize,
                    "RangeError: start {start} > end {end_index}"
                );
                Some((*offset, lexeme[start..end_index as usize].to_vec()))
            }
        }
    }
}

/// Dart `_DirectiveParser`.
struct DirectiveParser {
    /// The offset of the opening `{@` of this directive (Dart `_offset`).
    offset: i64,
    /// The offset in the compilation unit at which [content] is found (Dart
    /// `_contentOffset`).
    content_offset: i64,
    /// The offset in the compilation unit at which this directive's name is
    /// found.
    name_offset: i64,
    /// The offset in the compilation unit immediately after the end of this
    /// directive's name.
    name_end: i64,
    /// The content of the doc comment line.
    content: Vec<u16>,
    length: usize,
    /// The current position in [content].
    index: usize,
    /// The index immediately after the end of this directive (Dart `_end`).
    end: Option<i64>,
}

impl DirectiveParser {
    /// Parses a non-block (single line) doc directive (Dart `directive`).
    fn directive(
        &mut self,
        ty: DocDirectiveType,
        builder: &mut DocCommentBuilder<'_>,
    ) -> DocDirectiveTag {
        if self.index == self.length {
            self.end = Some(self.offset + self.index as i64);
        }
        let (positional_arguments, named_arguments) = self.parse_arguments(builder);
        DocDirectiveTag {
            offset: self.offset as u32,
            end: self.end.unwrap() as u32,
            name_offset: self.name_offset as u32,
            name_end: self.name_end as u32,
            ty,
            positional_arguments,
            named_arguments,
        }
    }

    /// Parses and returns a positional or named doc directive argument
    /// (Dart `_parseArgument`).
    fn parse_argument(&mut self) -> DocDirectiveArgument {
        // An equal sign is parsed as a delimiter between a named argument's
        // name and value only if the name consists only of alphanumeric
        // characters.
        let mut only_letters_or_digits = true;
        let argument_start = self.index;
        while self.index < self.length {
            let character = self.content[self.index];
            if ws(character) {
                break;
            }
            if is_right_curly_brace(character) {
                break;
            }
            if is_equal_sign(character) && only_letters_or_digits {
                // This is a valid named argument name/value delimiter.
                let argument_name = string_of(&self.content[argument_start..self.index]);
                self.index += 1;
                if self.index == self.length {
                    // Equal sign is followed by EOL.
                    return DocDirectiveArgument::Named(DocDirectiveNamedArgument {
                        offset: (self.content_offset + argument_start as i64) as u32,
                        end: (self.content_offset + self.index as i64) as u32,
                        name: argument_name.into(),
                        // Recover an unterminated named argument as having
                        // an empty argument value.
                        value: "".into(),
                    });
                }
                let argument_value_start = self.index;
                while self.index < self.length {
                    let character = self.content[self.index];
                    if ws(character) {
                        break;
                    }
                    if is_right_curly_brace(character) {
                        break;
                    }
                    self.index += 1;
                }
                return DocDirectiveArgument::Named(DocDirectiveNamedArgument {
                    offset: (self.content_offset + argument_start as i64) as u32,
                    end: (self.content_offset + self.index as i64) as u32,
                    name: argument_name.into(),
                    value: string_of(&self.content[argument_value_start..self.index]).into(),
                });
            }
            if !is_letter_or_digit(character as i32) {
                only_letters_or_digits = false;
            }
            self.index += 1;
        }
        let argument_value = string_of(&self.content[argument_start..self.index]);
        DocDirectiveArgument::Positional(DocDirectivePositionalArgument {
            offset: (self.content_offset + argument_start as i64) as u32,
            end: (self.content_offset + self.index as i64) as u32,
            value: argument_value.into(),
        })
    }

    /// Parses both positional and named doc directive arguments until either
    /// a closing curly brace or EOL are reached (Dart `_parseArguments`).
    ///
    /// Reports a warning if EOL is reached before a closing curly brace is
    /// found.
    fn parse_arguments(
        &mut self,
        builder: &mut DocCommentBuilder<'_>,
    ) -> (Vec<DocDirectiveArgument>, Vec<DocDirectiveNamedArgument>) {
        if self.end.is_some() {
            return (Vec::new(), Vec::new());
        }
        let mut positional_arguments = Vec::new();
        let mut named_arguments = Vec::new();
        while self.index < self.length {
            if is_right_curly_brace(self.content[self.index]) {
                self.index += 1;
                self.end = Some(self.offset + self.index as i64);
                return (positional_arguments, named_arguments);
            }
            match self.parse_argument() {
                DocDirectiveArgument::Named(a) => named_arguments.push(a),
                a => positional_arguments.push(a),
            }
            self.index = read_whitespace(&self.content, self.index);
        }

        // We've hit EOL without closing brace.
        self.end = Some(self.offset + self.index as i64);
        builder.report(
            diag::doc_directive_missing_closing_brace(),
            self.offset + self.index as i64 - 1,
            1,
        );
        (positional_arguments, named_arguments)
    }
}

/// Dart `_InfoString.parse`: the trimmed text, `None` if it is empty.
fn info_string_parse(text: &[u16]) -> Option<Box<str>> {
    let text = string_of(text);
    let text = text.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.into())
    }
}
