// Ported from package:yaml 3.1.4 lib/src/parser.dart.
//
// Copyright (c) 2014, the Dart project authors.
// Copyright (c) 2006, Kirill Simonov.
// Use of this source code is governed by an MIT-style license.

use std::collections::HashMap;

use crate::YamlException;
use crate::event::{Event, EventType, TagDirective, VersionDirective};
use crate::scanner::Scanner;
use crate::source::{FileSpan, SourceLocation};
use crate::style::{CollectionStyle, ScalarStyle};
use crate::token::{Token, TokenType};

#[allow(dead_code)] // These states exist in the Dart parser's state model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    StreamStart,
    DocumentStart,
    DocumentContent,
    DocumentEnd,
    BlockNode,
    BlockNodeOrIndentlessSequence,
    FlowNode,
    BlockSequenceFirstEntry,
    BlockSequenceEntry,
    IndentlessSequenceEntry,
    BlockMappingFirstKey,
    BlockMappingKey,
    BlockMappingValue,
    FlowSequenceFirstEntry,
    FlowSequenceEntry,
    FlowSequenceEntryMappingKey,
    FlowSequenceEntryMappingValue,
    FlowSequenceEntryMappingEnd,
    FlowMappingFirstKey,
    FlowMappingKey,
    FlowMappingValue,
    FlowMappingEmptyValue,
    End,
}

pub struct Parser {
    scanner: Scanner,
    states: Vec<State>,
    state: State,
    tag_directives: HashMap<String, TagDirective>,
}

impl Parser {
    pub fn new(source: &str, recover: bool) -> Self {
        Self {
            scanner: Scanner::new(source, recover),
            states: Vec::new(),
            state: State::StreamStart,
            tag_directives: HashMap::new(),
        }
    }

    pub fn is_done(&self) -> bool {
        self.state == State::End
    }

    pub fn take_errors(&mut self) -> Vec<YamlException> {
        self.scanner.take_errors()
    }

    pub fn parse(&mut self) -> Result<Event, YamlException> {
        if self.is_done() {
            panic!("No more events.");
        }
        match self.state {
            State::StreamStart => self.parse_stream_start(),
            State::DocumentStart => self.parse_document_start(),
            State::DocumentContent => self.parse_document_content(),
            State::DocumentEnd => self.parse_document_end(),
            State::BlockNode => self.parse_node(true, false),
            State::BlockNodeOrIndentlessSequence => self.parse_node(true, true),
            State::FlowNode => self.parse_node(false, false),
            State::BlockSequenceFirstEntry => {
                self.scanner.scan()?;
                self.parse_block_sequence_entry()
            }
            State::BlockSequenceEntry => self.parse_block_sequence_entry(),
            State::IndentlessSequenceEntry => self.parse_indentless_sequence_entry(),
            State::BlockMappingFirstKey => {
                self.scanner.scan()?;
                self.parse_block_mapping_key()
            }
            State::BlockMappingKey => self.parse_block_mapping_key(),
            State::BlockMappingValue => self.parse_block_mapping_value(),
            State::FlowSequenceFirstEntry => self.parse_flow_sequence_entry(true),
            State::FlowSequenceEntry => self.parse_flow_sequence_entry(false),
            State::FlowSequenceEntryMappingKey => self.parse_flow_sequence_entry_mapping_key(),
            State::FlowSequenceEntryMappingValue => self.parse_flow_sequence_entry_mapping_value(),
            State::FlowSequenceEntryMappingEnd => self.parse_flow_sequence_entry_mapping_end(),
            State::FlowMappingFirstKey => self.parse_flow_mapping_key(true),
            State::FlowMappingKey => self.parse_flow_mapping_key(false),
            State::FlowMappingValue => self.parse_flow_mapping_value(false),
            State::FlowMappingEmptyValue => self.parse_flow_mapping_value(true),
            State::End => unreachable!(),
        }
    }

    fn pop_state(&mut self) -> State {
        self.states
            .pop()
            .expect("parser state stack must not be empty")
    }

    /// Consumes the current token and returns the following token, matching
    /// `Scanner.advance()` in the Dart implementation.
    fn advance(&mut self) -> Result<Token, YamlException> {
        self.scanner.scan()?;
        self.scanner.peek()
    }

