// Dart source: dart_style lib/src/piece/chain.dart

use crate::back_end::code_writer::{CodeWriter, Indent, ShapeMode};

use super::{PieceId, PieceImpl, Pieces, ShapeSet, State, States};

/// A dotted series of property access or method calls, like:
///
///     target.getter.method().another.method();
///
/// This piece handles splitting before the `.` and controlling which argument
/// lists in the method calls are allowed to contain newlines.
///
/// Chains can split in four ways:
///
/// [State::UNSPLIT] The entire chain on one line:
///
///     target.getter.method().another.method();
///
/// [BLOCK_FORMAT_TRAILING_CALL] Don't split before any `.`. Split the last
/// (or next-to-last if there is a hanging unsplittable call at the end)
/// method call in the chain like a block while leaving other calls unsplit,
/// as in:
///
///     target.property.first(1).block(
///       argument,
///       argument,
///     );
///
/// [SPLIT_AFTER_PROPERTIES] Split the call chain at each method call, but
/// leave the leading properties on the same line as the target. We allow
/// leading properties to remain unsplit while splitting the rest of the chain
/// since property accesses often feel "closer" to the target then the
/// methods called on it, as in:
///
///     motorcycle.wheels.front
///         .rotate();
///
/// [State::SPLIT] Split before every `.` and indent the chain, like:
///
///     target
///         .getter
///         .method(
///           argument,
///           argument,
///         )
///         .another
///         .method(
///           argument,
///           argument,
///         );
///
/// The 3.7 formatter had a limited ability to handle a newline in the target
/// of a chain in an assignment. In 3.8 and later, [Shape::Headline] lets us
/// express the constraint we want here directly. Since the logic is
/// different, Dart has two different [ChainPiece] subclasses for each, which
/// are [ChainVariant] here.
pub struct ChainPiece {
    /// The target expression at the beginning of the call chain.
    target: PieceId,

    /// The series of calls.
    calls: Vec<ChainCall>,

    /// The number of contiguous calls at the beginning of the chain that are
    /// properties.
    leading_properties: usize,

    /// The index of the call in the chain that may be block formatted or `-1`
    /// if none can.
    ///
    /// This will either be the index of the last call, or the index of the
    /// second to last call if the last call is a property or unsplittable
    /// call and the last call's argument list can be block formatted.
    block_call_index: i32,

    /// How to indent the chain when it splits.
    ///
    /// This is [Indent::Expression] for regular chains or [Indent::Cascade]
    /// for cascades.
    indent: Indent,

    is_cascade: bool,

    variant: ChainVariant,
}

/// The Dart subclasses of `ChainPiece`.
enum ChainVariant {
    /// `_ChainPiece`: 3.8 and later style.
    ///
    /// Whether the target of the chain is a call or collection with a single
    /// argument or element and we're at a version where those should be
    /// formatted specially.
    Chain { has_single_element_target: bool },

    /// `_ChainPiece3Dot7`: 3.7 style.
    ///
    /// Whether the target expression may contain newlines when the chain is
    /// not fully split. (It may always contain newlines when the chain
    /// splits.)
    ///
    /// This is true for most expressions but false for delimited ones to
    /// avoid this weird output:
    ///
    ///     function(
    ///       argument,
    ///     )
    ///         .method();
    Chain3Dot7 { allow_split_in_target: bool },
}

/// Allow newlines in the last (or next-to-last) call but nowhere else.
const BLOCK_FORMAT_TRAILING_CALL: State = State::with_cost(1, 0);

/// Split the call chain at each method call, but leave the leading properties
/// on the same line as the target.
const SPLIT_AFTER_PROPERTIES: State = State::new(2);

impl ChainPiece {
    /// Creates a new ChainPiece.
    ///
    /// Instead of calling this directly, prefer using [ChainBuilder].
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        target: PieceId,
        calls: Vec<ChainCall>,
        cascade: bool,
        leading_properties: usize,
        block_call_index: i32,
        indent: Indent,
        allow_split_in_target: bool,
        has_single_element_target: bool,
        is_3_dot_7: bool,
    ) -> ChainPiece {
        // If there are no calls, we shouldn't have created a chain.
        assert!(!calls.is_empty());
        let variant = if is_3_dot_7 {
            ChainVariant::Chain3Dot7 {
                allow_split_in_target,
            }
        } else {
            ChainVariant::Chain {
                has_single_element_target,
            }
        };
        ChainPiece {
            target,
            calls,
            leading_properties,
            block_call_index,
            indent,
            is_cascade: cascade,
            variant,
        }
    }

    fn base_state_cost(&self, state: State) -> i32 {
        // If the chain is a cascade, lower the cost so that we prefer
        // splitting the cascades instead of the target. Prefers:
        //
        //     [element1, element2]
        //       ..cascade();
        //
        // Over:
        //
        //     [
        //       element1,
        //       element2,
        //     ]..cascade();
        if state == State::SPLIT && self.is_cascade {
            return 0;
        }

        state.cost()
    }
}

impl PieceImpl for ChainPiece {
    fn additional_states(&self) -> States {
        let mut states = States::EMPTY;
        if self.block_call_index != -1 {
            states.push(BLOCK_FORMAT_TRAILING_CALL);
        }
        if self.leading_properties > 0 {
            states.push(SPLIT_AFTER_PROPERTIES);
        }
        states.push(State::SPLIT);
        states
    }

