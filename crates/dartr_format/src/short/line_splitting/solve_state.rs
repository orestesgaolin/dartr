// Dart source: dart_style lib/src/short/line_splitting/solve_state.dart

use std::cell::OnceCell;
use std::cmp::Ordering;

use crate::constants::Indent;

use super::super::arena::{Arena, NestingId, RuleId};
use super::super::line_writer::WriterContext;
use super::super::rule::Rule;
use super::line_splitter::SplitterInfo;
use super::rule_set::{RuleSet, SplitSet};

/// The lazily computed parts of a [SolveState] that are only needed to
/// compare two states for overlap.
///
/// The Dart maps and sets keyed by rules are lists indexed by the position
/// of the rule in [SplitterInfo::rules]; all of their keys are rules of the
/// splitter.
struct OverlapData {
    /// Dart `_boundRulesInUnboundLines`: the bound rules that appear inside
    /// lines also containing unbound rules.
    ///
    /// By appearing in the same line, it means these bound rules may affect the
    /// results of binding those unbound rules. This is used to tell if two
    /// states may diverge by binding unbound rules or not.
    bound_rules_in_unbound_lines: Vec<bool>,

    /// Dart `_constraints`: the constraints the bound rules in this state
    /// have on the remaining unbound rules.
    constraints: Vec<Option<i32>>,

    /// Dart `_unboundConstraints`: the unbound rule values that are
    /// disallowed because they would place invalid constraints on the
    /// currently bound values. The values are sorted.
    ///
    /// For example, say rule A is bound to 1 and B is unbound. If B has value
    /// 1, it constrains A to be 1. Likewise, if B is 2, it constrains A to be
    /// 2 as well. Since we already know A is 1, that means we know B cannot be
    /// bound to value 2. This means B is more limited in this state than it
    /// might be in another state that binds A to a different value.
    ///
    /// It's important to track this, because we can't allow to states to overlap
    /// if one permits more values for some unbound rule than the other does.
    unbound_constraints: Vec<Option<Vec<i32>>>,
}

/// A possibly incomplete solution in the line splitting search space.
///
/// A single [SolveState] binds some subset of the rules to values while
/// leaving the rest unbound. If every rule is bound, the solve state describes
/// a complete solution to the line splitting problem. Even if rules are
/// unbound, a state can also usually be used as a solution by treating all
/// unbound rules as unsplit. (The usually is because a state that constrains
/// an unbound rule to split can't be used with that rule unsplit.)
///
/// From a given solve state, we can explore the search tree to more refined
/// solve states by producing new ones that add more bound rules to the current
/// state.
pub struct SolveState {
    rule_values: RuleSet,

    /// The unbound rules in this state that can be bound to produce new more
    /// refined states.
    ///
    /// Keeping this set small is the key to make the entire line splitter
    /// perform well. If we consider too many rules at each state, our
    /// exploration of the solution space is too branchy and we waste time on
    /// dead end solutions.
    ///
    /// Here is the key insight. The line splitter treats any unbound rule as
    /// being unsplit. This means refining a solution always means taking a rule
    /// that is unsplit and making it split. That monotonically increases the
    /// cost, but may help fit the solution inside the page.
    ///
    /// We want to keep the cost low, so the only reason to consider making a
    /// rule split is if it reduces an overflowing line. It's also the case that
    /// splitting an earlier rule will often reshuffle the rest of the line.
    ///
    /// Taking that into account, the only rules we consider binding to extend a
    /// solve state are *unbound rules inside the first line that is overflowing*.
    /// Even if a line has dozens of rules, this generally keeps the branching
    /// down to a few. It also means rules inside lines that already fit are
    /// never touched.
    ///
    /// There is one other set of rules that go in here. Sometimes a bound rule
    /// in the solve state constrains some other unbound rule to split. In that
    /// case, we also consider that active so we know to not leave it at zero.
    ///
    /// A set: the order does not matter.
    live_rules: Vec<RuleId>,

    /// The set of splits chosen for this state.
    splits: SplitSet,

    /// The number of characters that do not fit inside the page with this set of
    /// splits.
    overflow_chars: i32,

    /// Whether we can treat this state as a complete solution by leaving its
    /// unbound rules unsplit.
    ///
    /// This is generally true but will be false if the state contains any
    /// unbound rules that are constrained to not be zero by other bound rules.
    /// This avoids picking a solution that leaves those rules at zero when they
    /// aren't allowed to be.
    is_complete: bool,

    overlap: OnceCell<OverlapData>,
}

