// Dart source: pkg/_fe_analyzer_shared/lib/src/scanner/abstract_scanner.dart

//! The scanner. A port of Dart `AbstractScanner`; the character reader
//! (Dart `StringScanner`) is in `string_scanner.rs`.
//!
//! Character values are Dart `int`s: UTF-16 code units, [`EOF`] (-1) at the
//! end, and [`STX`] as the signal for "back in string scanning mode".

use crate::characters::*;
use crate::error_token::*;
use crate::keyword_state::{self, KeywordState};
use crate::string_scanner::stop_table;
use crate::token::{Token, TokenId, Tokens, flags};
use crate::token_constants::*;
use crate::token_type::{Keyword, TokenType};

/// Dart `ScannerConfiguration`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScannerConfiguration {
    /// Scan `>>>` and `>>>=` (language version 2.14 and later).
    pub enable_triple_shift: bool,
    /// Scan `augment` as a built-in identifier (experiment).
    pub enable_augmentations: bool,
}

impl Default for ScannerConfiguration {
    /// The configuration of the latest language version without experiments.
    fn default() -> Self {
        ScannerConfiguration {
            enable_triple_shift: true,
            enable_augmentations: false,
        }
    }
}

/// Information about a language version comment (`// @dart = 2.12`), given
/// to the [`LanguageVersionChanged`] callback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LanguageVersionInfo {
    pub token: TokenId,
    pub major: i64,
    pub minor: i64,
    pub offset: u32,
    pub length: u32,
}

/// Dart `LanguageVersionChanged`. Returns a new configuration for the rest of
/// the file, or `None` to keep the current one.
pub type LanguageVersionChanged<'c> =
    dyn FnMut(LanguageVersionInfo) -> Option<ScannerConfiguration> + 'c;

/// A scan position: the UTF-16 offset (Dart `scanOffset`) and the byte offset
/// in the UTF-8 source.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Pos {
    pub off: i64,
    pub byte: usize,
}

pub(crate) const STX_SIGNAL: i32 = STX;

// Characters that end a run of characters that a string or comment loop only
// skips (see `advance_until`).
static STOP_STRING_DQ: [bool; 256] = stop_table(b"\"\\$\n\r");
static STOP_STRING_SQ: [bool; 256] = stop_table(b"'\\$\n\r");
static STOP_MULTI_LINE_STRING_DQ: [bool; 256] = stop_table(b"\"\\$\n");
static STOP_MULTI_LINE_STRING_SQ: [bool; 256] = stop_table(b"'\\$\n");
static STOP_RAW_STRING_DQ: [bool; 256] = stop_table(b"\"\n\r");
static STOP_RAW_STRING_SQ: [bool; 256] = stop_table(b"'\n\r");
static STOP_RAW_MULTI_LINE_DQ: [bool; 256] = stop_table(b"\"\n");
static STOP_RAW_MULTI_LINE_SQ: [bool; 256] = stop_table(b"'\n");
static STOP_MULTI_LINE_COMMENT: [bool; 256] = stop_table(b"*/\n");

pub struct AbstractScanner<'a, 'c> {
    // ---- StringScanner state (see string_scanner.rs) ----
    pub(crate) src: &'a [u8],
    /// Byte offset of the current code point.
    pub(crate) pos: usize,
    /// Byte length of the current code point (0 before the first advance and
    /// at the end).
    pub(crate) cur_len: usize,
    /// Low surrogate to return on the next advance (current is a high
    /// surrogate), or 0.
    pub(crate) pending_low: i32,
    /// The current code unit is a low surrogate.
    pub(crate) in_low: bool,
    /// The current code unit (Dart `current()`).
    pub(crate) cur: i32,
    /// Dart `scanOffset` (UTF-16).
    pub(crate) scan_offset: i64,

    // ---- AbstractScanner state ----
    pub include_comments: bool,
    pub(crate) language_version_changed: Option<&'c mut LanguageVersionChanged<'c>>,
    pub(crate) enable_triple_shift: bool,
    pub(crate) enable_augmentations: bool,
    /// Dart `tokenStart`.
    pub(crate) token_start: Pos,
    /// Dart `tokens`: the sentinel before the first token.
    pub(crate) head: TokenId,
    pub(crate) tail: TokenId,
    pub(crate) error_tail: TokenId,
    pub has_errors: bool,
    pub(crate) open_brace_with_missing_end_for_possible_recovery: TokenId,
    pub offset_for_curly_bracket_recovery_start: Option<u32>,
    pub(crate) comments: TokenId,
    pub(crate) comments_tail: TokenId,
    pub line_starts: Vec<u32>,
    /// Dart `groupingStack`; the head of the Dart list is the last element.
    pub(crate) grouping_stack: Vec<TokenId>,
    pub(crate) in_recovery_option: bool,
    pub(crate) recovery_count: i64,
    pub(crate) arena: Tokens,
    pub(crate) language_version: Option<LanguageVersionInfo>,
}

impl<'a, 'c> AbstractScanner<'a, 'c> {
    /// `source` is the source text without a byte order mark.
    pub fn new(
        source: &'a str,
        arena: Tokens,
        configuration: Option<ScannerConfiguration>,
        include_comments: bool,
        language_version_changed: Option<&'c mut LanguageVersionChanged<'c>>,
    ) -> Self {
        let mut s = Self::empty(source.as_bytes(), arena, include_comments, false);
        s.language_version_changed = language_version_changed;
        // LineStarts: the first line starts at character offset 0.
        s.line_starts.reserve(1 + source.len() / 22);
        // About one token (or comment) per 10 bytes in typical code.
        s.arena.tokens.reserve(16 + source.len() / 8);
        s.line_starts.push(0);
        s.set_configuration(configuration);
        s
    }

    fn empty(
        src: &'a [u8],
        mut arena: Tokens,
        include_comments: bool,
        in_recovery_option: bool,
    ) -> Self {
        // Dart `new Token.eof(-1)`: points to itself.
        let head = TokenId(arena.tokens.len() as u32);
        arena.tokens.push(Token {
            ty: TokenType::EOF,
            flags: flags::FIXED_LEXEME,
            offset: 0,
            length: 0,
            lex_start: 0,
            lex_end: 0,
            next: head,
            previous: head,
            preceding_comments: TokenId::NONE,
            end_group: TokenId::NONE,
            before_synthetic: TokenId::NONE,
        });
        AbstractScanner {
            src,
            pos: 0,
            cur_len: 0,
            pending_low: 0,
            in_low: false,
            cur: EOF,
            scan_offset: -1,
            include_comments,
            language_version_changed: None,
            enable_triple_shift: true,
            enable_augmentations: false,
            token_start: Pos { off: -1, byte: 0 },
            head,
            tail: head,
            error_tail: head,
            has_errors: false,
            open_brace_with_missing_end_for_possible_recovery: TokenId::NONE,
            offset_for_curly_bracket_recovery_start: None,
            comments: TokenId::NONE,
            comments_tail: TokenId::NONE,
            line_starts: Vec::new(),
            grouping_stack: Vec::new(),
            in_recovery_option,
            recovery_count: 0,
            arena,
            language_version: None,
        }
    }

    /// Dart `AbstractScanner.recoveryOptionScanner`.
    fn create_recovery_option_scanner(&mut self) -> AbstractScanner<'a, 'static> {
        let arena = std::mem::take(&mut self.arena);
        let mut s = AbstractScanner::empty(self.src, arena, false, true);
        s.enable_triple_shift = self.enable_triple_shift;
        s.token_start = self.token_start;
        s.grouping_stack = self.grouping_stack.clone();
        // StringScanner.recoveryOptionScanner copies the scan position.
        s.pos = self.pos;
        s.cur_len = self.cur_len;
        s.pending_low = self.pending_low;
        s.in_low = self.in_low;
        s.cur = self.cur;
        s.scan_offset = self.scan_offset;
        s.line_starts.push(0);
        s
    }

    /// Dart `set configuration`.
    pub fn set_configuration(&mut self, config: Option<ScannerConfiguration>) {
        if let Some(config) = config {
            self.enable_triple_shift = config.enable_triple_shift;
            self.enable_augmentations = config.enable_augmentations;
        }
    }

    /// Dart `firstToken()`.
    pub fn first_token(&self) -> TokenId {
        self.arena.get(self.head).next
    }

    /// Takes the token arena out of the scanner.
    pub fn into_tokens(self) -> Tokens {
        self.arena
    }

    pub fn tokens(&self) -> &Tokens {
        &self.arena
    }

    /// The language version comment found in the header, if any.
    pub fn language_version(&self) -> Option<LanguageVersionInfo> {
        self.language_version
    }

    // ---------------------------------------------------------------------
    // Token creation helpers.

    #[inline(always)]
    fn tok(&self, id: TokenId) -> &Token {
        &self.arena.tokens[id.index()]
    }

