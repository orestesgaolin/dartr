// Dart source: dart_style lib/src/piece/piece.dart

//! The pieces of the "tall" style (dart_style `lib/src/piece`).
//!
//! Dart has a class hierarchy of pieces that reference each other. Here the
//! pieces live in an arena ([`Pieces`]) and reference each other with
//! [`PieceId`]s. [`PieceKind`] has one variant per Dart piece class, and the
//! Dart virtual methods are the methods of [`PieceImpl`]. The lazily
//! computed Dart `late final` fields (`containsHardNewline`,
//! `totalCharacters`, `statefulOffspring`) are cached in [`Piece`].

pub mod adjacent;
pub mod assign;
pub mod assign_3_dot_7;
pub mod case;
pub mod chain;
pub mod clause;
pub mod constructor;
pub mod control_flow;
pub mod r#for;
pub mod grouping;
pub mod if_case;
pub mod infix;
pub mod leading_comment;
pub mod list;
pub mod prefix;
pub mod sequence;
pub mod text;
pub mod r#type;
pub mod type_parameter_bound;
pub mod type_test;
pub mod variable;

use std::cell::{Cell, OnceCell};
use std::cmp::Ordering;

use crate::back_end::code_writer::CodeWriter;

pub use adjacent::AdjacentPiece;
pub use assign::AssignPiece;
pub use assign_3_dot_7::AssignPiece3Dot7;
pub use case::CaseExpressionPiece;
pub use chain::{CallType, ChainCall, ChainPiece};
pub use clause::ClausePiece;
pub use constructor::ConstructorPiece;
pub use control_flow::ControlFlowPiece;
pub use r#for::{ForInPiece, ForPiece};
pub use grouping::GroupingPiece;
pub use if_case::IfCasePiece;
pub use infix::InfixPiece;
pub use leading_comment::LeadingCommentPiece;
pub use list::{Commas, ListElementPiece, ListPiece, ListStyle};
pub use prefix::PrefixPiece;
pub use sequence::{BlockPiece, SequenceElementPiece, SequencePiece};
pub use text::{NewlinePiece, SpacePiece, TextKind, TextPiece};
pub use r#type::{PrimaryTypePiece, TypeBodyType, TypePiece};
pub use type_parameter_bound::TypeParameterBoundPiece;
pub use type_test::TypeTestPiece;
pub use variable::VariablePiece;

/// The index of a [Piece] in a [Pieces] arena. Dart compares pieces by
/// identity; two ids are equal if they are the same piece.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct PieceId(pub u32);

impl PieceId {
    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// Dart `typedef Constrain = void Function(Piece other, State constrainedState)`.
pub type Constrain<'f> = dyn FnMut(PieceId, State) + 'f;

/// A state that a piece can be in.
///
/// Each state identifies one way that a piece can be split into multiple
/// lines. Each piece determines how its states are interpreted.
///
/// Dart compares states by identity, and all states are canonicalized
/// constants, so two states are equal if their value and cost are equal.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct State {
    value: u8,

    /// How much a solution is penalized when this state is chosen.
    cost: u8,
}

impl State {
    pub const UNSPLIT: State = State::with_cost(0, 0);

    /// The maximally split state a piece can be in.
    ///
    /// The value here is somewhat arbitrary. It just needs to be larger than
    /// any other value used by any [Piece] that uses this [State].
    pub const SPLIT: State = State::new(255);

    /// Dart `State(value)` (cost 1).
    pub const fn new(value: u8) -> State {
        State { value, cost: 1 }
    }

    /// Dart `State(value, cost: cost)`.
    pub const fn with_cost(value: u8, cost: u8) -> State {
        State { value, cost }
    }

    #[inline]
    pub fn cost(self) -> i32 {
        self.cost as i32
    }

    /// Dart `State.compareTo`: compares the values only.
    #[inline]
    pub fn compare_to(self, other: State) -> Ordering {
        self.value.cmp(&other.value)
    }
}

