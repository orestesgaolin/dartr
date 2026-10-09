// Dart source: dart_style lib/src/back_end/solution.dart

use std::cmp::Ordering;
use std::rc::Rc;

use rustc_hash::FxHashMap;

use crate::piece::{PieceId, Pieces, ShapeSet, State};

use super::code::GroupCode;
use super::code_writer::CodeWriter;
use super::solution_cache::SolutionCache;

/// A single possible set of formatting choices.
///
/// Each solution binds some number of [Piece]s in the piece tree to
/// [State]s. (Any pieces whose states are not bound are treated as having a
/// default unsplit state.)
///
/// Given that set of states, we can create a [CodeWriter] and give that to
/// all of the pieces in the tree so they can format themselves. That in turn
/// yields a total number of overflow characters, cost, and formatted output,
/// which are all stored here.
pub struct Solution<'p> {
    /// The states that pieces have been bound to.
    ///
    /// This is a linked list of nodes, where each node adds a single piece
    /// binding. This allows "copying" the state map in O(1) time by just
    /// sharing the list tail. Lookups are O(N) where N is the number of bound
    /// pieces, but N is typically very small.
    piece_states: Option<Rc<StateNode>>,

    /// The set of states that pieces are allowed to be in without violating
    /// constraints of already bound pieces.
    ///
    /// Each key is a constrained piece and the values are the remaining
    /// states that the piece may take which aren't known to violate existing
    /// constraints. If a piece is not in this map, then there are no
    /// constraints on it.
    allowed_states: Option<FxHashMap<PieceId, Vec<State>>>,

    /// The cost of this solution based on pieces it has bound to states
    /// itself, excluding pieces from separately formatted subtrees.
    cost: i32,

    /// The cost of this solution from branches of the piece tree that were
    /// separately formatted and merged in using [merge_subtree()].
    ///
    /// We track this separately so that when expanding a solution, we don't
    /// double count the cost of separately formatted branches.
    subtree_cost: i32,

    /// The formatted code.
    code: Option<Rc<GroupCode<'p>>>,

    /// False if this Solution contains a newline where one is prohibited.
    ///
    /// An invalid solution may have no overflow characters and the lowest
    /// score, but is still considered worse than any other valid solution.
    is_valid: bool,

    /// Whether the solution contains an invalid newline and the piece that
    /// prohibits the newline is bound in this solution.
    ///
    /// When this is `true`, it means this solution and every solution that
    /// could be derived from it is invalid so the whole solution tree can be
    /// discarded.
    is_dead_end: bool,

    /// The total number of characters that do not fit inside the page width.
    overflow: i32,

    /// The unsolved piece in this solution that should be expanded next to
    /// produce new more refined solutions, if there is one.
    ///
    /// The tree of possible solutions is combinatorial in the number of
    /// pieces and exponential in the number of states those pieces can take.
    /// We can't afford to brute force explore the whole tree, even with the
    /// optimization that we stop as soon as we find a solution with no
    /// overflow.
    ///
    /// Most possible solutions add unnecessary splits in regions of the code
    /// that already fit within the page width. Exploring those is wasted
    /// time. To avoid that, we rely on a couple of insights:
    ///
    /// First, the solver treats any piece with an unselected state as being
    /// unsplit. This means that refining a solution always takes a piece
    /// that is unsplit and makes it split more. That monotonically increases
    /// the cost, but may help fit the solution inside the page.
    ///
    /// Therefore, we don't want to select states for most pieces. Only
    /// pieces that need to split in order to find a solution that fits in
    /// the page width or that are necessary because the unsplit state is
    /// invalid. (The latter usually means a line comment or statement occurs
    /// inside the piece.)
    ///
    /// So we skip past any pieces that aren't on overflowing lines or on
    /// lines whose newline led to an invalid solution. Further, it's also the
    /// case that splitting earlier pieces will often reshuffle the
    /// formatting of much of the code following it.
    ///
    /// Thus we only worry about unsolved pieces on the *first* problematic
    /// line when expanding. If selecting states for those pieces still
    /// doesn't help, the solver will work its way through later pieces from
    /// those subsequent partial solutions.
    ///
    /// This lets us efficiently skip through almost all of the pieces that
    /// don't need to be touched in order to find a valid solution.
    ///
    /// If this is empty, then there are no further solutions to generate
    /// from this one. It's either a dead end or a winner.
    expand_pieces: Vec<PieceId>,
}

/// A node in a linked list of piece-to-state bindings.
struct StateNode {
    piece: PieceId,
    state: State,
    parent: Option<Rc<StateNode>>,
}

