// Ported from package:yaml 3.1.4 lib/src/scanner.dart.
//
// Copyright (c) 2014, the Dart project authors.
// Copyright (c) 2006, Kirill Simonov.
// Use of this source code is governed by an MIT-style license.

use std::collections::VecDeque;

use crate::YamlException;
use crate::source::{FileSpan, SourceFile, SourceLocation};
use crate::style::ScalarStyle;
use crate::token::{Token, TokenType};

const TAB: u16 = 0x09;
const LF: u16 = 0x0a;
const CR: u16 = 0x0d;
const SP: u16 = 0x20;
const NEL: u16 = 0x85;
const BOM: u16 = 0xfeff;

#[derive(Clone, Copy)]
struct State {
    pos: usize,
    line: usize,
    column: usize,
}

struct Cursor {
    units: Vec<u16>,
    state: State,
}

impl Cursor {
    fn new(text: &str) -> Self {
        Self {
            units: text.encode_utf16().collect(),
            state: State {
                pos: 0,
                line: 0,
                column: 0,
            },
        }
    }
    fn done(&self) -> bool {
        self.state.pos >= self.units.len()
    }
    fn peek(&self, offset: isize) -> Option<u16> {
        self.state
            .pos
            .checked_add_signed(offset)
            .and_then(|i| self.units.get(i).copied())
    }
    fn read_char(&mut self) -> Option<u16> {
        let ch = self.peek(0)?;
        self.state.pos += 1;
        if ch == LF || (ch == CR && self.peek(0) != Some(LF)) {
            self.state.line += 1;
            self.state.column = 0;
        } else {
            self.state.column += 1;
        }
        Some(ch)
    }
    fn read_code_point(&mut self) -> Option<u32> {
        let first = self.read_char()?;
        if (0xd800..=0xdbff).contains(&first)
            && let Some(second @ 0xdc00..=0xdfff) = self.peek(0)
        {
            self.read_char();
            return Some(0x10000 + (((first - 0xd800) as u32) << 10) + (second - 0xdc00) as u32);
        }
        Some(first as u32)
    }
    fn matches(&self, text: &str) -> bool {
        let other: Vec<u16> = text.encode_utf16().collect();
        self.units.get(self.state.pos..self.state.pos + other.len()) == Some(other.as_slice())
    }
    fn slice(&self, start: usize, end: usize) -> String {
        String::from_utf16_lossy(&self.units[start..end])
    }
    fn location(&self) -> SourceLocation {
        SourceLocation {
            offset: self.state.pos,
            line: self.state.line,
            column: self.state.column,
        }
    }
}

#[derive(Clone)]
struct SimpleKey {
    token_number: usize,
    location: SourceLocation,
    line: usize,
    column: usize,
    required: bool,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Chomping {
    Strip,
    Clip,
    Keep,
}

pub struct Scanner {
    recover: bool,
    source: SourceFile,
    cursor: Cursor,
    stream_start_produced: bool,
    stream_end_produced: bool,
    tokens: VecDeque<Token>,
    tokens_parsed: usize,
    token_available: bool,
    indent_levels: Vec<(isize, Option<TokenType>)>,
    simple_key_allowed: bool,
    simple_keys: Vec<Option<SimpleKey>>,
    pub errors: Vec<YamlException>,
}

impl Scanner {
    pub fn new(text: &str, recover: bool) -> Self {
        Self {
            recover,
            source: SourceFile::new(text),
            cursor: Cursor::new(text),
            stream_start_produced: false,
            stream_end_produced: false,
            tokens: VecDeque::new(),
            tokens_parsed: 0,
            token_available: false,
            indent_levels: vec![(-1, None)],
            simple_key_allowed: true,
            simple_keys: vec![None],
            errors: Vec::new(),
        }
    }

    pub fn take_errors(&mut self) -> Vec<YamlException> {
        std::mem::take(&mut self.errors)
    }

    pub fn peek(&mut self) -> Result<Token, YamlException> {
        if self.stream_end_produced {
            return Err(self.error("Out of tokens.", self.empty_span()));
        }
        if !self.token_available {
            self.fetch_more_tokens()?;
        }
        Ok(self
            .tokens
            .front()
            .expect("scanner token queue cannot be empty")
            .clone())
    }

    pub fn scan(&mut self) -> Result<Token, YamlException> {
        if self.stream_end_produced {
            return Err(self.error("Out of tokens.", self.empty_span()));
        }
        if !self.token_available {
            self.fetch_more_tokens()?;
        }
        let token = self
            .tokens
            .pop_front()
            .expect("scanner token queue cannot be empty");
        self.token_available = false;
        self.tokens_parsed += 1;
        self.stream_end_produced = token.kind == TokenType::StreamEnd;
        Ok(token)
    }

    pub fn advance(&mut self) -> Result<Token, YamlException> {
        self.scan()?;
        self.peek()
    }