    fn state_cost(&self, state: State) -> i32 {
        match self.variant {
            ChainVariant::Chain {
                has_single_element_target,
            } => {
                // When the target is a single-element argument list or
                // collection, try to avoid splitting it. Prefers:
                //
                //     function(argument)
                //         .method();
                //
                // Over:
                //
                //     function(
                //       argument,
                //     ).method();
                if has_single_element_target
                    && (state == SPLIT_AFTER_PROPERTIES || state == State::SPLIT)
                {
                    return 0;
                }

                // If the chain is only properties, try to keep them together.
                // Prefers:
                //
                //     variable =
                //         target.property.another;
                //
                // Over:
                //
                //     variable = target
                //         .property
                //         .another;
                if !self.is_cascade
                    && self.leading_properties == self.calls.len()
                    && state == State::SPLIT
                {
                    return 2;
                }

                self.base_state_cost(state)
            }
            ChainVariant::Chain3Dot7 { .. } => self.base_state_cost(state),
        }
    }

    fn allowed_child_shapes(&self, _pieces: &Pieces, state: State, child: PieceId) -> ShapeSet {
        match self.variant {
            ChainVariant::Chain { .. } => {
                if child == self.target {
                    // If the chain itself isn't fully split, only allow block
                    // splitting in the target.
                    if state == State::UNSPLIT
                        || state == BLOCK_FORMAT_TRAILING_CALL
                        || state == SPLIT_AFTER_PROPERTIES
                    {
                        ShapeSet::INLINE_OR_BLOCK
                    } else {
                        ShapeSet::ALL
                    }
                } else {
                    if state == State::UNSPLIT {
                        return ShapeSet::ONLY_INLINE;
                    } else if state == SPLIT_AFTER_PROPERTIES {
                        // Don't allow splitting inside the properties.
                        for i in 0..self.leading_properties {
                            if self.calls[i].call == child {
                                return ShapeSet::ONLY_INLINE;
                            }
                        }
                    } else if state == BLOCK_FORMAT_TRAILING_CALL {
                        return ShapeSet::any_if(
                            self.calls[self.block_call_index as usize].call == child,
                        );
                    }

                    ShapeSet::ALL
                }
            }
            ChainVariant::Chain3Dot7 {
                allow_split_in_target,
            } => {
                if child == self.target {
                    return ShapeSet::any_if(allow_split_in_target || state == State::SPLIT);
                }
                if state == State::UNSPLIT {
                    return ShapeSet::ONLY_INLINE;
                } else if state == SPLIT_AFTER_PROPERTIES {
                    for i in 0..self.leading_properties {
                        if self.calls[i].call == child {
                            return ShapeSet::ONLY_INLINE;
                        }
                    }
                } else if state == BLOCK_FORMAT_TRAILING_CALL {
                    return ShapeSet::any_if(
                        self.calls[self.block_call_index as usize].call == child,
                    );
                }

                ShapeSet::ALL
            }
        }
    }

    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, state: State) {
        if state == State::UNSPLIT {
            writer.format(self.target, false);

            for call in &self.calls {
                writer.format(call.call, false);
            }
        } else if state == SPLIT_AFTER_PROPERTIES {
            writer.push_indent(self.indent);
            writer.set_shape_mode(ShapeMode::BeforeHeadline);
            writer.format(self.target, false);

            for i in 0..self.leading_properties {
                writer.format(self.calls[i].call, false);
            }

            writer.set_shape_mode(ShapeMode::AfterHeadline);

            for i in self.leading_properties..self.calls.len() {
                writer.newline();

                // Every non-property call except the last will be on its own
                // line.
                writer.format(self.calls[i].call, i < self.calls.len() - 1);
            }

            writer.pop_indent();
        } else if state == BLOCK_FORMAT_TRAILING_CALL {
            // Don't treat a cascade as block-shaped in the surrounding
            // context even if it block splits. Prefer:
            //
            //     variable = target
            //       ..cascade(argument);
            //
            // Over:
            //
            //     variable = target..cascade(
            //       argument,
            //     );
            //
            // Note how the former makes it clearer that `variable` will be
            // assigned the value `target` and that the cascade is a secondary
            // side-effect.
            if self.is_cascade {
                writer.set_shape_mode(ShapeMode::Other);
            }

            writer.format(self.target, false);

            for call in &self.calls {
                writer.format(call.call, false);
            }
        } else if state == State::SPLIT {
            writer.push_indent(self.indent);
            writer.set_shape_mode(ShapeMode::BeforeHeadline);
            writer.format(self.target, false);
            writer.set_shape_mode(ShapeMode::AfterHeadline);

            for i in 0..self.calls.len() {
                writer.newline();

                // The chain is fully split so every call except for the last
                // is on its own line.
                writer.format(self.calls[i].call, i < self.calls.len() - 1);
            }

            writer.pop_indent();
        }
    }

    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId)) {
        callback(self.target);

        for call in &self.calls {
            callback(call.call);
        }
    }
}

/// A method or getter call in a call chain, along with any postfix operations
/// applies to it.
#[derive(Clone, Copy)]
pub struct ChainCall {
    /// Piece for the call.
    pub call: PieceId,

    pub ty: CallType,
}

impl ChainCall {
    pub fn new(call: PieceId, ty: CallType) -> ChainCall {
        ChainCall { call, ty }
    }

    pub fn can_split(&self) -> bool {
        self.ty == CallType::SplittableCall || self.ty == CallType::BlockFormatCall
    }
}

/// What kind of "call" a dotted expression in a call chain is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CallType {
    /// A property access, like `.foo`.
    Property,

    /// A method call with an empty argument list that can't split.
    UnsplittableCall,

    /// A method call with a non-empty argument list that can split but not
    /// block format.
    SplittableCall,

    /// A method call with a non-empty argument list that can be block
    /// formatted.
    BlockFormatCall,
}
