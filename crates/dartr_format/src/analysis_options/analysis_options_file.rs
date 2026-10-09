// Dart source: dart_style lib/src/analysis_options/analysis_options_file.dart
// Dart source: dart_style lib/src/analysis_options/io_file_system.dart
// Dart source: dart_style lib/src/analysis_options/file_system.dart

//! Finds and reads "analysis_options.yaml" files, resolving `include:`.
//!
//! The Dart code abstracts the file system (`FileSystem`, `IOFileSystem`);
//! the only implementation the CLI uses is `IOFileSystem`, ported here as the
//! functions [file_exists], [join], [parent_directory] and [read_file].

use dartr_project::paths;
use dartr_project::yaml::{NodeKind, Scalar, YamlNode, load_yaml_node};

use crate::analysis_options::merge_options::merge;

/// A dynamically-typed value of an analysis options file (the result of
/// lowering a `YamlNode` to Dart values).
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    List(Vec<Value>),
    /// Entries in insertion order.
    Map(Vec<(Value, Value)>),
}

impl Value {
    /// Lowers a YAML node to a Dart value (`YamlNode.value` / `YamlMap`
    /// spread).
    pub fn from_yaml(node: &YamlNode) -> Value {
        match &node.kind {
            NodeKind::Scalar(Scalar::Null) => Value::Null,
            NodeKind::Scalar(Scalar::Bool(v)) => Value::Bool(*v),
            NodeKind::Scalar(Scalar::Int(v)) => Value::Int(*v),
            NodeKind::Scalar(Scalar::Float(v)) => Value::Float(*v),
            NodeKind::Scalar(Scalar::String(v)) => Value::String(v.clone()),
            NodeKind::List(nodes) => Value::List(nodes.iter().map(Value::from_yaml).collect()),
            NodeKind::Map(entries) => Value::Map(
                entries
                    .iter()
                    .map(|(k, v)| (Value::from_yaml(k), Value::from_yaml(v)))
                    .collect(),
            ),
        }
    }

    /// Dart `==` for map keys and set elements: scalars by value, lists and
    /// maps by identity (never equal to a different instance; YAML keys that
    /// are collections are compared structurally here, which only matters for
    /// pathological files).
    pub fn key_equals(&self, other: &Value) -> bool {
        self == other
    }

    /// The value of [key] in a map value.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Map(entries) => entries
                .iter()
                .find(|(k, _)| matches!(k, Value::String(s) if s == key))
                .map(|(_, v)| v),
            _ => None,
        }
    }

    /// Dart `toString()` of the value.
    pub fn to_dart_string(&self) -> String {
        match self {
            Value::Null => "null".into(),
            Value::Bool(v) => v.to_string(),
            Value::Int(v) => v.to_string(),
            Value::Float(v) => Scalar::Float(*v).to_dart_string(),
            Value::String(v) => v.clone(),
            Value::List(items) => {
                let items: Vec<String> = items.iter().map(Value::to_dart_string).collect();
                format!("[{}]", items.join(", "))
            }
            Value::Map(entries) => {
                let entries: Vec<String> = entries
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k.to_dart_string(), v.to_dart_string()))
                    .collect();
                format!("{{{}}}", entries.join(", "))
            }
        }
    }
}

/// The analysis options configuration is a dynamically-typed JSON-like data
/// structure: a [Value::Map].
pub type AnalysisOptions = Value;

/// Exception thrown when an analysis options file contains a "package:" URI in
/// an include and resolving the URI to a file path failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageResolutionException(pub String);

/// The errors of [find_analysis_options].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnalysisOptionsError {
    PackageResolution(PackageResolutionException),
    /// A YAML syntax error (Dart `YamlException`, which dart_style does not
    /// catch): the text of `YamlException.toString()`.
    Yaml(String),
}

impl From<PackageResolutionException> for AnalysisOptionsError {
    fn from(e: PackageResolutionException) -> Self {
        AnalysisOptionsError::PackageResolution(e)
    }
}

/// Dart `IOFileSystem.fileExists`.
fn file_exists(path: &str) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_file())
}

/// Dart `IOFileSystem.join`.
fn join(from: &str, to: &str) -> String {
    paths::join(from, to)
}

/// Dart `IOFileSystem.parentDirectory`.
fn parent_directory(path: &str) -> Option<String> {
    // Make [path] absolute (if not already) so that we can walk outside of the
    // literal path string passed.
    let result = paths::dirname(&paths::absolute_normalized(path)).to_string();

    // If the parent directory is the same as [path], we must be at the root.
    if result == path {
        return None;
    }

    Some(result)
}

/// Dart `IOFileSystem.readFile`.
fn read_file(path: &str) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    String::from_utf8(bytes)
        .map(crate::io::strip_bom)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// Reads an "analysis_options.yaml" file in [directory] or in the nearest
