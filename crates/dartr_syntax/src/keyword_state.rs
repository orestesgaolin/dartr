// Dart source: pkg/_fe_analyzer_shared/lib/src/scanner/keyword_state.dart

//! The keyword trie used by `tokenize_keyword_or_identifier`.
//!
//! Same layout as Dart `KeywordStateHelper`: blocks of [`BLOCK_SIZE`]
//! entries; entry 0 of a block is the keyword index + 1 (0: no keyword),
//! entries 1..=58 are the next state for the characters `A`..=`z`. State 0
//! is the null state.

use std::sync::OnceLock;

use crate::characters::A;
use crate::token_type::{Keyword, TokenType};

pub const BLOCK_SIZE: usize = 59;

/// A state in the trie. `KeywordState(0)` is the null state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KeywordState(u16);

static TABLE: OnceLock<Box<[u16]>> = OnceLock::new();

fn build() -> Box<[u16]> {
    let mut table = vec![0u16; 297 * BLOCK_SIZE];
    let mut next_empty = 2 * BLOCK_SIZE;
    for (i, keyword) in Keyword::VALUES.iter().enumerate() {
        let lexeme = keyword.lexeme().as_bytes();
        let mut offset = BLOCK_SIZE;
        for &c in lexeme {
            let char_offset = (c as i32 - A) as usize;
            let link = table[offset + 1 + char_offset] as usize;
            if link == 0 {
                table[offset + 1 + char_offset] = next_empty as u16;
                offset = next_empty;
                next_empty += BLOCK_SIZE;
            } else {
                offset = link;
            }
        }
        table[offset] = (i + 1) as u16;
    }
    assert_eq!(next_empty, table.len());
    table.into_boxed_slice()
}

#[inline(always)]
pub(crate) fn table() -> &'static [u16] {
    TABLE.get_or_init(build)
}

impl KeywordState {
    /// The root state (Dart `KeywordStateHelper.table`).
    #[inline(always)]
    pub fn root() -> KeywordState {
        KeywordState(BLOCK_SIZE as u16)
    }

    #[inline(always)]
    pub fn is_null(self) -> bool {
        self.0 == 0
    }

    /// `c` must be in `A`..=`z`.
    #[inline(always)]
    pub fn next(self, table: &[u16], c: i32) -> KeywordState {
        KeywordState(table[self.0 as usize + (c - A) as usize + 1])
    }

    #[inline(always)]
    pub fn keyword(self, table: &[u16]) -> Option<TokenType> {
        let i = table[self.0 as usize];
        if i == 0 {
            None
        } else {
            Some(Keyword::VALUES[i as usize - 1])
        }
    }
}
