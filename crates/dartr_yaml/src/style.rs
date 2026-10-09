// Ported from package:yaml 3.1.4 lib/src/style.dart.
//
// Copyright (c) 2014, the Dart project authors.
// Copyright (c) 2006, Kirill Simonov.
// Use of this source code is governed by an MIT-style license.

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ScalarStyle {
    #[default]
    Any,
    Plain,
    Literal,
    Folded,
    SingleQuoted,
    DoubleQuoted,
}

impl ScalarStyle {
    pub fn is_quoted(self) -> bool {
        matches!(self, Self::SingleQuoted | Self::DoubleQuoted)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CollectionStyle {
    #[default]
    Any,
    Block,
    Flow,
}
