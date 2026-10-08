// Ported from package:yaml 3.1.4 lib/yaml.dart.
// Copyright (c) 2012, the Dart project authors. MIT (see ../LICENSE).

pub mod event;
pub mod loader;
pub mod parser;
mod scalar;
pub mod scanner;
pub mod source;
pub mod style;
pub mod token;
pub mod utils;
pub mod yaml_node;
pub use loader::{
    LoadResult, Loader, YamlDocument, load_yaml_document, load_yaml_documents, load_yaml_node,
    load_yaml_node_with_options, load_yaml_stream,
};
pub use source::{FileSpan, SourceFile, SourceLocation, SourceSpan};
pub use style::{CollectionStyle, ScalarStyle};
pub use utils::YamlWarning;
pub use yaml_node::{NodeKind, Scalar, YamlList, YamlMap, YamlNode, YamlScalar};

/// package:yaml/src/yaml_exception.dart: a message and its source span.
#[derive(Clone, Debug, PartialEq)]
pub struct YamlException {
    /// Non-YAML exception thrown by the pinned Dart implementation, if any.
    /// These exceptions have no YAML source span; `span` is a point location
    /// for Rust callers and must not be used as a YAML diagnostic range.
    pub runtime_error: Option<String>,
    pub message: String,
    pub span: FileSpan,
}
impl std::fmt::Display for YamlException {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(runtime_error) = &self.runtime_error {
            return f.write_str(runtime_error);
        }
        write!(
            f,
            "{} at {}:{}",
            self.message,
            self.span.start.line + 1,
            self.span.start.column + 1
        )
    }
}
impl std::error::Error for YamlException {}

/// Receives each scanner error that recovery handles, in source order.
pub trait ErrorListener {
    fn on_error(&mut self, error: &YamlException);
}
impl<F: FnMut(&YamlException)> ErrorListener for F {
    fn on_error(&mut self, error: &YamlException) {
        self(error);
    }
}

pub fn load_yaml_node_with_listener(
    text: &str,
    listener: &mut dyn ErrorListener,
) -> Result<YamlNode, YamlException> {
    let result = load_yaml_node_with_options(text, true);
    // A fatal error is returned rather than delivered to ErrorListener.
    let recovered = result
        .errors
        .len()
        .saturating_sub(usize::from(result.node.is_none()));
    for error in &result.errors[..recovered] {
        listener.on_error(error);
    }
    result
        .node
        .ok_or_else(|| result.errors.last().cloned().expect("fatal loader error"))
}

/// Per-load counterpart of Dart's global yamlWarningCallback.
/// Keeping callbacks per load avoids process-global mutable state.
pub fn load_yaml_node_with_warning_callback(
    text: &str,
    recover: bool,
    callback: &mut dyn FnMut(&str, Option<FileSpan>),
) -> LoadResult {
    let result = load_yaml_node_with_options(text, recover);
    for warning in &result.warnings {
        callback(&warning.message, warning.span);
    }
    result
}
