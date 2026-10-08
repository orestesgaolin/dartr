// Dart source: pkg/_fe_analyzer_shared/lib/src/scanner/token.dart
// Dart source: pkg/_fe_analyzer_shared/lib/src/scanner/token_impl.dart

//! Tokens and the token arena.
//!
//! Dart uses a doubly linked list of heap objects (`SimpleToken`,
//! `StringToken`, `BeginToken`, `KeywordToken`, `CommentToken`,
//! `SyntheticToken`, ...). Here all tokens of a file live in one
//! [`Tokens`] arena (a `Vec<Token>`) and refer to each other with
//! [`TokenId`] indices: `next`, `previous`, `preceding_comments`,
//! `end_group` and `before_synthetic` are the Dart fields of the same name.
//! The Dart class of a token is encoded in [`Token::flags`].

use std::sync::Arc;

use crate::error_token::ErrorToken;
use crate::token_constants::*;
use crate::token_type::*;

/// Index of a token in a [`Tokens`] arena. [`TokenId::NONE`] is Dart `null`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct TokenId(pub u32);

impl TokenId {
    pub const NONE: TokenId = TokenId(u32::MAX);

    #[inline(always)]
    pub fn is_none(self) -> bool {
        self.0 == u32::MAX
    }

    #[inline(always)]
    pub fn is_some(self) -> bool {
        self.0 != u32::MAX
    }

    #[inline(always)]
    pub fn index(self) -> usize {
        self.0 as usize
    }

    /// `None` for [`TokenId::NONE`].
    #[inline(always)]
    pub fn get(self) -> Option<TokenId> {
        if self.is_none() { None } else { Some(self) }
    }
}

/// Bits of [`Token::flags`].
pub mod flags {
    /// Dart `SyntheticToken`, `SyntheticStringToken`, `SyntheticBeginToken`,
    /// `SyntheticKeywordToken`: `isSynthetic` is always true.
    pub const SYNTHETIC: u8 = 1;
    /// Dart `DartDocToken` (a `///` or `/**` comment).
    pub const DOC_COMMENT: u8 = 2;
    /// Dart `LanguageVersionToken` (`// @dart = x.y`).
    pub const LANGUAGE_VERSION: u8 = 4;
    /// The lexeme is the lexeme of the token type (operators, keywords,
    /// synthetic closers, EOF).
    pub const FIXED_LEXEME: u8 = 8;
    /// The lexeme is in [`super::Tokens`] side table (synthetic lexemes that
    /// are not a substring of the source, for example `"abc"` for the
    /// unterminated string `"abc`).
    pub const OWNED_LEXEME: u8 = 16;
    /// Dart `ErrorToken`. `lex_start` is the index of the error data.
    pub const ERROR: u8 = 32;
    /// Dart `CommentToken`.
    pub const COMMENT: u8 = 64;
    /// Dart `ReplacementToken` (a synthetic token made by the parser).
    /// `end_group` is the replaced token (Dart `replacedToken`).
    pub const REPLACEMENT: u8 = 128;
}

/// One token. A port of Dart `SimpleToken` and its subclasses.
#[derive(Clone, Debug)]
pub struct Token {
    pub ty: TokenType,
    pub flags: u8,
    /// Offset in UTF-16 code units (Dart `offset`/`charOffset`).
    pub offset: u32,
    /// Length in UTF-16 code units (Dart `length`). Zero for most synthetic
    /// tokens; for `SyntheticStringToken` the length of the source text.
    pub length: u32,
    /// Byte range of the lexeme in the source text (when the lexeme is a
    /// substring of the source), or an index into a side table (see
    /// [`flags::OWNED_LEXEME`] and [`flags::ERROR`]).
    pub(crate) lex_start: u32,
    pub(crate) lex_end: u32,
    pub next: TokenId,
    pub previous: TokenId,
    /// First comment before this token (Dart `precedingComments`). Comment
    /// tokens are linked with `next`.
    pub preceding_comments: TokenId,
    /// Dart `BeginToken.endGroup` (`endToken`).
    pub end_group: TokenId,
    /// Dart `beforeSynthetic` of synthetic tokens.
    pub before_synthetic: TokenId,
}

