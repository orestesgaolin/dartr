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
pub mod yaml_node;
pub use loader::{
    LoadResult, Loader, YamlDocument, load_yaml_document, load_yaml_documents, load_yaml_node,
    load_yaml_node_with_options, load_yaml_stream,
};
pub use source::{FileSpan, SourceFile, SourceLocation, SourceSpan};
pub use style::{CollectionStyle, ScalarStyle};
pub use yaml_node::{NodeKind, Scalar, YamlList, YamlMap, YamlNode, YamlScalar};

/// package:yaml/src/yaml_exception.dart: a message and its source span.
#[derive(Clone, Debug, PartialEq)]
pub struct YamlException {
    pub message: String,
    pub span: FileSpan,
}
impl std::fmt::Display for YamlException {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
