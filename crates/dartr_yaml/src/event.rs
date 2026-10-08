// Ported from package:yaml 3.1.4 lib/src/event.dart and yaml_document.dart.
//
// Copyright (c) 2014, the Dart project authors.
// Copyright (c) 2006, Kirill Simonov.
// Use of this source code is governed by an MIT-style license.

use crate::source::FileSpan;
use crate::style::{CollectionStyle, ScalarStyle};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventType {
    StreamStart,
    StreamEnd,
    DocumentStart,
    DocumentEnd,
    Alias,
    Scalar,
    SequenceStart,
    SequenceEnd,
    MappingStart,
    MappingEnd,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionDirective {
    pub major: i64,
    pub minor: i64,
    pub span: FileSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TagDirective {
    pub handle: String,
    pub prefix: String,
    pub span: FileSpan,
}

/// A parser event. Fields which do not apply to the event's `kind` retain
/// their default value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event {
    pub kind: EventType,
    pub span: FileSpan,
    pub value: String,
    pub anchor: Option<String>,
    pub tag: Option<String>,
    pub scalar_style: ScalarStyle,
    pub collection_style: CollectionStyle,
    pub version_directive: Option<VersionDirective>,
    pub tag_directives: Vec<TagDirective>,
    pub is_implicit: bool,
}

impl Event {
    pub fn new(kind: EventType, span: FileSpan) -> Self {
        Self {
            kind,
            span,
            value: String::new(),
            anchor: None,
            tag: None,
            scalar_style: ScalarStyle::Any,
            collection_style: CollectionStyle::Any,
            version_directive: None,
            tag_directives: Vec::new(),
            is_implicit: true,
        }
    }
}
