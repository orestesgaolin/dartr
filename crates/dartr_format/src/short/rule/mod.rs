// Dart source: dart_style lib/src/short/rule/rule.dart

//! [Rule] and its subclasses. The Dart subclasses ([PositionalRule],
//! [NamedRule], [CombinatorRule], [TypeArgumentRule]) are the variants of
//! [RuleKind]; their behavior is in the submodules.

pub mod argument;
pub mod combinator;
pub mod type_argument;

use std::rc::Rc;

use crate::constants::Cost;

use super::arena::{Arena, ChunkId, RuleId};

/// The data of a Dart subclass of [Rule].
pub enum RuleKind {
    /// Dart `Rule`.
    Simple,
    /// Dart `PositionalRule`.
    Positional(argument::PositionalRule),
    /// Dart `NamedRule`.
    Named(argument::NamedRule),
    /// Dart `CombinatorRule`.
    Combinator(combinator::CombinatorRule),
    /// Dart `TypeArgumentRule`.
    TypeArgument(type_argument::TypeArgumentRule),
}

/// Describes a value constraint that one [Rule] places on another rule's
/// values.
///
/// If the first rule's selected value is within [min], [max] (inclusive), then
/// the other rule's value is forced to be [other_value].
#[derive(Clone, Copy, Debug)]
pub struct Constraint {
    /// The minimum of the range of values that rule can have to enable the
    /// constraint.
    pub min: i32,

    /// The maximum of the range of values that rule can have to enable the
    /// constraint.
    pub max: i32,

    /// When this constraint applies, then this is the value the other rule must
    /// have.
    ///
    /// If this is [Rule::FULL_SPLIT_CONSTRAINT], then forces the other rule to
    /// its fully split value. We don't just eagerly store the fully split value
    /// in here because some rules incrementally build the list of Chunks that
    /// are used to determine the the number of values the rule can take and thus
    /// its fully split value isn't known when the rule is created and its
    /// constraints are wired up. In particular, when using [TypeArgumentRule]
    /// for the elements in a collection literal with comments used to control
    /// splitting, it's not easy to eagerly calculate the number of values each
    /// rule will end up having.
    pub other_value: i32,
}

/// A constraint that determines the different ways a related set of chunks may
/// be split.
pub struct Rule {
    pub kind: RuleKind,

    cost: i32,

    /// During line splitting [LineSplitter] sets this to the index of this
    /// rule in its list of rules.
    pub index: Option<u32>,

    /// If `true`, the rule has been "hardened" meaning it's been placed into a
    /// permanent "must fully split" state.
    is_hardened: bool,

    /// Dart `SplitContainingRule._trackInnerRules`: if true, then inner rules
    /// that are written will force this rule to split. Only used by the
    /// [argument::ArgumentRule]s, the other rules always split on inner rules.
    track_inner_rules: bool,

    /// The constraints that this rule implies about other rule values.
    ///
    /// In many cases, if a split occurs inside an expression, surrounding rules
    /// also want to split too. For example, a split in the middle of an argument
    /// forces the entire argument list to also split.
    ///
    /// Also, there are sometimes more bespoke constraints between rules. For
    /// example, positional and named arguments in an argument list each have
    /// their own rules, but the way the positional arguments split restricts the
    /// way the named rules are allowed to split.
    ///
    /// This tracks those relationships. Each key is a rule whose values can be
    /// constrained by this rule. The entry values are a list of [Constraint]
    /// objects. Each specifies that when this rule has a value within a certain
    /// range, the constrained rule's value must be a certain given value.
    ///
    /// A Dart map: the keys are in insertion order.
    constraints: Vec<(RuleId, Vec<Constraint>)>,

    /// Cache of [Arena::all_constrained_rules].
    all_constrained_rules: Option<Rc<Vec<RuleId>>>,
}

impl Rule {
    /// Rule value that splits no chunks.
    ///
    /// Every rule is required to treat this value as fully unsplit.
    pub const UNSPLIT: i32 = 0;

    /// Rule constraint value that means "any value as long as something splits".
    ///
    /// It disallows [UNSPLIT] but allows any other value.
    pub const MUST_SPLIT: i32 = -1;

    /// Rule constraint that means the rule must split to its fully split value.
    ///
    /// This is used instead of the actual full split value because at the point
    /// that the constraint is added, the Rule may not have all of the chunks it
    /// needs to correctly calculate [num_values].
    const FULL_SPLIT_CONSTRAINT: i32 = -2;

    fn with_kind(kind: RuleKind, cost: i32) -> Rule {
        Rule {
            kind,
            cost,
            index: None,
            is_hardened: false,
            track_inner_rules: true,
            constraints: Vec::new(),
            all_constrained_rules: None,
        }
    }

    /// Dart `Rule([cost = Cost.normal])`.
    pub fn new(cost: i32) -> Rule {
        Rule::with_kind(RuleKind::Simple, cost)
    }

    /// Dart `Rule()`.
    pub fn normal() -> Rule {
        Rule::new(Cost::NORMAL)
    }

