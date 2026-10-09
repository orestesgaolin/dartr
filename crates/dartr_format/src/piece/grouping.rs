// Dart source: dart_style lib/src/piece/grouping.dart

use crate::back_end::code_writer::{CodeWriter, Indent, ShapeMode};

use super::{PieceId, PieceImpl, State};

/// A piece for a nested expression that should prevent its inner shape and
/// indentation from propagating outwards.
///
/// Used for index operands and string interpolation. Ensures we get:
///
///     variable =
///         '${a +
///             b}';
///
/// And not:
///
///     variable =
///         '${a +
///         b}';
pub struct GroupingPiece {
    content: PieceId,
}

impl GroupingPiece {
    pub fn new(content: PieceId) -> GroupingPiece {
        GroupingPiece { content }
    }
}

impl PieceImpl for GroupingPiece {
    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, _state: State) {
        writer.push_indent(Indent::Grouping);
        writer.set_shape_mode(ShapeMode::Other);
        writer.format(self.content, false);
        writer.pop_indent();
    }

    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId)) {
        callback(self.content);
    }
}
