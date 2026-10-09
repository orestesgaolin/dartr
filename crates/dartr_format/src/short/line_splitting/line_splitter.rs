// Dart source: dart_style lib/src/short/line_splitting/line_splitter.dart

use std::rc::Rc;

use rustc_hash::FxHashMap;

use super::super::arena::{ChunkId, RuleId};
use super::super::line_writer::WriterContext;
use super::rule_set::{RuleSet, SplitSet};
use super::solve_state::SolveState;
use super::solve_state_queue::SolveStateQueue;

/// To ensure the solver doesn't go totally pathological on giant code, we cap
/// it at a fixed number of attempts.
///
/// If the optimal solution isn't found after this many tries, it just uses the
/// best it found so far.
const MAX_ATTEMPTS: i32 = 5000;

/// The fixed data of a [LineSplitter] that the [SolveState]s read.
pub struct SplitterInfo<'c> {
    /// The list of chunks being split.
    pub chunks: &'c [ChunkId],

    /// The set of soft rules whose values are being selected.
    pub rules: Vec<RuleId>,

    /// The position of each rule in [rules].
    positions: FxHashMap<RuleId, usize>,

    /// The position in [rules] of the rule of each chunk.
    pub chunk_rule_positions: Vec<usize>,

    /// For each rule in [rules], the positions in [rules] of its constrained
    /// rules (in the order of [Rule::constrained_rule_at]), or `None` for a
    /// rule that is not in [rules].
    pub constrained_positions: Vec<Vec<Option<usize>>>,

    /// The number of characters of additional indentation to apply to each line.
    ///
    /// This is used when formatting blocks to get the output into the right
    /// column based on where the block appears.
    pub block_indentation: i32,
}

impl SplitterInfo<'_> {
    /// The position of [rule] in [rules] (which must contain it).
    #[inline]
    pub fn position_of(&self, rule: RuleId) -> usize {
        self.positions[&rule]
    }

    /// The position of [rule] in [rules], or `None`.
    #[inline]
    pub fn try_position_of(&self, rule: RuleId) -> Option<usize> {
        self.positions.get(&rule).copied()
    }
}

/// Takes a set of chunks and determines the best values for its rules in order
/// to fit it inside the page boundary.
///
/// This problem is exponential in the number of rules and a single expression
/// in Dart can be quite large, so it isn't feasible to brute force this. For
/// example:
///
///     outer(
///         fn(1 + 2, 3 + 4, 5 + 6, 7 + 8),
///         fn(1 + 2, 3 + 4, 5 + 6, 7 + 8),
///         fn(1 + 2, 3 + 4, 5 + 6, 7 + 8),
///         fn(1 + 2, 3 + 4, 5 + 6, 7 + 8));
///
/// There are 509,607,936 ways this can be split.
///
/// The problem is even harder because we may not be able to easily tell if a
/// given solution is the best one. It's possible that there is *no* solution
/// that fits in the page (due to long strings or identifiers) so the winning
/// solution may still have overflow characters. This makes it hard to know
/// when we are done and can stop looking.
///
/// There are a couple of pieces of domain knowledge we use to cope with this:
///
/// - Changing a rule from unsplit to split will never lower its cost. A
///   solution with all rules unsplit will always be the one with the lowest
///   cost (zero). Conversely, setting all of its rules to the maximum split
///   value will always have the highest cost.
///
///   (You might think there is a converse rule about overflow characters. The
///   solution with the fewest splits will have the most overflow, and the
///   solution with the most splits will have the least overflow. Alas, because
///   of indentation, that isn't always the case. Adding a split may *increase*
///   overflow in some cases.)
///
/// - If all of the chunks for a rule are inside lines that already fit in the
///   page, then splitting that rule will never improve the solution.
///
/// - If two partial solutions have the same cost and the bound rules don't
///   affect any of the remaining unbound rules, then whichever partial
///   solution is currently better will always be the winner regardless of what
///   the remaining unbound rules are bound to.
///
/// We start off with a [SolveState] where all rules are unbound (which
/// implicitly treats them as unsplit). For a given solve state, we can produce
/// a set of expanded states that takes some of the rules in the first long
/// line and binds them to split values. This always produces new solve states
/// with higher cost (but often fewer overflow characters) than the parent
/// state.
///
/// We take these expanded states and add them to a work list sorted by cost.
/// Since unsplit rules always have lower cost solutions, we know that no state
/// we enqueue later will ever have a lower cost than the ones we already have
/// enqueued.
///
/// Then we keep pulling states off the work list and expanding them and adding
/// the results back into the list. We do this until we hit a solution where
/// all characters fit in the page. The first one we find will have the lowest
/// cost and we're done.
///
/// We also keep running track of the best solution we've found so far that
/// has the fewest overflow characters and the lowest cost. If no solution fits,
/// we'll use this one.
///
/// When enqueing a solution, we can sometimes collapse it and a previously
/// queued one by preferring one or the other. If two solutions have the same
/// cost and we can prove that they won't diverge later as unbound rules are
/// set, we can pick the winner now and discard the other. This lets us avoid
/// redundantly exploring entire subtrees of the solution space.
///
/// As a final escape hatch for pathologically nasty code, after trying some
/// fixed maximum number of solve states, we just bail and return the best
/// solution found so far.
///
/// Even with the above algorithmic optimizations, complex code may still
/// require a lot of exploring to find an optimal solution. To make that fast,
/// this code is carefully profiled and optimized. If you modify this, make
/// sure to test against the benchmark to ensure you don't regress performance.
pub struct LineSplitter<'c> {
    info: SplitterInfo<'c>,

    /// The queue of solve states to explore further.
    ///
    /// This is sorted lowest-cost first. This ensures that as soon as we find a
    /// solution that fits in the page, we know it will be the lowest cost one
    /// and can stop looking.
    queue: SolveStateQueue,
}

