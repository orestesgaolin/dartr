// Dart source: dart_style lib/src/short/line_splitting/rule_set.dart

use super::super::arena::{Arena, RuleId};
use super::super::rule::Rule;

/// The value of an unbound rule in [RuleSet] (Dart `null`).
const UNBOUND: i32 = i32::MIN;

/// An optimized data structure for storing a set of values for some rules.
///
/// This conceptually behaves roughly like a `Map<Rule, int>`, but is much
/// faster since it avoids hashing. Instead, it assumes the line splitter has
/// provided an ordered list of [Rule]s and that each rule's [index] field has
/// been set to point to the rule in the list.
///
/// Internally, this then just stores the values in a sparse list whose indices
/// are the indices of the rules.
#[derive(Clone, PartialEq, Eq)]
pub struct RuleSet {
    values: Vec<i32>,
}

#[inline(always)]
fn index_of(arena: &Arena, rule: RuleId) -> usize {
    arena.rule(rule).index.unwrap() as usize
}

impl RuleSet {
    pub fn new(num_rules: usize) -> RuleSet {
        RuleSet {
            values: vec![UNBOUND; num_rules],
        }
    }

    /// Returns `true` of [rule] is bound in this set.
    #[inline]
    pub fn contains(&self, arena: &Arena, rule: RuleId) -> bool {
        // Treat hardened rules as implicitly bound.
        if arena.rule(rule).is_hardened() {
            return true;
        }

        self.values[index_of(arena, rule)] != UNBOUND
    }

    /// Gets the bound value for [rule] or [Rule::UNSPLIT] if it is not bound.
    #[inline]
    pub fn get_value(&self, arena: &Arena, rule: RuleId) -> i32 {
        let r = arena.rule(rule);

        // Hardened rules are implicitly bound.
        if r.is_hardened() {
            return r.fully_split_value();
        }

        let value = self.values[r.index.unwrap() as usize];
        if value != UNBOUND {
            return value;
        }

        Rule::UNSPLIT
    }

    /// The value of the rule at [position] in the splitter's rules, or `None`
    /// if it is unbound.
    #[inline(always)]
    pub fn value_at(&self, position: usize) -> Option<i32> {
        let value = self.values[position];
        if value == UNBOUND { None } else { Some(value) }
    }

    /// Invokes [callback] for each rule in [rules] with the rule's value, which
    /// will be `null` if it is not bound.
    #[inline]
    pub fn for_each(&self, rules: &[RuleId], mut callback: impl FnMut(RuleId, i32)) {
        for (i, &rule) in rules.iter().enumerate() {
            let value = self.values[i];
            if value != UNBOUND {
                callback(rule, value);
            }
        }
    }

    /// Binds [rule] to [value] then checks to see if that violates any
    /// constraints.
    ///
    /// Returns `true` if all constraints between the bound rules are valid. Even
    /// if not, this still modifies the [RuleSet].
    ///
    /// If an unbound rule gets constrained to `-1` (meaning it must split, but
    /// can split any way it wants), invokes [on_split_rule] with it.
    pub fn try_bind(
        &mut self,
        arena: &Arena,
        rule: RuleId,
        value: i32,
        on_split_rule: &mut dyn FnMut(RuleId),
    ) -> bool {
        debug_assert!(!arena.rule(rule).is_hardened());

        self.values[index_of(arena, rule)] = value;

        // Test this rule against the other rules being bound.
        let r = arena.rule(rule);
        for i in 0..r.constrained_rule_count() {
            let (other, constraints) = r.constrained_rule_at(i);
            let o = arena.rule(other);

            // Hardened rules are implicitly bound.
            let other_value = if o.is_hardened() {
                o.fully_split_value()
            } else {
                self.values[o.index.unwrap() as usize]
            };

            let constraint = arena.apply_constraints(constraints, value, other);

            if other_value == UNBOUND {
                // The other rule is unbound, so see if we can constrain it eagerly to
                // a value now.
                if constraint == Some(Rule::MUST_SPLIT) {
                    // If we know the rule has to split and there's only one way it can,
                    // just bind that.
                    if o.num_values() == 2 {
                        if !self.try_bind(arena, other, 1, on_split_rule) {
                            return false;
                        }
                    } else {
                        on_split_rule(other);
                    }
                } else if let Some(constraint) = constraint {
                    // Bind the other rule to its value and recursively propagate its
                    // constraints.
                    if !self.try_bind(arena, other, constraint, on_split_rule) {
                        return false;
                    }
                }
            } else {
                // It's already bound, so see if the new rule's constraint disallows
                // that value.
                if constraint == Some(Rule::MUST_SPLIT) {
                    if other_value == Rule::UNSPLIT {
                        return false;
                    }
                } else if let Some(constraint) = constraint {
                    if other_value != constraint {
                        return false;
                    }
                }

                // See if the other rule's constraint allows us to use this value.
                let constraint = arena.constrain(other, other_value, rule);
                if constraint == Some(Rule::MUST_SPLIT) {
                    if value == Rule::UNSPLIT {
                        return false;
                    }
                } else if let Some(constraint) = constraint {
                    if value != constraint {
                        return false;
                    }
                }
            }
        }

        true
    }
}

/// For each chunk, this tracks if it has been split and, if so, what the
/// chosen column is for the following line.
///
/// Internally, this uses a list where each element corresponds to the column
/// of the chunk at that index in the chunk list, or `-1` if that chunk did not
/// split. This had about a 10% perf improvement over using a [Set] of splits.
#[derive(Clone)]
pub struct SplitSet {
    columns: Vec<i32>,

    /// The cost of the solution that led to these splits.
    cost: i32,
}

impl SplitSet {
    /// Creates a new empty split set for a line with [num_chunks].
    pub fn new(num_chunks: usize) -> SplitSet {
        SplitSet {
            columns: vec![-1; num_chunks],
            cost: 0,
        }
    }

    /// Marks the chunk at [index] as starting at [column].
    #[inline]
    pub fn add(&mut self, index: usize, column: i32) {
        self.columns[index] = column;
    }

    /// Returns `true` if the chunk at [index] should be split.
    #[inline]
    pub fn should_split_at(&self, index: usize) -> bool {
        index < self.columns.len() && self.columns[index] != -1
    }

    /// Gets the zero-based starting column for the chunk at [index].
    #[inline]
    pub fn get_column(&self, index: usize) -> i32 {
        self.columns[index]
    }

    /// The cost of the solution that led to these splits.
    #[inline]
    pub fn cost(&self) -> i32 {
        self.cost
    }

    /// Sets the resulting [cost] for the splits.
    ///
    /// This can only be called once.
    pub fn set_cost(&mut self, cost: i32) {
        self.cost = cost;
    }
}
