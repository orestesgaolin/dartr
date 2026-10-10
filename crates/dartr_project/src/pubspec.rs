//! The parts of `pubspec.yaml` that the project model uses.
//!
//! Ports the relevant parts of `Pubspec.parse` in
//! `pkg/analyzer/lib/src/lint/pub.dart`: a file that is not valid YAML, or
//! not a map, is an empty pubspec. Entry text is `value?.toString()` of the
//! scalar.

use crate::yaml::{self, NodeKind, Scalar, YamlNode};

/// A parsed `pubspec.yaml`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pubspec {
    /// `name`, if it is a scalar.
    pub name: Option<Option<String>>,
    /// `resolution`, if it is a scalar (for example `workspace`).
    pub resolution: Option<Option<String>>,
    /// `workspace`, if it is a list: the text of each scalar entry.
    pub workspace: Option<Vec<Option<String>>>,
    /// `environment: sdk:`, if it is a scalar.
    pub environment_sdk: Option<Option<String>>,
    /// The names of `dependencies` (the keys of the map).
    pub dependencies: Vec<String>,
    /// The names of `dev_dependencies` (the keys of the map).
    pub dev_dependencies: Vec<String>,
}

impl Pubspec {
    /// Parses [text]. Never fails.
    pub fn parse(text: &str) -> Pubspec {
        match yaml::load_yaml_node(text) {
            Ok(node) => Pubspec::from_yaml(&node),
            Err(_) => Pubspec::default(),
        }
    }

    /// Reads and parses the file at [path], or `None` if it cannot be read.
    pub fn read(path: &str) -> Option<Pubspec> {
        crate::fs::read_string(path).map(|text| Pubspec::parse(&text))
    }

    fn from_yaml(node: &YamlNode) -> Pubspec {
        let mut result = Pubspec::default();
        let Some(entries) = node.as_map() else {
            return result;
        };
        for (key, value) in entries {
            let Some(key) = key.scalar() else { continue };
            match key.to_dart_string().as_str() {
                "name" => result.name = scalar_text(value),
                "resolution" => result.resolution = scalar_text(value),
                "workspace" => {
                    result.workspace = value.as_list().map(|nodes| {
                        nodes
                            .iter()
                            .filter_map(|n| n.scalar().map(text_of))
                            .collect()
                    })
                }
                "dependencies" | "dev_dependencies" => {
                    let names: Vec<String> = value
                        .as_map()
                        .map(|m| {
                            m.iter()
                                .filter_map(|(k, _)| k.scalar().map(|k| k.to_dart_string()))
                                .collect()
                        })
                        .unwrap_or_default();
                    if key.to_dart_string() == "dependencies" {
                        result.dependencies = names;
                    } else {
                        result.dev_dependencies = names;
                    }
                }
                "environment" => {
                    if let Some(env) = value.as_map() {
                        let mut sdk = None;
                        for (k, v) in env {
                            if k.scalar().is_some_and(|k| k.to_dart_string() == "sdk") {
                                sdk = scalar_text(v);
                            }
                        }
                        result.environment_sdk = sdk;
                    }
                }
                _ => {}
            }
        }
        result
    }

    /// The text of `resolution`.
    pub fn resolution_text(&self) -> Option<&str> {
        self.resolution.as_ref()?.as_deref()
    }

    /// The text of `name`.
    pub fn name_text(&self) -> Option<&str> {
        self.name.as_ref()?.as_deref()
    }
}

fn text_of(scalar: &Scalar) -> Option<String> {
    match scalar {
        Scalar::Null => None,
        other => Some(other.to_dart_string()),
    }
}

fn scalar_text(node: &YamlNode) -> Option<Option<String>> {
    match &node.kind {
        NodeKind::Scalar(scalar) => Some(text_of(scalar)),
        _ => None,
    }
}
