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
//! | [`reachability`] | 3593-3720 | `Reachability` |
//! | [`state_part1`] | 3721-3884 | `State` (first part) |
//! | [`state_part2`] | 3884-4900 | `State` (second part) |
//! | [`join_part1`] | 4902-5450 | `joinPromotionChains`, `joinTypesOfInterest`, `join`, `inheritTested` |
//! | [`patterns_part1`] | 7038-7330 | `Patterns:` (first part) |

mod common;

mod api_part1;
mod join_part1;
mod patterns_part1;
mod reachability;
mod state_part1;
mod state_part2;
