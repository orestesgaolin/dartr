// Dart source: pkg/_fe_analyzer_shared/test/exhaustiveness/ (env.dart, utils.dart)

//! Test support for the exhaustiveness tests. Each test file includes it with
//! `mod exhaustiveness_support;`.
#![allow(dead_code, unused_imports, unused_macros)]

pub mod env;
#[macro_use]
pub mod utils;

pub use env::TestEnvironment;
pub use utils::{
    Collector, Obj, expect_eq, expect_exhaustive, expect_exhaustive_only_all,
    expect_never_exhaustive, expect_not_exhaustive, parse_spaces,
};
