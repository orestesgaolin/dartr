// Ported from package:yaml 3.1.4 lib/src/token.dart.
//
// Copyright (c) 2014, the Dart project authors.
// Copyright (c) 2006, Kirill Simonov.
// Use of this source code is governed by an MIT-style license.

use crate::source::FileSpan;
use crate::style::ScalarStyle;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenType {
    StreamStart,
    StreamEnd,
    VersionDirective,
    TagDirective,
    DocumentStart,
    DocumentEnd,
    BlockSequenceStart,
    BlockMappingStart,
    BlockEnd,
    FlowSequenceStart,
    FlowSequenceEnd,
    FlowMappingStart,
    FlowMappingEnd,
    BlockEntry,
    FlowEntry,
    Key,
    Value,
    Alias,
    Anchor,
    Tag,
    Scalar,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Token {
    pub kind: TokenType,
    pub span: FileSpan,
    pub value: String,
    pub style: ScalarStyle,
    pub handle: Option<String>,
    pub suffix: String,
    pub major: i64,
    pub minor: i64,
}

impl Token {
    pub fn new(kind: TokenType, span: FileSpan) -> Self {
        Self {
            kind,
            span,
            value: String::new(),
            style: ScalarStyle::Any,
            handle: None,
            suffix: String::new(),
            major: 0,
            minor: 0,
        }
    }
}