impl SolveState {
    pub fn new(info: &SplitterInfo, ctx: &mut WriterContext, rule_values: RuleSet) -> SolveState {
        let mut state = SolveState {
            rule_values,
            live_rules: Vec::new(),
            splits: SplitSet::new(0),
            overflow_chars: 0,
            is_complete: true,
            overlap: OnceCell::new(),
        };
        state.calculate_splits(info, ctx.arena);
        state.calculate_cost(info, ctx);
        state
    }

    /// The set of splits chosen for this state.
    #[inline]
    pub fn splits(&self) -> &SplitSet {
        &self.splits
    }

    /// The number of characters that do not fit inside the page with this set of
    /// splits.
    #[inline]
    pub fn overflow_chars(&self) -> i32 {
        self.overflow_chars
    }

    /// Gets the value to use for [rule], either the bound value or
    /// [Rule::UNSPLIT] if it isn't bound.
    #[inline]
    pub fn get_value(&self, arena: &Arena, rule: RuleId) -> i32 {
        self.rule_values.get_value(arena, rule)
    }

    /// Returns `true` if this state is a better solution to use as the final
    /// result than [other].
    pub fn is_better_than(&self, other: &SolveState) -> bool {
        // If this state contains an unbound rule that we know can't be left
        // unsplit, we can't pick this as a solution.
        if !self.is_complete {
            return false;
        }

        // Prefer the solution that fits the most in the page.
        if self.overflow_chars != other.overflow_chars {
            return self.overflow_chars < other.overflow_chars;
        }

        // Otherwise, prefer the best cost.
        self.splits.cost() < other.splits.cost()
    }

    /// Determines if this state "overlaps" [other].
    ///
    /// Two states overlap if they currently have the same score and we can tell
    /// for certain that they won't diverge as their unbound rules are bound. If
    /// that's the case, then whichever state is better now (based on their
    /// currently bound rule values) is the one that will always win, regardless
    /// of how they get expanded.
    ///
    /// In other words, their entire expanded solution trees also overlap. In
    /// that case, there's no point in expanding both, so we can just pick the
    /// winner now and discard the other.
    ///
    /// For this to be true, we need to prove that binding an unbound rule won't
    /// affect one state differently from the other. We have to show that they
    /// are parallel.
    ///
    /// Two things could cause this *not* to be the case.
    ///
    /// 1. If one state's bound rules place different constraints on the unbound
    ///    rules than the other.
    ///
    /// 2. If one state's different bound rules are in the same line as an
    ///    unbound rule. That affects the indentation and length of the line,
    ///    which affects the context where the unbound rule is being chosen.
    ///
    /// If neither of these is the case, the states overlap. Returns `<0` if this
    /// state is better, or `>0` if [other] wins. If the states do not overlap,
    /// returns `0`.
    pub fn compare_overlap(&self, other: &SolveState, info: &SplitterInfo, arena: &Arena) -> Ordering {
        if !self.is_overlapping(other, info, arena) {
            return Ordering::Equal;
        }

        // They do overlap, so see which one wins.
        for &rule in &info.rules {
            let value = self.rule_values.get_value(arena, rule);
            let other_value = other.rule_values.get_value(arena, rule);

            if value != other_value {
                return value.cmp(&other_value);
            }
        }

        // The way SolveStates are expanded should guarantee that we never generate
        // the exact same state twice. Getting here implies that that failed.
        panic!("unreachable");
    }

    /// Enqueues more solve states to consider based on this one.
    ///
    /// For each unbound rule in this state that occurred in the first long line,
    /// enqueue solve states that bind that rule to each value it can have and
    /// bind all previous rules to zero. (In other words, try all subsolutions
    /// where that rule becomes the first new rule to split at.)
    pub fn expand(
        &self,
        info: &SplitterInfo,
        ctx: &mut WriterContext,
        enqueue: &mut dyn FnMut(SolveState, &SplitterInfo, &Arena),
    ) {
        let mut unsplit_rules = self.rule_values.clone();

        // Walk down the rules looking for unbound ones to try.
        let mut tried_rules = 0;
        for &rule in &info.rules {
            if self.live_rules.contains(&rule) {
                // We found one worth trying, so try all of its values.
                let mut value = 1;
                while value < ctx.arena.rule(rule).num_values() {
                    let mut bound_rules = unsplit_rules.clone();

                    let mut must_split_rules: Option<Vec<RuleId>> = None;
                    let valid = bound_rules.try_bind(ctx.arena, rule, value, &mut |rule| {
                        must_split_rules.get_or_insert_with(Vec::new).push(rule);
                    });

                    value += 1;

                    // Make sure we don't violate the constraints of the bound rules.
                    if !valid {
                        continue;
                    }

                    let mut state = SolveState::new(info, ctx, bound_rules);

                    // If some unbound rules are constrained to split, remember that.
                    if let Some(must_split_rules) = must_split_rules {
                        state.is_complete = false;
                        for rule in must_split_rules {
                            if !state.live_rules.contains(&rule) {
                                state.live_rules.push(rule);
                            }
                        }
                    }

                    enqueue(state, info, ctx.arena);
                }

                // Stop once we've tried all of the ones we can.
                tried_rules += 1;
                if tried_rules == self.live_rules.len() {
                    break;
                }
            }

            // Fill in previous unbound rules with zero.
            if !self.rule_values.contains(ctx.arena, rule) {
                // Pass a dummy callback because zero will never fail. (If it would
                // have, that rule would already be bound to some other value.)
                if !unsplit_rules.try_bind(ctx.arena, rule, 0, &mut |_| {}) {
                    break;
                }
            }
        }
    }

