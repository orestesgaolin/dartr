// Dart source: pkg/linter/lib/src/rules/unnecessary_ignore.dart
//! The rule has no node processors: when it is enabled, the ignore
//! validator of `dartr_cli` (Dart `IgnoreValidator`) reports the
//! unnecessary ignores after all other diagnostics of a file are known.
use crate::{LinterContext, RuleVisitorRegistry};

pub fn register(_: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {}