impl Token {
    /// A token whose lexeme is the lexeme of its type (an operator, a
    /// keyword, or with [synthetic] a synthetic token of length 0, Dart
    /// `SyntheticToken`), not linked into a stream. For use by the parser,
    /// with [`Tokens::push`] and [`Tokens::set_next`]. [byte_offset] is the
    /// position in the source text.
    pub fn fixed(ty: TokenType, offset: u32, byte_offset: u32, synthetic: bool) -> Token {
        let len = if synthetic {
            0
        } else {
            ty.lexeme().len() as u32
        };
        Token {
            ty,
            flags: flags::FIXED_LEXEME | if synthetic { flags::SYNTHETIC } else { 0 },
            offset,
            length: len,
            lex_start: byte_offset,
            lex_end: byte_offset + len,
            next: TokenId::NONE,
            previous: TokenId::NONE,
            preceding_comments: TokenId::NONE,
            end_group: TokenId::NONE,
            before_synthetic: TokenId::NONE,
        }
    }

    #[inline(always)]
    pub fn kind(&self) -> i32 {
        self.ty.kind()
    }

    /// Dart `Token.isSynthetic`.
    #[inline(always)]
    pub fn is_synthetic(&self) -> bool {
        self.flags & flags::SYNTHETIC != 0 || self.length == 0
    }

    #[inline(always)]
    pub fn is_eof(&self) -> bool {
        self.ty == TokenType::EOF
    }

    /// Dart `Token.end`.
    #[inline(always)]
    pub fn end(&self) -> u32 {
        self.offset.wrapping_add(self.length)
    }

    /// Dart `Token.offset` as a signed value: the synthetic token before
    /// the first token (Dart `Token.eof(-1)`) has offset -1 (`u32::MAX`).
    #[inline(always)]
    pub fn signed_offset(&self) -> i64 {
        self.offset as i32 as i64
    }

    /// Dart `ReplacementToken`: a synthetic token that replaces other
    /// tokens (`TokenStreamRewriter.replaceNextTokenWithSyntheticToken`).
    #[inline(always)]
    pub fn is_replacement(&self) -> bool {
        self.flags & flags::REPLACEMENT != 0
    }

    /// Dart `token is ErrorToken`.
    #[inline(always)]
    pub fn is_error(&self) -> bool {
        self.flags & flags::ERROR != 0
    }

    #[inline(always)]
    pub fn is_comment(&self) -> bool {
        self.flags & flags::COMMENT != 0
    }

    /// Dart `token is DocumentationCommentToken`.
    #[inline(always)]
    pub fn is_doc_comment(&self) -> bool {
        self.flags & flags::DOC_COMMENT != 0
    }

    /// Dart `Token.isIdentifier`.
    pub fn is_identifier(&self) -> bool {
        if self.ty.is_keyword() {
            matches!(
                self.ty.keyword_style(),
                Some(KeywordStyle::Pseudo | KeywordStyle::BuiltIn)
            )
        } else {
            self.kind() == IDENTIFIER_TOKEN
        }
    }

    /// Dart `Token.isKeywordOrIdentifier`.
    pub fn is_keyword_or_identifier(&self) -> bool {
        self.ty.is_keyword() || self.kind() == IDENTIFIER_TOKEN
    }
}

/// The token arena of one file, plus the side tables that the tokens use.
#[derive(Clone, Debug, Default)]
pub struct Tokens {
    /// The source text (without a leading byte order mark).
    pub source: Arc<str>,
    pub(crate) tokens: Vec<Token>,
    pub(crate) owned_lexemes: Vec<Box<str>>,
    pub(crate) errors: Vec<ErrorToken>,
}

