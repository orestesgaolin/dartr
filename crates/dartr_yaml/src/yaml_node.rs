// Ported from package:yaml 3.1.4 lib/src/yaml_node.dart and equality.dart.
// Copyright (c) 2012, 2014, the Dart project authors. MIT (see ../LICENSE).

use crate::{CollectionStyle, FileSpan, ScalarStyle};

#[derive(Clone, Debug, PartialEq)]
pub enum Scalar {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct YamlNode {
    pub kind: NodeKind,
    pub span: FileSpan,
    pub scalar_style: ScalarStyle,
    pub collection_style: CollectionStyle,
}
#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    Scalar(Scalar),
    List(Vec<YamlNode>),
    /// Mapping entries are stored in source order, including complex keys.
    Map(Vec<(YamlNode, YamlNode)>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct YamlScalar {
    pub value: Scalar,
    pub span: FileSpan,
    pub style: ScalarStyle,
}
#[derive(Clone, Debug, PartialEq)]
pub struct YamlList {
    pub nodes: Vec<YamlNode>,
    pub span: FileSpan,
    pub style: CollectionStyle,
}
#[derive(Clone, Debug, PartialEq)]
pub struct YamlMap {
    pub nodes: Vec<(YamlNode, YamlNode)>,
    pub span: FileSpan,
    pub style: CollectionStyle,
}

impl From<YamlScalar> for YamlNode {
    fn from(node: YamlScalar) -> Self {
        Self {
            kind: NodeKind::Scalar(node.value),
            span: node.span,
            scalar_style: node.style,
            collection_style: CollectionStyle::Any,
        }
    }
}
impl From<YamlList> for YamlNode {
    fn from(node: YamlList) -> Self {
        Self {
            kind: NodeKind::List(node.nodes),
            span: node.span,
            scalar_style: ScalarStyle::Any,
            collection_style: node.style,
        }
    }
}
impl From<YamlMap> for YamlNode {
    fn from(node: YamlMap) -> Self {
        Self {
            kind: NodeKind::Map(node.nodes),
            span: node.span,
            scalar_style: ScalarStyle::Any,
            collection_style: node.style,
        }
    }
}

impl YamlNode {
    pub fn scalar(value: Scalar, span: FileSpan, style: ScalarStyle) -> Self {
        YamlScalar { value, span, style }.into()
    }
    pub fn value_at(&self, key: &str) -> Option<&Self> {
        let NodeKind::Map(entries) = &self.kind else {
            return None;
        };
        entries
            .iter()
            .find(|(k, _)| matches!(&k.kind, NodeKind::Scalar(Scalar::String(s)) if s==key))
            .map(|(_, v)| v)
    }
    /// `deepEquals`, independent of source location or scalar style.
    pub fn value_equals(&self, other: &Self) -> bool {
        self.deep_equals(other, false)
    }

    /// `deepEqualsMap().containsKey`: the duplicate mapping key check of
    /// `Loader._loadMapping`. A `LinkedHashMap` with custom `equals` and
    /// `hashCode` only calls `deepEquals` when the `deepHashCode`s match.
    pub fn key_equals(&self, other: &Self) -> bool {
        self.deep_equals(other, true)
    }

    /// `hashed`: the comparison runs inside a `deepEqualsMap` lookup, so
    /// the `deepHashCode`s of the two values must also match.
    fn deep_equals(&self, other: &Self, hashed: bool) -> bool {
        match (&self.kind, &other.kind) {
            (NodeKind::Scalar(a), NodeKind::Scalar(b)) => match (a, b) {
                (Scalar::Float(a), Scalar::Float(b)) => a == b || (a.is_nan() && b.is_nan()),
                (Scalar::Int(a), Scalar::Float(b)) | (Scalar::Float(b), Scalar::Int(a)) => {
                    // Dart compares integers to doubles without first rounding the integer.
                    b.is_finite()
                        && *b >= i64::MIN as f64
                        && *b <= 9223372036854775808.0
                        && b.fract() == 0.0
                        && *a == *b as i64
                        && !(hashed && int_and_double_hashes_differ(*a, *b))
                }
                _ => a == b,
            },
            (NodeKind::List(a), NodeKind::List(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.deep_equals(b, hashed))
            }
            (NodeKind::Map(a), NodeKind::Map(b)) => {
                // `_mapEquals` looks up each key with `containsKey` (hashed)
                // and compares the values with `equals`.
                a.len() == b.len()
                    && a.iter().all(|(k, v)| {
                        b.iter()
                            .any(|(k2, v2)| k.deep_equals(k2, true) && v.deep_equals(v2, hashed))
                    })
            }
            _ => false,
        }
    }
}

/// Whether the Dart VM gives a different `hashCode` to the `int` [int] and
/// the `double` [double] when `int == double` is true.
///
/// `double.hashCode` uses the `int` hash when the double converts to an
/// `int` and back without change. The VM does this conversion with the
/// hardware instruction: on arm64 (`fcvtzs`) 2^63 saturates to
/// 9223372036854775807, which converts back to 2^63, so the hashes match;
/// on x86 (`cvttsd2si`) 2^63 gives -2^63, so 2^63 gets a `double` hash. Seen
/// with Dart 3.13.3: `9223372036854775808.0.hashCode` is 15352 (the hash of
/// `9223372036854775807`) on macOS arm64 and 279223178035724288 on Linux
/// x86-64. Only this pair has equal values and platform-dependent hashes.
fn int_and_double_hashes_differ(int: i64, double: f64) -> bool {
    cfg!(any(target_arch = "x86_64", target_arch = "x86"))
        && int == i64::MAX
        && double == 9223372036854775808.0
}