    #[inline(always)]
    fn tok_mut(&mut self, id: TokenId) -> &mut Token {
        &mut self.arena.tokens[id.index()]
    }

    #[inline(always)]
    fn kind_of(&self, id: TokenId) -> i32 {
        self.arena.tokens[id.index()].ty.kind()
    }

    #[inline(always)]
    pub(crate) fn push_token(
        &mut self,
        ty: TokenType,
        token_flags: u8,
        offset: i64,
        length: i64,
        lex_start: usize,
        lex_end: usize,
        preceding_comments: TokenId,
    ) -> TokenId {
        let id = TokenId(self.arena.tokens.len() as u32);
        self.arena.tokens.push(Token {
            ty,
            flags: token_flags,
            offset: offset as u32,
            length: length as u32,
            lex_start: lex_start as u32,
            lex_end: lex_end as u32,
            next: TokenId::NONE,
            previous: TokenId::NONE,
            preceding_comments,
            end_group: TokenId::NONE,
            before_synthetic: TokenId::NONE,
        });
        id
    }

    /// `new Token(type, tokenStart, comments)` / `new KeywordToken(...)` /
    /// `new BeginToken(...)`.
    #[inline(always)]
    fn new_fixed_token(&mut self, ty: TokenType) -> TokenId {
        let len = ty.lexeme().len();
        let start = self.token_start;
        let comments = self.comments;
        self.push_token(
            ty,
            flags::FIXED_LEXEME,
            start.off,
            len as i64,
            start.byte,
            start.byte + len,
            comments,
        )
    }

    fn new_error_token(&mut self, error: ErrorToken) -> TokenId {
        let index = self.arena.errors.len();
        let offset = error.char_offset as i64;
        let length = error.length() as i64;
        self.arena.errors.push(error);
        self.push_token(
            TokenType::BAD_INPUT,
            flags::ERROR,
            offset,
            length,
            index,
            0,
            TokenId::NONE,
        )
    }

    // ---------------------------------------------------------------------

    /// Dart `beginToken()`.
    #[inline(always)]
    pub(crate) fn begin_token(&mut self) {
        self.token_start = self.scan_pos();
    }

    /// Dart `appendSubstringToken`.
    #[inline(always)]
    fn append_substring_token(
        &mut self,
        ty: TokenType,
        start: Pos,
        ascii_only: bool,
        extra_offset: i64,
    ) {
        let token = self.create_substring_token(ty, start, ascii_only, extra_offset);
        self.append_token(token);
    }

    /// Dart `appendSyntheticSubstringToken`.
    fn append_synthetic_substring_token(
        &mut self,
        ty: TokenType,
        start: Pos,
        ascii_only: bool,
        synthetic_chars: &str,
    ) {
        let token = self.create_synthetic_substring_token(ty, start, ascii_only, synthetic_chars);
        self.append_token(token);
    }

    /// Dart `appendPrecedenceToken`.
    #[inline(always)]
    fn append_precedence_token(&mut self, ty: TokenType) {
        let token = self.new_fixed_token(ty);
        self.append_token(token);
    }

    /// Dart `select`.
    #[inline(always)]
    fn select(&mut self, choice: i32, yes: TokenType, no: TokenType) -> i32 {
        let next = self.advance();
        if next == choice {
            self.append_precedence_token(yes);
            self.advance()
        } else {
            self.append_precedence_token(no);
            next
        }
    }

    /// Dart `appendKeywordToken`.
    fn append_keyword_token(&mut self, keyword: TokenType) {
        // Type parameters and arguments cannot contain 'this'.
        if keyword == Keyword::THIS {
            self.discard_open_lt();
        }
        let token = self.new_fixed_token(keyword);
        self.append_token(token);
    }

    fn get_line_of(&self, token: TokenId) -> i64 {
        if self.line_starts.is_empty() {
            return -1;
        }
        let offset = self.tok(token).offset as i64;
        let (mut low, mut high) = (0i64, self.line_starts.len() as i64 - 1);
        while low < high {
            let mid = high - ((high - low) >> 1);
            let pivot = self.line_starts[mid as usize] as i64;
            if pivot <= offset {
                low = mid;
            } else {
                high = mid - 1;
            }
        }
        low
    }

    fn is_a(&self, id: TokenId, ty: TokenType) -> bool {
        id.is_some() && self.tok(id).ty == ty
    }

    /// Dart `_spacesAtStartOfLogicalLineOf`.
    fn spaces_at_start_of_logical_line_of(&self, mut token: TokenId) -> i64 {
        if self.line_starts.is_empty() {
            return -1;
        }
        if self.is_a(token, TokenType::OPEN_CURLY_BRACKET) {
            let previous = self.tok(token).previous;
            if previous.is_none() {
                return -1;
            }
            let mut found_wanted = false;
            if self.is_a(previous, TokenType::CLOSE_PAREN) {
                let close_paren = previous;
                let mut candidate = self.tok(close_paren).previous;
                while candidate.is_some() {
                    let c = self.tok(candidate);
                    if c.end_group == close_paren {
                        break;
                    }
                    if c.is_eof() {
                        break;
                    }
                    if c.end_group.is_some()
                        && self.tok(c.end_group).offset > self.tok(close_paren).offset
                    {
                        break;
                    }
                    candidate = c.previous;
                }
                if candidate.is_some()
                    && self.tok(candidate).end_group == close_paren
                    && self.tok(candidate).previous.is_some()
                {
                    token = self.tok(candidate).previous;
                    let ty = self.tok(token).ty;
                    if ty == Keyword::IF
                        || ty == Keyword::FOR
                        || ty == Keyword::WHILE
                        || ty == Keyword::SWITCH
                        || ty == Keyword::CATCH
                    {
                        found_wanted = true;
                    }
                }
            } else if self.is_a(previous, Keyword::ELSE)
                || self.is_a(previous, Keyword::TRY)
                || self.is_a(previous, Keyword::FINALLY)
            {
                found_wanted = true;
            } else if self.is_a(previous, TokenType::EQ)
                && self.is_a(self.tok(previous).previous, TokenType::IDENTIFIER)
            {
                // `someIdentifier = {`
                found_wanted = true;
            } else if self.is_a(previous, Keyword::CONST)
                && self.is_a(self.tok(previous).previous, TokenType::EQ)
                && {
                    let pp = self.tok(previous).previous;
                    self.is_a(self.tok(pp).previous, TokenType::IDENTIFIER)
                }
            {
                // `someIdentifier = const {`
                found_wanted = true;
            }
            if !found_wanted {
                return -1;
            }
        }

        let line_index = self.get_line_of(token);
        if line_index == 0 {
            let first = self.tok(self.head).next;
            return if first.is_some() {
                self.tok(first).offset as i64
            } else {
                -1
            };
        }

        let line_start_of_token = self.line_starts[line_index as usize] as i64;
        let mut candidate = self.tok(token).previous;
        while candidate.is_some() && self.token_offset_signed(candidate) >= line_start_of_token {
            candidate = self.tok(candidate).previous;
        }
        if candidate.is_some() {
            let next = self.tok(candidate).next;
            return self.tok(next).offset as i64 - line_start_of_token;
        }
        -1
    }

    /// The offset of a token; the sentinel has offset -1 (as in Dart).
    fn token_offset_signed(&self, id: TokenId) -> i64 {
        if id == self.head {
            -1
        } else {
            self.tok(id).offset as i64
        }
    }

    /// Dart `getOffsetForCurlyBracketRecoveryStart`.
    pub fn get_offset_for_curly_bracket_recovery_start(&self) -> Option<u32> {
        let open = self.open_brace_with_missing_end_for_possible_recovery;
        if open.is_none() {
            return None;
        }
        let mut next = self.tok(open).next;
        let mut last_mismatch = TokenId::NONE;
        while next.is_some() && !self.tok(next).is_eof() {
            let t = self.tok(next);
            if t.ty == TokenType::OPEN_CURLY_BRACKET
                || t.ty == TokenType::STRING_INTERPOLATION_EXPRESSION
            {
                let end_group = t.end_group;
                if end_group.is_some() && self.get_line_of(next) != self.get_line_of(end_group) {
                    let indent_of_next = self.spaces_at_start_of_logical_line_of(next);
                    if indent_of_next >= 0
                        && indent_of_next != self.spaces_at_start_of_logical_line_of(end_group)
                    {
                        last_mismatch = next;
                    }
                }
            }
            next = self.tok(next).next;
        }
        if last_mismatch.is_some() {
            Some(self.tok(last_mismatch).offset)
        } else {
            None
        }
    }