    /// Returns `true` if [other] overlaps this state.
    fn is_overlapping(&self, other: &SolveState, info: &SplitterInfo, arena: &Arena) -> bool {
        let this = self.overlap_data(info, arena);
        let that = other.overlap_data(info, arena);

        // Lines that contain both bound and unbound rules must have the same
        // bound values.
        if this.bound_rules_in_unbound_lines != that.bound_rules_in_unbound_lines {
            return false;
        }

        for (i, &bound) in this.bound_rules_in_unbound_lines.iter().enumerate() {
            if !bound {
                continue;
            }
            let rule = info.rules[i];
            if self.rule_values.get_value(arena, rule) != other.rule_values.get_value(arena, rule) {
                return false;
            }
        }

        if this.constraints != that.constraints {
            return false;
        }

        if this.unbound_constraints != that.unbound_constraints {
            return false;
        }

        true
    }

    fn overlap_data(&self, info: &SplitterInfo, arena: &Arena) -> &OverlapData {
        self.overlap.get_or_init(|| {
            let bound = self.init_bound_rules(info, arena);
            let bound_rules_in_unbound_lines = self.init_bound_rules_in_unbound_lines(info, &bound);
            let constraints = self.init_constraints(info, arena, &bound);
            let unbound_constraints = self.init_unbound_constraints(info, arena, &bound);
            OverlapData {
                bound_rules_in_unbound_lines,
                constraints,
                unbound_constraints,
            }
        })
    }

    /// Calculates the [SplitSet] for this solve state, assuming any unbound
    /// rules are set to zero.
    fn calculate_splits(&mut self, info: &SplitterInfo, arena: &mut Arena) {
        let chunks = info.chunks;

        // Figure out which expression nesting levels got split and need to be
        // assigned columns.
        let mut is_split = Vec::with_capacity(chunks.len());
        let mut used_nesting_levels: Vec<NestingId> = Vec::new();
        for &chunk in chunks {
            let c = arena.chunk(chunk);
            let split = arena
                .rule(c.rule)
                .is_split(self.rule_values.get_value(arena, c.rule), chunk);
            is_split.push(split);
            if split {
                let nesting = c.nesting;
                if arena.mark_nesting(nesting) {
                    used_nesting_levels.push(nesting);
                    arena.clear_total_used_indent(nesting);
                }
            }
        }

        for &nesting in &used_nesting_levels {
            arena.refresh_total_used_indent(nesting);
        }
        for &nesting in &used_nesting_levels {
            arena.unmark_nesting(nesting);
        }

        let mut splits = SplitSet::new(chunks.len());
        for (i, &chunk) in chunks.iter().enumerate() {
            if is_split[i] {
                let c = arena.chunk(chunk);
                let mut indent = 0;
                if !c.flush_left() {
                    // Add in the chunk's indent.
                    indent = info.block_indentation + c.indent;

                    // And any expression nesting.
                    indent += arena.nesting(c.nesting).total_used_indent();

                    if c.is_block()
                        && arena.indent_block(chunk, |rule| self.rule_values.get_value(arena, rule))
                    {
                        indent += Indent::EXPRESSION;
                    }
                }

                splits.add(i, indent);
            }
        }
        self.splits = splits;
    }