/// The spatial "shape" of a formatted [Piece].
///
/// Much of the formatting style is defined by placing constraints on whether
/// a newline inside a child forces the parent to enter certain states. For
/// example, a newline inside a binary operator's operands forces the
/// surrounding binary operator to split.
///
/// But some pieces have more specific constraints than just "one line or
/// not". In particular assignment-like constructs allow "block" and "header"
/// shaped children in certain states and not others.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Shape {
    /// The piece fits entirely on one line.
    Inline,

    /// A delimited block-indented structure like a function body, collection
    /// literal, or argument list.
    Block,

    /// The piece has a single line of code for some kind of "header" or other
    /// leading construct followed by multiple lines of other code. Examples:
    ///
    ///     conditionHeader
    ///         ? thenBody
    ///         : elseBody
    ///
    ///     target.header
    ///         .method()
    ///         .chain()
    Headline,

    /// The piece is split across multiple lines but not in any other
    /// well-defined shape.
    Other,
}

impl Shape {
    fn bit(self) -> u8 {
        match self {
            Shape::Inline => 1,
            Shape::Block => 2,
            Shape::Headline => 4,
            Shape::Other => 8,
        }
    }

    /// Determines the resulting shape of a parent when it has children with
    /// this shape and [other] shape.
    pub fn merge(self, other: Shape) -> Shape {
        match (self, other) {
            // If one shape is inline, it doesn't affect the result.
            (Shape::Inline, _) => other,
            (_, Shape::Inline) => self,

            // Otherwise, it's some other shape.
            (_, _) => Shape::Other,
        }
    }
}

/// Dart `Set<Shape>`, as a bit set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ShapeSet(u8);

impl ShapeSet {
    /// Allows all shapes.
    pub const ALL: ShapeSet = ShapeSet(1 | 2 | 4 | 8);

    /// Must be block shaped.
    pub const ONLY_BLOCK: ShapeSet = ShapeSet(2);

    /// Prohibits any newlines at all.
    pub const ONLY_INLINE: ShapeSet = ShapeSet(1);

    /// The only way it can split is block-style.
    pub const INLINE_OR_BLOCK: ShapeSet = ShapeSet(1 | 2);

    /// Dart `const {Shape.inline, Shape.other}`.
    pub const INLINE_OR_OTHER: ShapeSet = ShapeSet(1 | 8);

    /// Dart `const {Shape.block, Shape.headline}`.
    pub const BLOCK_OR_HEADLINE: ShapeSet = ShapeSet(2 | 4);

    /// Allows all shapes if [condition] is `true` or only an inline piece
    /// otherwise.
    ///
    /// This is a convenience for pieces that only care about disallowing
    /// newlines entirely.
    pub fn any_if(condition: bool) -> ShapeSet {
        if condition {
            ShapeSet::ALL
        } else {
            ShapeSet::ONLY_INLINE
        }
    }

    #[inline]
    pub fn contains(self, shape: Shape) -> bool {
        self.0 & shape.bit() != 0
    }

    /// Dart `allowedShapes.length == 1 && allowedShapes.contains(Shape.inline)`.
    #[inline]
    pub fn is_only_inline(self) -> bool {
        self == ShapeSet::ONLY_INLINE
    }
}

/// The result of [PieceImpl::additional_states]: at most four states,
/// without allocation.
#[derive(Clone, Copy)]
pub struct States {
    buf: [State; 4],
    len: u8,
}

impl States {
    pub const EMPTY: States = States {
        buf: [State::UNSPLIT; 4],
        len: 0,
    };

    pub fn of(states: &[State]) -> States {
        let mut result = States::EMPTY;
        for &state in states {
            result.push(state);
        }
        result
    }

    #[inline]
    pub fn push(&mut self, state: State) {
        self.buf[self.len as usize] = state;
        self.len += 1;
    }
}

impl std::ops::Deref for States {
    type Target = [State];
    #[inline]
    fn deref(&self) -> &[State] {
        &self.buf[..self.len as usize]
    }
}

