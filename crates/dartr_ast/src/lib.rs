//! dartr_ast: the AST node model of the analyzer
//! (`pkg/analyzer/lib/src/dart/ast`).
//!
//! See the module documentation of [`arena`] for the design.

#![allow(clippy::collapsible_if, clippy::too_many_arguments)]

pub mod arena;
pub mod generated;
pub mod node_impl;
pub mod sort;
pub mod token;

pub use arena::{Ast, Concrete, Entity, Id, NodeId, NodeList, NodeMap, NodeType, SubtypeOf, TokenList};
pub use generated::children::{FieldValue, build_node};
pub use generated::nodes::*;
pub use generated::visitor::{AstVisitor, AstVisitorMut, GeneralizingAstVisitor};
pub use node_impl::ParameterKind;
pub mod dump;
pub mod to_source;
#[cfg(feature = "testing")]
pub mod testing;