    /// Dart `appendEofToken`.
    fn append_eof_token(&mut self) {
        self.begin_token();
        self.discard_open_lt();
        if self.grouping_stack.len() == 1
            && self.is_a(self.grouping_stack[0], TokenType::OPEN_CURLY_BRACKET)
        {
            // We have a single `{` that's missing a `}`. Maybe the user is typing?
            self.open_brace_with_missing_end_for_possible_recovery = self.grouping_stack[0];
        }
        while let Some(&head) = self.grouping_stack.last() {
            self.unmatched_begin_group(head);
            self.grouping_stack.pop();
        }
        let start = self.token_start;
        let comments = self.comments;
        let eof = self.push_token(
            TokenType::EOF,
            flags::FIXED_LEXEME,
            start.off,
            0,
            start.byte,
            start.byte,
            comments,
        );
        // EOF points to itself so there's always infinite look-ahead.
        self.tok_mut(eof).next = eof;
        self.append_token(eof);
        // `Token.eof` sets `previous = eof`; appendToken overwrites it.
    }

    /// Dart `lineFeedInMultiline`.
    #[inline(always)]
    fn line_feed_in_multiline(&mut self) {
        let off = self.scan_offset;
        self.line_starts.push((off + 1) as u32);
    }

    /// Dart `appendBeginGroup`.
    fn append_begin_group(&mut self, ty: TokenType) {
        let token = self.new_fixed_token(ty);
        self.append_token(token);
        // { [ ${ cannot appear inside a type parameters / arguments.
        if ty.kind() != LT_TOKEN && ty.kind() != OPEN_PAREN_TOKEN {
            self.discard_open_lt();
        }
        self.grouping_stack.push(token);
    }

    /// Dart `appendEndGroup`.
    fn append_end_group(&mut self, ty: TokenType, open_kind: i32) -> i32 {
        debug_assert!(open_kind != LT_TOKEN);
        let found_matching_brace = self.discard_begin_group_until(open_kind);
        self.append_end_group_internal(found_matching_brace, ty, open_kind)
    }

    /// Dart `appendEndGroupInternal`.
    fn append_end_group_internal(
        &mut self,
        found_matching_brace: bool,
        ty: TokenType,
        open_kind: i32,
    ) -> i32 {
        if !found_matching_brace {
            // No begin group. Leave the grouping stack alone and just continue.
            self.append_precedence_token(ty);
            return self.advance();
        }
        self.append_precedence_token(ty);
        let close = self.tail;
        let begin = *self.grouping_stack.last().unwrap();
        if self.kind_of(begin) != open_kind {
            debug_assert!(
                self.kind_of(begin) == STRING_INTERPOLATION_TOKEN
                    && open_kind == OPEN_CURLY_BRACKET_TOKEN
            );
            // We're ending an interpolated expression.
            self.tok_mut(begin).end_group = close;
            self.grouping_stack.pop();
            // Using "start-of-text" to signal that we're back in string
            // scanning mode.
            return STX_SIGNAL;
        }
        self.tok_mut(begin).end_group = close;
        self.grouping_stack.pop();
        self.advance()
    }

    /// Dart `appendGt`.
    fn append_gt(&mut self, ty: TokenType) {
        self.append_precedence_token(ty);
        let Some(&head) = self.grouping_stack.last() else {
            return;
        };
        if self.kind_of(head) == LT_TOKEN {
            let tail = self.tail;
            self.tok_mut(head).end_group = tail;
            self.grouping_stack.pop();
        }
    }

    /// Dart `appendGtGt`.
    fn append_gt_gt(&mut self, ty: TokenType) {
        self.append_precedence_token(ty);
        let Some(&head) = self.grouping_stack.last() else {
            return;
        };
        if self.kind_of(head) == LT_TOKEN {
            // Don't assign endGroup: in "T<U<V>>", the '>>' token closes the
            // outer '<', the inner '<' is left without endGroup.
            self.grouping_stack.pop();
        }
        let Some(&head) = self.grouping_stack.last() else {
            return;
        };
        if self.kind_of(head) == LT_TOKEN {
            let tail = self.tail;
            self.tok_mut(head).end_group = tail;
            self.grouping_stack.pop();
        }
    }

    /// Dart `appendGtGtGt`.
    fn append_gt_gt_gt(&mut self, ty: TokenType) {
        self.append_precedence_token(ty);
        let Some(&head) = self.grouping_stack.last() else {
            return;
        };
        // Don't assign endGroup: in "T<U<V<X>>>", the '>>>' token closes the
        // outer '<', all the inner '<' are left without endGroups.
        if self.kind_of(head) == LT_TOKEN {
            self.grouping_stack.pop();
        }
        let Some(&head) = self.grouping_stack.last() else {
            return;
        };
        if self.kind_of(head) == LT_TOKEN {
            self.grouping_stack.pop();
        }
        let Some(&head) = self.grouping_stack.last() else {
            return;
        };
        if self.kind_of(head) == LT_TOKEN {
            let tail = self.tail;
            self.tok_mut(head).end_group = tail;
            self.grouping_stack.pop();
        }
    }

    /// Dart `prependErrorToken`.
    fn prepend_error_token(&mut self, error: ErrorToken) {
        self.has_errors = true;
        let token = self.new_error_token(error);
        if self.error_tail == self.tail {
            self.append_token(token);
            self.error_tail = self.tail;
        } else {
            let error_tail = self.error_tail;
            let next = self.tok(error_tail).next;
            self.tok_mut(token).next = next;
            self.tok_mut(next).previous = token;
            self.tok_mut(error_tail).next = token;
            self.tok_mut(token).previous = error_tail;
            self.error_tail = token;
        }
    }

    /// Dart `discardBeginGroupUntil`.
    fn discard_begin_group_until(&mut self, open_kind: i32) -> bool {
        // Fast path: the first non-`<` group is the expected opener (the Dart
        // loop returns `true` on the first pass and only discards `<`).
        if let Some(&begin) = self
            .grouping_stack
            .iter()
            .rev()
            .find(|&&t| self.kind_of(t) != LT_TOKEN)
        {
            let kind = self.kind_of(begin);
            if open_kind == kind
                || (open_kind == OPEN_CURLY_BRACKET_TOKEN && kind == STRING_INTERPOLATION_TOKEN)
            {
                self.discard_open_lt();
                return true;
            }
        }

        let original_stack = self.grouping_stack.clone();
        let mut first = true;
        loop {
            // Don't report unmatched errors for <; it is also the less-than
            // operator.
            self.discard_open_lt();
            let Some(&begin) = self.grouping_stack.last() else {
                break;
            };
            let kind = self.kind_of(begin);
            if open_kind == kind
                || (open_kind == OPEN_CURLY_BRACKET_TOKEN && kind == STRING_INTERPOLATION_TOKEN)
            {
                if first {
                    return true;
                }
                break;
            }
            first = false;
            self.grouping_stack.pop();
            if self.grouping_stack.is_empty() {
                break;
            }
        }

        self.recovery_count += 1;

        // If the stack does not have any opener of the given type,
        // then return without discarding anything.
        // This recovers nicely from situations like "{foo());}".
        if self.grouping_stack.is_empty() {
            self.grouping_stack = original_stack;
            return false;
        }

        // We found a matching group somewhere in the stack, but generally
        // don't know if we should recover by inserting synthetic closers or
        // basically ignore the current token. Try both and see which is
        // better (i.e. gives fewest rewrites later), but not nested.
        if !self.in_recovery_option {
            let ty = match open_kind {
                OPEN_SQUARE_BRACKET_TOKEN => TokenType::CLOSE_SQUARE_BRACKET,
                OPEN_CURLY_BRACKET_TOKEN => TokenType::CLOSE_CURLY_BRACKET,
                OPEN_PAREN_TOKEN => TokenType::CLOSE_PAREN,
                _ => panic!("Unexpected openKind"),
            };
            let saved = (
                self.arena.tokens.len(),
                self.arena.owned_lexemes.len(),
                self.arena.errors.len(),
            );
            let entry_len = self.grouping_stack.len();

            // Option #1: Insert synthetic closers.
            let option1_recoveries = {
                let mut option1 = self.create_recovery_option_scanner();
                option1.insert_synthetic_closers(&original_stack, entry_len);
                let next = option1.append_end_group_internal(true, ty, open_kind);
                let mut recoveries = option1.recovery_option_tokenizer(next);
                recoveries += option1.grouping_stack.len() as i64;
                self.arena = option1.arena;
                self.arena.truncate(saved.0, saved.1, saved.2);
                recoveries
            };

            // Option #2: ignore this token.
            let option2_recoveries = {
                let mut option2 = self.create_recovery_option_scanner();
                option2.grouping_stack = original_stack.clone();
                let next = option2.append_end_group_internal(false, ty, open_kind);
                let mut recoveries = option2.recovery_option_tokenizer(next);
                // We add 1 to make this option pay for ignoring this token.
                recoveries += option2.grouping_stack.len() as i64 + 1;
                self.arena = option2.arena;
                self.arena.truncate(saved.0, saved.1, saved.2);
                recoveries
            };

            // The option-runs might have set invalid endGroup pointers. Reset
            // them.
            for &t in &original_stack {
                self.tok_mut(t).end_group = TokenId::NONE;
            }

            if option2_recoveries < option1_recoveries {
                // Perform option #2 recovery.
                self.grouping_stack = original_stack;
                return false;
            }
            // option #1 is the default, so fall though.
        }

        // Insert synthetic closers and report errors for any unbalanced
        // openers. This recovers nicely from situations like "{[}".
        let entry_len = self.grouping_stack.len();
        self.insert_synthetic_closers(&original_stack, entry_len);
        true
    }