/// surrounding folder that contains that file.
///
/// Stops walking parent directories as soon as it finds one that contains an
/// "analysis_options.yaml" file. If it reaches the root directory without
/// finding one, returns an empty map.
///
/// If an "analysis_options.yaml" file is found, reads it and parses it to a
/// map. If the map contains an `include` key whose value is a list, then
/// reads any of the other referenced YAML files and merges them into this one.
/// Returns the resulting map with the `include` key removed.
///
/// If an IO error occurs when reading a file, then [report_failed_read] is
/// called with the file path, an empty YAML file is used instead, and the
/// process continues.
///
/// If there any "package:" includes, then they are resolved to file paths
/// using [resolve_package_uri].
pub fn find_analysis_options(
    directory: &str,
    report_failed_read: &mut dyn FnMut(&str),
    resolve_package_uri: &mut dyn FnMut(&str) -> Option<String>,
) -> Result<AnalysisOptions, AnalysisOptionsError> {
    let mut directory = directory.to_string();
    loop {
        let options_path = join(&directory, "analysis_options.yaml");
        if file_exists(&options_path) {
            return read_analysis_options(&options_path, report_failed_read, resolve_package_uri);
        }

        match parent_directory(&directory) {
            None => break,
            Some(parent) => directory = parent,
        }
    }

    // If we get here, we didn't find an analysis_options.yaml.
    Ok(Value::Map(Vec::new()))
}

/// Reads the analysis options file at [options_path].
///
/// If there any "package:" includes, then they are resolved to file paths
/// using [resolve_package_uri].
pub fn read_analysis_options(
    options_path: &str,
    report_failed_read: &mut dyn FnMut(&str),
    resolve_package_uri: &mut dyn FnMut(&str) -> Option<String>,
) -> Result<AnalysisOptions, AnalysisOptionsError> {
    let yaml = match read_file(options_path) {
        Ok(text) => match load_yaml_node(&text) {
            Ok(node) => Value::from_yaml(&node),
            Err(error) => {
                return Err(AnalysisOptionsError::Yaml(yaml_exception_text(
                    &text, &error,
                )));
            }
        },
        Err(_) => {
            // Don't crash on failed reads, just report and continue as if the
            // file was empty.
            report_failed_read(options_path);
            Value::Map(Vec::new())
        }
    };

    // If for some reason the YAML isn't a map, consider it malformed and yield
    // a default empty map.
    let Value::Map(mut entries) = yaml else {
        return Ok(Value::Map(Vec::new()));
    };

    let mut options_from_include =
        |include: &str| -> Result<AnalysisOptions, AnalysisOptionsError> {
            let mut include = include.to_string();
            // If the include path is "package:", resolve it to a file path first.
            if uri_scheme(&include) == Some("package") {
                match resolve_package_uri(&include) {
                    Some(file_path) => include = file_path,
                    None => {
                        return Err(PackageResolutionException(format!(
                            "Failed to resolve package URI \"{include}\" in include at \"{options_path}\"."
                        ))
                        .into());
                    }
                }
            }

            // The include path may be relative to the directory containing the
            // current options file.
            let include_path = join(&parent_directory(options_path).unwrap(), &include);

            read_analysis_options(&include_path, report_failed_read, resolve_package_uri)
        };

    // If there is an `include:` key with a String value, then load that and merge
    // it with these options. If there is an `include:` key with a List value,
    // then load each value, merging successive included options, overriding
    // previous results with each set of included options, finally merging with
    // these options.
    let include_index = entries
        .iter()
        .position(|(k, _)| matches!(k, Value::String(s) if s == "include"));
    let Some(include_index) = include_index else {
        return Ok(Value::Map(entries));
    };
    let include = entries[include_index].1.clone();
    let options = match include {
        Value::String(include) => {
            entries.remove(include_index);
            let include_options = options_from_include(&include)?;
            merge(&include_options, &Value::Map(entries))
        }
        Value::List(include_list) => {
            entries.remove(include_index);
            let mut merged_include_options = Value::Map(Vec::new());
            for include in &include_list {
                let Value::String(include) = include else {
                    return Err(PackageResolutionException(format!(
                        "Unsupported \"include\" value in analysis options include list: \"{}\".",
                        include.to_dart_string()
                    ))
                    .into());
                };
                let include_options = options_from_include(include)?;
                merged_include_options = merge(&merged_include_options, &include_options);
            }
            merge(&merged_include_options, &Value::Map(entries))
        }
        Value::Null => Value::Map(entries),
        include => {
            return Err(PackageResolutionException(format!(
                "Unsupported \"include\" value in analysis options: \"{}\".",
                include.to_dart_string()
            ))
            .into());
        }
    };

    Ok(options)
}

/// The scheme of `Uri.tryParse(text)`, if it has one: `ALPHA *( ALPHA /
/// DIGIT / "+" / "-" / "." )` followed by `:`.
fn uri_scheme(text: &str) -> Option<&str> {
    let colon = text.find(':')?;
    let scheme = &text[..colon];
    let mut chars = scheme.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.') {
        return None;
    }
    // Dart lowercases the scheme.
    if scheme.eq_ignore_ascii_case("package") {
        Some("package")
    } else {
        Some(scheme)
    }
}

/// Dart `YamlException.toString()` (`SourceSpanException.toString()`):
/// `Error on line L, column C: message` and the highlighted span.
fn yaml_exception_text(text: &str, error: &dartr_project::yaml::YamlError) -> String {
    match error.utf16_range(text) {
        Some((start, length)) => crate::source_span::span_exception_text(
            text,
            None,
            start,
            start + length,
            &error.message,
        ),
        None => error.message.clone(),
    }
}