    fn parse_stream_start(&mut self) -> Result<Event, YamlException> {
        let token = self.scanner.scan()?;
        debug_assert_eq!(token.kind, TokenType::StreamStart);
        self.state = State::DocumentStart;
        Ok(Event::new(EventType::StreamStart, token.span))
    }

    fn parse_document_start(&mut self) -> Result<Event, YamlException> {
        let mut token = self.scanner.peek()?;
        while token.kind == TokenType::DocumentEnd {
            token = self.advance()?;
        }
        if !matches!(
            token.kind,
            TokenType::VersionDirective
                | TokenType::TagDirective
                | TokenType::DocumentStart
                | TokenType::StreamEnd
        ) {
            self.process_directives()?;
            self.states.push(State::DocumentEnd);
            self.state = State::BlockNode;
            return Ok(Event::new(
                EventType::DocumentStart,
                point(&token.span.start),
            ));
        }
        if token.kind == TokenType::StreamEnd {
            self.state = State::End;
            self.scanner.scan()?;
            return Ok(Event::new(EventType::StreamEnd, token.span));
        }
        let start = token.span;
        let (version_directive, tag_directives) = self.process_directives()?;
        token = self.scanner.peek()?;
        if token.kind != TokenType::DocumentStart {
            return Err(error("Expected document start.", token.span));
        }
        self.states.push(State::DocumentEnd);
        self.state = State::DocumentContent;
        self.scanner.scan()?;
        let mut event = Event::new(EventType::DocumentStart, expand(&start, &token.span));
        event.version_directive = version_directive;
        event.tag_directives = tag_directives;
        event.is_implicit = false;
        Ok(event)
    }

    fn parse_document_content(&mut self) -> Result<Event, YamlException> {
        let token = self.scanner.peek()?;
        if matches!(
            token.kind,
            TokenType::VersionDirective
                | TokenType::TagDirective
                | TokenType::DocumentStart
                | TokenType::DocumentEnd
                | TokenType::StreamEnd
        ) {
            self.state = self.pop_state();
            Ok(empty_scalar(&token.span.start))
        } else {
            self.parse_node(true, false)
        }
    }

    fn parse_document_end(&mut self) -> Result<Event, YamlException> {
        self.tag_directives.clear();
        self.state = State::DocumentStart;
        let token = self.scanner.peek()?;
        if token.kind == TokenType::DocumentEnd {
            self.scanner.scan()?;
            let mut event = Event::new(EventType::DocumentEnd, token.span);
            event.is_implicit = false;
            Ok(event)
        } else {
            Ok(Event::new(EventType::DocumentEnd, point(&token.span.start)))
        }
    }

