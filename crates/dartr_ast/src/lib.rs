//! dartr_ast: the AST node model of the analyzer
//! (`pkg/analyzer/lib/src/dart/ast`).
//!
//! - [`arena`]: the arena, ids, node lists, parent links, replacing nodes
//!   (the design is described there).
//! - [`generated`]: the 183 node structs ([`NodeKind`]), the node categories
//!   (`Expression`, `Statement`, ...), begin/end tokens, `childEntities`,
//!   `visitChildren` order, `replaceChild`, `removeChild`, the visitors
//!   ([`AstVisitor`], [`AstVisitorMut`], [`GeneralizingAstVisitor`]) and a
//!   reflection table ([`NODE_INFOS`]).
//! - [`node_impl`]: the hand-written parts (`CommentImpl`,
//!   `CompilationUnitImpl`, `@DoNotGenerate` members).
//! - [`copy`]: deep copy of a subtree into another [`Ast`]
//!   ([`copy_subtree`]).
//! - [`dump`]: the `ast` dump of the oracle; [`to_source`]: `toSource()`;
//!   [`precedence`]: `Expression.precedence`.
//! - `testing` (feature `testing`): builds an AST from the oracle `ast`
//!   dump, for round trip tests without a parser.
//!
//! # Regenerating
//!
//! The node schema (`schema/ast.json`) is extracted from the pinned
//! analyzer, and the code in `src/generated` is generated from it:
//!
//! ```text
//! (cd tools/oracle && dart run bin/ast_schema.dart > ../../crates/dartr_ast/schema/ast.json)
//! python3 tools/codegen/gen_ast.py
//! ```

#![allow(clippy::collapsible_if, clippy::too_many_arguments)]

pub mod arena;
pub mod copy;
pub mod doc_comment;
pub mod generated;
pub mod node_impl;
pub mod precedence;
pub mod sort;
pub mod token;

pub use arena::{
    Ast, Concrete, Entity, Id, NodeId, NodeList, NodeMap, NodeType, SubtypeOf, TokenList,
};
pub use copy::{AstCopier, copy_subtree};
pub use generated::children::{FieldValue, build_node};
pub use generated::nodes::*;
pub use generated::visitor::{AstVisitor, AstVisitorMut, GeneralizingAstVisitor};
pub use node_impl::ParameterKind;
pub mod dump;
pub mod extensions;
#[cfg(feature = "testing")]
pub mod testing;
pub mod to_source;
pub mod utilities;