/// The virtual methods of Dart `Piece`.
pub trait PieceImpl {
    /// The ordered list of all possible ways this piece could split.
    ///
    /// Piece subclasses should override this if they support being split in
    /// multiple different ways.
    ///
    /// Each piece determines what each [State] in the list represents,
    /// including the automatically included [State::UNSPLIT]. The list
    /// returned by this function should be sorted so that earlier states in
    /// the list compare less than later states.
    fn additional_states(&self) -> States {
        States::EMPTY
    }

    /// Apply any constraints that this piece places on other pieces when this
    /// piece is bound to [state].
    ///
    /// A piece class can override this. For any child piece that it wants to
    /// constrain when this piece is in [state], call [constrain] and pass in
    /// the child piece and the state that child should be constrained to.
    fn apply_constraints(&self, _state: State, _constrain: &mut Constrain) {}

    /// What shapes the [child] of this piece may take when this piece is in
    /// [state].
    fn allowed_child_shapes(&self, _pieces: &Pieces, _state: State, _child: PieceId) -> ShapeSet {
        ShapeSet::ALL
    }

    /// Whether this piece contains a newline when this piece is in [state].
    ///
    /// This should only return `true` if the piece will *always* write at
    /// least one newline -- either itself or one of its children -- when in
    /// this state. If a piece may contain a newline or may not in some state,
    /// this should return `false`.
    ///
    /// By default, we assume that any piece not in [State::UNSPLIT] or that
    /// has a hard newline will contain a newline.
    fn contains_newline(&self, pieces: &Pieces, id: PieceId, state: State) -> bool {
        state != State::UNSPLIT || pieces.contains_hard_newline(id)
    }

    /// Given that this piece is in [state], use [writer] to produce its
    /// formatted output.
    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, state: State);

    /// Invokes [callback] on each piece contained in this piece.
    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId));

    /// Dart `calculateContainsHardNewline()` override, or `None` to use the
    /// default (any child has a hard newline).
    fn calculate_contains_hard_newline(&self) -> Option<bool> {
        None
    }

    /// Dart `calculateTotalCharacters()` override, or `None` to use the
    /// default (the sum of the children).
    fn calculate_total_characters(&self) -> Option<i32> {
        None
    }

    /// If the piece can determine that it will always end up in a certain
    /// state given [page_width] and size metrics returned by calling
    /// [Pieces::contains_hard_newline] and [Pieces::total_characters] on its
    /// children, then returns that [State].
    ///
    /// For example, a series of infix operators wider than a page will always
    /// split one per operator. If we can determine this eagerly just based on
    /// the size of the children and the page width, then we can pin the Piece
    /// to that State. That in turn heavily prunes the search space that the
    /// [Solver] is exploring.
    ///
    /// If it's not possible to determine whether a piece will split from its
    /// metrics, this returns `None`.
    ///
    /// This is purely an optimization: Running the [Solver] without ever
    /// calling this and pinning the resulting [State] should yield the same
    /// formatting.
    fn fixed_state_for_page_width(&self, _pieces: &Pieces, _page_width: i32) -> Option<State> {
        None
    }

    /// The cost that this piece should apply to the solution when in [state].
    ///
    /// This is usually just the state's cost, but some pieces may want to
    /// tweak the cost in certain circumstances.
    fn state_cost(&self, state: State) -> i32 {
        state.cost()
    }
}