    fn parse_node(
        &mut self,
        block: bool,
        indentless_sequence: bool,
    ) -> Result<Event, YamlException> {
        let mut token = self.scanner.peek()?;
        if token.kind == TokenType::Alias {
            self.scanner.scan()?;
            self.state = self.pop_state();
            let mut event = Event::new(EventType::Alias, token.span);
            event.value = token.value;
            return Ok(event);
        }

        let mut anchor = None;
        let mut tag_token = None;
        let mut span = point(&token.span.start);
        if token.kind == TokenType::Anchor {
            anchor = Some(token.value.clone());
            span = expand(&span, &token.span);
            token = self.advance()?;
            if token.kind == TokenType::Tag {
                span = expand(&span, &token.span);
                tag_token = Some(token.clone());
                token = self.advance()?;
            }
        } else if token.kind == TokenType::Tag {
            span = expand(&span, &token.span);
            tag_token = Some(token.clone());
            token = self.advance()?;
            if token.kind == TokenType::Anchor {
                anchor = Some(token.value.clone());
                span = expand(&span, &token.span);
                token = self.advance()?;
            }
        }

        let tag = if let Some(tag_token) = tag_token {
            if let Some(handle) = &tag_token.handle {
                let Some(directive) = self.tag_directives.get(handle) else {
                    return Err(error("Undefined tag handle.", tag_token.span));
                };
                Some(format!("{}{}", directive.prefix, tag_token.suffix))
            } else {
                Some(tag_token.suffix)
            }
        } else {
            None
        };

        if indentless_sequence && token.kind == TokenType::BlockEntry {
            self.state = State::IndentlessSequenceEntry;
            return Ok(collection_start(
                EventType::SequenceStart,
                expand(&span, &token.span),
                CollectionStyle::Block,
                anchor,
                tag,
            ));
        }
        if token.kind == TokenType::Scalar {
            let tag = if tag.is_none() && token.style != ScalarStyle::Plain {
                Some("!".to_owned())
            } else {
                tag
            };
            self.state = self.pop_state();
            self.scanner.scan()?;
            let mut event = Event::new(EventType::Scalar, expand(&span, &token.span));
            event.value = token.value;
            event.scalar_style = token.style;
            event.anchor = anchor;
            event.tag = tag;
            return Ok(event);
        }
        if token.kind == TokenType::FlowSequenceStart {
            self.state = State::FlowSequenceFirstEntry;
            return Ok(collection_start(
                EventType::SequenceStart,
                expand(&span, &token.span),
                CollectionStyle::Flow,
                anchor,
                tag,
            ));
        }
        if token.kind == TokenType::FlowMappingStart {
            self.state = State::FlowMappingFirstKey;
            return Ok(collection_start(
                EventType::MappingStart,
                expand(&span, &token.span),
                CollectionStyle::Flow,
                anchor,
                tag,
            ));
        }
        if block && token.kind == TokenType::BlockSequenceStart {
            self.state = State::BlockSequenceFirstEntry;
            return Ok(collection_start(
                EventType::SequenceStart,
                expand(&span, &token.span),
                CollectionStyle::Block,
                anchor,
                tag,
            ));
        }
        if block && token.kind == TokenType::BlockMappingStart {
            self.state = State::BlockMappingFirstKey;
            return Ok(collection_start(
                EventType::MappingStart,
                expand(&span, &token.span),
                CollectionStyle::Block,
                anchor,
                tag,
            ));
        }
        if anchor.is_some() || tag.is_some() {
            self.state = self.pop_state();
            let mut event = empty_scalar_at(span);
            event.anchor = anchor;
            event.tag = tag;
            return Ok(event);
        }
        Err(error("Expected node content.", span))
    }

    fn parse_block_sequence_entry(&mut self) -> Result<Event, YamlException> {
        let mut token = self.scanner.peek()?;
        if token.kind == TokenType::BlockEntry {
            let start = token.span.start;
            token = self.advance()?;
            if matches!(token.kind, TokenType::BlockEntry | TokenType::BlockEnd) {
                self.state = State::BlockSequenceEntry;
                return Ok(empty_scalar(&start));
            }
            self.states.push(State::BlockSequenceEntry);
            return self.parse_node(true, false);
        }
        if token.kind == TokenType::BlockEnd {
            self.scanner.scan()?;
            self.state = self.pop_state();
            return Ok(Event::new(EventType::SequenceEnd, token.span));
        }
        if token.kind == TokenType::Key {
            self.scanner.scan()?;
            self.states.push(State::BlockSequenceEntry);
            return self.parse_node(true, false);
        }
        Err(error(
            "While parsing a block collection, expected '-'.",
            point(&token.span.start),
        ))
    }

    fn parse_indentless_sequence_entry(&mut self) -> Result<Event, YamlException> {
        let mut token = self.scanner.peek()?;
        if token.kind != TokenType::BlockEntry {
            self.state = self.pop_state();
            return Ok(Event::new(EventType::SequenceEnd, point(&token.span.start)));
        }
        let start = token.span.start;
        token = self.advance()?;
        if matches!(
            token.kind,
            TokenType::BlockEntry | TokenType::Key | TokenType::Value | TokenType::BlockEnd
        ) {
            self.state = State::IndentlessSequenceEntry;
            Ok(empty_scalar(&start))
        } else {
            self.states.push(State::IndentlessSequenceEntry);
            self.parse_node(true, false)
        }
    }

