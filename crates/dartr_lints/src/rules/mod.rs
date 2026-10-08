// Dart source: pkg/linter/lib/src/rules.dart
use crate::{LinterContext, RuleVisitorRegistry};
pub mod a_f;
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
}
pub fn implemented_rules() -> Vec<&'static str> {
    [a_f::RULES, g_p::RULES, q_z::RULES].concat()
}
