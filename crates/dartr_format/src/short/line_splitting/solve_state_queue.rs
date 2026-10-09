// Dart source: dart_style lib/src/short/line_splitting/solve_state_queue.dart

use std::cmp::Ordering;
use std::rc::Rc;

use rustc_hash::FxHashMap;

use super::super::arena::Arena;
use super::line_splitter::SplitterInfo;
use super::solve_state::SolveState;

/// A priority queue of [SolveStates] to consider while line splitting.
///
/// This is based on the [HeapPriorityQueue] class from the "collection"
/// package, but is modified to handle the "overlap" logic that allows one
/// [SolveState] to supercede another.
///
/// States are stored internally in a heap ordered by cost, the number of
/// overflow characters. When a new state is added to the heap, it will be
/// discarded, or a previously enqueued one will be discarded, if two overlap.
#[derive(Default)]
pub struct SolveStateQueue {
    /// List implementation of a heap.
    queue: Vec<Rc<SolveState>>,

    /// The score (cost, overflow characters) of each state in [queue], kept
    /// in a separate list so that [try_overlap] scans compact memory.
    scores: Vec<(i32, i32)>,

    /// The number of states in [queue] with each score. [try_overlap] can
    /// only find an overlapping state with the same score, so it skips the
    /// heap traversal when there is none.
    score_counts: FxHashMap<(i32, i32), u32>,
}

#[inline(always)]
fn score_of(state: &SolveState) -> (i32, i32) {
    (state.splits().cost(), state.overflow_chars())
}

impl SolveStateQueue {
    pub fn is_not_empty(&self) -> bool {
        !self.queue.is_empty()
    }

    /// Add [state] to the queue.
    ///
    /// Grows the capacity if the backing list is full.
    pub fn add(&mut self, state: Rc<SolveState>, info: &SplitterInfo, arena: &Arena) {
        if self.try_overlap(&state, info, arena) {
            return;
        }

        let index = self.queue.len();
        self.queue.push(state.clone());
        let score = score_of(&state);
        self.scores.push(score);
        *self.score_counts.entry(score).or_insert(0) += 1;
        self.bubble_up(state, index, info, arena);
    }

    pub fn remove_first(&mut self, info: &SplitterInfo, arena: &Arena) -> Rc<SolveState> {
        debug_assert!(!self.queue.is_empty());

        // Remove the highest priority state.
        let first_score = self.scores[0];
        if let Some(count) = self.score_counts.get_mut(&first_score) {
            *count -= 1;
        }
        let last = self.queue.pop().unwrap();
        self.scores.pop();
        if self.queue.is_empty() {
            return last;
        }

        let result = self.queue[0].clone();

        // Fill the gap with the one at the end of the list and re-heapify.
        self.bubble_down(last, 0, info, arena);

        result
    }

    /// Orders this state relative to [other].
    ///
    /// This is a best-first ordering that prefers cheaper states even if they
    /// overflow because this ensures it finds the best solution first as soon as
    /// it finds one that fits in the page so it can early out.
    fn compare(a: &SolveState, b: &SolveState, info: &SplitterInfo, arena: &Arena) -> Ordering {
        // TODO(rnystrom): It may be worth sorting by the estimated lowest number
        // of overflow characters first. That doesn't help in cases where there is
        // a solution that fits, but may help in corner cases where there is no
        // fitting solution.

        let comparison = Self::compare_score(a, b);
        if comparison != Ordering::Equal {
            return comparison;
        }

        Self::compare_rules(a, b, info, arena)
    }

    /// Compares the overflow and cost of [a] to [b].
    fn compare_score(a: &SolveState, b: &SolveState) -> Ordering {
        let a_cost = a.splits().cost();
        let b_cost = b.splits().cost();
        if a_cost != b_cost {
            return a_cost.cmp(&b_cost);
        }

        a.overflow_chars().cmp(&b.overflow_chars())
    }

    /// Distinguish states based on the rule values just so that states with the
    /// same cost range but different rule values don't get considered identical
    /// and inadvertantly merged.
    fn compare_rules(a: &SolveState, b: &SolveState, info: &SplitterInfo, arena: &Arena) -> Ordering {
        for &rule in &info.rules {
            let a_value = a.get_value(arena, rule);
            let b_value = b.get_value(arena, rule);

            if a_value != b_value {
                return a_value.cmp(&b_value);
            }
        }

        // The way SolveStates are expanded should guarantee that we never generate
        // the exact same state twice. Getting here implies that that failed.
        panic!("unreachable");
    }