    /// Dart `insertSyntheticClosers`. `entry_to_use` is the Dart list
    /// `original_stack` without its first `original_stack.len() - entry_len`
    /// elements, i.e. `original_stack[..entry_len]` here.
    fn insert_synthetic_closers(&mut self, original_stack: &[TokenId], entry_len: usize) {
        let entry_head = original_stack[entry_len - 1];
        let mut i = original_stack.len();
        while i != entry_len {
            // Don't report unmatched errors for <; it is also the less-than
            // operator.
            if self.kind_of(entry_head) != LT_TOKEN {
                self.unmatched_begin_group(original_stack[i - 1]);
            }
            i -= 1;
        }
    }

    /// Dart `discardOpenLt`.
    #[inline(always)]
    fn discard_open_lt(&mut self) {
        while let Some(&head) = self.grouping_stack.last() {
            if self.kind_of(head) != LT_TOKEN {
                break;
            }
            self.grouping_stack.pop();
        }
    }

    /// Dart `discardInterpolation`.
    fn discard_interpolation(&mut self) {
        while let Some(&begin_token) = self.grouping_stack.last() {
            self.unmatched_begin_group(begin_token);
            self.grouping_stack.pop();
            if self.kind_of(begin_token) == STRING_INTERPOLATION_TOKEN {
                break;
            }
        }
    }

    /// Dart `unmatchedBeginGroup`.
    fn unmatched_begin_group(&mut self, begin: TokenId) {
        let ty = close_brace_info_for(self.tok(begin).ty);
        let start = self.token_start;
        let synthetic = self.push_token(
            ty,
            flags::SYNTHETIC | flags::FIXED_LEXEME,
            start.off,
            0,
            start.byte,
            start.byte,
            TokenId::NONE,
        );
        let tail = self.tail;
        self.tok_mut(synthetic).before_synthetic = tail;
        self.append_token(synthetic);
        let tail = self.tail;
        self.tok_mut(begin).end_group = tail;
        let char_offset = self.tok(begin).offset;
        self.prepend_error_token(ErrorToken {
            kind: ErrorKind::UnmatchedToken { begin },
            char_offset,
        });
        self.recovery_count += 1;
    }

    /// Dart `tokenize`. Returns the first token.
    pub fn tokenize(&mut self) -> TokenId {
        while !self.at_end_of_file() {
            let mut next = self.advance();

            // Scan the header looking for a language version
            if next != EOF {
                let mut old_tail = self.tail;
                next = self.big_header_switch(next);
                if next != EOF && self.kind_of(self.tail) == SCRIPT_TOKEN {
                    old_tail = self.tail;
                    next = self.big_header_switch(next);
                }
                while next != EOF && self.tail == old_tail {
                    next = self.big_header_switch(next);
                }
            }

            while next != EOF {
                next = self.big_switch(next);
            }
            self.append_eof_token();
        }

        // Always pretend that there's a line at the end of the file.
        let off = self.scan_offset;
        self.line_starts.push((off + 1) as u32);

        self.first_token()
    }

    /// Dart `recoveryOptionTokenizer`. Returns the number of recoveries.
    fn recovery_option_tokenizer(&mut self, mut next: i32) -> i64 {
        let mut iterations = 0;
        while !self.at_end_of_file() {
            while next != EOF {
                next = self.big_switch(next);
                iterations += 1;
                if iterations > 100 {
                    return self.recovery_count;
                }
            }
        }
        self.recovery_count
    }

    /// Dart `bigHeaderSwitch`.
    fn big_header_switch(&mut self, next: i32) -> i32 {
        if next != SLASH {
            return self.big_switch(next);
        }
        self.begin_token();
        if SLASH != self.peek() {
            return self.tokenize_slash_or_comment(next);
        }
        self.tokenize_language_version_or_single_line_comment(next)
    }

    /// Dart `bigSwitch`.
    pub(crate) fn big_switch(&mut self, next: i32) -> i32 {
        self.begin_token();
        if next == SPACE || next == TAB || next == CR {
            return self.skip_spaces();
        }
        if next == LF {
            let off = self.scan_offset;
            self.line_starts.push((off + 1) as u32); // +1, the line starts after the $LF.
            return self.skip_spaces();
        }

        let next_lower = next | 0x20;

        if LC_a <= next_lower && next_lower <= LC_z {
            if LC_r == next {
                return self.tokenize_raw_string_keyword_or_identifier(next);
            }
            return self.tokenize_keyword_or_identifier(next, true);
        }

        match next {
            CLOSE_PAREN => self.append_end_group(TokenType::CLOSE_PAREN, OPEN_PAREN_TOKEN),
            OPEN_PAREN => {
                self.append_begin_group(TokenType::OPEN_PAREN);
                self.advance()
            }
            SEMICOLON => {
                self.append_precedence_token(TokenType::SEMICOLON);
                // Type parameters and arguments cannot contain semicolon.
                self.discard_open_lt();
                self.advance()
            }
            PERIOD => self.tokenize_dots_or_number(next),
            COMMA => {
                self.append_precedence_token(TokenType::COMMA);
                self.advance()
            }
            EQ => self.tokenize_equals(next),
            CLOSE_CURLY_BRACKET => {
                if let Some(recovery_start) = self.offset_for_curly_bracket_recovery_start {
                    if let Some(&head) = self.grouping_stack.last() {
                        if self.tok(head).ty == TokenType::OPEN_CURLY_BRACKET
                            && self.tok(head).offset == recovery_start
                        {
                            // This instance of the scanner was instructed to
                            // recover this opening curly bracket.
                            self.unmatched_begin_group(head);
                            self.grouping_stack.pop();
                        }
                    }
                }
                self.append_end_group(TokenType::CLOSE_CURLY_BRACKET, OPEN_CURLY_BRACKET_TOKEN)
            }
            SLASH => self.tokenize_slash_or_comment(next),
            OPEN_CURLY_BRACKET => {
                self.append_begin_group(TokenType::OPEN_CURLY_BRACKET);
                self.advance()
            }
            DQ | SQ => {
                let start = self.scan_pos();
                self.tokenize_string(next, start, false)
            }
            UNDERSCORE => self.tokenize_keyword_or_identifier(next, true),
            COLON => {
                self.append_precedence_token(TokenType::COLON);
                self.advance()
            }
            LT => self.tokenize_less_than(next),
            GT => self.tokenize_greater_than(next),
            BANG => self.tokenize_exclamation(next),
            OPEN_SQUARE_BRACKET => self.tokenize_open_square_bracket(next),
            CLOSE_SQUARE_BRACKET => {
                self.append_end_group(TokenType::CLOSE_SQUARE_BRACKET, OPEN_SQUARE_BRACKET_TOKEN)
            }
            AT => self.tokenize_at(next),
            D1..=D9 => self.tokenize_number(next),
            AMPERSAND => self.tokenize_ampersand(next),
            D0 => self.tokenize_hex_or_number(next),
            QUESTION => self.tokenize_question(next),
            BAR => self.tokenize_bar(next),
            PLUS => self.tokenize_plus(next),
            DOLLAR => self.tokenize_keyword_or_identifier(next, true),
            MINUS => self.tokenize_minus(next),
            STAR => self.tokenize_multiply(next),
            CARET => self.tokenize_caret(next),
            TILDE => self.tokenize_tilde(next),
            PERCENT => self.tokenize_percent(next),
            BACKPING => {
                self.append_precedence_token(TokenType::BACKPING);
                self.advance()
            }
            BACKSLASH => {
                self.append_precedence_token(TokenType::BACKSLASH);
                self.advance()
            }
            HASH => self.tokenize_tag(next),
            _ => {
                if next < 0x1f {
                    return self.unexpected(next);
                }
                let next = self.current_as_unicode(next);
                self.unexpected(next)
            }
        }
    }

    /// Dart `tokenizeTag`.
    fn tokenize_tag(&mut self, _next: i32) -> i32 {
        let mut next: i32;
        // # or #!.*[\n\r]
        if self.scan_offset == 0 && self.peek() == BANG {
            let start = self.scan_pos();
            let mut ascii_only = true;
            loop {
                next = self.advance();
                if next > 127 {
                    ascii_only = false;
                }
                if next == LF || next == CR || next == EOF {
                    break;
                }
            }
            self.append_substring_token(TokenType::SCRIPT_TAG, start, ascii_only, 0);
            return next;
        }
        self.append_precedence_token(TokenType::HASH);
        self.advance()
    }