impl Tokens {
    pub fn new(source: Arc<str>) -> Tokens {
        Tokens {
            source,
            tokens: Vec::new(),
            owned_lexemes: Vec::new(),
            errors: Vec::new(),
        }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    #[inline(always)]
    pub fn get(&self, id: TokenId) -> &Token {
        &self.tokens[id.index()]
    }

    #[inline(always)]
    pub fn get_mut(&mut self, id: TokenId) -> &mut Token {
        &mut self.tokens[id.index()]
    }

    #[inline(always)]
    pub fn ty(&self, id: TokenId) -> TokenType {
        self.tokens[id.index()].ty
    }

    #[inline(always)]
    pub fn next(&self, id: TokenId) -> TokenId {
        self.tokens[id.index()].next
    }

    #[inline(always)]
    pub fn previous(&self, id: TokenId) -> TokenId {
        self.tokens[id.index()].previous
    }

    #[inline(always)]
    pub fn offset(&self, id: TokenId) -> u32 {
        self.tokens[id.index()].offset
    }

    /// Dart `Token.lexeme`. For error tokens (where Dart throws) this
    /// returns the empty string.
    pub fn lexeme(&self, id: TokenId) -> &str {
        let t = &self.tokens[id.index()];
        if t.flags & flags::FIXED_LEXEME != 0 {
            t.ty.lexeme()
        } else if t.flags & flags::OWNED_LEXEME != 0 {
            &self.owned_lexemes[t.lex_start as usize]
        } else if t.flags & flags::ERROR != 0 {
            ""
        } else {
            &self.source[t.lex_start as usize..t.lex_end as usize]
        }
    }

    /// Byte range of the token in the source text. For synthetic tokens the
    /// range is empty or covers the source part of the lexeme.
    pub fn byte_range(&self, id: TokenId) -> std::ops::Range<usize> {
        let t = &self.tokens[id.index()];
        if t.flags & (flags::OWNED_LEXEME | flags::ERROR) != 0 {
            let s = t.lex_end as usize;
            s..s
        } else {
            t.lex_start as usize..t.lex_end as usize
        }
    }

    /// The error data of an error token (Dart `ErrorToken`).
    pub fn error(&self, id: TokenId) -> Option<&ErrorToken> {
        let t = &self.tokens[id.index()];
        if t.flags & flags::ERROR != 0 {
            Some(&self.errors[t.lex_start as usize])
        } else {
            None
        }
    }

    /// Adds a token to the arena (not linked into the stream). Use this to
    /// insert synthetic tokens: push, then set `next`/`previous`.
    pub fn push(&mut self, token: Token) -> TokenId {
        let id = TokenId(self.tokens.len() as u32);
        self.tokens.push(token);
        id
    }

    /// Adds a token whose lexeme is not a substring of the source.
    pub fn push_with_lexeme(&mut self, mut token: Token, lexeme: &str) -> TokenId {
        token.flags |= flags::OWNED_LEXEME;
        token.flags &= !flags::FIXED_LEXEME;
        token.lex_start = self.owned_lexemes.len() as u32;
        self.owned_lexemes.push(lexeme.into());
        self.push(token)
    }

    /// Copies token [id] of [src] (the arena of another file) into this
    /// arena and returns the new id. The copy keeps the type, the flags, the
    /// offset and the length. Its lexeme does not use the source text of
    /// this arena: a fixed lexeme stays the lexeme of the type, an error
    /// token gets a copy of its error data, and every other token gets an
    /// owned lexeme. The byte position of the copy is 0. The links (`next`,
    /// `previous`, `preceding_comments`, `end_group`, `before_synthetic`)
    /// are none; the caller links the copy (for example with
    /// [`Tokens::set_next`]).
    pub fn push_copy(&mut self, src: &Tokens, id: TokenId) -> TokenId {
        let mut token = src.get(id).clone();
        token.next = TokenId::NONE;
        token.previous = TokenId::NONE;
        token.preceding_comments = TokenId::NONE;
        token.end_group = TokenId::NONE;
        token.before_synthetic = TokenId::NONE;
        token.lex_end = 0;
        if token.flags & flags::FIXED_LEXEME != 0 {
            token.lex_start = 0;
            self.push(token)
        } else if token.flags & (flags::OWNED_LEXEME | flags::ERROR) == flags::ERROR {
            token.lex_start = self.errors.len() as u32;
            self.errors
                .push(src.errors[src.get(id).lex_start as usize].clone());
            self.push(token)
        } else {
            self.push_with_lexeme(token, src.lexeme(id))
        }
    }

    /// The byte offset in the source text where token [id] starts; for
    /// tokens made by the parser, the position they were inserted at.
    pub fn byte_offset(&self, id: TokenId) -> u32 {
        let t = &self.tokens[id.index()];
        if t.flags & (flags::OWNED_LEXEME | flags::ERROR) != 0 {
            t.lex_end
        } else {
            t.lex_start
        }
    }

    /// Dart `new SimpleToken(type, offset, precedingComments)` /
    /// `new BeginToken(...)` / `new Token(type, offset)`: a non-synthetic
    /// token with the lexeme of its type (used to split `>>`, `[]`, ...).
    /// [byte_offset] is the position in the source text.
    pub fn push_simple(
        &mut self,
        ty: TokenType,
        offset: u32,
        byte_offset: u32,
        preceding_comments: TokenId,
    ) -> TokenId {
        let mut token = Token::fixed(ty, offset, byte_offset, false);
        token.preceding_comments = preceding_comments;
        self.push(token)
    }

    /// Dart `new SyntheticToken(type, offset)`, `new SyntheticBeginToken`,
    /// `new SyntheticKeywordToken(keyword, offset)`: a synthetic token of
    /// length 0 with the lexeme of its type.
    pub fn push_synthetic(&mut self, ty: TokenType, offset: u32, byte_offset: u32) -> TokenId {
        self.push(Token::fixed(ty, offset, byte_offset, true))
    }

    /// Dart `new SyntheticStringToken(type, value, offset, length)`. Without
    /// [length] the length is the length of [value] (in UTF-16 code units).
    pub fn push_synthetic_string(
        &mut self,
        ty: TokenType,
        value: &str,
        offset: u32,
        byte_offset: u32,
        length: Option<u32>,
    ) -> TokenId {
        let mut token = Token::fixed(ty, offset, byte_offset, true);
        token.length = length.unwrap_or_else(|| value.encode_utf16().count() as u32);
        if value.is_empty() {
            token.flags &= !flags::FIXED_LEXEME;
            token.lex_start = byte_offset;
            token.lex_end = byte_offset;
            self.push(token)
        } else {
            token.lex_end = byte_offset;
            self.push_with_lexeme(token, value)
        }
    }

    /// Dart `new StringToken(type, source.lexeme, source.charOffset)`: a
    /// non-synthetic token of type [ty] with the lexeme, offset and length of
    /// [source] (used to turn `new` into an identifier).
    pub fn push_string_like(&mut self, ty: TokenType, source: TokenId) -> TokenId {
        let s = &self.tokens[source.index()];
        let lexeme_is_fixed = s.flags & flags::FIXED_LEXEME != 0;
        let owned = s.flags & flags::OWNED_LEXEME != 0;
        let mut token = Token {
            ty,
            flags: 0,
            offset: s.offset,
            length: s.length,
            lex_start: s.lex_start,
            lex_end: s.lex_end,
            next: TokenId::NONE,
            previous: TokenId::NONE,
            preceding_comments: TokenId::NONE,
            end_group: TokenId::NONE,
            before_synthetic: TokenId::NONE,
        };
        if lexeme_is_fixed || owned {
            let lexeme = self.lexeme(source).to_string();
            token.lex_end = self.byte_offset(source);
            return self.push_with_lexeme(token, &lexeme);
        }
        self.push(token)
    }

    /// Dart `new ReplacementToken(type, replacedToken)`: a synthetic token
    /// with the lexeme of [ty] at the offset of [replaced].
    pub fn push_replacement(&mut self, ty: TokenType, replaced: TokenId) -> TokenId {
        let offset = self.tokens[replaced.index()].offset;
        let byte_offset = self.byte_offset(replaced);
        let mut token = Token::fixed(ty, offset, byte_offset, true);
        token.flags |= flags::REPLACEMENT;
        token.end_group = replaced;
        self.push(token)
    }

    /// Dart `ReplacementToken.replacedToken`.
    pub fn replaced_token(&self, id: TokenId) -> Option<TokenId> {
        let t = &self.tokens[id.index()];
        if t.flags & flags::REPLACEMENT != 0 {
            Some(t.end_group)
        } else {
            None
        }
    }

    /// Dart `Token.setNext`: links `a.next = b`, `b.previous = a`.
    pub fn set_next(&mut self, a: TokenId, b: TokenId) {
        self.tokens[a.index()].next = b;
        self.tokens[b.index()].previous = a;
        if self.tokens[b.index()].flags & flags::SYNTHETIC != 0 {
            self.tokens[b.index()].before_synthetic = a;
        }
    }

    /// Iterates the tokens from `first` following `next`, up to and including
    /// the EOF token.
    pub fn iter_from(&self, first: TokenId) -> TokenIter<'_> {
        TokenIter {
            tokens: self,
            current: first,
        }
    }

