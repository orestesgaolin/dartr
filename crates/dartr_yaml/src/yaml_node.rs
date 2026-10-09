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
                }
                _ => a == b,
            },
            (NodeKind::List(a), NodeKind::List(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.value_equals(b))
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
