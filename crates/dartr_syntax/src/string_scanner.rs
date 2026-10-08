// Dart source: pkg/_fe_analyzer_shared/lib/src/scanner/string_scanner.dart

//! The character reader of the scanner.
//!
//! The analyzer scans a Dart `String` with `StringScanner`, which reads
//! UTF-16 code units, so all offsets are UTF-16 offsets and a character
//! outside the BMP is two code units (two surrogates). This reader reads the
//! UTF-8 bytes of a `&str` and returns the same sequence of UTF-16 code
//! units, and keeps the byte offset next to the UTF-16 offset so that
//! lexemes can be slices of the source.

use crate::abstract_scanner::{AbstractScanner, Pos};
use crate::characters::*;
use crate::token::{TokenId, flags};
use crate::token_type::TokenType;

/// `true` for `[a-zA-Z0-9_$]`.
static IDENTIFIER_CHAR_ALLOW_DOLLAR: [bool; 256] = {
    let mut t = [false; 256];
    let mut c = 0;
    while c < 256 {
        t[c] = (c >= b'a' as usize && c <= b'z' as usize)
            || (c >= b'A' as usize && c <= b'Z' as usize)
            || (c >= b'0' as usize && c <= b'9' as usize)
            || c == b'_' as usize
            || c == b'$' as usize;
        c += 1;
    }
    t
};

/// Decodes the UTF-8 sequence at `p` (`src` is valid UTF-8). Returns the
/// code point and its byte length.
#[inline(always)]
fn decode(src: &[u8], p: usize) -> (u32, usize) {
    let b0 = src[p] as u32;
    if b0 < 0x80 {
        (b0, 1)
    } else if b0 < 0xE0 {
        (((b0 & 0x1F) << 6) | (src[p + 1] as u32 & 0x3F), 2)
    } else if b0 < 0xF0 {
        (
            ((b0 & 0x0F) << 12) | ((src[p + 1] as u32 & 0x3F) << 6) | (src[p + 2] as u32 & 0x3F),
            3,
        )
    } else {
        (
            ((b0 & 0x07) << 18)
                | ((src[p + 1] as u32 & 0x3F) << 12)
                | ((src[p + 2] as u32 & 0x3F) << 6)
                | (src[p + 3] as u32 & 0x3F),
            4,
        )
    }
}

/// Number of UTF-16 code units of a byte range of valid UTF-8 that starts
/// and ends at character boundaries.
#[inline]
pub(crate) fn utf16_len(bytes: &[u8]) -> i64 {
    let mut n = 0i64;
    for &b in bytes {
        // Count lead bytes; 4-byte sequences are two code units.
        n += ((b & 0xC0) != 0x80) as i64 + (b >= 0xF0) as i64;
    }
    n
}

impl<'a, 'c> AbstractScanner<'a, 'c> {
    /// Dart `advance()`: advances and returns the next code unit, or [`EOF`].
    #[inline(always)]
    pub(crate) fn advance(&mut self) -> i32 {
        self.scan_offset += 1;
        if self.pending_low != 0 {
            let c = self.pending_low;
            self.pending_low = 0;
            self.in_low = true;
            self.cur = c;
            return c;
        }
        self.in_low = false;
        self.pos += self.cur_len;
        if let Some(&b) = self.src.get(self.pos) {
            if b < 0x80 {
                self.cur_len = 1;
                self.cur = b as i32;
                return b as i32;
            }
            return self.advance_non_ascii();
        }
        self.cur_len = 0;
        self.cur = EOF;
        EOF
    }

    #[inline(never)]
    fn advance_non_ascii(&mut self) -> i32 {
        let (cp, len) = decode(self.src, self.pos);
        self.cur_len = len;
        if cp >= 0x10000 {
            let v = cp - 0x10000;
            self.pending_low = (0xDC00 + (v & 0x3FF)) as i32;
            self.cur = (0xD800 + (v >> 10)) as i32;
        } else {
            self.cur = cp as i32;
        }
        self.cur
    }

    /// Dart `current()`.
    #[inline(always)]
    pub(crate) fn current(&self) -> i32 {
        self.cur
    }

    /// Dart `peek()`: the code unit after the current one.
    #[inline(always)]
    pub(crate) fn peek(&self) -> i32 {
        if self.pending_low != 0 {
            return self.pending_low;
        }
        let p = self.pos + self.cur_len;
        match self.src.get(p) {
            None => EOF,
            Some(&b) if b < 0x80 => b as i32,
            Some(_) => {
                let (cp, _) = decode(self.src, p);
                if cp >= 0x10000 {
                    (0xD800 + ((cp - 0x10000) >> 10)) as i32
                } else {
                    cp as i32
                }
            }
        }
    }

    /// Dart `currentAsUnicode`: `StringScanner` returns the code unit.
    #[inline(always)]
    pub(crate) fn current_as_unicode(&self, next: i32) -> i32 {
        next
    }

    /// Dart `scanOffset`, with the byte offset.
    #[inline(always)]
    pub(crate) fn scan_pos(&self) -> Pos {
        // In the middle of a surrogate pair, the byte offset is the end of
        // the character.
        Pos {
            off: self.scan_offset,
            byte: if self.in_low { self.pos + 4 } else { self.pos },
        }
    }

    /// Dart `atEndOfFile()`.
    #[inline(always)]
    pub(crate) fn at_end_of_file(&self) -> bool {
        self.scan_offset >= 0 && self.pos >= self.src.len()
    }

