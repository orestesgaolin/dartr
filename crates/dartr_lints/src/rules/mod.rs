// Dart source: pkg/linter/lib/src/rules.dart
use crate::{LinterContext, RuleVisitorRegistry};
pub mod a_f;
pub mod batch_a1;
pub mod batch_a2;
pub mod batch_a3;
pub mod g_p;
pub mod q_z;
pub fn register(
    name: &str,
    registry: &mut RuleVisitorRegistry,
    context: &LinterContext<'_>,
) -> bool {
    a_f::register(name, registry, context)
        || g_p::register(name, registry, context)
        || q_z::register(name, registry, context)
        || batch_a1::register(name, registry, context)
        || batch_a2::register(name, registry, context)
        || batch_a3::register(name, registry, context)
}
pub fn implemented_rules() -> Vec<&'static str> {
    [
        a_f::RULES,
        g_p::RULES,
        q_z::RULES,
        batch_a1::RULES,
        batch_a2::RULES,
        batch_a3::RULES,
    ]
    .concat()
}

/// Rules that support the legacy parse-only runner.
pub fn parsed_rules() -> Vec<&'static str> {
    [a_f::RULES, g_p::RULES, q_z::RULES].concat()
}
