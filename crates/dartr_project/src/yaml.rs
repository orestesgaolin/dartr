//! YAML nodes with source spans, with the value semantics of `package:yaml`
//! (`loadYamlNode`), plus the analyzer helpers from
//! `pkg/analyzer/lib/src/util/yaml.dart` (`Merger`, `valueAt`, `toBool`).
//!
//! The events come from `saphyr-parser`. This module composes them into
//! nodes the same way `package:yaml/src/loader.dart` does:
//!
//! - plain scalars are resolved with the core schema of `package:yaml`
//!   (`null`, `~`, empty, `true`/`True`/`TRUE`, ints, `0x`, `0o`, floats,
//!   `.inf`, `.nan`), quoted and block scalars are strings;
//! - a duplicate mapping key is an error;
//! - only one document is allowed.
//!
//! Spans are UTF-8 byte offsets into the source text.

use saphyr_parser::{Event, Parser, ScalarStyle};

/// A range of the source text, in UTF-8 bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    /// Zero-based line of [start].
    pub line: usize,
    /// Zero-based column (in characters) of [start].
    pub column: usize,
}

impl Span {
    pub fn length(&self) -> usize {
        self.end - self.start
    }
}

/// The value of a scalar.
#[derive(Clone, Debug, PartialEq)]
pub enum Scalar {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

impl Scalar {
    /// The result of Dart's `value.toString()`.
    pub fn to_dart_string(&self) -> String {
        match self {
            Scalar::Null => "null".to_string(),
            Scalar::Bool(value) => value.to_string(),
            Scalar::Int(value) => value.to_string(),
            Scalar::Float(value) => dart_double_to_string(*value),
            Scalar::String(value) => value.clone(),
        }
    }
}

fn dart_double_to_string(value: f64) -> String {
    if value.is_nan() {
        "NaN".to_string()
    } else if value.is_infinite() {
        if value > 0.0 {
            "Infinity".to_string()
        } else {
            "-Infinity".to_string()
        }
    } else if value == value.trunc() && value.abs() < 1e21 {
        format!("{value:.1}")
    } else {
        format!("{value}")
    }
}

/// A YAML node.
#[derive(Clone, Debug, PartialEq)]
pub struct YamlNode {
    pub kind: NodeKind,
    pub span: Span,
}

/// The kind and content of a YAML node.
#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    Scalar(Scalar),
    List(Vec<YamlNode>),
    /// Entries in source order.
    Map(Vec<(YamlNode, YamlNode)>),
}

/// A YAML syntax error.
#[derive(Clone, Debug, PartialEq)]
pub struct YamlError {
    pub message: String,
    pub span: Option<Span>,
}

impl YamlNode {
    pub fn scalar(&self) -> Option<&Scalar> {
        match &self.kind {
            NodeKind::Scalar(scalar) => Some(scalar),
            _ => None,
        }
    }

    pub fn is_scalar(&self) -> bool {
        matches!(self.kind, NodeKind::Scalar(_))
    }