    /// Iterates the comments that precede `id` (Dart `precedingComments`
    /// and then `next` until `null`).
    pub fn comments(&self, id: TokenId) -> CommentIter<'_> {
        CommentIter {
            tokens: self,
            current: self.tokens[id.index()].preceding_comments,
        }
    }

    pub(crate) fn truncate(&mut self, tokens: usize, owned: usize, errors: usize) {
        self.tokens.truncate(tokens);
        self.owned_lexemes.truncate(owned);
        self.errors.truncate(errors);
    }
}

pub struct TokenIter<'a> {
    tokens: &'a Tokens,
    current: TokenId,
}

impl Iterator for TokenIter<'_> {
    type Item = TokenId;

    fn next(&mut self) -> Option<TokenId> {
        let id = self.current;
        if id.is_none() {
            return None;
        }
        let t = self.tokens.get(id);
        self.current = if t.ty == TokenType::EOF {
            TokenId::NONE
        } else {
            t.next
        };
        Some(id)
    }
}

pub struct CommentIter<'a> {
    tokens: &'a Tokens,
    current: TokenId,
}

impl Iterator for CommentIter<'_> {
    type Item = TokenId;

    fn next(&mut self) -> Option<TokenId> {
        let id = self.current.get()?;
        self.current = self.tokens.get(id).next;
        Some(id)
    }
}