    /// Determines if any already enqueued state overlaps [state].
    ///
    /// If so, chooses the best and discards the other. Returns `true` in this
    /// case. Otherwise, returns `false`.
    fn try_overlap(&mut self, state: &Rc<SolveState>, info: &SplitterInfo, arena: &Arena) -> bool {
        let length = self.queue.len();
        if length == 0 {
            return false;
        }

        // Dart walks the heap in pre-order, depth first, from position 1 (positions
        // count from one: the children of p are 2p and 2p + 1). It descends
        // into a node only if the node orders before [state]: a smaller score,
        // or the same score, no overlap, and smaller bound rule values. Only
        // nodes with the same score as [state] can overlap, and all nodes below
        // a node with a greater score have a greater score too.
        //
        // So the walk does the same comparisons as Dart on the nodes with the
        // same score, in pre-order, skipping the subtrees of the nodes it would
        // not descend into. This avoids visiting all of the cheaper nodes.
        let state_score = score_of(state);
        if self.score_counts.get(&state_score).is_none_or(|&count| count == 0) {
            return false;
        }

        let mut candidates: Vec<usize> = Vec::new();
        for (index, &score) in self.scores.iter().enumerate() {
            if score == state_score {
                candidates.push(index + 1);
            }
        }

        // Sort the positions in pre-order: align each position to the depth of
        // the deepest level, ancestors first.
        let depth = |position: usize| usize::BITS - 1 - position.leading_zeros();
        let max_depth = depth(length);
        candidates.sort_by_key(|&position| {
            let d = depth(position);
            ((position as u64) << (max_depth - d), d)
        });

        let mut pruned: Vec<usize> = Vec::new();
        for position in candidates {
            let d = depth(position);
            if pruned.iter().any(|&p| {
                let pd = depth(p);
                pd < d && position >> (d - pd) == p
            }) {
                continue;
            }

            let index = position - 1;
            let enqueued = &self.queue[index];
            let overlap = enqueued.compare_overlap(state, info, arena);
            if overlap == Ordering::Less {
                // The old state is better, so just discard the new one.
                return true;
            } else if overlap == Ordering::Greater {
                // The new state is better than the enqueued one, so replace it.
                self.queue[index] = state.clone();
                self.scores[index] = state_score;
                return true;
            }

            // We can't merge them, so sort by their bound rule values.
            if Self::compare_rules(enqueued, state, info, arena) != Ordering::Less {
                pruned.push(position);
            }
        }

        false
    }

    /// Place [element] in heap at [index] or above.
    ///
    /// Put element into the empty cell at `index`. While the `element` has
    /// higher priority than the parent, swap it with the parent.
    fn bubble_up(&mut self, element: Rc<SolveState>, mut index: usize, info: &SplitterInfo, arena: &Arena) {
        while index > 0 {
            let parent_index = (index - 1) / 2;
            let parent = &self.queue[parent_index];

            if Self::compare(&element, parent, info, arena) == Ordering::Greater {
                break;
            }

            self.queue[index] = parent.clone();
            self.scores[index] = self.scores[parent_index];
            index = parent_index;
        }

        self.scores[index] = score_of(&element);
        self.queue[index] = element;
    }

    /// Place [element] in heap at [index] or above.
    ///
    /// Put element into the empty cell at `index`. While the `element` has lower
    /// priority than either child, swap it with the highest priority child.
    fn bubble_down(&mut self, element: Rc<SolveState>, mut index: usize, info: &SplitterInfo, arena: &Arena) {
        let length = self.queue.len();
        let mut right_child_index = index * 2 + 2;

        while right_child_index < length {
            let left_child_index = right_child_index - 1;
            let left_child = &self.queue[left_child_index];
            let right_child = &self.queue[right_child_index];

            let comparison = Self::compare(left_child, right_child, info, arena);
            let (min_child_index, min_child) = if comparison == Ordering::Less {
                (left_child_index, left_child.clone())
            } else {
                (right_child_index, right_child.clone())
            };

            let comparison = Self::compare(&element, &min_child, info, arena);

            if comparison != Ordering::Greater {
                self.scores[index] = score_of(&element);
                self.queue[index] = element;
                return;
            }

            self.queue[index] = min_child;
            self.scores[index] = self.scores[min_child_index];
            index = min_child_index;
            right_child_index = index * 2 + 2;
        }

        let left_child_index = right_child_index - 1;
        if left_child_index < length {
            let child = &self.queue[left_child_index];
            let comparison = Self::compare(&element, child, info, arena);

            if comparison == Ordering::Greater {
                self.queue[index] = child.clone();
                self.scores[index] = self.scores[left_child_index];
                index = left_child_index;
            }
        }

        self.scores[index] = score_of(&element);
        self.queue[index] = element;
    }
}
