// Dart source: dart_style lib/src/piece/prefix.dart

use crate::back_end::code_writer::{CodeWriter, Indent};

use super::{PieceId, PieceImpl, State};

/// A piece for a prefix expression.
///
/// Prevents the inner construct's indentation from being merged with the
/// surrounding context. Ensures we get:
///
///     variable =
///         throw 'long adjacent string'
///            'more string';
///
/// And not:
///
///     variable =
///         throw 'long adjacent string'
///         'more string';
pub struct PrefixPiece {
    content: PieceId,
}

impl PrefixPiece {
    pub fn new(content: PieceId) -> PrefixPiece {
        PrefixPiece { content }
    }
}

impl PieceImpl for PrefixPiece {
    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, _state: State) {
        writer.push_indent(Indent::Grouping);
        writer.format(self.content, false);
        writer.pop_indent();
    }

    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId)) {
        callback(self.content);
    }
}