    fn tokenize_tilde(&mut self, _next: i32) -> i32 {
        // ~ ~/ ~/=
        let next = self.advance();
        if next == SLASH {
            self.select(EQ, TokenType::TILDE_SLASH_EQ, TokenType::TILDE_SLASH)
        } else {
            self.append_precedence_token(TokenType::TILDE);
            next
        }
    }

    fn tokenize_open_square_bracket(&mut self, _next: i32) -> i32 {
        // [ [] []=
        let next = self.advance();
        if next == CLOSE_SQUARE_BRACKET {
            return self.select(EQ, TokenType::INDEX_EQ, TokenType::INDEX);
        }
        self.append_begin_group(TokenType::OPEN_SQUARE_BRACKET);
        next
    }

    fn tokenize_caret(&mut self, _next: i32) -> i32 {
        // ^ ^=
        self.select(EQ, TokenType::CARET_EQ, TokenType::CARET)
    }

    fn tokenize_question(&mut self, _next: i32) -> i32 {
        // ? ?. ?.. ?? ??=
        let mut next = self.advance();
        if next == QUESTION {
            self.select(
                EQ,
                TokenType::QUESTION_QUESTION_EQ,
                TokenType::QUESTION_QUESTION,
            )
        } else if next == PERIOD {
            next = self.advance();
            if PERIOD == next {
                self.append_precedence_token(TokenType::QUESTION_PERIOD_PERIOD);
                return self.advance();
            }
            self.append_precedence_token(TokenType::QUESTION_PERIOD);
            next
        } else {
            self.append_precedence_token(TokenType::QUESTION);
            next
        }
    }

    fn tokenize_bar(&mut self, _next: i32) -> i32 {
        // | || |= ||=
        let mut next = self.advance();
        if next == BAR {
            next = self.advance();
            // LAZY_ASSIGNMENT_ENABLED is false.
            self.append_precedence_token(TokenType::BAR_BAR);
            next
        } else if next == EQ {
            self.append_precedence_token(TokenType::BAR_EQ);
            self.advance()
        } else {
            self.append_precedence_token(TokenType::BAR);
            next
        }
    }

    fn tokenize_ampersand(&mut self, _next: i32) -> i32 {
        // && &= & &&=
        let mut next = self.advance();
        if next == AMPERSAND {
            next = self.advance();
            // LAZY_ASSIGNMENT_ENABLED is false.
            self.append_precedence_token(TokenType::AMPERSAND_AMPERSAND);
            next
        } else if next == EQ {
            self.append_precedence_token(TokenType::AMPERSAND_EQ);
            self.advance()
        } else {
            self.append_precedence_token(TokenType::AMPERSAND);
            next
        }
    }

    fn tokenize_percent(&mut self, _next: i32) -> i32 {
        // % %=
        self.select(EQ, TokenType::PERCENT_EQ, TokenType::PERCENT)
    }

    fn tokenize_multiply(&mut self, _next: i32) -> i32 {
        // * *=
        self.select(EQ, TokenType::STAR_EQ, TokenType::STAR)
    }

    fn tokenize_minus(&mut self, _next: i32) -> i32 {
        // - -- -=
        let next = self.advance();
        if next == MINUS {
            self.append_precedence_token(TokenType::MINUS_MINUS);
            self.advance()
        } else if next == EQ {
            self.append_precedence_token(TokenType::MINUS_EQ);
            self.advance()
        } else {
            self.append_precedence_token(TokenType::MINUS);
            next
        }
    }

    fn tokenize_plus(&mut self, _next: i32) -> i32 {
        // + ++ +=
        let next = self.advance();
        if PLUS == next {
            self.append_precedence_token(TokenType::PLUS_PLUS);
            self.advance()
        } else if EQ == next {
            self.append_precedence_token(TokenType::PLUS_EQ);
            self.advance()
        } else {
            self.append_precedence_token(TokenType::PLUS);
            next
        }
    }

    fn tokenize_exclamation(&mut self, _next: i32) -> i32 {
        // ! !=
        // !== is kept for user-friendly error reporting.
        let next = self.advance();
        if next == EQ {
            let next = self.advance();
            if next == EQ {
                self.append_precedence_token(TokenType::BANG_EQ_EQ);
                let token = self.tail;
                let char_offset = self.token_start.off as u32;
                self.prepend_error_token(ErrorToken {
                    kind: ErrorKind::UnsupportedOperator { token },
                    char_offset,
                });
                return self.advance();
            } else {
                self.append_precedence_token(TokenType::BANG_EQ);
                return next;
            }
        }
        self.append_precedence_token(TokenType::BANG);
        next
    }

    fn tokenize_equals(&mut self, _next: i32) -> i32 {
        // = == =>
        // === is kept for user-friendly error reporting.

        // Type parameters and arguments cannot contain any token that
        // starts with '='.
        self.discard_open_lt();

        let next = self.advance();
        if next == EQ {
            let next = self.advance();
            if next == EQ {
                self.append_precedence_token(TokenType::EQ_EQ_EQ);
                let token = self.tail;
                let char_offset = self.token_start.off as u32;
                self.prepend_error_token(ErrorToken {
                    kind: ErrorKind::UnsupportedOperator { token },
                    char_offset,
                });
                return self.advance();
            } else {
                self.append_precedence_token(TokenType::EQ_EQ);
                return next;
            }
        } else if next == GT {
            self.append_precedence_token(TokenType::FUNCTION);
            return self.advance();
        }
        self.append_precedence_token(TokenType::EQ);
        next
    }

    fn tokenize_greater_than(&mut self, _next: i32) -> i32 {
        // > >= >> >>= >>> >>>=
        let mut next = self.advance();
        if EQ == next {
            // Saw `>=` only.
            self.append_precedence_token(TokenType::GT_EQ);
            self.advance()
        } else if GT == next {
            // Saw `>>` so far.
            next = self.advance();
            if EQ == next {
                // Saw `>>=` only.
                self.append_precedence_token(TokenType::GT_GT_EQ);
                self.advance()
            } else if self.enable_triple_shift && GT == next {
                // Saw `>>>` so far.
                next = self.advance();
                if EQ == next {
                    // Saw `>>>=` only.
                    self.append_precedence_token(TokenType::GT_GT_GT_EQ);
                    self.advance()
                } else {
                    // Saw `>>>` only.
                    self.append_gt_gt_gt(TokenType::GT_GT_GT);
                    next
                }
            } else {
                // Saw `>>` only.
                self.append_gt_gt(TokenType::GT_GT);
                next
            }
        } else {
            // Saw `>` only.
            self.append_gt(TokenType::GT);
            next
        }
    }

    fn tokenize_less_than(&mut self, _next: i32) -> i32 {
        // < <= << <<=
        let next = self.advance();
        if EQ == next {
            self.append_precedence_token(TokenType::LT_EQ);
            self.advance()
        } else if LT == next {
            self.select(EQ, TokenType::LT_LT_EQ, TokenType::LT_LT)
        } else {
            self.append_begin_group(TokenType::LT);
            next
        }
    }

    fn unexpected_separator(&mut self, start: Pos) {
        let end_offset = self.scan_offset as u32;
        self.prepend_error_token(ErrorToken {
            kind: ErrorKind::UnterminatedToken {
                code: ScannerMessageCode::UnexpectedSeparatorInNumber,
                end_offset,
            },
            char_offset: start.off as u32,
        });
    }

    fn tokenize_number(&mut self, _next: i32) -> i32 {
        let mut next: i32;
        let start = self.scan_pos();
        let mut has_separators = false;
        let mut previous_was_separator = false;
        loop {
            next = self.advance();
            if D0 <= next && next <= D9 {
                previous_was_separator = false;
                continue;
            } else if next == UNDERSCORE {
                has_separators = true;
                previous_was_separator = true;
                continue;
            } else if next == LC_e || next == E {
                if previous_was_separator {
                    // Not allowed.
                    self.unexpected_separator(start);
                }
                return self.tokenize_fraction_part(next, start, has_separators);
            } else {
                if next == PERIOD {
                    if previous_was_separator {
                        // Not allowed.
                        self.unexpected_separator(start);
                    }
                    let nextnext = self.peek();
                    if D0 <= nextnext && nextnext <= D9 {
                        // Use the peeked character.
                        self.advance();
                        return self.tokenize_fraction_part(nextnext, start, has_separators);
                    } else {
                        let ty = if has_separators {
                            TokenType::INT_WITH_SEPARATORS
                        } else {
                            TokenType::INT
                        };
                        self.append_substring_token(ty, start, true, 0);
                        return next;
                    }
                }
                if previous_was_separator {
                    // End of the number is a separator; not allowed.
                    self.unexpected_separator(start);
                }
                let ty = if has_separators {
                    TokenType::INT_WITH_SEPARATORS
                } else {
                    TokenType::INT
                };
                self.append_substring_token(ty, start, true, 0);
                return next;
            }
        }
    }