/// One variant per Dart piece class.
pub enum PieceKind {
    Adjacent(AdjacentPiece),
    Assign(AssignPiece),
    Assign3Dot7(AssignPiece3Dot7),
    CaseExpression(CaseExpressionPiece),
    Chain(ChainPiece),
    Clause(ClausePiece),
    Constructor(ConstructorPiece),
    ControlFlow(ControlFlowPiece),
    For(ForPiece),
    ForIn(ForInPiece),
    Grouping(GroupingPiece),
    IfCase(IfCasePiece),
    Infix(InfixPiece),
    LeadingComment(LeadingCommentPiece),
    List(ListPiece),
    ListElement(ListElementPiece),
    Prefix(PrefixPiece),
    Sequence(SequencePiece),
    Block(BlockPiece),
    SequenceElement(SequenceElementPiece),
    Text(TextPiece),
    Space(SpacePiece),
    Newline(NewlinePiece),
    Type(TypePiece),
    PrimaryType(PrimaryTypePiece),
    TypeParameterBound(TypeParameterBoundPiece),
    TypeTest(TypeTestPiece),
    Variable(VariablePiece),
}

macro_rules! dispatch {
    ($kind:expr, $p:ident => $body:expr) => {
        match $kind {
            PieceKind::Adjacent($p) => $body,
            PieceKind::Assign($p) => $body,
            PieceKind::Assign3Dot7($p) => $body,
            PieceKind::CaseExpression($p) => $body,
            PieceKind::Chain($p) => $body,
            PieceKind::Clause($p) => $body,
            PieceKind::Constructor($p) => $body,
            PieceKind::ControlFlow($p) => $body,
            PieceKind::For($p) => $body,
            PieceKind::ForIn($p) => $body,
            PieceKind::Grouping($p) => $body,
            PieceKind::IfCase($p) => $body,
            PieceKind::Infix($p) => $body,
            PieceKind::LeadingComment($p) => $body,
            PieceKind::List($p) => $body,
            PieceKind::ListElement($p) => $body,
            PieceKind::Prefix($p) => $body,
            PieceKind::Sequence($p) => $body,
            PieceKind::Block($p) => $body,
            PieceKind::SequenceElement($p) => $body,
            PieceKind::Text($p) => $body,
            PieceKind::Space($p) => $body,
            PieceKind::Newline($p) => $body,
            PieceKind::Type($p) => $body,
            PieceKind::PrimaryType($p) => $body,
            PieceKind::TypeParameterBound($p) => $body,
            PieceKind::TypeTest($p) => $body,
            PieceKind::Variable($p) => $body,
        }
    };
}

macro_rules! impl_from {
    ($($variant:ident($ty:ty)),* $(,)?) => {
        $(impl From<$ty> for PieceKind {
            fn from(piece: $ty) -> PieceKind {
                PieceKind::$variant(piece)
            }
        })*
    };
}

impl_from!(
    Adjacent(AdjacentPiece),
    Assign(AssignPiece),
    Assign3Dot7(AssignPiece3Dot7),
    CaseExpression(CaseExpressionPiece),
    Chain(ChainPiece),
    Clause(ClausePiece),
    Constructor(ConstructorPiece),
    ControlFlow(ControlFlowPiece),
    For(ForPiece),
    ForIn(ForInPiece),
    Grouping(GroupingPiece),
    IfCase(IfCasePiece),
    Infix(InfixPiece),
    LeadingComment(LeadingCommentPiece),
    List(ListPiece),
    ListElement(ListElementPiece),
    Prefix(PrefixPiece),
    Sequence(SequencePiece),
    Block(BlockPiece),
    SequenceElement(SequenceElementPiece),
    Text(TextPiece),
    Space(SpacePiece),
    Newline(NewlinePiece),
    Type(TypePiece),
    PrimaryType(PrimaryTypePiece),
    TypeParameterBound(TypeParameterBoundPiece),
    TypeTest(TypeTestPiece),
    Variable(VariablePiece),
);

impl PieceKind {
    #[inline]
    pub fn as_impl(&self) -> &dyn PieceImpl {
        dispatch!(self, p => p as &dyn PieceImpl)
    }
}

/// Base data of a Dart `Piece`: the piece itself, the pinned state and the
/// lazily computed metrics.
pub struct Piece {
    pub kind: PieceKind,

    /// If this piece has been pinned to a specific state, that state.
    ///
    /// This is used when a piece which otherwise supports multiple ways of
    /// splitting should be eagerly constrained to a specific splitting choice
    /// because of the context where it appears.
    pinned_state: Option<State>,