    fn span(&self, start: usize, end: usize) -> FileSpan {
        self.source.span(start, end)
    }
    fn empty_span(&self) -> FileSpan {
        self.span(self.cursor.state.pos, self.cursor.state.pos)
    }
    fn state_span(&self, start: State) -> FileSpan {
        self.span(start.pos, self.cursor.state.pos)
    }
    fn error(&self, message: &str, span: FileSpan) -> YamlException {
        YamlException {
            message: message.into(),
            span,
        }
    }
    fn indent(&self) -> isize {
        self.indent_levels.last().unwrap().0
    }
    fn in_block_context(&self) -> bool {
        self.simple_keys.len() == 1
    }
    fn is_break_at(&self, offset: isize) -> bool {
        matches!(self.cursor.peek(offset), Some(CR | LF))
    }
    fn is_break(&self) -> bool {
        self.is_break_at(0)
    }
    fn is_break_or_end(&self) -> bool {
        self.cursor.done() || self.is_break()
    }
    fn is_blank_at(&self, offset: isize) -> bool {
        matches!(self.cursor.peek(offset), Some(SP | TAB))
    }
    fn is_blank(&self) -> bool {
        self.is_blank_at(0)
    }
    fn is_blank_or_end_at(&self, offset: isize) -> bool {
        matches!(self.cursor.peek(offset), None | Some(SP | TAB | CR | LF))
    }
    fn is_blank_or_end(&self) -> bool {
        self.is_blank_or_end_at(0)
    }
    fn is_digit(&self) -> bool {
        matches!(self.cursor.peek(0), Some(0x30..=0x39))
    }
    fn is_hex(&self) -> bool {
        matches!(
            self.cursor.peek(0),
            Some(0x30..=0x39 | 0x61..=0x66 | 0x41..=0x46)
        )
    }
    fn is_non_break(&self) -> bool {
        match self.cursor.peek(0) {
            None | Some(LF | CR | BOM) => false,
            Some(TAB | NEL) => true,
            _ => self.is_standard_character_at(0),
        }
    }
    fn is_non_space(&self) -> bool {
        match self.cursor.peek(0) {
            None | Some(LF | CR | BOM | SP) => false,
            Some(NEL) => true,
            _ => self.is_standard_character_at(0),
        }
    }
    fn is_document_indicator(&self) -> bool {
        self.cursor.state.column == 0
            && self.is_blank_or_end_at(3)
            && (self.cursor.matches("---") || self.cursor.matches("..."))
    }

    fn fetch_more_tokens(&mut self) -> Result<(), YamlException> {
        loop {
            if !self.tokens.is_empty() {
                self.stale_simple_keys()?;
                if self.tokens.back().unwrap().kind == TokenType::StreamEnd {
                    break;
                }
                if !self
                    .simple_keys
                    .iter()
                    .flatten()
                    .any(|key| key.token_number == self.tokens_parsed)
                {
                    break;
                }
            }
            self.fetch_next_token()?;
        }
        self.token_available = true;
        Ok(())
    }

    fn fetch_next_token(&mut self) -> Result<(), YamlException> {
        if !self.stream_start_produced {
            self.fetch_stream_start();
            return Ok(());
        }
        self.scan_to_next_token()?;
        self.stale_simple_keys()?;
        self.unroll_indent(self.cursor.state.column as isize);
        if self.cursor.done() {
            return self.fetch_stream_end();
        }
        if self.cursor.state.column == 0 {
            if self.cursor.peek(0) == Some(b'%' as u16) {
                return self.fetch_directive();
            }
            if self.is_blank_or_end_at(3) {
                if self.cursor.matches("---") {
                    return self.fetch_document_indicator(TokenType::DocumentStart);
                }
                if self.cursor.matches("...") {
                    return self.fetch_document_indicator(TokenType::DocumentEnd);
                }
            }
        }
        match self.cursor.peek(0).unwrap() {
            0x5b => self.fetch_flow_collection_start(TokenType::FlowSequenceStart),
            0x7b => self.fetch_flow_collection_start(TokenType::FlowMappingStart),
            0x5d => self.fetch_flow_collection_end(TokenType::FlowSequenceEnd),
            0x7d => self.fetch_flow_collection_end(TokenType::FlowMappingEnd),
            0x2c => self.fetch_flow_entry(),
            0x2a => self.fetch_anchor(false),
            0x26 => self.fetch_anchor(true),
            0x21 => self.fetch_tag(),
            0x27 => self.fetch_flow_scalar(true),
            0x22 => self.fetch_flow_scalar(false),
            0x7c => {
                if !self.in_block_context() {
                    return Err(self.error(
                        "Unexpected character.",
                        self.span(self.cursor.state.pos, self.cursor.state.pos + 1),
                    ));
                }
                self.fetch_block_scalar(true)
            }
            0x3e => {
                if !self.in_block_context() {
                    return Err(self.error(
                        "Unexpected character.",
                        self.span(self.cursor.state.pos, self.cursor.state.pos + 1),
                    ));
                }
                self.fetch_block_scalar(false)
            }
            0x25 | 0x40 | 0x60 => Err(self.error(
                "Unexpected character.",
                self.span(self.cursor.state.pos, self.cursor.state.pos + 1),
            )),
            0x2d => {
                if self.is_plain_safe_at(1) {
                    self.fetch_plain_scalar()
                } else {
                    self.fetch_block_entry()
                }
            }
            0x3f => {
                if self.is_plain_safe_at(1) {
                    self.fetch_plain_scalar()
                } else {
                    self.fetch_key()
                }
            }
            0x3a => {
                if !self.in_block_context()
                    && self.tokens.back().is_some_and(|t| {
                        matches!(
                            t.kind,
                            TokenType::FlowSequenceEnd | TokenType::FlowMappingEnd
                        ) || (t.kind == TokenType::Scalar && t.style.is_quoted())
                    })
                {
                    self.fetch_value()
                } else if self.is_plain_safe_at(1) {
                    self.fetch_plain_scalar()
                } else {
                    self.fetch_value()
                }
            }
            _ => {
                if !self.is_non_break() {
                    return Err(self.error(
                        "Unexpected character.",
                        self.span(self.cursor.state.pos, self.cursor.state.pos + 1),
                    ));
                }
                self.fetch_plain_scalar()
            }
        }
    }