    fn tokenize_hex_or_number(&mut self, next: i32) -> i32 {
        let x = self.peek();
        if x == LC_x || x == X {
            return self.tokenize_hex(next);
        }
        self.tokenize_number(next)
    }

    fn tokenize_hex(&mut self, _next: i32) -> i32 {
        let start = self.scan_pos();
        self.advance(); // Advance past the $x or $X.
        let mut has_digits = false;
        let mut has_separators = false;
        let mut previous_was_separator = false;
        loop {
            let next = self.advance();
            if (D0 <= next && next <= D9)
                || (A <= next && next <= F)
                || (LC_a <= next && next <= LC_f)
            {
                has_digits = true;
                previous_was_separator = false;
            } else if next == UNDERSCORE {
                if !has_digits {
                    // Not allowed.
                    self.unexpected_separator(start);
                }
                has_separators = true;
                previous_was_separator = true;
            } else {
                if !has_digits {
                    let end_offset = self.scan_offset as u32;
                    self.prepend_error_token(ErrorToken {
                        kind: ErrorKind::UnterminatedToken {
                            code: ScannerMessageCode::ExpectedHexDigit,
                            end_offset,
                        },
                        char_offset: start.off as u32,
                    });
                    // Recovery
                    self.append_synthetic_substring_token(TokenType::HEXADECIMAL, start, true, "0");
                    return next;
                }
                if previous_was_separator {
                    // End of the number is a separator; not allowed.
                    self.unexpected_separator(start);
                }
                let ty = if has_separators {
                    TokenType::HEXADECIMAL_WITH_SEPARATORS
                } else {
                    TokenType::HEXADECIMAL
                };
                self.append_substring_token(ty, start, true, 0);
                return next;
            }
        }
    }

    fn tokenize_dots_or_number(&mut self, _next: i32) -> i32 {
        let start = self.scan_pos();
        let mut next = self.advance();
        if D0 <= next && next <= D9 {
            self.tokenize_fraction_part(next, start, false)
        } else if PERIOD == next {
            next = self.advance();
            if next == PERIOD {
                next = self.advance();
                if next == QUESTION {
                    self.append_precedence_token(TokenType::PERIOD_PERIOD_PERIOD_QUESTION);
                    self.advance()
                } else {
                    self.append_precedence_token(TokenType::PERIOD_PERIOD_PERIOD);
                    next
                }
            } else {
                self.append_precedence_token(TokenType::PERIOD_PERIOD);
                next
            }
        } else {
            self.append_precedence_token(TokenType::PERIOD);
            next
        }
    }

    /// Dart `tokenizeFractionPart`. [next] has to be in [0-9eE].
    fn tokenize_fraction_part(
        &mut self,
        mut next: i32,
        start: Pos,
        mut has_separators: bool,
    ) -> i32 {
        let mut done = false;
        let mut previous_was_separator = false;
        while !done {
            if D0 <= next && next <= D9 {
                previous_was_separator = false;
            } else if UNDERSCORE == next {
                has_separators = true;
                previous_was_separator = true;
            } else if LC_e == next || E == next {
                if previous_was_separator {
                    // Not allowed.
                    self.unexpected_separator(start);
                }
                previous_was_separator = false;
                next = self.advance();
                while next == UNDERSCORE {
                    self.unexpected_separator(start);
                    has_separators = true;
                    previous_was_separator = true;
                    next = self.advance();
                }
                if next == PLUS || next == MINUS {
                    previous_was_separator = false;
                    next = self.advance();
                }
                let mut has_exponent_digits = false;
                loop {
                    if D0 <= next && next <= D9 {
                        has_exponent_digits = true;
                        previous_was_separator = false;
                    } else if next == UNDERSCORE {
                        if !has_exponent_digits {
                            self.unexpected_separator(start);
                        }
                        has_separators = true;
                        previous_was_separator = true;
                    } else {
                        if !has_exponent_digits {
                            let ty = if has_separators {
                                TokenType::DOUBLE_WITH_SEPARATORS
                            } else {
                                TokenType::DOUBLE
                            };
                            self.append_synthetic_substring_token(ty, start, true, "0");
                            let end_offset = self.scan_offset as u32;
                            let char_offset = self.token_start.off as u32;
                            self.prepend_error_token(ErrorToken {
                                kind: ErrorKind::UnterminatedToken {
                                    code: ScannerMessageCode::MissingExponent,
                                    end_offset,
                                },
                                char_offset,
                            });
                            return next;
                        }
                        break;
                    }
                    next = self.advance();
                }
                if previous_was_separator {
                    // End of the number is a separator; not allowed.
                    self.unexpected_separator(start);
                }
                done = true;
                continue;
            } else {
                if previous_was_separator {
                    // End of the number is a separator; not allowed.
                    self.unexpected_separator(start);
                }
                done = true;
                continue;
            }
            next = self.advance();
        }
        let ty = if has_separators {
            TokenType::DOUBLE_WITH_SEPARATORS
        } else {
            TokenType::DOUBLE
        };
        self.append_substring_token(ty, start, true, 0);
        next
    }

    fn tokenize_slash_or_comment(&mut self, _next: i32) -> i32 {
        let start = self.scan_pos();
        let next = self.advance();
        if STAR == next {
            self.tokenize_multi_line_comment(next, start)
        } else if SLASH == next {
            self.tokenize_single_line_comment(next, start)
        } else if EQ == next {
            self.append_precedence_token(TokenType::SLASH_EQ);
            self.advance()
        } else {
            self.append_precedence_token(TokenType::SLASH);
            next
        }
    }

    fn tokenize_language_version_or_single_line_comment(&mut self, _next: i32) -> i32 {
        let start = self.scan_pos();
        let mut next = self.advance();
        debug_assert!(next == SLASH);

        // Dart doc
        if SLASH == self.peek() {
            return self.tokenize_single_line_comment(next, start);
        }

        // "@dart"
        next = self.advance();
        while SPACE == next {
            next = self.advance();
        }
        for expected in [AT, LC_d, LC_a, LC_r, LC_t] {
            if expected != next {
                return self.tokenize_single_line_comment_rest(next, start, false);
            }
            next = self.advance();
        }

        // "="
        while SPACE == next {
            next = self.advance();
        }
        if EQ != next {
            return self.tokenize_single_line_comment_rest(next, start, false);
        }
        next = self.advance();

        // major
        while SPACE == next {
            next = self.advance();
        }
        let mut major: i64 = 0;
        let major_start = self.scan_offset;
        while is_digit(next) {
            major = major.wrapping_mul(10).wrapping_add((next - D0) as i64);
            next = self.advance();
        }
        if self.scan_offset == major_start {
            return self.tokenize_single_line_comment_rest(next, start, false);
        }

        // minor
        if PERIOD != next {
            return self.tokenize_single_line_comment_rest(next, start, false);
        }
        next = self.advance();
        let mut minor: i64 = 0;
        let minor_start = self.scan_offset;
        while is_digit(next) {
            minor = minor.wrapping_mul(10).wrapping_add((next - D0) as i64);
            next = self.advance();
        }
        if self.scan_offset == minor_start {
            return self.tokenize_single_line_comment_rest(next, start, false);
        }

        // trailing spaces
        while SPACE == next {
            next = self.advance();
        }
        if next != LF && next != CR && next != EOF {
            return self.tokenize_single_line_comment_rest(next, start, false);
        }

        let token = self.create_language_version_token(start, major, minor);
        let t = self.tok(token);
        let info = LanguageVersionInfo {
            token,
            major,
            minor,
            offset: t.offset,
            length: t.length,
        };
        self.language_version = Some(info);
        if let Some(callback) = self.language_version_changed.as_mut() {
            let config = callback(info);
            self.set_configuration(config);
        }
        if self.include_comments {
            self.append_to_comment_stream(token);
        }
        next
    }

    fn tokenize_single_line_comment(&mut self, _next: i32, start: Pos) -> i32 {
        let next = self.advance();
        let dartdoc = SLASH == next;
        self.tokenize_single_line_comment_rest(next, start, dartdoc)
    }

    fn tokenize_single_line_comment_rest(&mut self, next: i32, start: Pos, dartdoc: bool) -> i32 {
        let mut ascii_only = true;
        if next > 127 {
            ascii_only = false;
        }
        if LF == next || CR == next || EOF == next {
            self.tokenize_single_line_comment_append(ascii_only, start, dartdoc);
            return next;
        }
        ascii_only &= self.scan_until_line_end();
        self.tokenize_single_line_comment_append(ascii_only, start, dartdoc);
        self.current()
    }

    fn tokenize_single_line_comment_append(&mut self, ascii_only: bool, start: Pos, dartdoc: bool) {
        if dartdoc {
            self.append_dart_doc(start, TokenType::SINGLE_LINE_COMMENT, ascii_only);
        } else {
            self.append_comment(start, TokenType::SINGLE_LINE_COMMENT, ascii_only);
        }
    }

