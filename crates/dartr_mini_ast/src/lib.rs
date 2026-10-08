//! dartr_mini_ast: test support crate (dev-only). Port of the "mini AST",
//! "mini types" and "mini IR" of `pkg/_fe_analyzer_shared/test/`, used to
//! drive flow analysis (and later the shared type analyzer) in unit tests.
//!
//! | Dart file (`_fe_analyzer_shared/test/...`) | module |
//! |---|---|
//! | `mini_types.dart` | [`mini_types`] |

pub mod mini_types;