    fn parse_block_mapping_key(&mut self) -> Result<Event, YamlException> {
        let mut token = self.scanner.peek()?;
        if token.kind == TokenType::Key {
            let start = token.span.start;
            token = self.advance()?;
            if matches!(
                token.kind,
                TokenType::Key | TokenType::Value | TokenType::BlockEnd
            ) {
                self.state = State::BlockMappingValue;
                return Ok(empty_scalar(&start));
            }
            self.states.push(State::BlockMappingValue);
            return self.parse_node(true, true);
        }
        if token.kind == TokenType::Value {
            self.state = State::BlockMappingValue;
            return Ok(empty_scalar(&token.span.start));
        }
        if token.kind == TokenType::BlockEnd {
            self.scanner.scan()?;
            self.state = self.pop_state();
            return Ok(Event::new(EventType::MappingEnd, token.span));
        }
        Err(error(
            "Expected a key while parsing a block mapping.",
            point(&token.span.start),
        ))
    }

    fn parse_block_mapping_value(&mut self) -> Result<Event, YamlException> {
        let mut token = self.scanner.peek()?;
        if token.kind != TokenType::Value {
            self.state = State::BlockMappingKey;
            return Ok(empty_scalar(&token.span.start));
        }
        let start = token.span.start;
        token = self.advance()?;
        if matches!(
            token.kind,
            TokenType::Key | TokenType::Value | TokenType::BlockEnd
        ) {
            self.state = State::BlockMappingKey;
            Ok(empty_scalar(&start))
        } else {
            self.states.push(State::BlockMappingKey);
            self.parse_node(true, true)
        }
    }

    fn parse_flow_sequence_entry(&mut self, first: bool) -> Result<Event, YamlException> {
        if first {
            self.scanner.scan()?;
        }
        let mut token = self.scanner.peek()?;
        if token.kind != TokenType::FlowSequenceEnd {
            if !first {
                if token.kind != TokenType::FlowEntry {
                    return Err(error(
                        "While parsing a flow sequence, expected ',' or ']'.",
                        point(&token.span.start),
                    ));
                }
                token = self.advance()?;
            }
            if token.kind == TokenType::Key {
                self.state = State::FlowSequenceEntryMappingKey;
                self.scanner.scan()?;
                return Ok(collection_start(
                    EventType::MappingStart,
                    token.span,
                    CollectionStyle::Flow,
                    None,
                    None,
                ));
            } else if token.kind != TokenType::FlowSequenceEnd {
                self.states.push(State::FlowSequenceEntry);
                return self.parse_node(false, false);
            }
        }
        self.scanner.scan()?;
        self.state = self.pop_state();
        Ok(Event::new(EventType::SequenceEnd, token.span))
    }

    fn parse_flow_sequence_entry_mapping_key(&mut self) -> Result<Event, YamlException> {
        let token = self.scanner.peek()?;
        if matches!(
            token.kind,
            TokenType::Value | TokenType::FlowEntry | TokenType::FlowSequenceEnd
        ) {
            self.state = State::FlowSequenceEntryMappingValue;
            Ok(empty_scalar(&token.span.start))
        } else {
            self.states.push(State::FlowSequenceEntryMappingValue);
            self.parse_node(false, false)
        }
    }

    fn parse_flow_sequence_entry_mapping_value(&mut self) -> Result<Event, YamlException> {
        let mut token = self.scanner.peek()?;
        if token.kind == TokenType::Value {
            token = self.advance()?;
            if !matches!(
                token.kind,
                TokenType::FlowEntry | TokenType::FlowSequenceEnd
            ) {
                self.states.push(State::FlowSequenceEntryMappingEnd);
                return self.parse_node(false, false);
            }
        }
        self.state = State::FlowSequenceEntryMappingEnd;
        Ok(empty_scalar(&token.span.start))
    }

    fn parse_flow_sequence_entry_mapping_end(&mut self) -> Result<Event, YamlException> {
        self.state = State::FlowSequenceEntry;
        let token = self.scanner.peek()?;
        Ok(Event::new(EventType::MappingEnd, point(&token.span.start)))
    }