    fn stale_simple_keys(&mut self) -> Result<(), YamlException> {
        for i in 0..self.simple_keys.len() {
            let Some(key) = self.simple_keys[i].clone() else {
                continue;
            };
            if !self.in_block_context() || key.line == self.cursor.state.line {
                continue;
            }
            if key.required {
                let in_sequence = self
                    .indent_levels
                    .iter()
                    .rfind(|x| x.0 == key.column as isize)
                    .is_some_and(|x| x.1 == Some(TokenType::BlockSequenceStart));
                let mut message = "Expected ':'.".to_string();
                if in_sequence {
                    message.push_str(" If this is a list entry, it must start with '- '.");
                }
                let exception = self.error(&message, self.empty_span());
                self.report_error(exception)?;
                self.tokens.insert(
                    key.token_number - self.tokens_parsed,
                    Token::new(
                        TokenType::Key,
                        self.source.span(key.location.offset, key.location.offset),
                    ),
                );
            }
            self.simple_keys[i] = None;
        }
        Ok(())
    }

    fn report_error(&mut self, exception: YamlException) -> Result<(), YamlException> {
        if self.recover {
            self.errors.push(exception);
            Ok(())
        } else {
            Err(exception)
        }
    }

    fn save_simple_key(&mut self) -> Result<(), YamlException> {
        let required =
            self.in_block_context() && self.indent() == self.cursor.state.column as isize;
        if !self.simple_key_allowed {
            return Ok(());
        }
        self.remove_simple_key()?;
        let location = self.cursor.location();
        *self.simple_keys.last_mut().unwrap() = Some(SimpleKey {
            token_number: self.tokens_parsed + self.tokens.len(),
            location,
            line: location.line,
            column: location.column,
            required,
        });
        Ok(())
    }

    fn remove_simple_key(&mut self) -> Result<(), YamlException> {
        if let Some(key) = self.simple_keys.last().unwrap()
            && key.required
        {
            return Err(self.error(
                "Could not find expected ':' for simple key.",
                self.source.span(key.location.offset, key.location.offset),
            ));
        }
        *self.simple_keys.last_mut().unwrap() = None;
        Ok(())
    }

