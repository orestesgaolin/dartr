// Dart source: dart_style lib/src/short/line_splitting/solve_state_queue.dart

use std::cmp::Ordering;
use std::rc::Rc;

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
        self.bubble_up(state, index, info, arena);
    }

    pub fn remove_first(&mut self, info: &SplitterInfo, arena: &Arena) -> Rc<SolveState> {
        debug_assert!(!self.queue.is_empty());

        // Remove the highest priority state.
        let last = self.queue.pop().unwrap();
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

        // Count positions from one instead of zero. This gives the numbers some
        // nice properties. For example, all right children are odd, their left
        // sibling is even, and the parent is found by shifting right by one.
        // Valid range for position is [1.._length], inclusive.
        let mut position = 1;

        // Pre-order depth first search, omit child nodes if the current node has
        // lower priority than [object], because all nodes lower in the heap will
        // also have lower priority.
        loop {
            let index = position - 1;
            let enqueued = &self.queue[index];

            let mut comparison = Self::compare_score(enqueued, state);

            if comparison == Ordering::Equal {
                let overlap = enqueued.compare_overlap(state, info, arena);
                if overlap == Ordering::Less {
                    // The old state is better, so just discard the new one.
                    return true;
                } else if overlap == Ordering::Greater {
                    // The new state is better than the enqueued one, so replace it.
                    self.queue[index] = state.clone();
                    return true;
                } else {
                    // We can't merge them, so sort by their bound rule values.
                    comparison = Self::compare_rules(enqueued, state, info, arena);
                }
            }

            if comparison == Ordering::Less {
                // Element may be in subtree. Continue with the left child, if any.
                let left_child_position = position * 2;
                if left_child_position <= length {
                    position = left_child_position;
                    continue;
                }
            }

            // Find the next right sibling or right ancestor sibling.
            loop {
                while position % 2 == 1 {
                    // While position is a right child, go to the parent.
                    position >>= 1;
                }

                // Then go to the right sibling of the left child.
                position += 1;

                // Happens if last element is a left child.
                if position <= length {
                    break;
                }
            }

            // At root again. Happens for right-most element.
            if position == 1 {
                break;
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
            index = parent_index;
        }

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
                self.queue[index] = element;
                return;
            }

            self.queue[index] = min_child;
            index = min_child_index;
            right_child_index = index * 2 + 2;
        }

        let left_child_index = right_child_index - 1;
        if left_child_index < length {
            let child = &self.queue[left_child_index];
            let comparison = Self::compare(&element, child, info, arena);

            if comparison == Ordering::Greater {
                self.queue[index] = child.clone();
                index = left_child_index;
            }
        }

        self.queue[index] = element;
    }
}