    /// Creates a new rule that is already fully split.
    pub fn hard() -> Rule {
        // Set the cost to zero since it will always be applied, so there's no
        // point in penalizing it.
        //
        // Also, this avoids doubled counting in literal blocks where there is both
        // a split in the outer chunk containing the block and the inner hard split
        // between the elements or statements.
        let mut rule = Rule::with_kind(RuleKind::Simple, 0);
        rule.harden();
        rule
    }

    pub(crate) fn from_kind(kind: RuleKind) -> Rule {
        Rule::with_kind(kind, Cost::NORMAL)
    }

    /// The number of different states this rule can be in.
    ///
    /// Each state determines which set of chunks using this rule are split and
    /// which aren't. Values range from zero to one minus this. Value zero
    /// always means "no chunks are split" and increasing values by convention
    /// mean increasingly undesirable splits.
    ///
    /// By default, a rule has two values: fully unsplit and fully split.
    #[inline]
    pub fn num_values(&self) -> i32 {
        match &self.kind {
            RuleKind::Simple => 2,
            RuleKind::Positional(rule) => rule.num_values(),
            RuleKind::Named(_) => 3,
            RuleKind::Combinator(rule) => rule.num_values(),
            RuleKind::TypeArgument(rule) => rule.num_values(),
        }
    }

    /// The rule value that forces this rule into its maximally split state.
    ///
    /// By convention, this is the highest of the range of allowed values.
    #[inline]
    pub fn fully_split_value(&self) -> i32 {
        self.num_values() - 1
    }

    #[inline]
    pub fn cost(&self) -> i32 {
        match &self.kind {
            RuleKind::TypeArgument(_) => Cost::TYPE_ARGUMENT,
            _ => self.cost,
        }
    }

    /// If `true`, the rule has been "hardened" meaning it's been placed into a
    /// permanent "must fully split" state.
    #[inline]
    pub fn is_hardened(&self) -> bool {
        self.is_hardened
    }

    /// Whether this rule cares about rules that it contains.
    ///
    /// If `true` then inner rules will constrain this one and force it to split
    /// when they split. Otherwise, it can split independently of any contained
    /// rules.
    #[inline]
    pub fn splits_on_inner_rules(&self) -> bool {
        match &self.kind {
            RuleKind::Positional(_) | RuleKind::Named(_) => self.track_inner_rules,
            _ => true,
        }
    }

    /// Dart `SplitContainingRule.disableSplitOnInnerRules`: disables tracking
    /// inner rules while a collection argument is written.
    pub fn disable_split_on_inner_rules(&mut self) {
        debug_assert!(self.track_inner_rules);
        self.track_inner_rules = false;
    }

    /// Dart `SplitContainingRule.enableSplitOnInnerRules`: re-enables tracking
    /// inner rules.
    pub fn enable_split_on_inner_rules(&mut self) {
        debug_assert!(!self.track_inner_rules);
        self.track_inner_rules = true;
    }

    /// Fixes this rule into a "fully split" state.
    pub fn harden(&mut self) {
        self.is_hardened = true;
    }

    /// Returns `true` if [chunk] should split when this rule has [value].
    #[inline]
    pub fn is_split(&self, value: i32, chunk: ChunkId) -> bool {
        if let RuleKind::TypeArgument(rule) = &self.kind {
            return rule.is_split(value, chunk);
        }

        if self.is_hardened {
            return true;
        }

        if value == Rule::UNSPLIT {
            return false;
        }

        // Let the subclass decide.
        self.is_split_at_value(value, chunk)
    }

    /// Subclasses can override this to determine which values split which chunks.
    ///
    /// By default, this assumes every chunk splits.
    fn is_split_at_value(&self, value: i32, chunk: ChunkId) -> bool {
        match &self.kind {
            RuleKind::Simple | RuleKind::TypeArgument(_) => true,
            RuleKind::Positional(rule) => rule.is_split_at_value(value, chunk),
            RuleKind::Named(rule) => rule.is_split_at_value(value, chunk),
            RuleKind::Combinator(rule) => rule.is_split_at_value(value, chunk),
        }
    }

    /// When this rule has [value], constrains [other] to [other_value].
    pub fn add_constraint(&mut self, value: i32, other: RuleId, other_value: i32) {
        self.add_range_constraint(value, value, other, other_value);
    }

    /// When this rule's value is between [min] and [max] (inclusive), constrains
    /// [other] to [other_value].
    pub fn add_range_constraint(&mut self, min: i32, max: i32, other: RuleId, other_value: i32) {
        let constraint = Constraint {
            min,
            max,
            other_value,
        };
        match self.constraints.iter_mut().find(|(rule, _)| *rule == other) {
            Some((_, list)) => list.push(constraint),
            None => self.constraints.push((other, vec![constraint])),
        }
    }

    /// Constrains [other] to its fully split value when this rule is split in
    /// any way.
    pub fn constrain_when_split(&mut self, other: RuleId) {
        // We want the constraint to apply to any non-zero value, so use an
        // arbitrary but sufficiently large number.
        self.add_range_constraint(1, 100000, other, Rule::FULL_SPLIT_CONSTRAINT);
    }