    fn roll_indent(
        &mut self,
        column: isize,
        kind: TokenType,
        location: SourceLocation,
        token_number: Option<usize>,
    ) {
        if !self.in_block_context() || (self.indent() != -1 && self.indent() >= column) {
            return;
        }
        self.indent_levels.push((column, Some(kind)));
        let token = Token::new(kind, self.source.span(location.offset, location.offset));
        if let Some(number) = token_number {
            self.tokens.insert(number - self.tokens_parsed, token);
        } else {
            self.tokens.push_back(token);
        }
    }
    fn unroll_indent(&mut self, column: isize) {
        if !self.in_block_context() {
            return;
        }
        while self.indent() > column {
            self.tokens
                .push_back(Token::new(TokenType::BlockEnd, self.empty_span()));
            self.indent_levels.pop();
        }
    }
    fn fetch_stream_start(&mut self) {
        self.stream_start_produced = true;
        self.tokens
            .push_back(Token::new(TokenType::StreamStart, self.empty_span()));
    }
    fn fetch_stream_end(&mut self) -> Result<(), YamlException> {
        self.unroll_indent(-1);
        self.remove_simple_key()?;
        self.simple_key_allowed = false;
        self.tokens
            .push_back(Token::new(TokenType::StreamEnd, self.empty_span()));
        Ok(())
    }
    fn add_char_token(&mut self, kind: TokenType) {
        let start = self.cursor.state;
        self.cursor.read_code_point();
        self.tokens
            .push_back(Token::new(kind, self.state_span(start)));
    }
    fn fetch_document_indicator(&mut self, kind: TokenType) -> Result<(), YamlException> {
        self.unroll_indent(-1);
        self.remove_simple_key()?;
        self.simple_key_allowed = false;
        let start = self.cursor.state;
        for _ in 0..3 {
            self.cursor.read_code_point();
        }
        self.tokens
            .push_back(Token::new(kind, self.state_span(start)));
        Ok(())
    }
    fn fetch_flow_collection_start(&mut self, kind: TokenType) -> Result<(), YamlException> {
        self.save_simple_key()?;
        self.simple_keys.push(None);
        self.simple_key_allowed = true;
        self.add_char_token(kind);
        Ok(())
    }
    fn fetch_flow_collection_end(&mut self, kind: TokenType) -> Result<(), YamlException> {
        self.remove_simple_key()?;
        if !self.in_block_context() {
            self.simple_keys.pop();
        }
        self.simple_key_allowed = false;
        self.add_char_token(kind);
        Ok(())
    }
    fn fetch_flow_entry(&mut self) -> Result<(), YamlException> {
        self.remove_simple_key()?;
        self.simple_key_allowed = true;
        self.add_char_token(TokenType::FlowEntry);
        Ok(())
    }
    fn fetch_block_entry(&mut self) -> Result<(), YamlException> {
        if self.in_block_context() {
            if !self.simple_key_allowed {
                return Err(self.error(
                    "Block sequence entries are not allowed here.",
                    self.empty_span(),
                ));
            }
            self.roll_indent(
                self.cursor.state.column as isize,
                TokenType::BlockSequenceStart,
                self.cursor.location(),
                None,
            );
        }
        self.remove_simple_key()?;
        self.simple_key_allowed = true;
        self.add_char_token(TokenType::BlockEntry);
        Ok(())
    }
    fn fetch_key(&mut self) -> Result<(), YamlException> {
        if self.in_block_context() {
            if !self.simple_key_allowed {
                return Err(self.error("Mapping keys are not allowed here.", self.empty_span()));
            }
            self.roll_indent(
                self.cursor.state.column as isize,
                TokenType::BlockMappingStart,
                self.cursor.location(),
                None,
            );
        }
        self.simple_key_allowed = self.in_block_context();
        self.add_char_token(TokenType::Key);
        Ok(())
    }
    fn fetch_value(&mut self) -> Result<(), YamlException> {
        if let Some(key) = self.simple_keys.last().unwrap().clone() {
            self.tokens.insert(
                key.token_number - self.tokens_parsed,
                Token::new(
                    TokenType::Key,
                    self.source.span(key.location.offset, key.location.offset),
                ),
            );
            self.roll_indent(
                key.column as isize,
                TokenType::BlockMappingStart,
                key.location,
                Some(key.token_number),
            );
            *self.simple_keys.last_mut().unwrap() = None;
            self.simple_key_allowed = false;
        } else if self.in_block_context() {
            if !self.simple_key_allowed {
                return Err(self.error(
                    "Mapping values are not allowed here. Did you miss a colon earlier?",
                    self.empty_span(),
                ));
            }
            self.roll_indent(
                self.cursor.state.column as isize,
                TokenType::BlockMappingStart,
                self.cursor.location(),
                None,
            );
            self.simple_key_allowed = true;
        } else if self.simple_key_allowed {
            self.simple_key_allowed = false;
            self.add_char_token(TokenType::Key);
        }
        self.add_char_token(TokenType::Value);
        Ok(())
    }
    fn fetch_anchor(&mut self, anchor: bool) -> Result<(), YamlException> {
        self.save_simple_key()?;
        self.simple_key_allowed = false;
        let token = self.scan_anchor(anchor)?;
        self.tokens.push_back(token);
        Ok(())
    }
    fn fetch_tag(&mut self) -> Result<(), YamlException> {
        self.save_simple_key()?;
        self.simple_key_allowed = false;
        let token = self.scan_tag()?;
        self.tokens.push_back(token);
        Ok(())
    }
    fn fetch_block_scalar(&mut self, literal: bool) -> Result<(), YamlException> {
        self.remove_simple_key()?;
        self.simple_key_allowed = true;
        let token = self.scan_block_scalar(literal)?;
        self.tokens.push_back(token);
        Ok(())
    }
    fn fetch_flow_scalar(&mut self, single: bool) -> Result<(), YamlException> {
        self.save_simple_key()?;
        self.simple_key_allowed = false;
        let token = self.scan_flow_scalar(single)?;
        self.tokens.push_back(token);
        Ok(())
    }
    fn fetch_plain_scalar(&mut self) -> Result<(), YamlException> {
        self.save_simple_key()?;
        self.simple_key_allowed = false;
        let token = self.scan_plain_scalar()?;
        self.tokens.push_back(token);
        Ok(())
    }

    fn scan_to_next_token(&mut self) -> Result<(), YamlException> {
        let mut after_line_break = false;
        loop {
            if self.cursor.state.column == 0 && self.cursor.peek(0) == Some(BOM) {
                self.cursor.read_char();
            }
            while self.cursor.peek(0) == Some(SP)
                || ((!self.in_block_context() || !after_line_break)
                    && self.cursor.peek(0) == Some(TAB))
            {
                self.cursor.read_char();
            }
            if self.cursor.peek(0) == Some(TAB) {
                return Err(self.error(
                    "Tab characters are not allowed as indentation.",
                    self.span(self.cursor.state.pos, self.cursor.state.pos + 1),
                ));
            }
            self.skip_comment();
            if self.is_break() {
                self.skip_line();
                if self.in_block_context() {
                    self.simple_key_allowed = true;
                }
                after_line_break = true;
            } else {
                break;
            }
        }
        Ok(())
    }

    fn fetch_directive(&mut self) -> Result<(), YamlException> {
        self.unroll_indent(-1);
        self.remove_simple_key()?;
        self.simple_key_allowed = false;
        if let Some(token) = self.scan_directive()? {
            self.tokens.push_back(token);
        }
        Ok(())
    }

    fn scan_directive(&mut self) -> Result<Option<Token>, YamlException> {
        let start = self.cursor.state;
        self.cursor.read_char();
        let name = self.scan_directive_name()?;
        let token = if name == "YAML" {
            Some(self.scan_version_directive_value(start)?)
        } else if name == "TAG" {
            Some(self.scan_tag_directive_value(start)?)
        } else {
            while !self.is_break_or_end() {
                self.cursor.read_code_point();
            }
            None
        };
        self.skip_blanks();
        self.skip_comment();
        if !self.is_break_or_end() {
            return Err(self.error(
                "Expected comment or line break after directive.",
                self.state_span(start),
            ));
        }
        self.skip_line();
        Ok(token)
    }

