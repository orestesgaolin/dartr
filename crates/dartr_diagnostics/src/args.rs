//! Diagnostic arguments and their conversion to text.
//!
//! Port of `convertTypeNames` in `pkg/analyzer/lib/src/error/listener.dart`.
//!
//! The analyzer accepts `String`, `int`, `Uri`, `DartType` and `Element`
//! arguments (and `Object` for lints). Strings, ints and URIs use
//! `toString()`. Types and elements use their display string. If two or more
//! type/element arguments have the same display string, the analyzer adds
//! the location of the elements with the same name, so that the message is
//! not ambiguous, and adds one context message per element.
//!
//! This crate does not know the element model, so a type or element argument
//! carries its display string and the elements it refers to ([`ElementRef`]).
//! The semantic crates compute them.

use crate::diagnostic::DiagnosticMessage;

/// An element referenced by a type or element argument. Only used to make
/// messages with equal display strings unambiguous.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElementRef {
    /// An identity for the element: two refs with the same `id` are the same
    /// element.
    pub id: u64,
    /// `element.name`; `None` for unnamed elements.
    pub name: Option<String>,
    /// Whether the element is an extension (unnamed extensions are shown as
    /// `<unnamed extension>`).
    pub is_extension: bool,
    /// `element.nonSynthetic.name`.
    pub non_synthetic_name: Option<String>,
    /// Whether `element.nonSynthetic` is an extension.
    pub non_synthetic_is_extension: bool,
    /// The full path of the source that declares `element.nonSynthetic`.
    pub source_path: String,
    /// Offset of the name of `element.nonSynthetic`, the offset of its first
    /// token, or -1.
    pub offset: i64,
    /// Length of the name, or 0.
    pub length: i64,
}

/// A `DartType` argument.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeArg {
    /// `type.getDisplayString(preferTypeAlias: true)`.
    pub display: String,
    /// The named elements in the type (interface types, recursively through
    /// type arguments, function and record types), in Dart's order.
    pub elements: Vec<ElementRef>,
}

impl TypeArg {
    /// A type argument without element information.
    pub fn new(display: impl Into<String>) -> TypeArg {
        TypeArg {
            display: display.into(),
            elements: Vec::new(),
        }
    }
}

impl From<&str> for TypeArg {
    fn from(s: &str) -> TypeArg {
        TypeArg::new(s)
    }
}

/// An `Element` argument.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElementArg {
    /// `element.displayString()`.
    pub display: String,
    pub element: Option<ElementRef>,
}

impl ElementArg {
    /// An element argument without element information.
    pub fn new(display: impl Into<String>) -> ElementArg {
        ElementArg {
            display: display.into(),
            element: None,
        }
    }
}

impl From<&str> for ElementArg {
    fn from(s: &str) -> ElementArg {
        ElementArg::new(s)
    }
}

/// One argument of a diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiagnosticArg {
    String(String),
    Int(i64),
    /// A URI, as text (`Uri.toString()`).
    Uri(String),
    /// Any other object, as `toString()` (lint codes).
    Object(String),
    Type(TypeArg),
    Element(ElementArg),
}

impl DiagnosticArg {
    /// The text of the argument, without disambiguation.
    pub fn text(&self) -> String {
        match self {
            DiagnosticArg::String(s) | DiagnosticArg::Uri(s) | DiagnosticArg::Object(s) => {
                s.clone()
            }
            DiagnosticArg::Int(i) => i.to_string(),
            DiagnosticArg::Type(t) => t.display.clone(),
            DiagnosticArg::Element(e) => e.display.clone(),
        }
    }
}

impl From<&str> for DiagnosticArg {
    fn from(s: &str) -> Self {
        DiagnosticArg::String(s.to_string())
    }
}

impl From<String> for DiagnosticArg {
    fn from(s: String) -> Self {
        DiagnosticArg::String(s)
    }
}

impl From<i64> for DiagnosticArg {
    fn from(i: i64) -> Self {
        DiagnosticArg::Int(i)
    }
}

impl From<TypeArg> for DiagnosticArg {
    fn from(t: TypeArg) -> Self {
        DiagnosticArg::Type(t)
    }
}

impl From<ElementArg> for DiagnosticArg {
    fn from(e: ElementArg) -> Self {
        DiagnosticArg::Element(e)
    }
}