impl<'p> Solution<'p> {
    /// Creates a new [Solution] with no pieces set to any state (which
    /// implicitly means they have state [State::UNSPLIT] unless they're
    /// pinned to another state).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cache: &mut SolutionCache<'p>,
        pieces: &'p Pieces,
        root: PieceId,
        page_width: i32,
        leading_indent: i32,
        subsequent_indent: i32,
        root_state: Option<State>,
    ) -> Solution<'p> {
        let mut solution = Solution::internal(pieces, root, 0, None, None, root_state);
        solution.format(cache, pieces, root, page_width, leading_indent, subsequent_indent);
        solution
    }

    fn internal(
        pieces: &Pieces,
        root: PieceId,
        cost: i32,
        piece_states: Option<Rc<StateNode>>,
        allowed_states: Option<FxHashMap<PieceId, Vec<State>>>,
        root_state: Option<State>,
    ) -> Solution<'p> {
        let mut solution = Solution {
            piece_states,
            allowed_states,
            cost,
            subtree_cost: 0,
            code: None,
            is_valid: true,
            is_dead_end: false,
            overflow: 0,
            expand_pieces: Vec::new(),
        };

        // If we're formatting a subtree of a larger Piece tree that binds
        // [root] to [rootState], then bind it in this solution too.
        if let Some(root_state) = root_state {
            solution.bind(pieces, root, root_state);
        }
        solution
    }

    /// The amount of penalties applied based on the chosen line splits.
    #[inline]
    pub fn cost(&self) -> i32 {
        self.cost + self.subtree_cost
    }

    /// The formatted code.
    pub fn code(&self) -> &Rc<GroupCode<'p>> {
        self.code.as_ref().unwrap()
    }

    /// False if this Solution contains a newline where one is prohibited.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.is_valid
    }

    /// The total number of characters that do not fit inside the page width.
    #[inline]
    pub fn overflow(&self) -> i32 {
        self.overflow
    }

    /// Attempt to eagerly bind [piece] to a state given that it must fit
    /// within [page_width] (which is the overall page width minus any
    /// leading indentation in the solution where this is called).
    ///
    /// If it can, binds the piece to that state in this solution and returns
    /// `true`. Otherwise returns `false`.
    ///
    /// This code has a bug, but is kept for older language versions to avoid
    /// unexpected formatting changes.
    ///
    /// See: https://github.com/dart-lang/dart_style/issues/1847
    pub fn try_bind_by_page_width(&mut self, pieces: &Pieces, piece: PieceId, page_width: i32) -> bool {
        if let Some(state) = pieces.fixed_state_for_page_width(piece, page_width) {
            self.bind(pieces, piece, state);
            return true;
        }

        false
    }

    /// The state that [piece] is pinned to or that this solution selects.
    ///
    /// If no state has been selected, defaults to the first state.
    #[inline]
    pub fn piece_state(&self, pieces: &Pieces, piece: PieceId) -> State {
        self.piece_state_if_bound(pieces, piece)
            .unwrap_or(State::UNSPLIT)
    }

    /// The state that [piece] is pinned to or that this solution selects.
    ///
    /// If no state has been selected, returns `None`.
    pub fn piece_state_if_bound(&self, pieces: &Pieces, piece: PieceId) -> Option<State> {
        if let Some(pinned) = pieces.pinned_state(piece) {
            return Some(pinned);
        }

        let mut node = self.piece_states.as_deref();
        while let Some(n) = node {
            if n.piece == piece {
                return Some(n.state);
            }
            node = n.parent.as_deref();
        }

        None
    }

    /// Whether [piece] has been bound to a state in this set (or is pinned).
    #[inline]
    pub fn is_bound(&self, pieces: &Pieces, piece: PieceId) -> bool {
        self.piece_state_if_bound(pieces, piece).is_some()
    }

    /// Increases the total overflow for this solution by [overflow].
    ///
    /// This should only be called by [CodeWriter].
    pub fn add_overflow(&mut self, overflow: i32) {
        self.overflow += overflow;
    }

    /// Apply the overflow and cost from a separately solved subtree solution
    /// to this solution.
    ///
    /// This is called when a subtree of a Piece tree is solved separately
    /// and the resulting solution is being merged with this one.
    pub fn merge_subtree(&mut self, overflow: i32, cost: i32) {
        self.overflow += overflow;
        self.subtree_cost += cost;
    }

    /// Mark this solution as having a newline where none is permitted by
    /// [piece] and is thus not a valid solution.
    ///
    /// This should only be called by [CodeWriter].
    pub fn invalidate(&mut self, pieces: &Pieces, piece: PieceId) {
        // Don't invalidate if the piece is pinned and can't do anything. This
        // only comes into play when a mandatory newline inside a string
        // interpolation causes some surrounding interpolation expression to
        // try to split but the expression is aready pinned to be unsplit to
        // prevent newlines inside interpolation.
        if pieces.pinned_state(piece).is_some() {
            return;
        }

        self.is_valid = false;
    }

    /// Derives new potential solutions from this one by binding
    /// [expand_pieces] to all of their possible states.
    ///
    /// If there is no potential piece to expand, or all attempts to expand
    /// it fail, returns an empty list.
    #[allow(clippy::too_many_arguments)]
    pub fn expand(
        &self,
        cache: &mut SolutionCache<'p>,
        pieces: &'p Pieces,
        root: PieceId,
        page_width: i32,
        leading_indent: i32,
        subsequent_indent: i32,
        out: &mut Vec<Solution<'p>>,
    ) {
        // If there is no piece that we can expand on this solution, it's a
        // dead end (or a winner).
        if self.expand_pieces.is_empty() {
            return;
        }

        for i in 0..self.expand_pieces.len() {
            // For each non-default state that the expanding piece can be in,
            // create a new solution that inherits all of the bindings of this
            // one, and binds the expanding piece to that state (along with
            // any further pieces constrained by that one).
            let expand_piece = self.expand_pieces[i];
            let additional;
            let states: &[State] = match self
                .allowed_states
                .as_ref()
                .and_then(|allowed| allowed.get(&expand_piece))
            {
                Some(states) => states,
                None => {
                    additional = pieces.additional_states(expand_piece);
                    &additional
                }
            };

            for &state in states {
                let mut expanded = Solution::internal(
                    pieces,
                    root,
                    self.cost,
                    self.piece_states.clone(),
                    self.allowed_states.clone(),
                    None,
                );

                // Bind all preceding expand pieces to their unsplit state.
                // Their other states have already been expanded by earlier
                // iterations of the outer for loop.
                let mut valid = true;
                for j in 0..i {
                    expanded.bind(pieces, self.expand_pieces[j], State::UNSPLIT);

                    if expanded.is_dead_end {
                        valid = false;
                        break;
                    }
                }

                // Discard the solution if we hit a constraint violation.
                if !valid {
                    continue;
                }

                expanded.bind(pieces, expand_piece, state);

                // Discard the solution if we hit a constraint violation.
                if !expanded.is_dead_end {
                    expanded.format(
                        cache,
                        pieces,
                        root,
                        page_width,
                        leading_indent,
                        subsequent_indent,
                    );

                    // TODO(rnystrom): These come mostly (entirely?) from hard
                    // newlines in sequences, comments, and multiline strings.
                    // It should be possible to handle those during piece
                    // construction too. If we do, remove this check.
                    // We may not detect some newline violations until
                    // formatting.
                    if !expanded.is_dead_end {
                        out.push(expanded);
                    }
                }
            }
        }
    }

    /// Compares two solutions where a more desirable solution comes first.
    ///
    /// For performance, we want to stop checking solutions as soon as we find
    /// the best one. Best means the fewest overflow characters and the lowest
    /// code.
    pub fn compare_to(&self, other: &Solution<'_>, pieces: &Pieces) -> Ordering {
        // Even though overflow is "worse" than cost, we order in terms of cost
        // because a solution with overflow may lead to a low-cost solution
        // without overflow, so we want to explore in cost order.
        if self.cost() != other.cost() {
            return self.cost().cmp(&other.cost());
        }

        if self.overflow != other.overflow {
            return self.overflow.cmp(&other.overflow);
        }

        // If all else is equal, prefer lower states in earlier bound pieces.
        // Since our linked list is in reverse order (newest first), we need
        // to reverse it to get the pieces in insertion order.
        let mut bound = Vec::new();
        let mut node = self.piece_states.as_deref();
        while let Some(n) = node {
            bound.push(n.piece);
            node = n.parent.as_deref();
        }

        for &piece in bound.iter().rev() {
            let this_state = self.piece_state(pieces, piece);
            let other_state = other.piece_state(pieces, piece);
            if this_state != other_state {
                return this_state.compare_to(other_state);
            }
        }

        Ordering::Equal
    }

    /// Run a [CodeWriter] on this solution to produce the final formatted
    /// output and calculate the overflow and expand pieces.
    fn format(
        &mut self,
        cache: &mut SolutionCache<'p>,
        pieces: &'p Pieces,
        root: PieceId,
        page_width: i32,
        leading_indent: i32,
        subsequent_indent: i32,
    ) {
        let mut writer = CodeWriter::new(
            page_width,
            leading_indent,
            subsequent_indent,
            pieces,
            cache,
            self,
        );

        writer.format(root, false);

        let (code, expand_pieces) = writer.finish();
        self.code = Some(Rc::new(code));
        self.expand_pieces = expand_pieces;
    }

    /// Attempts to add a binding from [piece] to [state] to the solution, and
    /// then adds any further bindings from constraints that [piece] applies
    /// to its children, recursively.
    ///
    /// This may invalidate the solution if [piece] is already bound to a
    /// different [state], or if any constrained pieces are bound to
    /// different states.
    ///
    /// If successful, adds the cost required to bind [piece] to [state]
    /// (along with any other applied constrained pieces). Otherwise, marks
    /// the solution as a dead end.
    fn bind(&mut self, pieces: &Pieces, piece: PieceId, state: State) {
        // If we've already failed from a previous violation, early out.
        if self.is_dead_end {
            return;
        }

        // Apply the new binding if it doesn't conflict with an existing one.
        match self.piece_state_if_bound(pieces, piece) {
            None => {
                // Binding a unbound piece to a state.
                self.cost += pieces.state_cost(piece, state);
                self.piece_states = Some(Rc::new(StateNode {
                    piece,
                    state,
                    parent: self.piece_states.take(),
                }));

                // This piece may in turn place further constraints on others.
                pieces.apply_constraints(piece, state, &mut |other, constrained_state| {
                    self.bind(pieces, other, constrained_state);
                });

                // If this piece's state prevents some of its children from
                // having newlines, then further constrain those children.
                if !self.is_dead_end {
                    pieces.for_each_child(piece, &mut |child| {
                        // Stop as soon as we fail.
                        if self.is_dead_end {
                            return;
                        }

                        // If the child can't have newlines in any shape, then
                        // constrain it.
                        // TODO(rnystrom): We can probably constrain this more
                        // specifically by asking what shapes the child might
                        // have.
                        let allowed_shapes: ShapeSet =
                            pieces.allowed_child_shapes(piece, state, child);
                        if allowed_shapes.is_only_inline() {
                            self.constrain_offspring(pieces, child);
                        }
                    });
                }
            }

            Some(already_bound) if already_bound != state => {
                // Already bound to a different state, so there's a conflict.
                self.is_dead_end = true;
                self.is_valid = false;
            }

            Some(_) => {} // Already bound to the same state, so nothing to do.
        }
    }

    /// For [piece] and its transitive offspring subtree, eliminate any state
    /// that will always produce a newline since that state is not permitted
    /// because the parent of [piece] doesn't allow [piece] to have any
    /// newlines.
    fn constrain_offspring(&mut self, pieces: &Pieces, piece: PieceId) {
        for &offspring in pieces.stateful_offspring(piece) {
            if self.is_dead_end {
                break;
            }

            if let Some(bound_state) = self.piece_state_if_bound(pieces, offspring) {
                // This offspring is already pinned or bound to a state. If
                // that state will emit newlines, then this solution is
                // invalid.
                if pieces.contains_newline(offspring, bound_state) {
                    self.is_dead_end = true;
                    self.is_valid = false;
                }
            } else if self
                .allowed_states
                .as_ref()
                .is_none_or(|allowed| !allowed.contains_key(&offspring))
            {
                // If we get here, the offspring isn't bound to a state and we
                // haven't already constrained it. Eliminate any of its states
                // that will emit newlines.
                let allowed_unsplit = !pieces.contains_newline(offspring, State::UNSPLIT);

                let states = pieces.additional_states(offspring);
                let mut remaining_states = Vec::new();
                for &state in states.iter() {
                    if !pieces.contains_newline(offspring, state) {
                        remaining_states.push(state);
                    }
                }

                if !allowed_unsplit && remaining_states.is_empty() {
                    // There is no state this child can take that won't emit
                    // newlines, and it's not allowed to, so this solution is
                    // bad.
                    self.is_dead_end = true;
                    self.is_valid = false;
                } else if remaining_states.is_empty() {
                    // The only valid state is unsplit so bind it to that.
                    self.bind(pieces, offspring, State::UNSPLIT);
                } else if !allowed_unsplit && remaining_states.len() == 1 {
                    // There's only one valid state, so bind it to that.
                    self.bind(pieces, offspring, remaining_states[0]);
                } else if remaining_states.len() < states.len() {
                    // There are some constrained states, so keep the remaining
                    // ones.
                    self.allowed_states
                        .get_or_insert_with(FxHashMap::default)
                        .insert(offspring, remaining_states);
                }
            }
        }
    }
}