    fn scan_directive_name(&mut self) -> Result<String, YamlException> {
        let start = self.cursor.state.pos;
        while self.is_non_space() {
            self.cursor.read_code_point();
        }
        let name = self.cursor.slice(start, self.cursor.state.pos);
        if name.is_empty() {
            Err(self.error("Expected directive name.", self.empty_span()))
        } else if !self.is_blank_or_end() {
            Err(self.error("Unexpected character in directive name.", self.empty_span()))
        } else {
            Ok(name)
        }
    }

    fn scan_version_directive_value(&mut self, start: State) -> Result<Token, YamlException> {
        self.skip_blanks();
        let major = self.scan_version_directive_number()?;
        self.expect_char(b'.' as u16)?;
        let minor = self.scan_version_directive_number()?;
        let mut token = Token::new(TokenType::VersionDirective, self.state_span(start));
        token.major = major;
        token.minor = minor;
        Ok(token)
    }
    fn scan_version_directive_number(&mut self) -> Result<i64, YamlException> {
        let start = self.cursor.state.pos;
        while self.is_digit() {
            self.cursor.read_char();
        }
        let number = self.cursor.slice(start, self.cursor.state.pos);
        if number.is_empty() {
            return Err(self.error("Expected version number.", self.empty_span()));
        }
        number.parse().map_err(|_| {
            self.error(
                "Invalid version number.",
                self.span(start, self.cursor.state.pos),
            )
        })
    }
    fn scan_tag_directive_value(&mut self, start: State) -> Result<Token, YamlException> {
        self.skip_blanks();
        let handle = self.scan_tag_handle(true)?;
        if !self.is_blank() {
            return Err(self.error("Expected whitespace.", self.empty_span()));
        }
        self.skip_blanks();
        let prefix = self.scan_tag_uri(None, true)?;
        if !self.is_blank_or_end() {
            return Err(self.error("Expected whitespace.", self.empty_span()));
        }
        let mut token = Token::new(TokenType::TagDirective, self.state_span(start));
        token.handle = Some(handle);
        token.suffix = prefix;
        Ok(token)
    }

    fn is_anchor_char(&self) -> bool {
        self.is_non_space()
            && !matches!(self.cursor.peek(0), Some(0x2c | 0x5b | 0x5d | 0x7b | 0x7d))
    }
    fn scan_anchor(&mut self, anchor: bool) -> Result<Token, YamlException> {
        let start = self.cursor.state;
        self.cursor.read_code_point();
        let name_start = self.cursor.state.pos;
        while self.is_anchor_char() {
            self.cursor.read_code_point();
        }
        let name = self.cursor.slice(name_start, self.cursor.state.pos);
        let next = self.cursor.peek(0);
        if name.is_empty()
            || (!self.is_blank_or_end()
                && !matches!(
                    next,
                    Some(0x3f | 0x3a | 0x2c | 0x5d | 0x7d | 0x25 | 0x40 | 0x60)
                ))
        {
            return Err(self.error("Expected alphanumeric character.", self.empty_span()));
        }
        let mut token = Token::new(
            if anchor {
                TokenType::Anchor
            } else {
                TokenType::Alias
            },
            self.state_span(start),
        );
        token.value = name;
        Ok(token)
    }

    fn scan_tag(&mut self) -> Result<Token, YamlException> {
        let start = self.cursor.state;
        let (handle, suffix);
        if self.cursor.peek(1) == Some(b'<' as u16) {
            self.cursor.read_char();
            self.cursor.read_char();
            handle = Some(String::new());
            suffix = self.scan_tag_uri(None, true)?;
            self.expect_char(b'>' as u16)?;
        } else {
            let scanned = self.scan_tag_handle(false)?;
            if scanned.len() > 1 && scanned.starts_with('!') && scanned.ends_with('!') {
                handle = Some(scanned);
                suffix = self.scan_tag_uri(None, false)?;
            } else {
                let decoded = self.scan_tag_uri(Some(&scanned), false)?;
                if decoded.is_empty() {
                    handle = None;
                    suffix = "!".into();
                } else {
                    handle = Some("!".into());
                    suffix = decoded;
                }
            }
        }
        let mut token = Token::new(TokenType::Tag, self.state_span(start));
        token.handle = handle;
        token.suffix = suffix;
        Ok(token)
    }
    fn is_tag_char(&self) -> bool {
        matches!(
            self.cursor.peek(0),
            Some(
                0x2d
                | 0x3b
                | 0x2f
                | 0x3a
                | 0x40
                | 0x26
                | 0x3d
                | 0x2b
                | 0x24
                | 0x2e
                | 0x7e
                | 0x3f
                | 0x2a
                | 0x27
                | 0x28
                | 0x29
                | 0x25
                | 0x30..=0x39
                | 0x61..=0x7a
                | 0x41..=0x5a,
            )
        )
    }
    fn scan_tag_handle(&mut self, directive: bool) -> Result<String, YamlException> {
        self.expect_char(b'!' as u16)?;
        let mut out = "!".to_string();
        let start = self.cursor.state.pos;
        while self.is_tag_char() {
            self.cursor.read_char();
        }
        out.push_str(&self.cursor.slice(start, self.cursor.state.pos));
        if self.cursor.peek(0) == Some(b'!' as u16) {
            self.cursor.read_code_point();
            out.push('!');
        } else if directive && out != "!" {
            self.expect_char(b'!' as u16)?;
        }
        Ok(out)
    }
    fn scan_tag_uri(
        &mut self,
        head: Option<&str>,
        flow_separators: bool,
    ) -> Result<String, YamlException> {
        let mut raw = String::new();
        if let Some(head) = head
            && head.encode_utf16().count() > 1
        {
            raw.push_str(&head[1..]);
        }
        let start = self.cursor.state.pos;
        loop {
            let ch = self.cursor.peek(0);
            if self.is_tag_char() || (flow_separators && matches!(ch, Some(0x2c | 0x5b | 0x5d))) {
                self.cursor.read_char();
            } else {
                break;
            }
        }
        raw.push_str(&self.cursor.slice(start, self.cursor.state.pos));
        decode_percent(&raw).map_err(|_| {
            self.error(
                "Invalid percent-encoding in tag URI.",
                self.span(start, self.cursor.state.pos),
            )
        })
    }