    pub fn as_list(&self) -> Option<&[YamlNode]> {
        match &self.kind {
            NodeKind::List(nodes) => Some(nodes),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&[(YamlNode, YamlNode)]> {
        match &self.kind {
            NodeKind::Map(entries) => Some(entries),
            _ => None,
        }
    }

    /// `true` if this is a scalar with the value `null`.
    pub fn is_null_scalar(&self) -> bool {
        matches!(self.kind, NodeKind::Scalar(Scalar::Null))
    }

    /// The string value of a scalar, or `None` (`stringValue` in the analyzer).
    pub fn string_value(&self) -> Option<&str> {
        match &self.kind {
            NodeKind::Scalar(Scalar::String(value)) => Some(value),
            _ => None,
        }
    }

    /// The bool value of a scalar: a bool, or a string `true`/`false` in any
    /// case (`YamlScalarExtension.toBool`).
    pub fn to_bool(&self) -> Option<bool> {
        match &self.kind {
            NodeKind::Scalar(Scalar::Bool(value)) => Some(*value),
            NodeKind::Scalar(Scalar::String(value)) => match value.to_lowercase().as_str() {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            },
            _ => None,
        }
    }

    /// The value of the map entry whose scalar key is the string [key]
    /// (`YamlMapExtensions.valueAt`).
    pub fn value_at(&self, key: &str) -> Option<&YamlNode> {
        self.as_map()?
            .iter()
            .find(|(k, _)| matches!(&k.kind, NodeKind::Scalar(Scalar::String(s)) if s == key))
            .map(|(_, v)| v)
    }

    /// The key node of the map entry whose scalar key is the string [key]
    /// (`YamlMapExtensions.getKey`).
    pub fn get_key(&self, key: &str) -> Option<&YamlNode> {
        self.as_map()?
            .iter()
            .find(|(k, _)| matches!(&k.kind, NodeKind::Scalar(Scalar::String(s)) if s == key))
            .map(|(k, _)| k)
    }

    /// Deep equality of values, ignoring spans (`deepEquals`).
    pub fn value_equals(&self, other: &YamlNode) -> bool {
        match (&self.kind, &other.kind) {
            (NodeKind::Scalar(a), NodeKind::Scalar(b)) => match (a, b) {
                (Scalar::Int(a), Scalar::Float(b)) | (Scalar::Float(b), Scalar::Int(a)) => {
                    (*a as f64) == *b
                }
                _ => a == b,
            },
            (NodeKind::List(a), NodeKind::List(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.value_equals(y))
            }
            (NodeKind::Map(a), NodeKind::Map(b)) => {
                a.len() == b.len()
                    && a.iter().all(|(k, v)| {
                        b.iter()
                            .any(|(k2, v2)| k.value_equals(k2) && v.value_equals(v2))
                    })
            }
            _ => false,
        }
    }
}

/// Parses [text] as a single YAML document (`loadYamlNode`).
pub fn load_yaml_node(text: &str) -> Result<YamlNode, YamlError> {
    Loader::new(text).load()
}

struct Loader<'a> {
    parser: Parser<'a, saphyr_parser::StrInput<'a>>,
    /// Byte offset of each character index, plus the end.
    char_to_byte: Vec<usize>,
    anchors: std::collections::HashMap<usize, YamlNode>,
}

type SpannedEvent<'a> = (Event<'a>, saphyr_parser::Span);

impl<'a> Loader<'a> {
    fn new(text: &'a str) -> Self {
        let mut char_to_byte: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
        char_to_byte.push(text.len());
        Loader {
            parser: Parser::new_from_str(text),
            char_to_byte,
            anchors: Default::default(),
        }
    }

    fn byte(&self, char_index: usize) -> usize {
        *self
            .char_to_byte
            .get(char_index)
            .unwrap_or(self.char_to_byte.last().unwrap())
    }

    fn span(&self, span: &saphyr_parser::Span) -> Span {
        Span {
            start: self.byte(span.start.index()),
            end: self.byte(span.end.index()),
            line: span.start.line().saturating_sub(1),
            column: span.start.col(),
        }
    }

