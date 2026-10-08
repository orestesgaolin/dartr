// Dart source: pkg/_fe_analyzer_shared/test/flow_analysis/flow_analysis_test.dart

//! The flow analysis tests (`flow_analysis_test.dart`), run through the
//! mini AST harness of `dartr_mini_ast` (see its crate documentation for
//! the translation guide).
//!
//! One submodule per range of lines of the Dart file, so that several
//! translators can work in parallel without conflicts. Dart groups are
//! nested modules; Dart tests are `#[test]` functions in Dart order.
//!
//! | module | Dart lines | Dart group |
//! |---|---|---|
//! | [`common`] | 19-35, 13305-13489 | `setUp`, helpers |
//! | [`api_part1`] | 37-1005 | `API` (first part) |
//! | [`state_part1`] | 3721-3884 | `State` (first part) |
//! | [`patterns_part1`] | 7038-7330 | `Patterns:` (first part) |
//! | [`patterns_part3`] | 8703-9666 | `Patterns:` (third part: `Null-assert:` to `Relational pattern:`) |
//! | [`patterns_part3b`] | 9667-10407 | `Patterns:` (third part: `Switch expression:`, `Switch statement:`) |

mod common;

mod api_part1;
mod patterns_part1;
mod patterns_part3;
mod patterns_part3b;
mod state_part1;
