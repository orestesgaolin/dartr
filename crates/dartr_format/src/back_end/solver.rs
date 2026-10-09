// Dart source: dart_style lib/src/back_end/solver.dart

use crate::piece::{PieceId, Pieces, State};

use super::priority_queue::HeapPriorityQueue;
use super::solution::Solution;
use super::solution_cache::SolutionCache;

/// To ensure the solver doesn't go totally pathological on giant code, we
/// cap it at a fixed number of attempts.
///
/// If the optimal solution isn't found after this many tries, it just uses
/// the best it found so far.
const MAX_ATTEMPTS: usize = 10000;

/// Selects states for each piece in a tree of pieces to find the best set of
/// line splits that minimizes overflow characters and line splitting costs.
///
/// This problem is combinatorial over the number of pieces and each of their
/// possible states, so it isn't feasible to brute force. There are a few
/// techniques we use to avoid that:
///
/// -   The initial state for each piece has no line splits or only mandatory
///     ones. Thus, it tries solutions with a minimum number of line splits
///     first.
///
/// -   Solutions are explored in priority order. We explore solutions with
///     the the lowest cost first. This way, as soon as we find a solution
///     with no overflow characters, we know it will be the best solution and
///     can stop.
///
/// -   When selecting states for pieces to expand solutions, we only look at
///     pieces in the first line containing overflow characters or invalid
///     newlines. See [Solution::expand_pieces] for more details.
///
/// -   If a subtree Piece is sufficiently isolated from surrounding content
///     (usually this means it is on its own line), then we hoist that entire
///     subtree out, format it with a separate Solver, and then insert the
///     result into the Solution. We also memoize the result of doing this and
///     use it across different Solutions. This enables us to both divide and
///     conquer the Piece tree and solve portions separately, while also
///     reusing work across different solutions.
pub struct Solver {
    page_width: i32,

    /// The number of spaces of indentation on the first line.
    leading_indent: i32,

    /// The number of spaces of indentation on all lines after the first.
    subsequent_indent: i32,
}

impl Solver {
    /// Creates a solver that fits code into the given [page_width].
    ///
    /// The first line is indented by [leading_indent] spaces and all lines
    /// after that are indented by [subsequent_indent].
    pub fn new(page_width: i32, leading_indent: i32, subsequent_indent: i32) -> Solver {
        Solver {
            page_width,
            leading_indent,
            subsequent_indent,
        }
    }

    /// Finds the best set of line splits for [root] piece and returns the
    /// resulting formatted code.
    ///
    /// If [root_state] is given, then [root] is bound to that state.
    pub fn format<'p>(
        &self,
        cache: &mut SolutionCache<'p>,
        pieces: &'p Pieces,
        root: PieceId,
        root_state: Option<State>,
    ) -> Solution<'p> {
        let mut queue = HeapPriorityQueue::new();
        let mut compare = |a: &Solution<'p>, b: &Solution<'p>| a.compare_to(b, pieces);

        let solution = Solution::new(
            cache,
            pieces,
            root,
            self.page_width,
            self.leading_indent,
            self.subsequent_indent,
            root_state,
        );

        queue.add(solution, &mut compare);

        // The lowest cost solution found so far that does overflow.
        let mut best: Option<Solution<'p>> = None;

        let mut attempts = 0;
        let mut expanded = Vec::new();

        while !queue.is_empty() && attempts < MAX_ATTEMPTS {
            let solution = queue.remove_first(&mut compare);

            attempts += 1;

            if solution.is_valid() {
                // Since we process the solutions from lowest cost up, as soon
                // as we find a valid one that fits, it's the best.
                if solution.overflow() == 0 {
                    best = Some(solution);
                    break;
                }
            }

            // Otherwise, try to expand the solution to explore different
            // splitting options.
            solution.expand(
                cache,
                pieces,
                root,
                self.page_width,
                self.leading_indent,
                self.subsequent_indent,
                &mut expanded,
            );
            for expanded in expanded.drain(..) {
                queue.add(expanded, &mut compare);
            }

            // If not, keep track of the least-bad one we've found so far.
            if solution.is_valid() {
                let replace = match &best {
                    None => true,
                    Some(best) => !best.is_valid() || solution.overflow() < best.overflow(),
                };
                if replace {
                    best = Some(solution);
                }
            } else if best.is_none() {
                // Dart initializes `best` to the initial solution.
                best = Some(solution);
            }
        }

        // If we didn't find a solution without overflow, pick the least bad
        // one.
        best.unwrap()
    }
}