    fn next(&mut self) -> Result<SpannedEvent<'a>, YamlError> {
        match self.parser.next_event() {
            Some(Ok(event)) => Ok(event),
            Some(Err(error)) => {
                let marker = error.marker();
                let offset = self.byte(marker.index());
                Err(YamlError {
                    message: error.info().to_string(),
                    span: Some(Span {
                        start: offset,
                        end: offset,
                        line: marker.line().saturating_sub(1),
                        column: marker.col(),
                    }),
                })
            }
            None => Err(YamlError {
                message: "Unexpected end of input.".into(),
                span: None,
            }),
        }
    }

    fn load(mut self) -> Result<YamlNode, YamlError> {
        let mut result: Option<YamlNode> = None;
        loop {
            let (event, span) = self.next()?;
            match event {
                Event::StreamStart | Event::Nothing | Event::DocumentEnd => {}
                Event::StreamEnd => break,
                Event::DocumentStart(_) => {
                    if result.is_some() {
                        return Err(YamlError {
                            message: "Only expected one document.".into(),
                            span: Some(self.span(&span)),
                        });
                    }
                    let (event, span) = self.next()?;
                    if matches!(event, Event::DocumentEnd | Event::StreamEnd) {
                        let at = self.span(&span);
                        result = Some(YamlNode {
                            kind: NodeKind::Scalar(Scalar::Null),
                            span: Span {
                                end: at.start,
                                ..at
                            },
                        });
                        if matches!(event, Event::StreamEnd) {
                            break;
                        }
                    } else {
                        result = Some(self.load_node(event, span)?);
                    }
                }
                _ => {
                    let node = self.load_node(event, span)?;
                    result = Some(node);
                }
            }
        }
        Ok(result.unwrap_or(YamlNode {
            kind: NodeKind::Scalar(Scalar::Null),
            span: Span::default(),
        }))
    }

    fn load_node(
        &mut self,
        event: Event<'a>,
        span: saphyr_parser::Span,
    ) -> Result<YamlNode, YamlError> {
        match event {
            Event::Alias(id) => self.anchors.get(&id).cloned().ok_or(YamlError {
                message: "Undefined alias.".into(),
                span: Some(self.span(&span)),
            }),
            Event::Scalar(value, style, anchor, tag) => {
                let node_span = self.span(&span);
                let scalar = match tag {
                    Some(tag) => {
                        let full = if tag.handle == "!!" {
                            format!("tag:yaml.org,2002:{}", tag.suffix)
                        } else {
                            format!("{}{}", tag.handle, tag.suffix)
                        };
                        parse_by_tag(&full, &value).ok_or_else(|| YamlError {
                            message: format!("Invalid scalar for tag {full}."),
                            span: Some(node_span),
                        })?
                    }
                    None if style != ScalarStyle::Plain => Scalar::String(value.into_owned()),
                    None => parse_plain_scalar(&value),
                };
                let node = YamlNode {
                    kind: NodeKind::Scalar(scalar),
                    span: node_span,
                };
                if anchor > 0 {
                    self.anchors.insert(anchor, node.clone());
                }
                Ok(node)
            }
            Event::SequenceStart(anchor, _) => {
                let start = self.span(&span);
                let mut nodes = Vec::new();
                let end;
                loop {
                    let (event, span) = self.next()?;
                    if let Event::SequenceEnd = event {
                        end = self.span(&span).end;
                        break;
                    }
                    nodes.push(self.load_node(event, span)?);
                }
                // Block collections end at their last child; the end event of
                // a block collection is at the start of the next token.
                let end = nodes.last().map_or(end, |n| n.span.end);
                let node = YamlNode {
                    kind: NodeKind::List(nodes),
                    span: Span {
                        end: end.max(start.start),
                        ..start
                    },
                };
                if anchor > 0 {
                    self.anchors.insert(anchor, node.clone());
                }
                Ok(node)
            }
            Event::MappingStart(anchor, _) => {
                let start = self.span(&span);
                let mut entries: Vec<(YamlNode, YamlNode)> = Vec::new();
                let end;
                loop {
                    let (event, span) = self.next()?;
                    if let Event::MappingEnd = event {
                        end = self.span(&span).end;
                        break;
                    }
                    let key = self.load_node(event, span)?;
                    let (event, span) = self.next()?;
                    let value = self.load_node(event, span)?;
                    if entries.iter().any(|(k, _)| k.value_equals(&key)) {
                        return Err(YamlError {
                            message: "Duplicate mapping key.".into(),
                            span: Some(key.span),
                        });
                    }
                    entries.push((key, value));
                }
                let end = entries.last().map_or(end, |(_, v)| v.span.end);
                let node = YamlNode {
                    kind: NodeKind::Map(entries),
                    span: Span {
                        end: end.max(start.start),
                        ..start
                    },
                };
                if anchor > 0 {
                    self.anchors.insert(anchor, node.clone());
                }
                Ok(node)
            }
            _ => Err(YamlError {
                message: "Unexpected event.".into(),
                span: Some(self.span(&span)),
            }),
        }
    }
}

fn parse_by_tag(tag: &str, value: &str) -> Option<Scalar> {
    match tag {
        "!" | "tag:yaml.org,2002:str" => Some(Scalar::String(value.to_string())),
        "tag:yaml.org,2002:null" => parse_null(value),
        "tag:yaml.org,2002:bool" => parse_bool(value),
        "tag:yaml.org,2002:int" => parse_number(value, true, false),
        "tag:yaml.org,2002:float" => parse_number(value, false, true),
        _ => Some(parse_plain_scalar(value)),
    }
}

