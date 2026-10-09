// Dart source: dart_style lib/src/short/rule/combinator.dart

use super::super::arena::ChunkId;
use super::{Rule, RuleKind};

/// Handles a list of "combinators".
///
/// A combinator is a keyword followed by a list of nodes used to modify some
/// declaration. It's used for actual hide and show combinators as well as
/// "with" and "implements" clauses in class declarations.
///
/// Combinators can be split in a few different ways:
///
///     // All on one line:
///     import 'animals.dart' show Ant hide Cat;
///
///     // Wrap before each keyword:
///     import 'animals.dart'
///         show Ant, Baboon
///         hide Cat;
///
///     // Wrap either or both of the name lists:
///     import 'animals.dart'
///         show
///             Ant,
///             Baboon
///         hide Cat;
///
/// These are not allowed:
///
///     // Wrap list but not keyword:
///     import 'animals.dart' show
///             Ant,
///             Baboon
///         hide Cat;
///
///     // Wrap one keyword but not both:
///     import 'animals.dart'
///         show Ant, Baboon hide Cat;
///
/// This ensures that when any wrapping occurs, the keywords are always at
/// the beginning of the line.
#[derive(Default)]
pub struct CombinatorRule {
    /// The set of chunks before the combinators.
    combinators: Vec<ChunkId>,

    /// A list of sets of chunks prior to each name in a combinator.
    ///
    /// The outer list is a list of combinators (i.e. "hide", "show", etc.). Each
    /// inner set is the set of names for that combinator.
    names: Vec<Vec<ChunkId>>,
}

impl CombinatorRule {
    /// Dart `CombinatorRule()`.
    pub fn new_rule() -> Rule {
        Rule::from_kind(RuleKind::Combinator(CombinatorRule::default()))
    }

    pub fn num_values(&self) -> i32 {
        let mut count = 2; // No wrapping, or wrap just before each combinator.

        if self.names.len() == 2 {
            count += 3; // Wrap first set of names, second, or both.
        } else {
            count += 1; // Wrap the names.
        }

        count
    }

    pub fn is_split_at_value(&self, value: i32, chunk: ChunkId) -> bool {
        match value {
            // Just split at the combinators.
            1 => self.combinators.contains(&chunk),

            // Split at the combinators and the first set of names.
            2 => self.is_combinator_split(0, chunk),

            // If there is two combinators, just split at the combinators and the
            // second set of names.
            3 => {
                if self.names.len() == 2 {
                    // Two sets of combinators, so just split at the combinators and
                    // the second set of names.
                    return self.is_combinator_split(1, chunk);
                }

                // Split everything.
                true
            }

            _ => true,
        }
    }

    /// Returns `true` if [chunk] is for a combinator or a name in the
    /// combinator at index [combinator].
    fn is_combinator_split(&self, combinator: usize, chunk: ChunkId) -> bool {
        self.combinators.contains(&chunk) || self.names[combinator].contains(&chunk)
    }
}

impl Rule {
    /// Dart `CombinatorRule.addCombinator`: adds a new combinator to the list
    /// of combinators.
    ///
    /// This must be called before adding any names.
    pub fn add_combinator(&mut self, chunk: ChunkId) {
        let RuleKind::Combinator(rule) = &mut self.kind else {
            panic!("not a CombinatorRule");
        };
        if !rule.combinators.contains(&chunk) {
            rule.combinators.push(chunk);
        }
        rule.names.push(Vec::new());
    }

    /// Dart `CombinatorRule.addName`: adds a chunk prior to a name to the
    /// current combinator.
    pub fn add_name(&mut self, chunk: ChunkId) {
        let RuleKind::Combinator(rule) = &mut self.kind else {
            panic!("not a CombinatorRule");
        };
        let names = rule.names.last_mut().unwrap();
        if !names.contains(&chunk) {
            names.push(chunk);
        }
    }
}
