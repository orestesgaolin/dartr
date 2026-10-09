// Dart source: dart_style lib/src/short/rule/argument.dart

//! Dart `ArgumentRule` is a [SplitContainingRule] (see
//! [Rule::splits_on_inner_rules]) with a list of chunks before each
//! argument; [PositionalRule] and [NamedRule] are its subclasses.

use super::super::arena::{ChunkId, RuleId};
use super::{Rule, RuleKind};

/// Rule for handling positional argument lists.
///
/// The number of values is based on the number of arguments and whether or not
/// there are bodies. The first two values are always:
///
/// * 0: Do not split at all.
/// * 1: Split only before the first argument.
///
/// Then there is a value for each argument, to split before that argument.
/// These values work back to front. So, for a two-argument list, value 2 splits
/// after the second argument and value 3 splits after the first.
///
/// Then there is a value that splits before every argument.
///
/// Finally, if there are collection arguments, there is another value that
/// splits before all of the non-collection arguments, but does not split
/// before the collections, so that they can split internally.
pub struct PositionalRule {
    /// The chunks prior to each positional argument.
    arguments: Vec<Option<ChunkId>>,

    /// The number of leading collection arguments.
    ///
    /// This and [trailing_collections] cannot both be positive. If every
    /// argument is a collection, this will be [arguments.length] and
    /// [trailing_collections] will be 0.
    leading_collections: i32,

    /// The number of trailing collections.
    ///
    /// This and [leading_collections] cannot both be positive.
    trailing_collections: i32,
}

impl PositionalRule {
    /// Creates a new rule for a positional argument list.
    ///
    /// [argument_count] is the number of arguments that will be added to the
    /// rule by later calls to [before_argument()].
    ///
    /// If [collection_rule] is given, it is the rule used to split the
    /// collection arguments in the list. It must be provided if
    /// [leading_collections] or [trailing_collections] is non-zero.
    pub fn new_rule(
        collection_rule: Option<RuleId>,
        argument_count: i32,
        leading_collections: i32,
        trailing_collections: i32,
    ) -> Rule {
        let mut rule = Rule::from_kind(RuleKind::Positional(PositionalRule {
            arguments: Vec::new(),
            leading_collections,
            trailing_collections,
        }));

        // Don't split inside collections if there are leading collections and
        // we split before the first argument.
        if leading_collections > 0 {
            rule.add_constraint(1, collection_rule.unwrap(), Rule::UNSPLIT);
        }

        // If we're only splitting before the non-collection arguments, the
        // intent is to split inside the collections, so force that here.
        if leading_collections > 0 || trailing_collections > 0 {
            rule.add_constraint(argument_count + 1, collection_rule.unwrap(), 1);
        }

        // Split before a single argument. If it's in the middle of the collection
        // arguments, don't allow them to split.
        for argument in 0..leading_collections {
            let value = argument_count - argument + 1;
            rule.add_constraint(value, collection_rule.unwrap(), Rule::UNSPLIT);
        }

        for argument in (argument_count - trailing_collections)..argument_count {
            let value = argument_count - argument + 1;
            rule.add_constraint(value, collection_rule.unwrap(), Rule::UNSPLIT);
        }

        rule
    }

    pub fn num_values(&self) -> i32 {
        // Can split before any one argument or none.
        let mut result = self.arguments.len() as i32 + 1;

        // If there are multiple arguments, can split before all of them.
        if self.arguments.len() > 1 {
            result += 1;
        }

        // When there are collection arguments, there are two ways we can split on
        // "all" arguments:
        //
        // - Split on just the non-collection arguments, and force the collection
        //   arguments to split internally.
        // - Split on all of them including the collection arguments, and do not
        //   allow the collection arguments to split internally.
        if self.leading_collections > 0 || self.trailing_collections > 0 {
            result += 1;
        }

        result
    }

    pub fn is_split_at_value(&self, value: i32, chunk: ChunkId) -> bool {
        let chunk = Some(chunk);
        let length = self.arguments.len() as i32;

        // Split only before the first argument. Keep the entire argument list
        // together on the next line.
        if value == 1 {
            return chunk == self.arguments[0];
        }

        // Split before a single argument. Try later arguments before earlier ones
        // to try to keep as much on the first line as possible.
        if value <= length {
            let argument = length - value + 1;
            return chunk == self.arguments[argument as usize];
        }

        // Only split before the non-collection arguments.
        if value == length + 1 {
            for i in 0..self.leading_collections {
                if chunk == self.arguments[i as usize] {
                    return false;
                }
            }

            for i in (length - self.trailing_collections)..length {
                if chunk == self.arguments[i as usize] {
                    return false;
                }
            }

            return true;
        }

        // Split before all of the arguments, even the collections.
        true
    }
}

/// Splitting rule for a list of named arguments or parameters. Its values mean:
///
/// * Do not split at all.
/// * Split only before first argument.
/// * Split before all arguments.
pub struct NamedRule {
    /// The chunks prior to each argument.
    arguments: Vec<Option<ChunkId>>,
}

impl NamedRule {
    /// Creates a new rule for a named argument list.
    ///
    /// If [collection_rule] is given, it is the rule used to split the
    /// collection arguments in the list. It must be provided if
    /// [leading_collections] or [trailing_collections] is non-zero.
    pub fn new_rule(
        collection_rule: Option<RuleId>,
        leading_collections: i32,
        trailing_collections: i32,
    ) -> Rule {
        let mut rule = Rule::from_kind(RuleKind::Named(NamedRule {
            arguments: Vec::new(),
        }));

        if leading_collections > 0 || trailing_collections > 0 {
            // Split only before the first argument. Don't allow the collections to
            // split.
            rule.add_constraint(1, collection_rule.unwrap(), Rule::UNSPLIT);
        }

        rule
    }

    pub fn is_split_at_value(&self, value: i32, chunk: ChunkId) -> bool {
        // Move all arguments to the second line as a unit.
        if value == 1 {
            return Some(chunk) == self.arguments[0];
        }

        // Otherwise, split before all arguments.
        true
    }
}

impl Rule {
    /// Dart `ArgumentRule.beforeArgument` (and
    /// `TypeArgumentRule.beforeArgument`): remembers [chunk] as containing
    /// the split that occurs right before an argument in the list.
    pub fn before_argument(&mut self, chunk: Option<ChunkId>) {
        match &mut self.kind {
            RuleKind::Positional(rule) => rule.arguments.push(chunk),
            RuleKind::Named(rule) => rule.arguments.push(chunk),
            RuleKind::TypeArgument(rule) => rule.before_argument(chunk.unwrap()),
            _ => unreachable!("before_argument on a rule without arguments"),
        }
    }

    /// Dart `PositionalRule.addNamedArgsConstraints`: builds any constraints
    /// from this positional argument rule onto the [rule] used for the
    /// subsequent named arguments in the same argument list.
    ///
    /// The [rule] is normally a [NamedRule] but [PositionalRule] is also used for
    /// the property accesses at the beginning of a call chain, in which case this
    /// is just a [SimpleRule].
    pub fn add_named_args_constraints(&mut self, rule: RuleId) {
        // If the positional args are one-per-line, the named args are too.
        self.constrain_when_fully_split(rule);

        // Otherwise, if there is any split in the positional arguments, don't
        // allow the named arguments on the same line as them.
        let fully_split_value = self.fully_split_value();
        self.add_range_constraint(1, fully_split_value, rule, Rule::MUST_SPLIT);
    }
}