    /// `containsHardNewline`: 0 = not computed, 1 = false, 2 = true.
    contains_hard_newline: Cell<u8>,

    /// `totalCharacters`: -1 = not computed.
    total_characters: Cell<i32>,

    /// `statefulOffspring`.
    stateful_offspring: OnceCell<Box<[PieceId]>>,

    /// Cached `additionalStates.isNotEmpty`: 0 = not computed, 1 = false,
    /// 2 = true.
    has_states: Cell<u8>,
}

/// The arena of all pieces of one format operation.
#[derive(Default)]
pub struct Pieces {
    pieces: Vec<Piece>,
}

impl Pieces {
    pub fn new() -> Pieces {
        Pieces { pieces: Vec::new() }
    }

    /// Adds [piece] to the arena (Dart `Piece()` constructor).
    pub fn add(&mut self, piece: impl Into<PieceKind>) -> PieceId {
        let id = PieceId(self.pieces.len() as u32);
        self.pieces.push(Piece {
            kind: piece.into(),
            pinned_state: None,
            contains_hard_newline: Cell::new(0),
            total_characters: Cell::new(-1),
            stateful_offspring: OnceCell::new(),
            has_states: Cell::new(0),
        });
        id
    }

    pub fn len(&self) -> usize {
        self.pieces.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pieces.is_empty()
    }

    #[inline]
    pub fn get(&self, id: PieceId) -> &Piece {
        &self.pieces[id.index()]
    }

    #[inline]
    pub fn kind(&self, id: PieceId) -> &PieceKind {
        &self.pieces[id.index()].kind
    }

    #[inline]
    pub fn kind_mut(&mut self, id: PieceId) -> &mut PieceKind {
        &mut self.pieces[id.index()].kind
    }

    #[inline]
    fn imp(&self, id: PieceId) -> &dyn PieceImpl {
        self.pieces[id.index()].kind.as_impl()
    }

    /// Dart `piece.pinnedState`.
    #[inline]
    pub fn pinned_state(&self, id: PieceId) -> Option<State> {
        self.pieces[id.index()].pinned_state
    }

    /// Dart `piece.additionalStates`.
    #[inline]
    pub fn additional_states(&self, id: PieceId) -> States {
        self.imp(id).additional_states()
    }

    /// Dart `piece.additionalStates.isNotEmpty`.
    #[inline]
    pub fn has_additional_states(&self, id: PieceId) -> bool {
        let piece = &self.pieces[id.index()];
        match piece.has_states.get() {
            1 => false,
            2 => true,
            _ => {
                let result = !piece.kind.as_impl().additional_states().is_empty();
                piece.has_states.set(if result { 2 } else { 1 });
                result
            }
        }
    }

    /// Whether this piece or any of its children contain an explicit
    /// mandatory newline.
    ///
    /// This is lazily computed and cached for performance, so should only be
    /// accessed after all of the piece's children are known.
    pub fn contains_hard_newline(&self, id: PieceId) -> bool {
        let piece = &self.pieces[id.index()];
        match piece.contains_hard_newline.get() {
            1 => false,
            2 => true,
            _ => {
                let imp = piece.kind.as_impl();
                let result = match imp.calculate_contains_hard_newline() {
                    Some(result) => result,
                    None => {
                        let mut any_has_newline = false;
                        imp.for_each_child(&mut |child| {
                            any_has_newline |= self.contains_hard_newline(child);
                        });
                        any_has_newline
                    }
                };
                piece
                    .contains_hard_newline
                    .set(if result { 2 } else { 1 });
                result
            }
        }
    }