fn parse_null(value: &str) -> Option<Scalar> {
    matches!(value, "" | "null" | "Null" | "NULL" | "~").then_some(Scalar::Null)
}

fn parse_bool(value: &str) -> Option<Scalar> {
    match value {
        "true" | "True" | "TRUE" => Some(Scalar::Bool(true)),
        "false" | "False" | "FALSE" => Some(Scalar::Bool(false)),
        _ => None,
    }
}

/// `_tryParseScalar` of `package:yaml`, falling back to a string.
fn parse_plain_scalar(value: &str) -> Scalar {
    let bytes = value.as_bytes();
    if bytes.is_empty() {
        return Scalar::Null;
    }
    let length = value.chars().count();
    let parsed = match bytes[0] {
        b'.' | b'+' | b'-' => parse_number(value, true, true),
        b'n' | b'N' if length == 4 => parse_null(value),
        b't' | b'T' if length == 4 => parse_bool(value),
        b'f' | b'F' if length == 5 => parse_bool(value),
        b'~' if length == 1 => Some(Scalar::Null),
        b'0'..=b'9' => parse_number(value, true, true),
        _ => None,
    };
    parsed.unwrap_or_else(|| Scalar::String(value.to_string()))
}

/// `_parseNumberValue` of `package:yaml`.
fn parse_number(contents: &str, allow_int: bool, allow_float: bool) -> Option<Scalar> {
    let bytes = contents.as_bytes();
    let first = *bytes.first()?;
    if allow_int && bytes.len() == 1 {
        return first
            .is_ascii_digit()
            .then(|| Scalar::Int((first - b'0') as i64));
    }
    let second = *bytes.get(1)?;
    if allow_int && first == b'0' {
        if second == b'x' {
            return i64::from_str_radix(&contents[2..], 16)
                .ok()
                .map(Scalar::Int);
        }
        if second == b'o' {
            return i64::from_str_radix(&contents[2..], 8).ok().map(Scalar::Int);
        }
    }
    if first.is_ascii_digit() || ((first == b'+' || first == b'-') && second.is_ascii_digit()) {
        if allow_int && let Some(value) = dart_int_try_parse(contents) {
            return Some(Scalar::Int(value));
        }
        if allow_float {
            return dart_double_try_parse(contents).map(Scalar::Float);
        }
        return None;
    }
    if !allow_float {
        return None;
    }
    if (first == b'.' && second.is_ascii_digit())
        || ((first == b'-' || first == b'+') && second == b'.')
    {
        if bytes.len() == 5 {
            match contents {
                "+.inf" | "+.Inf" | "+.INF" => return Some(Scalar::Float(f64::INFINITY)),
                "-.inf" | "-.Inf" | "-.INF" => return Some(Scalar::Float(f64::NEG_INFINITY)),
                _ => {}
            }
        }
        return dart_double_try_parse(contents).map(Scalar::Float);
    }
    if bytes.len() == 4 && first == b'.' {
        return match contents {
            ".inf" | ".Inf" | ".INF" => Some(Scalar::Float(f64::INFINITY)),
            ".nan" | ".NaN" | ".NAN" => Some(Scalar::Float(f64::NAN)),
            _ => None,
        };
    }
    None
}

/// `int.tryParse(contents, radix: 10)`: optional sign, decimal digits.
fn dart_int_try_parse(contents: &str) -> Option<i64> {
    let digits = contents.strip_prefix(['+', '-']).unwrap_or(contents);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    contents.parse::<i64>().ok()
}

/// `double.tryParse`: a decimal floating point literal, or `NaN`/`Infinity`.
fn dart_double_try_parse(contents: &str) -> Option<f64> {
    let body = contents.strip_prefix(['+', '-']).unwrap_or(contents);
    match body {
        "NaN" => return Some(f64::NAN),
        "Infinity" => {
            return Some(if contents.starts_with('-') {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            });
        }
        _ => {}
    }
    let valid = body
        .bytes()
        .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-'));
    if !valid {
        return None;
    }
    contents.parse::<f64>().ok()
}