impl TokenType {
    #[inline(always)]
    pub(crate) fn info(self) -> &'static TokenTypeInfo {
        &TOKEN_TYPE_INFO[self.0 as usize]
    }

    /// Dart `TokenType.name` (for keywords: the uppercase lexeme).
    #[inline(always)]
    pub fn name(self) -> &'static str {
        self.info().name
    }

    /// Dart `TokenType.lexeme`.
    #[inline(always)]
    pub fn lexeme(self) -> &'static str {
        self.info().lexeme
    }

    /// Dart `TokenType.kind`.
    #[inline(always)]
    pub fn kind(self) -> i32 {
        self.info().kind
    }

    /// Dart `TokenType.index`.
    #[inline(always)]
    pub fn index(self) -> u8 {
        self.0
    }

    #[inline(always)]
    pub fn precedence(self) -> u8 {
        self.info().precedence
    }

    #[inline(always)]
    pub fn is_keyword(self) -> bool {
        self.kind() == KEYWORD_TOKEN
    }

    pub fn keyword_style(self) -> Option<KeywordStyle> {
        self.info().keyword_style
    }

    pub fn is_built_in(self) -> bool {
        self.keyword_style() == Some(KeywordStyle::BuiltIn)
    }

    pub fn is_pseudo(self) -> bool {
        self.keyword_style() == Some(KeywordStyle::Pseudo)
    }

    pub fn is_reserved_word(self) -> bool {
        self.keyword_style() == Some(KeywordStyle::Reserved)
    }

    pub fn is_operator(self) -> bool {
        self.info().flags & F_OPERATOR != 0
    }

    pub fn is_binary_operator(self) -> bool {
        self.info().flags & F_BINARY_OPERATOR != 0
    }

    pub fn is_modifier(self) -> bool {
        self.info().flags & F_MODIFIER != 0
    }

    pub fn is_top_level_keyword(self) -> bool {
        self.info().flags & F_TOP_LEVEL_KEYWORD != 0
    }

    pub fn is_user_definable_operator(self) -> bool {
        self.info().flags & F_USER_DEFINABLE_OPERATOR != 0
    }

    /// Dart `TokenType.stringValue`.
    pub fn string_value(self) -> Option<&'static str> {
        if self.info().flags & F_STRING_VALUE_NULL != 0 {
            None
        } else {
            Some(self.lexeme())
        }
    }

    /// Dart `TokenType.binaryOperatorOfCompoundAssignment`.
    pub fn binary_operator_of_compound_assignment(self) -> Option<TokenType> {
        self.info().compound
    }

    pub fn is_assignment_operator(self) -> bool {
        self.precedence() == 1
    }
}

impl std::fmt::Debug for TokenType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}