/// Converts `arguments` to text, like `convertTypeNames`.
///
/// Returns the text of each argument and the context messages that the
/// analyzer adds for ambiguous type and element names.
pub fn convert_type_names(arguments: &[DiagnosticArg]) -> (Vec<String>, Vec<DiagnosticMessage>) {
    let mut texts: Vec<String> = arguments.iter().map(DiagnosticArg::text).collect();

    struct ToConvert<'a> {
        index: usize,
        display: &'a str,
        elements: Vec<&'a ElementRef>,
    }

    // Groups in first-seen order (Dart map iteration order).
    let mut groups: Vec<(&str, Vec<ToConvert>)> = Vec::new();
    for (index, argument) in arguments.iter().enumerate() {
        let item = match argument {
            DiagnosticArg::Type(t) => ToConvert {
                index,
                display: &t.display,
                elements: t
                    .elements
                    .iter()
                    .filter(|e| e.name.as_deref().is_some_and(|n| !n.is_empty()))
                    .collect(),
            },
            DiagnosticArg::Element(e) => ToConvert {
                index,
                display: &e.display,
                elements: e.element.iter().collect(),
            },
            _ => continue,
        };
        match groups.iter_mut().find(|(d, _)| *d == item.display) {
            Some((_, g)) => g.push(item),
            None => groups.push((item.display, vec![item])),
        }
    }

    const UNNAMED_EXTENSION: &str = "<unnamed extension>";
    const UNNAMED: &str = "<unnamed>";

    let mut messages = Vec::new();
    for (_, group) in &groups {
        if group.len() == 1 {
            continue;
        }
        // name -> distinct element ids.
        let mut name_to_ids: Vec<(String, Vec<u64>)> = Vec::new();
        for item in group {
            for element in &item.elements {
                let name = element.name.clone().unwrap_or_else(|| {
                    if element.is_extension {
                        UNNAMED_EXTENSION
                    } else {
                        UNNAMED
                    }
                    .to_string()
                });
                match name_to_ids.iter_mut().find(|(n, _)| *n == name) {
                    Some((_, ids)) => {
                        if !ids.contains(&element.id) {
                            ids.push(element.id);
                        }
                    }
                    None => name_to_ids.push((name, vec![element.id])),
                }
            }
        }
        for item in group {
            let mut buffer: Option<String> = None;
            for element in &item.elements {
                let name = element.non_synthetic_name.clone().unwrap_or_else(|| {
                    if element.non_synthetic_is_extension {
                        UNNAMED_EXTENSION
                    } else {
                        UNNAMED
                    }
                    .to_string()
                });
                let source_path = &element.source_path;
                let ambiguous = name_to_ids
                    .iter()
                    .find(|(n, _)| *n == name)
                    .is_some_and(|(_, ids)| ids.len() > 1);
                if ambiguous {
                    match &mut buffer {
                        None => buffer = Some(format!("where {name} is defined in {source_path}")),
                        Some(b) => {
                            b.push_str(&format!(", {name} is defined in {source_path}"));
                        }
                    }
                }
                messages.push(DiagnosticMessage {
                    file_path: source_path.clone(),
                    message: format!("{name} is defined in {source_path}"),
                    offset: element.offset,
                    length: element.length,
                    url: None,
                });
            }
            texts[item.index] = match buffer {
                Some(b) => format!("{} ({b})", item.display),
                None => item.display.to_string(),
            };
        }
    }
    (texts, messages)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class(id: u64, name: &str, path: &str) -> ElementRef {
        ElementRef {
            id,
            name: Some(name.into()),
            is_extension: false,
            non_synthetic_name: Some(name.into()),
            non_synthetic_is_extension: false,
            source_path: path.into(),
            offset: 6,
            length: name.len() as i64,
        }
    }

    #[test]
    fn same_display_name_is_disambiguated() {
        let a = TypeArg {
            display: "A".into(),
            elements: vec![class(1, "A", "/a.dart")],
        };
        let b = TypeArg {
            display: "A".into(),
            elements: vec![class(2, "A", "/b.dart")],
        };
        let (texts, messages) =
            convert_type_names(&[DiagnosticArg::Type(a), DiagnosticArg::Type(b)]);
        assert_eq!(
            texts,
            [
                "A (where A is defined in /a.dart)",
                "A (where A is defined in /b.dart)"
            ]
        );
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].message, "A is defined in /b.dart");
    }

    #[test]
    fn distinct_display_names_are_kept() {
        let (texts, messages) = convert_type_names(&[
            DiagnosticArg::Type(TypeArg::new("int")),
            DiagnosticArg::Int(3),
            DiagnosticArg::Type(TypeArg::new("String")),
        ]);
        assert_eq!(texts, ["int", "3", "String"]);
        assert!(messages.is_empty());
    }
}