    /// Constrains [other] to its fully split value when this rule is fully split.
    pub fn constrain_when_fully_split(&mut self, other: RuleId) {
        let value = self.fully_split_value();
        self.add_constraint(value, other, Rule::FULL_SPLIT_CONSTRAINT);
    }

    /// The other [Rule]s that this rule places immediate constraints on.
    #[inline]
    pub fn constrained_rules(&self) -> impl Iterator<Item = RuleId> + '_ {
        self.constraints.iter().map(|(rule, _)| *rule)
    }

    /// Whether this rule has constraints.
    pub fn has_constraints(&self) -> bool {
        !self.constraints.is_empty()
    }

    /// The constraints on [other], if any.
    #[inline]
    fn constraints_on(&self, other: RuleId) -> Option<&[Constraint]> {
        self.constraints
            .iter()
            .find(|(rule, _)| *rule == other)
            .map(|(_, list)| list.as_slice())
    }

    /// The number of constrained rules.
    #[inline]
    pub fn constrained_rule_count(&self) -> usize {
        self.constraints.len()
    }

    /// The constrained rule at [i] and its constraints.
    #[inline]
    pub fn constrained_rule_at(&self, i: usize) -> (RuleId, &[Constraint]) {
        let (rule, list) = &self.constraints[i];
        (*rule, list)
    }
}

impl Arena {
    /// Dart `Rule.constrain`: given that [rule] has [value], determine if
    /// [other]'s value should be constrained.
    ///
    /// Allows relationships between rules like "if I split, then this should
    /// split too". Returns a non-negative value to force [other] to take that
    /// value. Returns -1 to allow [other] to take any non-zero value. Returns
    /// `None` to not constrain other.
    #[inline]
    pub fn constrain(&self, rule: RuleId, value: i32, other: RuleId) -> Option<i32> {
        // By default, any containing rule will be fully split if this one is split.
        if value == Rule::UNSPLIT {
            return None;
        }

        let constrained = self.rule(rule).constraints_on(other)?;
        self.apply_constraints(constrained, value, other)
    }

    /// The part of [constrain] after the lookup of the constraints on [other].
    #[inline]
    pub fn apply_constraints(
        &self,
        constrained: &[Constraint],
        value: i32,
        other: RuleId,
    ) -> Option<i32> {
        if value == Rule::UNSPLIT {
            return None;
        }

        for constraint in constrained {
            if value >= constraint.min && value <= constraint.max {
                if constraint.other_value == Rule::FULL_SPLIT_CONSTRAINT {
                    return Some(self.rule(other).fully_split_value());
                }

                return Some(constraint.other_value);
            }
        }

        None
    }

    /// Dart `Rule.forgetUnusedRules`: discards constraints on any rule that
    /// doesn't have an index.
    ///
    /// This is called by [LineSplitter] after it has indexed all of the in-use
    /// rules. A rule may end up with a constraint on a rule that's no longer
    /// used by any chunk. This can happen if the rule gets hardened, or if it
    /// simply never got used by a chunk. For example, a rule for splitting an
    /// empty list of metadata annotations.
    ///
    /// This removes all of those.
    pub fn forget_unused_rules(&mut self, rule: RuleId) {
        let rules = &mut self.rules;
        let mut constraints = std::mem::take(&mut rules[rule.index()].constraints);
        constraints.retain(|(other, _)| rules[other.index()].index.is_some());
        let rule = &mut rules[rule.index()];
        rule.constraints = constraints;

        // Clear the cached ones too.
        rule.all_constrained_rules = None;
    }

    /// Dart `Rule.allConstrainedRules`: the transitive closure of all of the
    /// rules this rule places constraints on, directly or indirectly,
    /// including itself.
    pub fn all_constrained_rules(&mut self, rule: RuleId) -> Rc<Vec<RuleId>> {
        if let Some(rules) = &self.rule(rule).all_constrained_rules {
            return rules.clone();
        }

        let mut rules = Vec::new();
        let mut seen = rustc_hash::FxHashSet::default();
        self.traverse_constraints(&mut rules, &mut seen, rule);
        let rules = Rc::new(rules);
        self.rule_mut(rule).all_constrained_rules = Some(rules.clone());
        rules
    }

    /// Traverses the constraint graph of [rule] adding everything to [rules].
    fn traverse_constraints(
        &self,
        rules: &mut Vec<RuleId>,
        seen: &mut rustc_hash::FxHashSet<RuleId>,
        rule: RuleId,
    ) {
        if !seen.insert(rule) {
            return;
        }

        rules.push(rule);
        for other in self.rule(rule).constrained_rules() {
            self.traverse_constraints(rules, seen, other);
        }
    }

    /// Dart `Rule()`: adds a new rule with the default cost.
    pub fn new_rule(&mut self) -> RuleId {
        self.add_rule(Rule::normal())
    }

    /// Dart `Rule(cost)`.
    pub fn new_rule_with_cost(&mut self, cost: i32) -> RuleId {
        self.add_rule(Rule::new(cost))
    }

    /// Dart `Rule.hard()`.
    pub fn new_hard_rule(&mut self) -> RuleId {
        self.add_rule(Rule::hard())
    }
}
