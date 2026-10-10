// Dart source: pkg/analysis_server/lib/src/lsp/client_configuration.dart

//! The `dart` settings of the client (`workspace/configuration`): global
//! settings and settings per workspace folder (Dart
//! `LspClientConfiguration`). dartr reads only the settings of the features
//! it implements.

use serde_json::{Map, Value};

/// Dart `LspClientConfiguration`: the global settings and the settings of
/// each workspace folder.
#[derive(Clone, Debug, Default)]
pub struct LspClientConfiguration {
    /// Dart `_globalSettings`.
    global: Map<String, Value>,
    /// Dart `_resourceSettings`: the folder (without trailing separators)
    /// and its settings.
    resource_settings: Vec<(String, Map<String, Value>)>,
}

/// Dart `LspResourceClientConfiguration`: the settings of a workspace
/// folder with the global settings as fallback, or the global settings.
#[derive(Clone, Copy, Debug)]
pub struct LspResourceClientConfiguration<'a> {
    settings: &'a Map<String, Value>,
    fallback: Option<&'a Map<String, Value>>,
}

impl LspResourceClientConfiguration<'_> {
    fn setting(&self, name: &str) -> Option<&Value> {
        self.settings.get(name)
    }

    /// Dart `enableSdkFormatter`: whether the SDK formatter is enabled
    /// (default `true`).
    pub fn enable_sdk_formatter(&self) -> bool {
        self.setting("enableSdkFormatter")
            .and_then(Value::as_bool)
            .or_else(|| {
                self.fallback
                    .and_then(|f| f.get("enableSdkFormatter"))
                    .and_then(Value::as_bool)
            })
            .unwrap_or(true)
    }

    fn setting_or_fallback(&self, name: &str) -> Option<&Value> {
        self.setting(name)
            .or_else(|| self.fallback.and_then(|f| f.get(name)))
    }

    /// Dart `completeFunctionCalls` (default `false`).
    pub fn complete_function_calls(&self) -> bool {
        self.setting("completeFunctionCalls")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    /// Dart `previewCommitCharacters` (default `false`).
    pub fn preview_commit_characters(&self) -> bool {
        self.setting("previewCommitCharacters")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    /// Dart `enableSnippets` (default `true`).
    pub fn enable_snippets(&self) -> bool {
        if self.setting("enableServerSnippets") == Some(&Value::Bool(false)) {
            return false;
        }
        self.setting_or_fallback("enableSnippets")
            .and_then(Value::as_bool)
            .unwrap_or(true)
    }

    /// Dart `maxCompletionItems` (default 2000).
    pub fn max_completion_items(&self) -> i64 {
        self.setting_or_fallback("maxCompletionItems")
            .and_then(Value::as_i64)
            .unwrap_or(2000)
    }

    /// Dart `preferredDocumentation`: `none`, `summary` or `full`.
    pub fn preferred_documentation(&self) -> &'static str {
        match self.setting("documentation").and_then(Value::as_str) {
            Some("none") => "none",
            Some("summary") => "summary",
            _ => "full",
        }
    }

    /// Dart `lineLength`: the page width of the formatter, `None` for the
    /// default of the formatter.
    pub fn line_length(&self) -> Option<i64> {
        self.setting("lineLength")
            .and_then(Value::as_i64)
            .or_else(|| {
                self.fallback
                    .and_then(|f| f.get("lineLength"))
                    .and_then(Value::as_i64)
            })
    }
}

/// Dart `InlayHintsParameterNamesMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InlayHintsParameterNamesMode {
    None,
    Literal,
    All,
}

/// Dart `LspClientInlayHintsConfiguration`: the `dart.inlayHints` setting
/// (`true`, `false` or a map of the kinds).
#[derive(Clone, Copy, Debug)]
pub struct InlayHintsConfiguration {
    pub dot_shorthand_types: bool,
    pub parameter_names: InlayHintsParameterNamesMode,
    pub parameter_types: bool,
    pub return_types: bool,
    pub type_arguments: bool,
    pub variable_types: bool,
}