    fn parse_flow_mapping_key(&mut self, first: bool) -> Result<Event, YamlException> {
        if first {
            self.scanner.scan()?;
        }
        let mut token = self.scanner.peek()?;
        if token.kind != TokenType::FlowMappingEnd {
            if !first {
                if token.kind != TokenType::FlowEntry {
                    return Err(error(
                        "While parsing a flow mapping, expected ',' or '}'.",
                        point(&token.span.start),
                    ));
                }
                token = self.advance()?;
            }
            if token.kind == TokenType::Key {
                token = self.advance()?;
                if !matches!(
                    token.kind,
                    TokenType::Value | TokenType::FlowEntry | TokenType::FlowMappingEnd
                ) {
                    self.states.push(State::FlowMappingValue);
                    return self.parse_node(false, false);
                }
                self.state = State::FlowMappingValue;
                return Ok(empty_scalar(&token.span.start));
            } else if token.kind != TokenType::FlowMappingEnd {
                self.states.push(State::FlowMappingEmptyValue);
                return self.parse_node(false, false);
            }
        }
        self.scanner.scan()?;
        self.state = self.pop_state();
        Ok(Event::new(EventType::MappingEnd, token.span))
    }

    fn parse_flow_mapping_value(&mut self, empty: bool) -> Result<Event, YamlException> {
        let mut token = self.scanner.peek()?;
        if empty {
            self.state = State::FlowMappingKey;
            return Ok(empty_scalar(&token.span.start));
        }
        if token.kind == TokenType::Value {
            token = self.advance()?;
            if !matches!(token.kind, TokenType::FlowEntry | TokenType::FlowMappingEnd) {
                self.states.push(State::FlowMappingKey);
                return self.parse_node(false, false);
            }
        }
        self.state = State::FlowMappingKey;
        Ok(empty_scalar(&token.span.start))
    }

    fn process_directives(
        &mut self,
    ) -> Result<(Option<VersionDirective>, Vec<TagDirective>), YamlException> {
        let mut token = self.scanner.peek()?;
        let mut version_directive = None;
        let mut tag_directives = Vec::new();
        while matches!(
            token.kind,
            TokenType::VersionDirective | TokenType::TagDirective
        ) {
            if token.kind == TokenType::VersionDirective {
                if version_directive.is_some() {
                    return Err(error("Duplicate %YAML directive.", token.span));
                }
                if token.major != 1 || token.minor == 0 {
                    return Err(error(
                        "Incompatible YAML document. This parser only supports YAML 1.1 and 1.2.",
                        token.span,
                    ));
                }
                version_directive = Some(VersionDirective {
                    major: token.major,
                    minor: token.minor,
                    span: token.span,
                });
            } else {
                let directive = TagDirective {
                    handle: token.handle.clone().unwrap_or_default(),
                    prefix: token.suffix.clone(),
                    span: token.span,
                };
                self.append_tag_directive(directive.clone(), false)?;
                tag_directives.push(directive);
            }
            token = self.advance()?;
        }
        let default_span = point(&token.span.start);
        self.append_tag_directive(
            TagDirective {
                handle: "!".into(),
                prefix: "!".into(),
                span: default_span,
            },
            true,
        )?;
        self.append_tag_directive(
            TagDirective {
                handle: "!!".into(),
                prefix: "tag:yaml.org,2002:".into(),
                span: default_span,
            },
            true,
        )?;
        Ok((version_directive, tag_directives))
    }

    fn append_tag_directive(
        &mut self,
        directive: TagDirective,
        allow_duplicates: bool,
    ) -> Result<(), YamlException> {
        if self.tag_directives.contains_key(&directive.handle) {
            if allow_duplicates {
                return Ok(());
            }
            return Err(error("Duplicate %TAG directive.", directive.span));
        }
        self.tag_directives
            .insert(directive.handle.clone(), directive);
        Ok(())
    }
}

fn point(location: &SourceLocation) -> FileSpan {
    FileSpan {
        start: *location,
        end: *location,
    }
}

fn expand(first: &FileSpan, second: &FileSpan) -> FileSpan {
    (*first).expand(*second)
}

fn empty_scalar(location: &SourceLocation) -> Event {
    empty_scalar_at(point(location))
}

fn empty_scalar_at(span: FileSpan) -> Event {
    let mut event = Event::new(EventType::Scalar, span);
    event.scalar_style = ScalarStyle::Plain;
    event
}

fn collection_start(
    kind: EventType,
    span: FileSpan,
    style: CollectionStyle,
    anchor: Option<String>,
    tag: Option<String>,
) -> Event {
    let mut event = Event::new(kind, span);
    event.collection_style = style;
    event.anchor = anchor;
    event.tag = tag;
    event
}

fn error(message: &str, span: FileSpan) -> YamlException {
    YamlException {
        message: message.to_owned(),
        span,
    }
}