    /// Evaluates the cost (i.e. the relative "badness") of splitting the line
    /// into [lines] physical lines based on the current set of rules.
    fn calculate_cost(&mut self, info: &SplitterInfo, ctx: &mut WriterContext) {
        let chunks = info.chunks;
        let page_width = ctx.page_width;

        // Calculate the length of each line and apply the cost of any spans that
        // get split.
        let mut cost = 0;
        let mut length = 0;

        // The unbound rules in use by the current line. This will be null after
        // the first long line has completed.
        let mut found_overflow_rules = false;
        let mut start = 0;

        // The list of spans that contain chunks that ended up splitting. These are
        // made unique by marking the spans during the run, adding them to this list
        // to be able to unmark them again. We have to keep track of uniqueness to
        // avoid double-counting if more than one split occurs in it.
        let mut split_spans = Vec::new();

        // The nesting level of the chunk that ended the previous line.
        let mut previous_nesting: Option<NestingId> = None;

        for i in 0..chunks.len() {
            let chunk = chunks[i];

            if self.splits.should_split_at(i) {
                self.end_line(info, ctx.arena, page_width, &mut length, &mut start, &mut found_overflow_rules, i);

                let arena = &mut *ctx.arena;
                for &span in &arena.chunks[chunk.index()].spans {
                    let span = &mut arena.spans[span.index()];
                    if !span.is_marked {
                        span.is_marked = true;
                        cost += span.cost;
                    }
                }
                split_spans.extend_from_slice(&arena.chunks[chunk.index()].spans);

                // Do not allow sequential lines to have the same indentation but for
                // different reasons. In other words, don't allow different expressions
                // to claim the same nesting level on subsequent lines.
                //
                // A contrived example would be:
                //
                //     function(inner(
                //         argument), second(
                //         another);
                //
                // For the most part, we prevent this by the constraints on splits.
                // For example, the above can't happen because the split before
                // "argument", forces the split before "second".
                //
                // But there are a couple of squirrely cases where it's hard to prevent
                // by construction. Instead, this outlaws it by penalizing it very
                // heavily if it happens to get this far.
                let nesting = arena.chunk(chunk).nesting;
                let total_indent = arena.nesting(nesting).total_used_indent();
                if let Some(previous) = previous_nesting {
                    if total_indent != 0
                        && total_indent == arena.nesting(previous).total_used_indent()
                        && nesting != previous
                    {
                        self.overflow_chars += 10000;
                    }
                }

                previous_nesting = Some(nesting);

                // Start the new line.
                length = self.splits.get_column(i);
            } else if ctx.arena.chunk(chunk).space_when_unsplit() {
                length += 1;
            }

            if ctx.arena.chunk(chunk).is_block() {
                if self.splits.should_split_at(i) {
                    // Include the cost of the nested block.
                    cost += ctx.format_block(chunk, self.splits.get_column(i)).cost;
                } else {
                    // Include the nested block inline, if any.
                    length += ctx.arena.unsplit_block_length(chunk);
                }
            }

            length += ctx.arena.chunk(chunk).text_length();
        }

        // Add the costs for the rules that have any splits.
        let arena = &*ctx.arena;
        self.rule_values.for_each(&info.rules, |rule, value| {
            if value != Rule::UNSPLIT {
                cost += arena.rule(rule).cost();
            }
        });

        // Unmark spans again so they're ready for another run.
        for span in split_spans {
            ctx.arena.spans[span.index()].is_marked = false;
        }

        // Finish the last line.
        self.end_line(
            info,
            ctx.arena,
            page_width,
            &mut length,
            &mut start,
            &mut found_overflow_rules,
            chunks.len(),
        );

        self.splits.set_cost(cost);
    }

    /// The `endLine` closure of Dart `_calculateCost`.
    #[allow(clippy::too_many_arguments)]
    fn end_line(
        &mut self,
        info: &SplitterInfo,
        arena: &mut Arena,
        page_width: i32,
        length: &mut i32,
        start: &mut usize,
        found_overflow_rules: &mut bool,
        end: usize,
    ) {
        // Track lines that went over the length. It is only rules contained in
        // long lines that we may want to split.
        if *length > page_width {
            self.overflow_chars += *length - page_width;

            // Only try rules that are in the first long line, since we know at
            // least one of them *will* be split.
            if !*found_overflow_rules {
                for i in *start..end {
                    let rule = arena.chunk(info.chunks[i]).rule;
                    if self.add_live_rules(arena, rule) {
                        *found_overflow_rules = true;
                    }
                }
            }
        }

        *start = end;
    }

    /// Adds [rule] and all of the rules it constrains to the set of [live_rules].
    ///
    /// Only does this if [rule] is a valid soft rule. Returns `true` if any new
    /// live rules were added.
    fn add_live_rules(&mut self, arena: &mut Arena, rule: RuleId) -> bool {
        let mut added = false;
        let constrained_rules = arena.all_constrained_rules(rule);
        for &constrained in constrained_rules.iter() {
            if self.rule_values.contains(arena, constrained) {
                continue;
            }

            if !self.live_rules.contains(&constrained) {
                self.live_rules.push(constrained);
            }
            added = true;
        }

        added
    }