    fn tokenize_multi_line_comment(&mut self, _next: i32, start: Pos) -> i32 {
        let mut ascii_only_comment = true; // Track if the entire comment is ASCII.
        let mut nesting = 1;
        let mut next = self.advance();
        let dartdoc = STAR == next;
        loop {
            if EOF == next {
                let end_offset = self.scan_offset as u32;
                let char_offset = self.token_start.off as u32;
                self.prepend_error_token(ErrorToken {
                    kind: ErrorKind::UnterminatedToken {
                        code: ScannerMessageCode::UnterminatedComment,
                        end_offset,
                    },
                    char_offset,
                });
                self.advance_after_error();
                break;
            } else if STAR == next {
                next = self.advance();
                if SLASH == next {
                    nesting -= 1;
                    if 0 == nesting {
                        next = self.advance();
                        if dartdoc {
                            self.append_dart_doc(
                                start,
                                TokenType::MULTI_LINE_COMMENT,
                                ascii_only_comment,
                            );
                        } else {
                            self.append_comment(
                                start,
                                TokenType::MULTI_LINE_COMMENT,
                                ascii_only_comment,
                            );
                        }
                        break;
                    } else {
                        next = self.advance();
                    }
                }
            } else if SLASH == next {
                next = self.advance();
                if STAR == next {
                    next = self.advance();
                    nesting += 1;
                }
            } else if next == LF {
                self.line_feed_in_multiline();
                next = self.advance();
            } else {
                if next > 127 {
                    ascii_only_comment = false;
                }
                // `next = advance()` until a character that the loop handles.
                next = self.advance_until(&STOP_MULTI_LINE_COMMENT);
            }
        }
        next
    }

    fn append_comment(&mut self, start: Pos, ty: TokenType, ascii_only: bool) {
        if !self.include_comments {
            return;
        }
        let comment = self.create_comment_token(ty, start, ascii_only, 0);
        self.append_to_comment_stream(comment);
    }

    fn append_dart_doc(&mut self, start: Pos, ty: TokenType, ascii_only: bool) {
        if !self.include_comments {
            return;
        }
        let comment = self.create_comment_token(ty, start, ascii_only, 0);
        self.tok_mut(comment).flags |= flags::DOC_COMMENT;
        self.append_to_comment_stream(comment);
    }

    /// Dart `appendToken`: appends to the tail of the token stream.
    #[inline(always)]
    pub(crate) fn append_token(&mut self, token: TokenId) {
        let tail = self.tail;
        self.tok_mut(tail).next = token;
        self.tok_mut(token).previous = tail;
        self.tail = token;
        if self.comments.is_some() && self.comments == self.tok(token).preceding_comments {
            self.comments = TokenId::NONE;
            self.comments_tail = TokenId::NONE;
        }
    }

    fn append_to_comment_stream(&mut self, new_comment: TokenId) {
        if self.comments.is_none() {
            self.comments = new_comment;
            self.comments_tail = new_comment;
        } else {
            let tail = self.comments_tail;
            self.tok_mut(tail).next = new_comment;
            self.tok_mut(new_comment).previous = tail;
            self.comments_tail = new_comment;
        }
    }

    fn tokenize_raw_string_keyword_or_identifier(&mut self, next: i32) -> i32 {
        // [next] is $r.
        let nextnext = self.peek();
        if nextnext == DQ || nextnext == SQ {
            let start = self.scan_pos();
            let next = self.advance();
            return self.tokenize_string(next, start, true);
        }
        self.tokenize_keyword_or_identifier(next, true)
    }

    /// Dart `tokenizeKeywordOrIdentifier`.
    fn tokenize_keyword_or_identifier(&mut self, mut next: i32, allow_dollar: bool) -> i32 {
        let table = keyword_state::table();
        let mut state = KeywordState::root();
        let start = self.scan_pos();
        // We allow a leading capital character.
        if A <= next && next <= LC_z {
            state = state.next(table, next);
            // The loop below on bytes: `next = advance()` while the state is
            // not null and `next` is in `a`..`z` (all ASCII).
            let src = self.src;
            let mut p = self.pos + 1;
            while !state.is_null() && p < src.len() && src[p].is_ascii_lowercase() {
                state = state.next(table, src[p] as i32);
                p += 1;
            }
            next = self.stop_at(p, (p - self.pos) as i64);
        }
        while !state.is_null() && LC_a <= next && next <= LC_z {
            state = state.next(table, next);
            next = self.advance();
        }
        if state.is_null() {
            return self.tokenize_identifier(next, start, allow_dollar);
        }
        let Some(keyword) = state.keyword(table) else {
            return self.tokenize_identifier(next, start, allow_dollar);
        };
        if !self.enable_augmentations && keyword == Keyword::AUGMENT {
            return self.tokenize_identifier(next, start, allow_dollar);
        }
        if (A <= next && next <= Z)
            || (D0 <= next && next <= D9)
            || next == UNDERSCORE
            || (allow_dollar && next == DOLLAR)
        {
            self.tokenize_identifier(next, start, allow_dollar)
        } else {
            self.append_keyword_token(keyword);
            next
        }
    }

    /// Dart `tokenizeIdentifier`. [allow_dollar] can exclude '$', which is
    /// not allowed as part of a string interpolation identifier.
    fn tokenize_identifier(&mut self, mut next: i32, start: Pos, allow_dollar: bool) -> i32 {
        if allow_dollar {
            // Normal case is to allow dollar.
            if is_identifier_char(next, true) {
                next = self.pass_identifier_char_allow_dollar();
                self.append_substring_token(TokenType::IDENTIFIER, start, true, 0);
            } else {
                // Identifier ends here.
                if start.off == self.scan_offset {
                    return self.unexpected(next);
                } else {
                    self.append_substring_token(TokenType::IDENTIFIER, start, true, 0);
                }
            }
        } else {
            loop {
                if is_identifier_char(next, false) {
                    next = self.advance();
                } else {
                    // Identifier ends here.
                    if start.off == self.scan_offset {
                        return self.unexpected(next);
                    } else {
                        self.append_substring_token(TokenType::IDENTIFIER, start, true, 0);
                    }
                    break;
                }
            }
        }
        next
    }

    fn tokenize_at(&mut self, _next: i32) -> i32 {
        self.append_precedence_token(TokenType::AT);
        self.advance()
    }

    /// Dart `tokenizeString`.
    fn tokenize_string(&mut self, mut next: i32, start: Pos, raw: bool) -> i32 {
        let quote_char = next;
        next = self.advance();
        if quote_char == next {
            next = self.advance();
            if quote_char == next {
                // Multiline string.
                return self.tokenize_multi_line_string(quote_char, start, raw);
            } else {
                // Empty string.
                self.append_substring_token(TokenType::STRING, start, true, 0);
                return next;
            }
        }
        if raw {
            self.tokenize_single_line_raw_string(next, quote_char, start)
        } else {
            self.tokenize_single_line_string(next, quote_char, start)
        }
    }

    /// Dart `tokenizeSingleLineString`.
    fn tokenize_single_line_string(
        &mut self,
        mut next: i32,
        quote_char: i32,
        quote_start: Pos,
    ) -> i32 {
        let mut start = quote_start;
        let mut ascii_only = true;
        while next != quote_char {
            if next == BACKSLASH {
                next = self.advance();
            } else if next == DOLLAR {
                next = self.tokenize_string_interpolation(start, ascii_only);
                start = self.scan_pos();
                ascii_only = true;
                continue;
            }
            if next <= CR && (next == LF || next == CR || next == EOF) {
                self.unterminated_string(quote_char, quote_start, start, ascii_only, false, false);
                return next;
            }
            if next > 127 {
                ascii_only = false;
            }
            // `next = advance()` until a character that the loop handles.
            next = self.advance_until(if quote_char == DQ {
                &STOP_STRING_DQ
            } else {
                &STOP_STRING_SQ
            });
        }
        // Advance past the quote character.
        next = self.advance();
        self.append_substring_token(TokenType::STRING, start, ascii_only, 0);
        next
    }

    fn tokenize_string_interpolation(&mut self, start: Pos, ascii_only: bool) -> i32 {
        self.append_substring_token(TokenType::STRING, start, ascii_only, 0);
        self.begin_token(); // $ starts here.
        let next = self.advance();
        if next == OPEN_CURLY_BRACKET {
            self.tokenize_interpolated_expression(next)
        } else {
            self.tokenize_interpolated_identifier(next)
        }
    }