    fn expect_char(&mut self, expected: u16) -> Result<(), YamlException> {
        if self.cursor.peek(0) == Some(expected) {
            self.cursor.read_char();
            Ok(())
        } else {
            Err(self.error(
                &format!("expected \"{}\".", char::from_u32(expected as u32).unwrap()),
                self.empty_span(),
            ))
        }
    }

    fn scan_block_scalar(&mut self, literal: bool) -> Result<Token, YamlException> {
        let start = self.cursor.state;
        self.cursor.read_code_point();
        let mut chomping = Chomping::Clip;
        let mut increment = 0usize;
        let mut ch = self.cursor.peek(0);
        if matches!(ch, Some(0x2b | 0x2d)) {
            chomping = if ch == Some(0x2b) {
                Chomping::Keep
            } else {
                Chomping::Strip
            };
            self.cursor.read_code_point();
            if self.is_digit() {
                if self.cursor.peek(0) == Some(0x30) {
                    return Err(self.error(
                        "0 may not be used as an indentation indicator.",
                        self.state_span(start),
                    ));
                }
                increment = (self.cursor.read_code_point().unwrap() - 0x30) as usize;
            }
        } else if self.is_digit() {
            if self.cursor.peek(0) == Some(0x30) {
                return Err(self.error(
                    "0 may not be used as an indentation indicator.",
                    self.state_span(start),
                ));
            }
            increment = (self.cursor.read_code_point().unwrap() - 0x30) as usize;
            ch = self.cursor.peek(0);
            if matches!(ch, Some(0x2b | 0x2d)) {
                chomping = if ch == Some(0x2b) {
                    Chomping::Keep
                } else {
                    Chomping::Strip
                };
                self.cursor.read_code_point();
            }
        }
        self.skip_blanks();
        self.skip_comment();
        if !self.is_break_or_end() {
            return Err(self.error("Expected comment or line break.", self.empty_span()));
        }
        self.skip_line();
        let mut indent = if increment != 0 {
            if self.indent() >= 0 {
                self.indent() as usize + increment
            } else {
                increment
            }
        } else {
            0
        };
        let pair = self.scan_block_scalar_breaks(indent)?;
        indent = pair.0;
        let mut trailing_breaks = pair.1;
        let mut out = String::new();
        let mut leading_break = String::new();
        let mut leading_blank = false;
        let mut end = self.cursor.state;
        while self.cursor.state.column == indent && !self.cursor.done() {
            if self.is_document_indicator() {
                break;
            }
            let trailing_blank = self.is_blank();
            if !literal && !leading_break.is_empty() && !leading_blank && !trailing_blank {
                if trailing_breaks.is_empty() {
                    out.push(' ');
                }
            } else {
                out.push_str(&leading_break);
            }
            leading_break.clear();
            out.push_str(&trailing_breaks);
            leading_blank = self.is_blank();
            let p = self.cursor.state.pos;
            while !self.is_break_or_end() {
                self.cursor.read_code_point();
            }
            out.push_str(&self.cursor.slice(p, self.cursor.state.pos));
            end = self.cursor.state;
            if !self.cursor.done() {
                leading_break = self.read_line()?;
            } else {
                leading_break = "\n".into();
            }
            let pair = self.scan_block_scalar_breaks(indent)?;
            indent = pair.0;
            trailing_breaks = pair.1;
        }
        if chomping != Chomping::Strip {
            out.push_str(&leading_break);
        }
        if chomping == Chomping::Keep {
            out.push_str(&trailing_breaks);
        }
        let mut token = Token::new(TokenType::Scalar, self.span(start.pos, end.pos));
        token.value = out;
        token.style = if literal {
            ScalarStyle::Literal
        } else {
            ScalarStyle::Folded
        };
        Ok(token)
    }
    fn scan_block_scalar_breaks(
        &mut self,
        mut indent: usize,
    ) -> Result<(usize, String), YamlException> {
        let mut max_indent = 0;
        let mut breaks = String::new();
        loop {
            while (indent == 0 || self.cursor.state.column < indent)
                && self.cursor.peek(0) == Some(SP)
            {
                self.cursor.read_char();
            }
            max_indent = max_indent.max(self.cursor.state.column);
            if !self.is_break() {
                break;
            }
            breaks.push_str(&self.read_line()?);
        }
        if indent == 0 {
            indent = max_indent.max((self.indent() + 1).max(0) as usize);
        }
        Ok((indent, breaks))
    }

