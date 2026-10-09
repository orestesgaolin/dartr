// Dart source: dart_style lib/src/short/rule/type_argument.dart

use super::super::arena::ChunkId;
use super::{Rule, RuleKind};

/// Rule for splitting a list of type arguments or type parameters. Type
/// parameters split a little differently from normal value argument lists. In
/// particular, this tries harder to avoid splitting before the first type
/// argument since that looks stranger with `<...>` than it does with `(...)`.
///
/// The values for a rule for `n` arguments are:
///
/// * `0`: No splits at all.
/// * `1 ... n`: Split before one argument, starting from the last.
/// * `n + 1`: Split before all arguments.
///
/// If there is only one type argument, the last two cases collapse and there
/// are only two values.
#[derive(Default)]
pub struct TypeArgumentRule {
    /// The chunks prior to each positional type argument.
    arguments: Vec<ChunkId>,
}

impl TypeArgumentRule {
    /// Dart `TypeArgumentRule()`.
    pub fn new_rule() -> Rule {
        Rule::from_kind(RuleKind::TypeArgument(TypeArgumentRule::default()))
    }

    pub fn num_values(&self) -> i32 {
        if self.arguments.len() == 1 {
            2
        } else {
            self.arguments.len() as i32 + 2
        }
    }

    /// Remembers [chunk] as containing the split that occurs right before a type
    /// argument in the list.
    pub fn before_argument(&mut self, chunk: ChunkId) {
        self.arguments.push(chunk);
    }

    pub fn is_split(&self, value: i32, chunk: ChunkId) -> bool {
        // Don't split at all.
        if value == Rule::UNSPLIT {
            return false;
        }

        // Split before every argument.
        if value == self.num_values() - 1 {
            return true;
        }

        // Split before a single argument. Try later arguments before earlier ones
        // to try to keep as much on the first line as possible.
        chunk == self.arguments[self.arguments.len() - value as usize]
    }
}