    fn tokenize_interpolated_expression(&mut self, _next: i32) -> i32 {
        let mut next: i32;
        self.append_begin_group(TokenType::STRING_INTERPOLATION_EXPRESSION);
        if let Some(recovery_start) = self.offset_for_curly_bracket_recovery_start {
            if self.token_start.off == recovery_start as i64 {
                // This instance of the scanner was instructed to recover this
                // string interpolation start.
                self.discard_interpolation();
                return self.advance();
            }
        }
        self.begin_token(); // The expression starts here.
        next = self.advance(); // Move past the curly bracket.
        while next != EOF && next != STX_SIGNAL {
            next = self.big_switch(next);
        }
        if next == EOF {
            self.begin_token();
            self.discard_interpolation();
            return next;
        }
        next = self.advance(); // Move past the $STX.
        self.begin_token(); // The string interpolation suffix starts here.
        next
    }

    fn tokenize_interpolated_identifier(&mut self, mut next: i32) -> i32 {
        self.append_precedence_token(TokenType::STRING_INTERPOLATION_IDENTIFIER);

        if (LC_a <= next && next <= LC_z) || (A <= next && next <= Z) || next == UNDERSCORE {
            self.begin_token(); // The identifier starts here.
            next = self.tokenize_keyword_or_identifier(next, false);
        } else {
            self.begin_token(); // The synthetic identifier starts here.
            let start = self.scan_pos();
            self.append_synthetic_substring_token(TokenType::IDENTIFIER, start, true, "");
            let end_offset = self.scan_offset as u32;
            let char_offset = self.token_start.off as u32;
            self.prepend_error_token(ErrorToken {
                kind: ErrorKind::UnterminatedToken {
                    code: ScannerMessageCode::UnexpectedDollarInString,
                    end_offset,
                },
                char_offset,
            });
        }
        self.begin_token(); // The string interpolation suffix starts here.
        next
    }

    fn tokenize_single_line_raw_string(
        &mut self,
        mut next: i32,
        quote_char: i32,
        quote_start: Pos,
    ) -> i32 {
        let mut ascii_only = true;
        while next != EOF {
            if next == quote_char {
                next = self.advance();
                self.append_substring_token(TokenType::STRING, quote_start, ascii_only, 0);
                return next;
            } else if next == LF || next == CR {
                self.unterminated_string(
                    quote_char,
                    quote_start,
                    quote_start,
                    ascii_only,
                    false,
                    true,
                );
                return next;
            } else if next > 127 {
                ascii_only = false;
            }
            // `next = advance()` until a character that the loop handles.
            next = self.advance_until(if quote_char == DQ {
                &STOP_RAW_STRING_DQ
            } else {
                &STOP_RAW_STRING_SQ
            });
        }
        self.unterminated_string(
            quote_char,
            quote_start,
            quote_start,
            ascii_only,
            false,
            true,
        );
        next
    }

    fn tokenize_multi_line_raw_string(&mut self, quote_char: i32, quote_start: Pos) -> i32 {
        let mut ascii_only_string = true;
        let ascii_only_line = true;
        let mut next = self.advance(); // Advance past the (last) quote (of three).
        'outer: while next != EOF {
            while next != quote_char {
                if next == LF {
                    self.line_feed_in_multiline();
                } else if next > 127 {
                    ascii_only_string = false;
                }
                // `next = advance()` until a character that the loop handles.
                next = self.advance_until(if quote_char == DQ {
                    &STOP_RAW_MULTI_LINE_DQ
                } else {
                    &STOP_RAW_MULTI_LINE_SQ
                });
                if next == EOF {
                    break 'outer;
                }
            }
            next = self.advance();
            if next == quote_char {
                next = self.advance();
                if next == quote_char {
                    next = self.advance();
                    self.append_substring_token(
                        TokenType::STRING,
                        quote_start,
                        ascii_only_string,
                        0,
                    );
                    return next;
                }
            }
        }
        self.unterminated_string(
            quote_char,
            quote_start,
            quote_start,
            ascii_only_line,
            true,
            true,
        );
        next
    }

    fn tokenize_multi_line_string(&mut self, quote_char: i32, quote_start: Pos, raw: bool) -> i32 {
        if raw {
            return self.tokenize_multi_line_raw_string(quote_char, quote_start);
        }
        let mut start = quote_start;
        let mut ascii_only_string = true;
        let mut next = self.advance(); // Advance past the (last) quote (of three).
        while next != EOF {
            if next == DOLLAR {
                next = self.tokenize_string_interpolation(start, ascii_only_string);
                start = self.scan_pos();
                ascii_only_string = true; // A new string token is created for the rest.
                continue;
            }
            if next == quote_char {
                next = self.advance();
                if next == quote_char {
                    next = self.advance();
                    if next == quote_char {
                        next = self.advance();
                        self.append_substring_token(TokenType::STRING, start, ascii_only_string, 0);
                        return next;
                    }
                }
                continue;
            }
            if next == BACKSLASH {
                next = self.advance();
                if next == EOF {
                    break;
                }
            }
            if next == LF {
                self.line_feed_in_multiline();
            } else if next > 127 {
                ascii_only_string = false;
            }
            // `next = advance()` until a character that the loop handles.
            next = self.advance_until(if quote_char == DQ {
                &STOP_MULTI_LINE_STRING_DQ
            } else {
                &STOP_MULTI_LINE_STRING_SQ
            });
        }
        self.unterminated_string(
            quote_char,
            quote_start,
            start,
            ascii_only_string,
            true,
            false,
        );
        next
    }

    /// Dart `unexpected`.
    fn unexpected(&mut self, character: i32) -> i32 {
        let token_start = self.token_start;
        let error = build_unexpected_character_token(character, token_start.off as u32);
        if matches!(error.kind, ErrorKind::NonAsciiIdentifier { .. }) {
            let tail = self.tail;
            let t = self.tok(tail);
            let start = if t.ty == TokenType::IDENTIFIER && t.end() as i64 == token_start.off {
                let start = Pos {
                    off: t.offset as i64,
                    byte: self.arena.byte_range(tail).start,
                };
                self.tail = t.previous;
                start
            } else {
                token_start
            };
            self.prepend_error_token(error);
            let mut next = self.advance_after_error();
            while is_identifier_char(next, true) {
                next = self.advance();
            }
            let end = self.scan_pos();
            let comments = self.comments;
            let token = self.push_token(
                TokenType::IDENTIFIER,
                0,
                start.off,
                end.off - start.off,
                start.byte,
                end.byte,
                comments,
            );
            self.append_token(token);
            next
        } else {
            self.prepend_error_token(error);
            self.advance_after_error()
        }
    }

    /// Dart `unterminatedString`.
    fn unterminated_string(
        &mut self,
        quote_char: i32,
        quote_start: Pos,
        start: Pos,
        ascii_only: bool,
        is_multi_line: bool,
        is_raw: bool,
    ) {
        let (suffix, prefix): (&str, &'static str) = match (quote_char == DQ, is_multi_line, is_raw)
        {
            (true, false, false) => ("\"", "\""),
            (true, false, true) => ("\"", "r\""),
            (true, true, false) => ("\"\"\"", "\"\"\""),
            (true, true, true) => ("\"\"\"", "r\"\"\""),
            (false, false, false) => ("'", "'"),
            (false, false, true) => ("'", "r'"),
            (false, true, false) => ("'''", "'''"),
            (false, true, true) => ("'''", "r'''"),
        };
        self.append_synthetic_substring_token(TokenType::STRING, start, ascii_only, suffix);
        // Ensure that the error is reported on a visible token
        let error_start = if self.token_start.off < self.scan_offset {
            self.token_start.off
        } else {
            quote_start.off
        };
        let end_offset = self.scan_offset as u32;
        self.prepend_error_token(ErrorToken {
            kind: ErrorKind::UnterminatedString {
                start: prefix,
                end_offset,
            },
            char_offset: error_start as u32,
        });
    }

    fn advance_after_error(&mut self) -> i32 {
        if self.at_end_of_file() {
            return EOF;
        }
        self.advance() // Ensure progress.
    }
}

/// Dart `closeBraceInfoFor`.
fn close_brace_info_for(begin: TokenType) -> TokenType {
    match begin {
        TokenType::OPEN_PAREN => TokenType::CLOSE_PAREN,
        TokenType::OPEN_SQUARE_BRACKET => TokenType::CLOSE_SQUARE_BRACKET,
        TokenType::OPEN_CURLY_BRACKET => TokenType::CLOSE_CURLY_BRACKET,
        TokenType::LT => TokenType::GT,
        TokenType::STRING_INTERPOLATION_EXPRESSION => TokenType::CLOSE_CURLY_BRACKET,
        _ => panic!("not a begin token: {begin:?}"),
    }
}

/// Dart `isIdentifierChar` (internal_utils.dart).
#[inline(always)]
pub fn is_identifier_char(next: i32, allow_dollar: bool) -> bool {
    (LC_a <= next && next <= LC_z)
        || (A <= next && next <= Z)
        || (D0 <= next && next <= D9)
        || next == UNDERSCORE
        || (next == DOLLAR && allow_dollar)
}
