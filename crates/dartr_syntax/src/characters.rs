// Dart source: pkg/_fe_analyzer_shared/lib/src/scanner/characters.dart

#![allow(non_upper_case_globals)]

//! Character code constants. `$X` in Dart is `X` here (`$$` is `DOLLAR`,
//! `$_` is `UNDERSCORE`, `$0` is `D0`, lowercase letters are `LC_x`).

#![allow(dead_code)]

pub const EOF: i32 = -1;
pub const STX: i32 = 2;
pub const BS: i32 = 8;
pub const TAB: i32 = 9;
pub const LF: i32 = 10;
pub const VTAB: i32 = 11;
pub const FF: i32 = 12;
pub const CR: i32 = 13;
pub const SPACE: i32 = 32;
pub const BANG: i32 = 33;
pub const DQ: i32 = 34;
pub const HASH: i32 = 35;
pub const DOLLAR: i32 = 36;
pub const PERCENT: i32 = 37;
pub const AMPERSAND: i32 = 38;
pub const SQ: i32 = 39;
pub const OPEN_PAREN: i32 = 40;
pub const CLOSE_PAREN: i32 = 41;
pub const STAR: i32 = 42;
pub const PLUS: i32 = 43;
pub const COMMA: i32 = 44;
pub const MINUS: i32 = 45;
pub const PERIOD: i32 = 46;
pub const SLASH: i32 = 47;
pub const D0: i32 = 48;
pub const D1: i32 = 49;
pub const D2: i32 = 50;
pub const D3: i32 = 51;
pub const D4: i32 = 52;
pub const D5: i32 = 53;
pub const D6: i32 = 54;
pub const D7: i32 = 55;
pub const D8: i32 = 56;
pub const D9: i32 = 57;
pub const COLON: i32 = 58;
pub const SEMICOLON: i32 = 59;
pub const LT: i32 = 60;
pub const EQ: i32 = 61;
pub const GT: i32 = 62;
pub const QUESTION: i32 = 63;
pub const AT: i32 = 64;
pub const A: i32 = 65;
pub const B: i32 = 66;
pub const C: i32 = 67;
pub const D: i32 = 68;
pub const E: i32 = 69;
pub const F: i32 = 70;
pub const G: i32 = 71;
pub const H: i32 = 72;
pub const I: i32 = 73;
pub const J: i32 = 74;
pub const K: i32 = 75;
pub const L: i32 = 76;
pub const M: i32 = 77;
pub const N: i32 = 78;
pub const O: i32 = 79;
pub const P: i32 = 80;
pub const Q: i32 = 81;
pub const R: i32 = 82;
pub const S: i32 = 83;
pub const T: i32 = 84;
pub const U: i32 = 85;
pub const V: i32 = 86;
pub const W: i32 = 87;
pub const X: i32 = 88;
pub const Y: i32 = 89;
pub const Z: i32 = 90;
pub const OPEN_SQUARE_BRACKET: i32 = 91;
pub const BACKSLASH: i32 = 92;
pub const CLOSE_SQUARE_BRACKET: i32 = 93;
pub const CARET: i32 = 94;
pub const UNDERSCORE: i32 = 95;
pub const BACKPING: i32 = 96;
pub const LC_a: i32 = 97;
pub const LC_b: i32 = 98;
pub const LC_c: i32 = 99;
pub const LC_d: i32 = 100;
pub const LC_e: i32 = 101;
pub const LC_f: i32 = 102;
pub const LC_g: i32 = 103;
pub const LC_h: i32 = 104;
pub const LC_i: i32 = 105;
pub const LC_j: i32 = 106;
pub const LC_k: i32 = 107;
pub const LC_l: i32 = 108;
pub const LC_m: i32 = 109;
pub const LC_n: i32 = 110;
pub const LC_o: i32 = 111;
pub const LC_p: i32 = 112;
pub const LC_q: i32 = 113;
pub const LC_r: i32 = 114;
pub const LC_s: i32 = 115;
pub const LC_t: i32 = 116;
pub const LC_u: i32 = 117;
pub const LC_v: i32 = 118;
pub const LC_w: i32 = 119;
pub const LC_x: i32 = 120;
pub const LC_y: i32 = 121;
pub const LC_z: i32 = 122;
pub const OPEN_CURLY_BRACKET: i32 = 123;
pub const BAR: i32 = 124;
pub const CLOSE_CURLY_BRACKET: i32 = 125;
pub const TILDE: i32 = 126;
pub const DEL: i32 = 127;
pub const NBSP: i32 = 160;
pub const LS: i32 = 0x2028;
pub const PS: i32 = 0x2029;
pub const FIRST_SURROGATE: i32 = 0xd800;
pub const LAST_SURROGATE: i32 = 0xdfff;
pub const LAST_CODE_POINT: i32 = 0x10ffff;

#[inline(always)]
pub fn is_digit(c: i32) -> bool {
    D0 <= c && c <= D9
}

#[inline(always)]
pub fn is_hex_digit(c: i32) -> bool {
    if c <= D9 {
        return D0 <= c;
    }
    let c = c | (LC_a ^ A);
    LC_a <= c && c <= LC_f
}

#[inline(always)]
pub fn hex_digit_value(c: i32) -> i32 {
    debug_assert!(is_hex_digit(c));
    if c <= D9 {
        return c - D0;
    }
    (c | (LC_a ^ A)) - (LC_a - 10)
}