    fn scan_flow_scalar(&mut self, single: bool) -> Result<Token, YamlException> {
        let start = self.cursor.state;
        let mut out = String::new();
        self.cursor.read_char();
        loop {
            if self.is_document_indicator() {
                return Err(self.error(
                    "Unexpected document indicator.",
                    self.span(self.cursor.state.pos, self.cursor.state.pos + 3),
                ));
            }
            if self.cursor.done() {
                return Err(self.error("Unexpected end of file.", self.empty_span()));
            }
            let mut leading_blanks = false;
            while !self.is_blank_or_end() {
                let ch = self.cursor.peek(0).unwrap();
                if single && ch == 0x27 && self.cursor.peek(1) == Some(0x27) {
                    self.cursor.read_char();
                    self.cursor.read_char();
                    out.push('\'');
                } else if ch == if single { 0x27 } else { 0x22 } {
                    break;
                } else if !single && ch == 0x5c && self.is_break_at(1) {
                    self.cursor.read_char();
                    self.skip_line();
                    leading_blanks = true;
                    break;
                } else if !single && ch == 0x5c {
                    let escape = self.cursor.state;
                    let next = self.cursor.peek(1);
                    let mut code_length = None;
                    match next {
                        Some(0x30) => out.push('\0'),
                        Some(0x61) => out.push('\x07'),
                        Some(0x62) => out.push('\x08'),
                        Some(0x74 | TAB) => out.push('\t'),
                        Some(0x6e) => out.push('\n'),
                        Some(0x76) => out.push('\x0b'),
                        Some(0x66) => out.push('\x0c'),
                        Some(0x72) => out.push('\r'),
                        Some(0x65) => out.push('\x1b'),
                        Some(SP | 0x22 | 0x2f | 0x5c) => {
                            out.push(char::from_u32(next.unwrap() as u32).unwrap())
                        }
                        Some(0x4e) => out.push('\u{85}'),
                        Some(0x5f) => out.push('\u{a0}'),
                        Some(0x4c) => out.push('\u{2028}'),
                        Some(0x50) => out.push('\u{2029}'),
                        Some(0x78) => code_length = Some(2),
                        Some(0x75) => code_length = Some(4),
                        Some(0x55) => code_length = Some(8),
                        _ => {
                            return Err(
                                self.error("Unknown escape character.", self.state_span(escape))
                            );
                        }
                    }
                    self.cursor.read_char();
                    self.cursor.read_char();
                    if let Some(length) = code_length {
                        let mut value = 0u32;
                        for _ in 0..length {
                            if !self.is_hex() {
                                if self.cursor.done() {
                                    return Err(self.error(
                                        "expected more input.",
                                        self.empty_span(),
                                    ));
                                }
                                self.cursor.read_char();
                                return Err(self.error(
                                    &format!("Expected {length}-digit hexidecimal number."),
                                    self.state_span(escape),
                                ));
                            }
                            let digit = self.cursor.read_char().unwrap();
                            value = (value << 4) + self.as_hex(digit) as u32;
                        }
                        if (0xd800..=0xdfff).contains(&value) || value > 0x10ffff {
                            return Err(self.error(
                                "Invalid Unicode character escape code.",
                                self.state_span(escape),
                            ));
                        }
                        out.push(char::from_u32(value).unwrap());
                    }
                } else {
                    out.push(
                        char::from_u32(self.cursor.read_code_point().unwrap())
                            .unwrap_or(char::REPLACEMENT_CHARACTER),
                    );
                }
            }
            if self.cursor.peek(0) == Some(if single { 0x27 } else { 0x22 }) {
                break;
            }
            let mut whitespace = String::new();
            let mut leading_break = String::new();
            let mut trailing_breaks = String::new();
            while self.is_blank() || self.is_break() {
                if self.is_blank() {
                    if !leading_blanks {
                        whitespace
                            .push(char::from_u32(self.cursor.read_char().unwrap() as u32).unwrap());
                    } else {
                        self.cursor.read_char();
                    }
                } else if !leading_blanks {
                    whitespace.clear();
                    leading_break = self.read_line()?;
                    leading_blanks = true;
                } else {
                    trailing_breaks.push_str(&self.read_line()?);
                }
            }
            if leading_blanks {
                if !leading_break.is_empty() && trailing_breaks.is_empty() {
                    out.push(' ');
                } else {
                    out.push_str(&trailing_breaks);
                }
            } else {
                out.push_str(&whitespace);
            }
        }
        self.cursor.read_char();
        let mut token = Token::new(TokenType::Scalar, self.state_span(start));
        token.value = out;
        token.style = if single {
            ScalarStyle::SingleQuoted
        } else {
            ScalarStyle::DoubleQuoted
        };
        Ok(token)
    }

