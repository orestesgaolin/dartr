// Ported from package:yaml 3.1.4 lib/src/utils.dart.
// Copyright (c) 2013, the Dart project authors.
// Copyright (c) 2006, Kirill Simonov. MIT (see ../LICENSE).

use crate::FileSpan;

/// A non-fatal warning required by the YAML specification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct YamlWarning {
    pub message: String,
    pub span: Option<FileSpan>,
}