/// Merges YAML nodes with override semantics (`Merger` in
/// `pkg/analyzer/lib/src/util/yaml.dart`).
pub fn merge(o1: &YamlNode, o2: &YamlNode) -> YamlNode {
    fn is_list_of_string(node: &YamlNode) -> bool {
        node.as_list()
            .is_some_and(|nodes| nodes.iter().all(|n| n.string_value().is_some()))
    }
    fn is_map_to_bools(node: &YamlNode) -> bool {
        node.as_map().is_some_and(|entries| {
            entries
                .iter()
                .all(|(_, v)| matches!(v.scalar(), Some(Scalar::Bool(_))))
        })
    }
    fn list_to_map(list: &YamlNode, span: Span) -> YamlNode {
        let entries = list
            .as_list()
            .unwrap()
            .iter()
            .map(|element| {
                (
                    element.clone(),
                    YamlNode {
                        kind: NodeKind::Scalar(Scalar::Bool(true)),
                        span,
                    },
                )
            })
            .collect();
        YamlNode {
            kind: NodeKind::Map(entries),
            span,
        }
    }

    let mut o1 = o1.clone();
    let mut o2 = o2.clone();
    if is_list_of_string(&o1) && is_map_to_bools(&o2) {
        o1 = list_to_map(&o1, o1.span);
    } else if is_map_to_bools(&o1) && is_list_of_string(&o2) {
        o2 = list_to_map(&o2, o1.span);
    }

    match (&o1.kind, &o2.kind) {
        (NodeKind::Map(m1), NodeKind::Map(m2)) => {
            let mut merged: Vec<(YamlNode, YamlNode)> = m1.clone();
            for (k, v) in m2 {
                let key_value = k.scalar();
                let existing = merged
                    .iter()
                    .position(|(key, _)| key.scalar().is_some() && key.scalar() == key_value);
                match existing {
                    Some(index) => {
                        let value = merge(&merged[index].1, v);
                        merged[index].1 = value;
                    }
                    None => merged.push((k.clone(), v.clone())),
                }
            }
            YamlNode {
                kind: NodeKind::Map(merged),
                span: o1.span,
            }
        }
        (NodeKind::List(l1), NodeKind::List(l2)) => {
            let mut list = l1.clone();
            for n2 in l2 {
                let contains = l1.iter().any(|n1| n1.value_equals(n2));
                if !contains {
                    list.push(n2.clone());
                }
            }
            YamlNode {
                kind: NodeKind::List(list),
                span: o1.span,
            }
        }
        _ => {
            if o2.is_null_scalar() {
                o1
            } else {
                o2
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalars_resolve_like_package_yaml() {
        let node =
            load_yaml_node("a: true\nb: 'true'\nc: 12\nd: ~\ne: 0x1f\nf: 1.5\ng: text\nh:\n")
                .unwrap();
        let get = |k: &str| node.value_at(k).unwrap().scalar().unwrap().clone();
        assert_eq!(get("a"), Scalar::Bool(true));
        assert_eq!(get("b"), Scalar::String("true".into()));
        assert_eq!(get("c"), Scalar::Int(12));
        assert_eq!(get("d"), Scalar::Null);
        assert_eq!(get("e"), Scalar::Int(31));
        assert_eq!(get("f"), Scalar::Float(1.5));
        assert_eq!(get("g"), Scalar::String("text".into()));
        assert_eq!(get("h"), Scalar::Null);
    }

    #[test]
    fn duplicate_keys_and_spans() {
        let error = load_yaml_node("a: 1\na: 2\n").unwrap_err();
        assert_eq!(error.message, "Duplicate mapping key.");
        assert_eq!(error.span.unwrap().start, 5);

        let text = "linter:\n  rules:\n    - avoid_print\n";
        let node = load_yaml_node(text).unwrap();
        let rule = &node
            .value_at("linter")
            .unwrap()
            .value_at("rules")
            .unwrap()
            .as_list()
            .unwrap()[0];
        assert_eq!(&text[rule.span.start..rule.span.end], "avoid_print");
        assert_eq!((rule.span.line, rule.span.column), (2, 6));
    }
}