    /// Moves to the byte offset `p`, which must be an ASCII character or the
    /// end, after a run of bytes that the caller scanned. `units` is the
    /// number of UTF-16 code units from the current position to `p`.
    #[inline(always)]
    fn jump_to(&mut self, p: usize, units: i64) -> i32 {
        debug_assert!(self.pending_low == 0);
        self.in_low = false;
        self.pos = p;
        self.scan_offset += units;
        match self.src.get(p) {
            Some(&b) => {
                self.cur_len = 1;
                self.cur = b as i32;
                b as i32
            }
            None => {
                self.cur_len = 0;
                self.cur = EOF;
                EOF
            }
        }
    }

    /// Dart `skipSpaces()`.
    #[inline(always)]
    pub(crate) fn skip_spaces(&mut self) -> i32 {
        let mut next = self.advance();
        // Sequences of spaces are common, so advance through them fast.
        while next == SPACE {
            next = self.advance();
        }
        next
    }

    /// Dart `passIdentifierCharAllowDollar()`.
    #[inline(always)]
    pub(crate) fn pass_identifier_char_allow_dollar(&mut self) -> i32 {
        if self.pending_low != 0 {
            return self.advance();
        }
        // The current character is an ASCII identifier character.
        let start = self.pos;
        let mut p = start + self.cur_len;
        let src = self.src;
        while p < src.len() && IDENTIFIER_CHAR_ALLOW_DOLLAR[src[p] as usize] {
            p += 1;
        }
        let units = (p - start) as i64;
        if p < src.len() && src[p] >= 0x80 {
            // Not an identifier character: stop before it and decode it.
            self.scan_offset += units - 1;
            self.pos = p;
            self.cur_len = 0;
            self.in_low = false;
            return self.advance();
        }
        self.jump_to(p, units)
    }

    /// Dart `scanUntilLineEnd()`. Returns true if the skipped characters are
    /// ASCII only.
    pub(crate) fn scan_until_line_end(&mut self) -> bool {
        if self.pending_low != 0 {
            // Rare: the current character is a high surrogate.
            let mut ascii_only = true;
            let mut next = self.advance();
            loop {
                if next > 127 {
                    ascii_only = false;
                }
                if LF == next || CR == next || EOF == next {
                    return ascii_only;
                }
                next = self.advance();
            }
        }
        let start = self.pos + self.cur_len;
        let src = self.src;
        let mut p = start;
        let mut ascii_only = true;
        while p < src.len() {
            let b = src[p];
            if b == b'\n' || b == b'\r' {
                break;
            }
            if b >= 0x80 {
                ascii_only = false;
            }
            p += 1;
        }
        let units = if ascii_only {
            (p - start) as i64
        } else {
            utf16_len(&src[start..p])
        };
        // `units` code units are skipped, then the line end (or EOF) is the
        // current character.
        self.jump_to(p, units + 1);
        ascii_only
    }

    /// Dart `createSubstringToken`.
    #[inline(always)]
    pub(crate) fn create_substring_token(
        &mut self,
        ty: TokenType,
        start: Pos,
        _ascii_only: bool,
        extra_offset: i64,
    ) -> TokenId {
        let end = self.scan_pos();
        let end_off = end.off + extra_offset;
        let end_byte = (end.byte as i64 + extra_offset) as usize;
        let token_start = self.token_start;
        let comments = self.comments;
        self.push_token(
            ty,
            0,
            token_start.off,
            end_off - start.off,
            start.byte,
            end_byte,
            comments,
        )
    }

    /// Dart `createSyntheticSubstringToken`: a `SyntheticStringToken` whose
    /// lexeme is the source from [start] to the current position plus
    /// [synthetic_chars], and whose length does not include them.
    pub(crate) fn create_synthetic_substring_token(
        &mut self,
        ty: TokenType,
        start: Pos,
        _ascii_only: bool,
        synthetic_chars: &str,
    ) -> TokenId {
        let end = self.scan_pos();
        let token_start = self.token_start;
        let length = end.off - start.off;
        if synthetic_chars.is_empty() {
            self.push_token(
                ty,
                flags::SYNTHETIC,
                token_start.off,
                length,
                start.byte,
                end.byte,
                TokenId::NONE,
            )
        } else {
            let mut value = String::with_capacity(end.byte - start.byte + synthetic_chars.len());
            value.push_str(std::str::from_utf8(&self.src[start.byte..end.byte]).unwrap_or(""));
            value.push_str(synthetic_chars);
            let index = self.arena.owned_lexemes.len();
            self.arena.owned_lexemes.push(value.into_boxed_str());
            self.push_token(
                ty,
                flags::SYNTHETIC | flags::OWNED_LEXEME,
                token_start.off,
                length,
                index,
                end.byte,
                TokenId::NONE,
            )
        }
    }

    /// Dart `createCommentToken` / `createDartDocToken`.
    pub(crate) fn create_comment_token(
        &mut self,
        ty: TokenType,
        start: Pos,
        _ascii_only: bool,
        extra_offset: i64,
    ) -> TokenId {
        let end = self.scan_pos();
        let end_off = end.off + extra_offset;
        let end_byte = (end.byte as i64 + extra_offset) as usize;
        let token_start = self.token_start;
        self.push_token(
            ty,
            flags::COMMENT,
            token_start.off,
            end_off - start.off,
            start.byte,
            end_byte,
            TokenId::NONE,
        )
    }

    /// Dart `createLanguageVersionToken`.
    pub(crate) fn create_language_version_token(
        &mut self,
        start: Pos,
        _major: i64,
        _minor: i64,
    ) -> TokenId {
        let token = self.create_comment_token(TokenType::SINGLE_LINE_COMMENT, start, true, 0);
        self.arena.tokens[token.index()].flags |= flags::LANGUAGE_VERSION;
        token
    }
}