impl<'c> LineSplitter<'c> {
    /// Creates a new splitter that tries to fit [chunks] into the page width.
    pub fn new(ctx: &mut WriterContext, chunks: &'c [ChunkId], block_indentation: i32) -> Self {
        // Collect the set of rules that we need to select values for.
        let mut rules = Vec::new();
        let mut positions = FxHashMap::default();
        for &chunk in chunks {
            let rule = ctx.arena.chunk(chunk).rule;
            if let std::collections::hash_map::Entry::Vacant(entry) = positions.entry(rule) {
                entry.insert(rules.len());
                rules.push(rule);
            }
        }

        // Store the rule's index in the rule so we can get from a chunk to a rule
        // index quickly.
        for (i, &rule) in rules.iter().enumerate() {
            ctx.arena.rule_mut(rule).index = Some(i as u32);
        }

        // Now that every used rule has an index, tell the rules to discard any
        // constraints on unindexed rules.
        for &rule in &rules {
            ctx.arena.forget_unused_rules(rule);
        }

        let chunk_rule_positions = chunks
            .iter()
            .map(|&chunk| positions[&ctx.arena.chunk(chunk).rule])
            .collect();
        let constrained_positions = rules
            .iter()
            .map(|&rule| {
                ctx.arena
                    .rule(rule)
                    .constrained_rules()
                    .map(|other| positions.get(&other).copied())
                    .collect()
            })
            .collect();

        LineSplitter {
            info: SplitterInfo {
                chunks,
                rules,
                positions,
                chunk_rule_positions,
                constrained_positions,
                block_indentation,
            },
            queue: SolveStateQueue::default(),
        }
    }

    /// Determine the best way to split the chunks into lines that fit in the
    /// page, if possible.
    ///
    /// Returns a [SplitSet] that defines where each split occurs and the
    /// indentation of each line.
    pub fn apply(mut self, ctx: &mut WriterContext) -> SplitSet {
        let info = &self.info;
        let queue = &mut self.queue;

        // Start with a completely unbound, unsplit solution.
        let mut best_solution = Rc::new(SolveState::new(info, ctx, RuleSet::new(info.rules.len())));
        queue.add(best_solution.clone(), info, ctx.arena);

        let mut attempts = 0;
        while queue.is_not_empty() {
            let state = queue.remove_first(info, ctx.arena);

            if state.is_better_than(&best_solution) {
                best_solution = state.clone();

                // Since we sort solutions by cost the first solution we find that
                // fits is the winner.
                if best_solution.overflow_chars() == 0 {
                    break;
                }
            }

            let over = attempts > MAX_ATTEMPTS;
            attempts += 1;
            if over {
                break;
            }

            // Try bumping the rule values for rules whose chunks are on long lines.
            state.expand(info, ctx, &mut |state, info, arena| {
                queue.add(Rc::new(state), info, arena);
            });
        }

        best_solution.splits().clone()
    }
}
