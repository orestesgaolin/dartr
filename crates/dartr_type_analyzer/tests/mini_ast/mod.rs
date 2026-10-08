// Dart source: pkg/_fe_analyzer_shared/test/mini_ast.dart (and mini_types.dart,
// mini_ir.dart, mini_type_constraint_gatherer.dart)

//! Test support: the Rust port of the shared mini-AST test harness.
//!
//! Each integration test file includes it with `mod mini_ast;`.
#![allow(dead_code, unused_imports, clippy::new_ret_no_self,
    // Keep the Dart structure (index loops, one branch per Dart case).
    clippy::needless_range_loop,
    clippy::if_same_then_else
)]

pub mod harness;
pub mod mini_flow;
pub mod mini_ir;
pub mod mini_type_constraint_gatherer;
pub mod mini_types;
pub mod node;
pub mod operations;
