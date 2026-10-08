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
//! | [`patterns_part4`] | 10408-10998 | `Patterns:` (fourth part) |
//! | [`sound_flow_analysis_part1`] | 10999-12238 | `Sound flow analysis:` (first part) |

mod common;

mod api_part1;
mod patterns_part1;
mod patterns_part4;
mod sound_flow_analysis_part1;
mod state_part1;