    /// Used to lazy initialize [_boundRulesInUnboundLines], which is needed to
    /// compare two states for overlap.
    ///
    /// We do this lazily because the calculation is a bit slow, and is only
    /// needed when we have two states with the same score.
    fn init_bound_rules_in_unbound_lines(&self, info: &SplitterInfo, bound: &[bool]) -> Vec<bool> {
        let mut rules = vec![false; info.rules.len()];
        let mut bound_in_line: Vec<usize> = Vec::new();
        let mut has_unbound = false;

        for i in 0..info.chunks.len() {
            if self.splits.should_split_at(i) {
                if has_unbound {
                    for &position in &bound_in_line {
                        rules[position] = true;
                    }
                }

                bound_in_line.clear();
                has_unbound = false;
            }

            let position = info.chunk_rule_positions[i];
            if bound[position] {
                bound_in_line.push(position);
            } else {
                has_unbound = true;
            }
        }

        if has_unbound {
            for &position in &bound_in_line {
                rules[position] = true;
            }
        }
        rules
    }

    /// Dart `_boundRules` (by position: `true` if bound).
    fn init_bound_rules(&self, info: &SplitterInfo, arena: &Arena) -> Vec<bool> {
        info.rules
            .iter()
            .map(|&rule| self.rule_values.contains(arena, rule))
            .collect()
    }

    /// Used to lazy initializes the [_constraints], which is needed to compare
    /// two states for overlap.
    ///
    /// We do this lazily because the calculation is a bit slow, and is only
    /// needed when we have two states with the same score.
    fn init_constraints(&self, info: &SplitterInfo, arena: &Arena, bound: &[bool]) -> Vec<Option<i32>> {
        let mut constraints = vec![None; info.rules.len()];

        for (position, &bound_rule) in info.rules.iter().enumerate() {
            if !bound[position] {
                continue;
            }
            let rule = arena.rule(bound_rule);
            let value = self.rule_values.get_value(arena, bound_rule);
            for (i, unbound_position) in info.constrained_positions[position].iter().enumerate() {
                let Some(unbound_position) = *unbound_position else {
                    continue;
                };
                if bound[unbound_position] {
                    continue;
                }

                let (unbound, list) = rule.constrained_rule_at(i);
                if let Some(constraint) = arena.apply_constraints(list, value, unbound) {
                    constraints[unbound_position] = Some(constraint);
                }
            }
        }

        constraints
    }

    /// Used to lazy initialize the [_unboundConstraints], which is needed to
    /// compare two states for overlap.
    ///
    /// We do this lazily because the calculation is a bit slow, and is only
    /// needed when we have two states with the same score.
    fn init_unbound_constraints(
        &self,
        info: &SplitterInfo,
        arena: &Arena,
        bound: &[bool],
    ) -> Vec<Option<Vec<i32>>> {
        let mut unbound_constraints: Vec<Option<Vec<i32>>> = vec![None; info.rules.len()];
        for (position, &unbound) in info.rules.iter().enumerate() {
            if bound[position] {
                continue;
            }

            let unbound_rule = arena.rule(unbound);
            for (i, bound_position) in info.constrained_positions[position].iter().enumerate() {
                let Some(bound_position) = *bound_position else {
                    continue;
                };
                if !bound[bound_position] {
                    continue;
                }

                let (bound_rule, list) = unbound_rule.constrained_rule_at(i);
                let bound_value = self.rule_values.get_value(arena, bound_rule);

                for value in 0..unbound_rule.num_values() {
                    let constraint = arena.apply_constraints(list, value, bound_rule);

                    // If the unbound rule doesn't place any constraint on this bound
                    // rule, we're fine.
                    let Some(constraint) = constraint else {
                        continue;
                    };

                    // If the bound rule's value already meets the constraint it applies,
                    // we don't need to track it. This way, two states that have the
                    // same bound value, one of which has a satisfied constraint, are
                    // still allowed to overlap.
                    if constraint == bound_value {
                        continue;
                    }
                    if constraint == Rule::MUST_SPLIT && bound_value != Rule::UNSPLIT {
                        continue;
                    }

                    // Lazily create and add the set to the constraints only if needed.
                    let disallowed_values =
                        unbound_constraints[position].get_or_insert_with(Vec::new);
                    if let Err(i) = disallowed_values.binary_search(&value) {
                        disallowed_values.insert(i, value);
                    }
                }
            }
        }

        unbound_constraints
    }
}