    /// The total number of characters of content in this piece and all of
    /// its children.
    ///
    /// This is lazily computed and cached for performance, so should only be
    /// accessed after all of the piece's children are known.
    pub fn total_characters(&self, id: PieceId) -> i32 {
        let piece = &self.pieces[id.index()];
        let cached = piece.total_characters.get();
        if cached >= 0 {
            return cached;
        }
        let imp = piece.kind.as_impl();
        let result = match imp.calculate_total_characters() {
            Some(result) => result,
            None => {
                let mut total = 0;
                imp.for_each_child(&mut |child| {
                    total += self.total_characters(child);
                });
                total
            }
        };
        piece.total_characters.set(result);
        result
    }

    /// All of the transitive children of this piece (including the piece
    /// itself) that have more than state.
    ///
    /// This calculated and cached because it's faster than traversing the
    /// child tree and having to skip past all of the stateless
    /// [AdjacentPiece], [SpacePiece], [SequencePiece], etc.
    pub fn stateful_offspring(&self, id: PieceId) -> &[PieceId] {
        self.pieces[id.index()].stateful_offspring.get_or_init(|| {
            let mut result = Vec::new();
            fn traverse(pieces: &Pieces, piece: PieceId, result: &mut Vec<PieceId>) {
                if pieces.has_additional_states(piece) {
                    result.push(piece);
                }
                pieces
                    .imp(piece)
                    .for_each_child(&mut |child| traverse(pieces, child, result));
            }
            traverse(self, id, &mut result);
            result.into_boxed_slice()
        })
    }

    /// Dart `piece.applyConstraints(state, constrain)`.
    #[inline]
    pub fn apply_constraints(&self, id: PieceId, state: State, constrain: &mut Constrain) {
        self.imp(id).apply_constraints(state, constrain)
    }

    /// Dart `piece.allowedChildShapes(state, child)`.
    #[inline]
    pub fn allowed_child_shapes(&self, id: PieceId, state: State, child: PieceId) -> ShapeSet {
        self.imp(id).allowed_child_shapes(self, state, child)
    }

    /// Dart `piece.containsNewline(state)`.
    #[inline]
    pub fn contains_newline(&self, id: PieceId, state: State) -> bool {
        self.imp(id).contains_newline(self, id, state)
    }

    /// Dart `piece.forEachChild(callback)`.
    #[inline]
    pub fn for_each_child(&self, id: PieceId, callback: &mut dyn FnMut(PieceId)) {
        self.imp(id).for_each_child(callback)
    }

    /// Dart `piece.fixedStateForPageWidth(pageWidth)`.
    #[inline]
    pub fn fixed_state_for_page_width(&self, id: PieceId, page_width: i32) -> Option<State> {
        self.imp(id).fixed_state_for_page_width(self, page_width)
    }

    /// Dart `piece.stateCost(state)`.
    #[inline]
    pub fn state_cost(&self, id: PieceId, state: State) -> i32 {
        self.imp(id).state_cost(state)
    }

    /// Forces this piece to always use [state].
    pub fn pin(&mut self, id: PieceId, state: State) {
        // Only pin a piece once. This can happen if the contents of an
        // interpolation expression (which are pinned to prevent splitting)
        // are large enough for [fixedStateForPageWidth()] to also try to pin
        // it to its fully split state.
        if self.pieces[id.index()].pinned_state.is_some() {
            return;
        }

        self.pieces[id.index()].pinned_state = Some(state);

        // If this piece's pinned state constrains any child pieces, pin those
        // too, recursively.
        let mut constrained = Vec::new();
        self.imp(id)
            .apply_constraints(state, &mut |other, constrained_state| {
                constrained.push((other, constrained_state));
            });
        for (other, constrained_state) in constrained {
            self.pin(other, constrained_state);
        }
    }

    /// Pin the piece to whatever state is needed to prevent it from
    /// splitting.
    pub fn prevent_split(&mut self, id: PieceId) {
        // Don't pin the ListElementPiece. Its state is only used to determine
        // whether or not to write a comma.
        if let PieceKind::ListElement(_) = self.kind(id) {
            return;
        }

        // For most pieces, the initial state does it.
        self.pin(id, State::UNSPLIT);
    }
}
