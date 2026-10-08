// Dart source: pkg/analyzer/lib/src/dart/ast/to_source_visitor.dart

//! `toSource()`.

use crate::arena::{Ast, NodeId};

/// Dart `AstNode.toSource()`.
pub fn to_source(_ast: &Ast, _id: impl Into<NodeId>) -> String {
    String::new()
}
