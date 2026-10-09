// Ported from pkg/analyzer/lib/src/util/yaml.dart.
//! Analyzer YAML helpers and byte-span adapter for package:yaml (`dartr_yaml`).
//! `dartr_yaml` owns scanning, parsing, construction and syntax errors.
//! This module keeps the analyzer's merge, valueAt and toBool behavior.

/// A range of the source text, in UTF-8 bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    /// Zero-based line of [start].
    pub line: usize,
    /// Zero-based column in UTF-16 code units of [start].
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
    /// Exact Dart range, including spans which end between surrogate code units.
    pub source_span: Option<dartr_yaml::FileSpan>,
}
impl YamlError {
    pub fn utf16_range(&self, text: &str) -> Option<(usize, usize)> {
        if let Some(span) = self.source_span {
            return Some((span.start.offset, span.length()));
        }
        self.span.map(|span| {
            (
                text[..span.start].encode_utf16().count(),
                text[span.start..span.end].encode_utf16().count(),
            )
        })
    }
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
    let source = dartr_yaml::SourceFile::new(text);
    fn span(source: &dartr_yaml::SourceFile, span: dartr_yaml::FileSpan) -> Span {
        Span {
            start: source.byte_offset(span.start.offset),
            end: source.byte_offset(span.end.offset),
            line: span.start.line,
            column: span.start.column,
        }
    }
    fn convert(source: &dartr_yaml::SourceFile, node: dartr_yaml::YamlNode) -> YamlNode {
        let kind = match node.kind {
            dartr_yaml::NodeKind::Scalar(value) => NodeKind::Scalar(match value {
                dartr_yaml::Scalar::Null => Scalar::Null,
                dartr_yaml::Scalar::Bool(value) => Scalar::Bool(value),
                dartr_yaml::Scalar::Int(value) => Scalar::Int(value),
                dartr_yaml::Scalar::Float(value) => Scalar::Float(value),
                dartr_yaml::Scalar::String(value) => Scalar::String(value),
            }),
            dartr_yaml::NodeKind::List(nodes) => NodeKind::List(
                nodes
                    .into_iter()
                    .map(|node| convert(source, node))
                    .collect(),
            ),
            dartr_yaml::NodeKind::Map(entries) => NodeKind::Map(
                entries
                    .into_iter()
                    .map(|(key, value)| (convert(source, key), convert(source, value)))
                    .collect(),
            ),
        };
        YamlNode {
            kind,
            span: span(source, node.span),
        }
    }
    dartr_yaml::load_yaml_node(text)
        .map(|node| convert(&source, node))
        .map_err(|error| YamlError {
            message: error.message,
            span: error
                .runtime_error
                .is_none()
                .then(|| span(&source, error.span)),
            source_span: error.runtime_error.is_none().then_some(error.span),
        })
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