    fn scan_plain_scalar(&mut self) -> Result<Token, YamlException> {
        let start = self.cursor.state;
        let mut end = start;
        let mut out = String::new();
        let mut leading_break = String::new();
        let mut trailing_breaks = String::new();
        let mut whitespace = String::new();
        let indent = self.indent() + 1;
        loop {
            if self.is_document_indicator() || self.cursor.peek(0) == Some(0x23) {
                break;
            }
            if self.is_plain_char_at(0) {
                if !leading_break.is_empty() {
                    if trailing_breaks.is_empty() {
                        out.push(' ');
                    } else {
                        out.push_str(&trailing_breaks);
                    }
                    leading_break.clear();
                    trailing_breaks.clear();
                } else {
                    out.push_str(&whitespace);
                    whitespace.clear();
                }
            }
            let p = self.cursor.state.pos;
            while self.is_plain_char_at(0) {
                self.cursor.read_code_point();
            }
            out.push_str(&self.cursor.slice(p, self.cursor.state.pos));
            end = self.cursor.state;
            if !self.is_blank() && !self.is_break() {
                break;
            }
            while self.is_blank() || self.is_break() {
                if self.is_blank() {
                    if !leading_break.is_empty()
                        && self.cursor.state.column < indent.max(0) as usize
                        && self.cursor.peek(0) == Some(TAB)
                    {
                        return Err(self.error(
                            "Expected a space but found a tab.",
                            self.span(self.cursor.state.pos, self.cursor.state.pos + 1),
                        ));
                    }
                    if leading_break.is_empty() {
                        whitespace
                            .push(char::from_u32(self.cursor.read_char().unwrap() as u32).unwrap());
                    } else {
                        self.cursor.read_char();
                    }
                } else if leading_break.is_empty() {
                    leading_break = self.read_line()?;
                    whitespace.clear();
                } else {
                    trailing_breaks = self.read_line()?;
                }
            }
            if self.in_block_context() && self.cursor.state.column < indent.max(0) as usize {
                break;
            }
        }
        if !leading_break.is_empty() {
            self.simple_key_allowed = true;
        }
        let mut token = Token::new(TokenType::Scalar, self.span(start.pos, end.pos));
        token.value = out;
        token.style = ScalarStyle::Plain;
        Ok(token)
    }

    fn skip_line(&mut self) {
        let ch = self.cursor.peek(0);
        if !matches!(ch, Some(CR | LF)) {
            return;
        }
        self.cursor.read_char();
        if ch == Some(CR) && self.cursor.peek(0) == Some(LF) {
            self.cursor.read_char();
        }
    }
    fn read_line(&mut self) -> Result<String, YamlException> {
        let ch = self.cursor.peek(0);
        if !matches!(ch, Some(CR | LF)) {
            return Err(self.error("Expected newline.", self.empty_span()));
        }
        self.cursor.read_char();
        if ch == Some(CR) && self.cursor.peek(0) == Some(LF) {
            self.cursor.read_char();
        }
        Ok("\n".into())
    }
    fn is_plain_char_at(&self, offset: isize) -> bool {
        match self.cursor.peek(offset) {
            Some(0x3a) => self.is_plain_safe_at(offset + 1),
            Some(0x23) => !matches!(self.cursor.peek(offset - 1), Some(SP | TAB)),
            _ => self.is_plain_safe_at(offset),
        }
    }
    fn is_plain_safe_at(&self, offset: isize) -> bool {
        match self.cursor.peek(offset) {
            None => false,
            Some(0x2c | 0x5b | 0x5d | 0x7b | 0x7d) => self.in_block_context(),
            Some(SP | TAB | LF | CR | BOM) => false,
            Some(NEL) => true,
            _ => self.is_standard_character_at(offset),
        }
    }
    fn is_standard_character_at(&self, offset: isize) -> bool {
        let Some(first) = self.cursor.peek(offset) else {
            return false;
        };
        if (0xd800..=0xdbff).contains(&first) {
            return self
                .cursor
                .peek(offset + 1)
                .is_some_and(|x| (0xdc00..=0xdfff).contains(&x));
        }
        matches!(first,0x20..=0x7e|0xa0..=0xd7ff|0xe000..=0xfffd)
    }
    fn as_hex(&self, ch: u16) -> u16 {
        if ch <= 0x39 {
            ch - 0x30
        } else if ch <= 0x46 {
            10 + ch - 0x41
        } else {
            10 + ch - 0x61
        }
    }
    fn skip_blanks(&mut self) {
        while self.is_blank() {
            self.cursor.read_char();
        }
    }
    fn skip_comment(&mut self) {
        if self.cursor.peek(0) != Some(0x23) {
            return;
        }
        while !self.is_break_or_end() {
            self.cursor.read_char();
        }
    }
}

fn decode_percent(raw: &str) -> Result<String, ()> {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return Err(());
            }
            let h = |b: u8| match b {
                b'0'..=b'9' => Some(b - b'0'),
                b'a'..=b'f' => Some(10 + b - b'a'),
                b'A'..=b'F' => Some(10 + b - b'A'),
                _ => None,
            };
            let a = h(bytes[i + 1]).ok_or(())?;
            let b = h(bytes[i + 2]).ok_or(())?;
            let decoded = (a << 4) | b;
            // Uri.decodeFull() preserves URI reserved characters. This is
            // significant for tag prefixes; Uri.decodeComponent() would not.
            if b":/?#[]@!$&'()*+,;=".contains(&decoded) {
                out.extend_from_slice(&bytes[i..i + 3]);
            } else {
                out.push(decoded);
            }
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| ())
}