impl InlayHintsConfiguration {
    pub fn new(user_preference: Option<&Value>) -> Self {
        let map = user_preference.and_then(Value::as_object);
        let boolean = user_preference.and_then(Value::as_bool);
        let default = boolean.unwrap_or(true);
        let enabled = |key: &str| match map.and_then(|m| m.get(key)) {
            Some(Value::Bool(b)) => *b,
            Some(Value::Object(o)) => o.get("enabled").and_then(Value::as_bool).unwrap_or(default),
            _ => default,
        };
        let default_mode = if default {
            InlayHintsParameterNamesMode::All
        } else {
            InlayHintsParameterNamesMode::None
        };
        let mode_of = |s: &str| match s {
            "none" => InlayHintsParameterNamesMode::None,
            "literal" => InlayHintsParameterNamesMode::Literal,
            _ => InlayHintsParameterNamesMode::All,
        };
        let from_bool = |b: bool| {
            if b {
                InlayHintsParameterNamesMode::All
            } else {
                InlayHintsParameterNamesMode::None
            }
        };
        let parameter_names = match map.and_then(|m| m.get("parameterNames")) {
            Some(Value::Bool(b)) => from_bool(*b),
            Some(Value::String(s)) => mode_of(s),
            Some(Value::Object(o)) => match o.get("enabled") {
                Some(Value::Bool(b)) => from_bool(*b),
                Some(Value::String(s)) => mode_of(s),
                _ => default_mode,
            },
            _ => default_mode,
        };
        InlayHintsConfiguration {
            dot_shorthand_types: enabled("dotShorthandTypes"),
            parameter_names,
            parameter_types: enabled("parameterTypes"),
            return_types: enabled("returnTypes"),
            type_arguments: enabled("typeArguments"),
            variable_types: enabled("variableTypes"),
        }
    }
}

impl LspResourceClientConfiguration<'_> {
    /// Dart `inlayHints` (of the settings, not of the fallback).
    pub fn inlay_hints(&self) -> InlayHintsConfiguration {
        InlayHintsConfiguration::new(self.setting("inlayHints"))
    }
}

/// Dart `_normaliseFolderPath`: without trailing path separators.
fn normalise_folder_path(path: &str) -> String {
    path.trim_end_matches('/').to_string()
}

fn as_map(v: &Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}

impl LspClientConfiguration {
    /// Dart `global`: the global settings as JSON.
    pub fn global(&self) -> LspResourceClientConfiguration<'_> {
        LspResourceClientConfiguration {
            settings: &self.global,
            fallback: None,
        }
    }

    /// The raw global settings.
    pub fn global_value(&self) -> Value {
        Value::Object(self.global.clone())
    }

    /// Dart `replace`: replaces all settings with [global_config] and the
    /// settings of the workspace folders.
    pub fn replace(&mut self, global_config: &Value, workspace_folder_config: &[(String, Value)]) {
        self.global = as_map(global_config);
        self.resource_settings = workspace_folder_config
            .iter()
            .map(|(folder, v)| (normalise_folder_path(folder), as_map(v)))
            .collect();
    }

    /// Dart `forResource`: the settings of the nearest workspace folder of
    /// [resource_path], or the global settings.
    pub fn for_resource(&self, resource_path: &str) -> LspResourceClientConfiguration<'_> {
        match self.get_workspace_folder_path(resource_path) {
            Some(i) => LspResourceClientConfiguration {
                settings: &self.resource_settings[i].1,
                fallback: Some(&self.global),
            },
            None => self.global(),
        }
    }

    /// Dart `_getWorkspaceFolderPath`: the index of the longest workspace
    /// folder that is [resource_path] or contains it.
    fn get_workspace_folder_path(&self, resource_path: &str) -> Option<usize> {
        let normalised = normalise_folder_path(resource_path);
        self.resource_settings
            .iter()
            .enumerate()
            .filter(|(_, (folder, _))| {
                *folder == normalised || dartr_project::paths::is_within(folder, resource_path)
            })
            .max_by_key(|(_, (folder, _))| folder.len())
            .map(|(i, _)| i)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resource_settings_fall_back_to_global() {
        let mut c = LspClientConfiguration::default();
        assert!(c.for_resource("/a/b.dart").enable_sdk_formatter());
        assert_eq!(c.for_resource("/a/b.dart").line_length(), None);
        c.replace(
            &json!({"lineLength": 100, "enableSdkFormatter": false}),
            &[
                ("/a/".to_string(), json!({"lineLength": 40})),
                ("/a/nested".to_string(), json!({"enableSdkFormatter": true})),
                ("/other".to_string(), json!(null)),
            ],
        );
        let a = c.for_resource("/a/lib/x.dart");
        assert_eq!(
            (a.line_length(), a.enable_sdk_formatter()),
            (Some(40), false)
        );
        // The nearest folder wins; missing settings come from the global ones.
        let nested = c.for_resource("/a/nested/x.dart");
        assert_eq!(
            (nested.line_length(), nested.enable_sdk_formatter()),
            (Some(100), true)
        );
        let outside = c.for_resource("/ab/x.dart");
        assert_eq!(
            (outside.line_length(), outside.enable_sdk_formatter()),
            (Some(100), false)
        );
    }
}
