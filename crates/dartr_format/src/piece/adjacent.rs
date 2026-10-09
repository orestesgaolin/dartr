// Dart source: dart_style lib/src/piece/adjacent.dart

use crate::back_end::code_writer::CodeWriter;

use super::{PieceId, PieceImpl, State};

/// A simple piece that just writes its child pieces one after the other.
pub struct AdjacentPiece {
    pub pieces: Vec<PieceId>,
}

impl AdjacentPiece {
    pub fn new(pieces: Vec<PieceId>) -> AdjacentPiece {
        AdjacentPiece { pieces }
    }
}

impl PieceImpl for AdjacentPiece {
    fn format<'p>(&'p self, writer: &mut CodeWriter<'p, '_>, _state: State) {
        for &piece in &self.pieces {
            writer.format(piece, false);
        }
    }

    fn for_each_child(&self, callback: &mut dyn FnMut(PieceId)) {
        for &piece in &self.pieces {
            callback(piece);
        }
    }
}
