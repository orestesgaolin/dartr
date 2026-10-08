// GENERATED FILE. DO NOT EDIT. Run `python3 tools/codegen/gen_token_types.py`.
//
// Dart source: pkg/_fe_analyzer_shared/lib/src/scanner/token.dart
// (`TokenType`, `Keyword`, `_tokenTypesByIndex`).

use crate::token_constants::*;

/// The type of a token: a port of Dart `TokenType` and `Keyword`.
///
/// The value is the Dart `TokenType.index`, so that `TokenType(i)` equals
/// `_tokenTypesByIndex[i]`. Keywords are token types too (as in Dart, where
/// `Keyword extends TokenType`); their constants are on [`Keyword`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TokenType(pub u8);

/// Namespace for the keyword token types (Dart `Keyword.X`).
pub struct Keyword;

/// Dart `KeywordStyle`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum KeywordStyle {
    Reserved,
    BuiltIn,
    Pseudo,
}

impl TokenType {
    pub const EOF: TokenType = TokenType(0);
    pub const DOUBLE: TokenType = TokenType(1);
    pub const DOUBLE_WITH_SEPARATORS: TokenType = TokenType(2);
    pub const HEXADECIMAL: TokenType = TokenType(3);
    pub const HEXADECIMAL_WITH_SEPARATORS: TokenType = TokenType(4);
    pub const IDENTIFIER: TokenType = TokenType(5);
    pub const INT: TokenType = TokenType(6);
    pub const INT_WITH_SEPARATORS: TokenType = TokenType(7);
    pub const MULTI_LINE_COMMENT: TokenType = TokenType(8);
    pub const SCRIPT_TAG: TokenType = TokenType(9);
    pub const SINGLE_LINE_COMMENT: TokenType = TokenType(10);
    pub const STRING: TokenType = TokenType(11);
    pub const AMPERSAND: TokenType = TokenType(12);
    pub const AMPERSAND_AMPERSAND: TokenType = TokenType(13);
    pub const AMPERSAND_AMPERSAND_EQ: TokenType = TokenType(14);
    pub const AMPERSAND_EQ: TokenType = TokenType(15);
    pub const AT: TokenType = TokenType(16);
    pub const BANG: TokenType = TokenType(17);
    pub const BANG_EQ: TokenType = TokenType(18);
    pub const BANG_EQ_EQ: TokenType = TokenType(19);
    pub const BAR: TokenType = TokenType(20);
    pub const BAR_BAR: TokenType = TokenType(21);
    pub const BAR_BAR_EQ: TokenType = TokenType(22);
    pub const BAR_EQ: TokenType = TokenType(23);
    pub const COLON: TokenType = TokenType(24);
    pub const COMMA: TokenType = TokenType(25);
    pub const CARET: TokenType = TokenType(26);
    pub const CARET_EQ: TokenType = TokenType(27);
    pub const CLOSE_CURLY_BRACKET: TokenType = TokenType(28);
    pub const CLOSE_PAREN: TokenType = TokenType(29);
    pub const CLOSE_SQUARE_BRACKET: TokenType = TokenType(30);
    pub const EQ: TokenType = TokenType(31);
    pub const EQ_EQ: TokenType = TokenType(32);
    pub const EQ_EQ_EQ: TokenType = TokenType(33);
    pub const FUNCTION: TokenType = TokenType(34);
    pub const GT: TokenType = TokenType(35);
    pub const GT_EQ: TokenType = TokenType(36);
    pub const GT_GT: TokenType = TokenType(37);
    pub const GT_GT_EQ: TokenType = TokenType(38);
    pub const GT_GT_GT: TokenType = TokenType(39);
    pub const GT_GT_GT_EQ: TokenType = TokenType(40);
    pub const HASH: TokenType = TokenType(41);
    pub const INDEX: TokenType = TokenType(42);
    pub const INDEX_EQ: TokenType = TokenType(43);
    pub const LT: TokenType = TokenType(44);
    pub const LT_EQ: TokenType = TokenType(45);
    pub const LT_LT: TokenType = TokenType(46);
    pub const LT_LT_EQ: TokenType = TokenType(47);
    pub const MINUS: TokenType = TokenType(48);
    pub const MINUS_EQ: TokenType = TokenType(49);
    pub const MINUS_MINUS: TokenType = TokenType(50);
    pub const OPEN_CURLY_BRACKET: TokenType = TokenType(51);
    pub const OPEN_PAREN: TokenType = TokenType(52);
    pub const OPEN_SQUARE_BRACKET: TokenType = TokenType(53);
    pub const PERCENT: TokenType = TokenType(54);
    pub const PERCENT_EQ: TokenType = TokenType(55);
    pub const PERIOD: TokenType = TokenType(56);
    pub const PERIOD_PERIOD: TokenType = TokenType(57);
    pub const PLUS: TokenType = TokenType(58);
    pub const PLUS_EQ: TokenType = TokenType(59);
    pub const PLUS_PLUS: TokenType = TokenType(60);
    pub const QUESTION: TokenType = TokenType(61);
    pub const QUESTION_PERIOD: TokenType = TokenType(62);
    pub const QUESTION_QUESTION: TokenType = TokenType(63);
    pub const QUESTION_QUESTION_EQ: TokenType = TokenType(64);
    pub const SEMICOLON: TokenType = TokenType(65);
    pub const SLASH: TokenType = TokenType(66);
    pub const SLASH_EQ: TokenType = TokenType(67);
    pub const STAR: TokenType = TokenType(68);
    pub const STAR_EQ: TokenType = TokenType(69);
    pub const STRING_INTERPOLATION_EXPRESSION: TokenType = TokenType(70);
    pub const STRING_INTERPOLATION_IDENTIFIER: TokenType = TokenType(71);
    pub const TILDE: TokenType = TokenType(72);
    pub const TILDE_SLASH: TokenType = TokenType(73);
    pub const TILDE_SLASH_EQ: TokenType = TokenType(74);
    pub const BACKPING: TokenType = TokenType(75);
    pub const BACKSLASH: TokenType = TokenType(76);
    pub const PERIOD_PERIOD_PERIOD: TokenType = TokenType(77);
    pub const PERIOD_PERIOD_PERIOD_QUESTION: TokenType = TokenType(78);
    pub const QUESTION_PERIOD_PERIOD: TokenType = TokenType(79);
    pub const BAD_INPUT: TokenType = TokenType(80);
    pub const RECOVERY: TokenType = TokenType(81);
    pub const UNUSED: TokenType = TokenType(255);
    pub const AS: TokenType = Keyword::AS;
    pub const IS: TokenType = Keyword::IS;
}

impl Keyword {
    pub const ABSTRACT: TokenType = TokenType(82);
    pub const AS: TokenType = TokenType(83);
    pub const ASSERT: TokenType = TokenType(84);
    pub const ASYNC: TokenType = TokenType(85);
    pub const AUGMENT: TokenType = TokenType(86);
    pub const AWAIT: TokenType = TokenType(87);
    pub const BASE: TokenType = TokenType(88);
    pub const BREAK: TokenType = TokenType(89);
    pub const CASE: TokenType = TokenType(90);
    pub const CATCH: TokenType = TokenType(91);
    pub const CLASS: TokenType = TokenType(92);
    pub const CONST: TokenType = TokenType(93);
    pub const CONTINUE: TokenType = TokenType(94);
    pub const COVARIANT: TokenType = TokenType(95);
    pub const DEFAULT: TokenType = TokenType(96);
    pub const DEFERRED: TokenType = TokenType(97);
    pub const DO: TokenType = TokenType(98);
    pub const DYNAMIC: TokenType = TokenType(99);
    pub const ELSE: TokenType = TokenType(100);
    pub const ENUM: TokenType = TokenType(101);
    pub const EXPORT: TokenType = TokenType(102);
    pub const EXTENDS: TokenType = TokenType(103);
    pub const EXTENSION: TokenType = TokenType(104);
    pub const EXTERNAL: TokenType = TokenType(105);
    pub const FACTORY: TokenType = TokenType(106);
    pub const FALSE: TokenType = TokenType(107);
    pub const FINAL: TokenType = TokenType(108);
    pub const FINALLY: TokenType = TokenType(109);
    pub const FOR: TokenType = TokenType(110);
    pub const FUNCTION: TokenType = TokenType(111);
    pub const GET: TokenType = TokenType(112);
    pub const HIDE: TokenType = TokenType(113);
    pub const IF: TokenType = TokenType(114);
    pub const IMPLEMENTS: TokenType = TokenType(115);
    pub const IMPORT: TokenType = TokenType(116);
    pub const IN: TokenType = TokenType(117);
    pub const INOUT: TokenType = TokenType(118);
    pub const INTERFACE: TokenType = TokenType(119);
    pub const IS: TokenType = TokenType(120);
    pub const LATE: TokenType = TokenType(121);
    pub const LIBRARY: TokenType = TokenType(122);
    pub const MIXIN: TokenType = TokenType(123);
    pub const NATIVE: TokenType = TokenType(124);
    pub const NEW: TokenType = TokenType(125);
    pub const NULL: TokenType = TokenType(126);
    pub const OF: TokenType = TokenType(127);
    pub const ON: TokenType = TokenType(128);
    pub const OPERATOR: TokenType = TokenType(129);
    pub const OUT: TokenType = TokenType(130);
    pub const PART: TokenType = TokenType(131);
    pub const PATCH: TokenType = TokenType(132);
    pub const REQUIRED: TokenType = TokenType(133);
    pub const RETHROW: TokenType = TokenType(134);
    pub const RETURN: TokenType = TokenType(135);
    pub const SEALED: TokenType = TokenType(136);
    pub const SET: TokenType = TokenType(137);
    pub const SHOW: TokenType = TokenType(138);
    pub const SOURCE: TokenType = TokenType(139);
    pub const STATIC: TokenType = TokenType(140);
    pub const SUPER: TokenType = TokenType(141);
    pub const SWITCH: TokenType = TokenType(142);
    pub const SYNC: TokenType = TokenType(143);
    pub const THIS: TokenType = TokenType(144);
    pub const THROW: TokenType = TokenType(145);
    pub const TRUE: TokenType = TokenType(146);
    pub const TRY: TokenType = TokenType(147);
    pub const TYPEDEF: TokenType = TokenType(148);
    pub const VAR: TokenType = TokenType(149);
    pub const VOID: TokenType = TokenType(150);
    pub const WHEN: TokenType = TokenType(151);
    pub const WHILE: TokenType = TokenType(152);
    pub const WITH: TokenType = TokenType(153);
    pub const YIELD: TokenType = TokenType(154);

    /// Dart `Keyword.values`, in declaration order.
    pub const VALUES: &[TokenType] = &[
        Keyword::ABSTRACT,
        Keyword::AS,
        Keyword::ASSERT,
        Keyword::ASYNC,
        Keyword::AUGMENT,
        Keyword::AWAIT,
        Keyword::BASE,
        Keyword::BREAK,
        Keyword::CASE,
        Keyword::CATCH,
        Keyword::CLASS,
        Keyword::CONST,
        Keyword::CONTINUE,
        Keyword::COVARIANT,
        Keyword::DEFAULT,
        Keyword::DEFERRED,
        Keyword::DO,
        Keyword::DYNAMIC,
        Keyword::ELSE,
        Keyword::ENUM,
        Keyword::EXPORT,
        Keyword::EXTENDS,
        Keyword::EXTENSION,
        Keyword::EXTERNAL,
        Keyword::FACTORY,
        Keyword::FALSE,
        Keyword::FINAL,
        Keyword::FINALLY,
        Keyword::FOR,
        Keyword::FUNCTION,
        Keyword::GET,
        Keyword::HIDE,
        Keyword::IF,
        Keyword::IMPLEMENTS,
        Keyword::IMPORT,
        Keyword::IN,
        Keyword::INOUT,
        Keyword::INTERFACE,
        Keyword::IS,
        Keyword::LATE,
        Keyword::LIBRARY,
        Keyword::MIXIN,
        Keyword::NATIVE,
        Keyword::NEW,
        Keyword::NULL,
        Keyword::OF,
        Keyword::ON,
        Keyword::OPERATOR,
        Keyword::OUT,
        Keyword::PART,
        Keyword::PATCH,
        Keyword::REQUIRED,
        Keyword::RETHROW,
        Keyword::RETURN,
        Keyword::SEALED,
        Keyword::SET,
        Keyword::SHOW,
        Keyword::SOURCE,
        Keyword::STATIC,
        Keyword::SUPER,
        Keyword::SWITCH,
        Keyword::SYNC,
        Keyword::THIS,
        Keyword::THROW,
        Keyword::TRUE,
        Keyword::TRY,
        Keyword::TYPEDEF,
        Keyword::VAR,
        Keyword::VOID,
        Keyword::WHEN,
        Keyword::WHILE,
        Keyword::WITH,
        Keyword::YIELD,
    ];
}

pub(crate) struct TokenTypeInfo {
    pub lexeme: &'static str,
    pub name: &'static str,
    pub precedence: u8,
    pub kind: i32,
    pub flags: u8,
    pub keyword_style: Option<KeywordStyle>,
    pub compound: Option<TokenType>,
}

pub(crate) const F_OPERATOR: u8 = 1;
pub(crate) const F_BINARY_OPERATOR: u8 = 2;
pub(crate) const F_MODIFIER: u8 = 4;
pub(crate) const F_TOP_LEVEL_KEYWORD: u8 = 8;
pub(crate) const F_USER_DEFINABLE_OPERATOR: u8 = 16;
pub(crate) const F_STRING_VALUE_NULL: u8 = 32;

const UNUSED_INFO: TokenTypeInfo = TokenTypeInfo {
    lexeme: "",
    name: "UNUSED",
    precedence: 0,
    kind: EOF_TOKEN,
    flags: 0,
    keyword_style: None,
    compound: None,
};

pub(crate) static TOKEN_TYPE_INFO: [TokenTypeInfo; 256] = {
    let mut t = [UNUSED_INFO; 256];
    t[0] = TokenTypeInfo {
        lexeme: "",
        name: "EOF",
        precedence: 0,
        kind: EOF_TOKEN,
        flags: 0,
        keyword_style: None,
        compound: None,
    };
    t[1] = TokenTypeInfo {
        lexeme: "double",
        name: "DOUBLE",
        precedence: 0,
        kind: DOUBLE_TOKEN,
        flags: F_STRING_VALUE_NULL,
        keyword_style: None,
        compound: None,
    };
    t[2] = TokenTypeInfo {
        lexeme: "double",
        name: "DOUBLE_WITH_SEPARATORS",
        precedence: 0,
        kind: DOUBLE_TOKEN,
        flags: F_STRING_VALUE_NULL,
        keyword_style: None,
        compound: None,
    };
    t[3] = TokenTypeInfo {
        lexeme: "hexadecimal",
        name: "HEXADECIMAL",
        precedence: 0,
        kind: HEXADECIMAL_TOKEN,
        flags: F_STRING_VALUE_NULL,
        keyword_style: None,
        compound: None,
    };
    t[4] = TokenTypeInfo {
        lexeme: "hexadecimal",
        name: "HEXADECIMAL_WITH_SEPARATORS",
        precedence: 0,
        kind: HEXADECIMAL_TOKEN,
        flags: F_STRING_VALUE_NULL,
        keyword_style: None,
        compound: None,
    };
    t[5] = TokenTypeInfo {
        lexeme: "identifier",
        name: "IDENTIFIER",
        precedence: 0,
        kind: IDENTIFIER_TOKEN,
        flags: F_STRING_VALUE_NULL,
        keyword_style: None,
        compound: None,
    };
    t[6] = TokenTypeInfo {
        lexeme: "int",
        name: "INT",
        precedence: 0,
        kind: INT_TOKEN,
        flags: F_STRING_VALUE_NULL,
        keyword_style: None,
        compound: None,
    };
    t[7] = TokenTypeInfo {
        lexeme: "int",
        name: "INT_WITH_SEPARATORS",
        precedence: 0,
        kind: INT_TOKEN,
        flags: F_STRING_VALUE_NULL,
        keyword_style: None,
        compound: None,
    };
    t[8] = TokenTypeInfo {
        lexeme: "comment",
        name: "MULTI_LINE_COMMENT",
        precedence: 0,
        kind: COMMENT_TOKEN,
        flags: F_STRING_VALUE_NULL,
        keyword_style: None,
        compound: None,
    };
    t[9] = TokenTypeInfo {
        lexeme: "script",
        name: "SCRIPT_TAG",
        precedence: 0,
        kind: SCRIPT_TOKEN,
        flags: 0,
        keyword_style: None,
        compound: None,
    };
    t[10] = TokenTypeInfo {
        lexeme: "comment",
        name: "SINGLE_LINE_COMMENT",
        precedence: 0,
        kind: COMMENT_TOKEN,
        flags: F_STRING_VALUE_NULL,
        keyword_style: None,
        compound: None,
    };
    t[11] = TokenTypeInfo {
        lexeme: "string",
        name: "STRING",
        precedence: 0,
        kind: STRING_TOKEN,
        flags: F_STRING_VALUE_NULL,
        keyword_style: None,
        compound: None,
    };
    t[12] = TokenTypeInfo {
        lexeme: "&",
        name: "AMPERSAND",
        precedence: 11,
        kind: AMPERSAND_TOKEN,
        flags: F_OPERATOR | F_BINARY_OPERATOR | F_USER_DEFINABLE_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[13] = TokenTypeInfo {
        lexeme: "&&",
        name: "AMPERSAND_AMPERSAND",
        precedence: 6,
        kind: AMPERSAND_AMPERSAND_TOKEN,
        flags: F_OPERATOR | F_BINARY_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[14] = TokenTypeInfo {
        lexeme: "&&=",
        name: "AMPERSAND_AMPERSAND_EQ",
        precedence: 1,
        kind: AMPERSAND_AMPERSAND_EQ_TOKEN,
        flags: F_OPERATOR,
        keyword_style: None,
        compound: Some(TokenType::AMPERSAND_AMPERSAND),
    };
    t[15] = TokenTypeInfo {
        lexeme: "&=",
        name: "AMPERSAND_EQ",
        precedence: 1,
        kind: AMPERSAND_EQ_TOKEN,
        flags: F_OPERATOR,
        keyword_style: None,
        compound: Some(TokenType::AMPERSAND),
    };
    t[16] = TokenTypeInfo {
        lexeme: "@",
        name: "AT",
        precedence: 0,
        kind: AT_TOKEN,
        flags: 0,
        keyword_style: None,
        compound: None,
    };
    t[17] = TokenTypeInfo {
        lexeme: "!",
        name: "BANG",
        precedence: 15,
        kind: BANG_TOKEN,
        flags: F_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[18] = TokenTypeInfo {
        lexeme: "!=",
        name: "BANG_EQ",
        precedence: 7,
        kind: BANG_EQ_TOKEN,
        flags: F_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[19] = TokenTypeInfo {
        lexeme: "!==",
        name: "BANG_EQ_EQ",
        precedence: 7,
        kind: BANG_EQ_EQ_TOKEN,
        flags: 0,
        keyword_style: None,
        compound: None,
    };
    t[20] = TokenTypeInfo {
        lexeme: "|",
        name: "BAR",
        precedence: 9,
        kind: BAR_TOKEN,
        flags: F_OPERATOR | F_BINARY_OPERATOR | F_USER_DEFINABLE_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[21] = TokenTypeInfo {
        lexeme: "||",
        name: "BAR_BAR",
        precedence: 5,
        kind: BAR_BAR_TOKEN,
        flags: F_OPERATOR | F_BINARY_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[22] = TokenTypeInfo {
        lexeme: "||=",
        name: "BAR_BAR_EQ",
        precedence: 1,
        kind: BAR_BAR_EQ_TOKEN,
        flags: F_OPERATOR,
        keyword_style: None,
        compound: Some(TokenType::BAR_BAR),
    };
    t[23] = TokenTypeInfo {
        lexeme: "|=",
        name: "BAR_EQ",
        precedence: 1,
        kind: BAR_EQ_TOKEN,
        flags: F_OPERATOR,
        keyword_style: None,
        compound: Some(TokenType::BAR),
    };
    t[24] = TokenTypeInfo {
        lexeme: ":",
        name: "COLON",
        precedence: 0,
        kind: COLON_TOKEN,
        flags: 0,
        keyword_style: None,
        compound: None,
    };
    t[25] = TokenTypeInfo {
        lexeme: ",",
        name: "COMMA",
        precedence: 0,
        kind: COMMA_TOKEN,
        flags: 0,
        keyword_style: None,
        compound: None,
    };
    t[26] = TokenTypeInfo {
        lexeme: "^",
        name: "CARET",
        precedence: 10,
        kind: CARET_TOKEN,
        flags: F_OPERATOR | F_BINARY_OPERATOR | F_USER_DEFINABLE_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[27] = TokenTypeInfo {
        lexeme: "^=",
        name: "CARET_EQ",
        precedence: 1,
        kind: CARET_EQ_TOKEN,
        flags: F_OPERATOR,
        keyword_style: None,
        compound: Some(TokenType::CARET),
    };
    t[28] = TokenTypeInfo {
        lexeme: "}",
        name: "CLOSE_CURLY_BRACKET",
        precedence: 0,
        kind: CLOSE_CURLY_BRACKET_TOKEN,
        flags: 0,
        keyword_style: None,
        compound: None,
    };
    t[29] = TokenTypeInfo {
        lexeme: ")",
        name: "CLOSE_PAREN",
        precedence: 0,
        kind: CLOSE_PAREN_TOKEN,
        flags: 0,
        keyword_style: None,
        compound: None,
    };
    t[30] = TokenTypeInfo {
        lexeme: "]",
        name: "CLOSE_SQUARE_BRACKET",
        precedence: 0,
        kind: CLOSE_SQUARE_BRACKET_TOKEN,
        flags: 0,
        keyword_style: None,
        compound: None,
    };
    t[31] = TokenTypeInfo {
        lexeme: "=",
        name: "EQ",
        precedence: 1,
        kind: EQ_TOKEN,
        flags: F_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[32] = TokenTypeInfo {
        lexeme: "==",
        name: "EQ_EQ",
        precedence: 7,
        kind: EQ_EQ_TOKEN,
        flags: F_OPERATOR | F_BINARY_OPERATOR | F_USER_DEFINABLE_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[33] = TokenTypeInfo {
        lexeme: "===",
        name: "EQ_EQ_EQ",
        precedence: 7,
        kind: EQ_EQ_EQ_TOKEN,
        flags: 0,
        keyword_style: None,
        compound: None,
    };
    t[34] = TokenTypeInfo {
        lexeme: "=>",
        name: "FUNCTION",
        precedence: 0,
        kind: FUNCTION_TOKEN,
        flags: 0,
        keyword_style: None,
        compound: None,
    };
    t[35] = TokenTypeInfo {
        lexeme: ">",
        name: "GT",
        precedence: 8,
        kind: GT_TOKEN,
        flags: F_OPERATOR | F_BINARY_OPERATOR | F_USER_DEFINABLE_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[36] = TokenTypeInfo {
        lexeme: ">=",
        name: "GT_EQ",
        precedence: 8,
        kind: GT_EQ_TOKEN,
        flags: F_OPERATOR | F_BINARY_OPERATOR | F_USER_DEFINABLE_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[37] = TokenTypeInfo {
        lexeme: ">>",
        name: "GT_GT",
        precedence: 12,
        kind: GT_GT_TOKEN,
        flags: F_OPERATOR | F_BINARY_OPERATOR | F_USER_DEFINABLE_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[38] = TokenTypeInfo {
        lexeme: ">>=",
        name: "GT_GT_EQ",
        precedence: 1,
        kind: GT_GT_EQ_TOKEN,
        flags: F_OPERATOR,
        keyword_style: None,
        compound: Some(TokenType::GT_GT),
    };
    t[39] = TokenTypeInfo {
        lexeme: ">>>",
        name: "GT_GT_GT",
        precedence: 12,
        kind: GT_GT_GT_TOKEN,
        flags: F_OPERATOR | F_BINARY_OPERATOR | F_USER_DEFINABLE_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[40] = TokenTypeInfo {
        lexeme: ">>>=",
        name: "GT_GT_GT_EQ",
        precedence: 1,
        kind: GT_GT_GT_EQ_TOKEN,
        flags: F_OPERATOR,
        keyword_style: None,
        compound: Some(TokenType::GT_GT_GT),
    };
    t[41] = TokenTypeInfo {
        lexeme: "#",
        name: "HASH",
        precedence: 0,
        kind: HASH_TOKEN,
        flags: 0,
        keyword_style: None,
        compound: None,
    };
    t[42] = TokenTypeInfo {
        lexeme: "[]",
        name: "INDEX",
        precedence: 17,
        kind: INDEX_TOKEN,
        flags: F_OPERATOR | F_USER_DEFINABLE_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[43] = TokenTypeInfo {
        lexeme: "[]=",
        name: "INDEX_EQ",
        precedence: 0,
        kind: INDEX_EQ_TOKEN,
        flags: F_OPERATOR | F_USER_DEFINABLE_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[44] = TokenTypeInfo {
        lexeme: "<",
        name: "LT",
        precedence: 8,
        kind: LT_TOKEN,
        flags: F_OPERATOR | F_BINARY_OPERATOR | F_USER_DEFINABLE_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[45] = TokenTypeInfo {
        lexeme: "<=",
        name: "LT_EQ",
        precedence: 8,
        kind: LT_EQ_TOKEN,
        flags: F_OPERATOR | F_BINARY_OPERATOR | F_USER_DEFINABLE_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[46] = TokenTypeInfo {
        lexeme: "<<",
        name: "LT_LT",
        precedence: 12,
        kind: LT_LT_TOKEN,
        flags: F_OPERATOR | F_BINARY_OPERATOR | F_USER_DEFINABLE_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[47] = TokenTypeInfo {
        lexeme: "<<=",
        name: "LT_LT_EQ",
        precedence: 1,
        kind: LT_LT_EQ_TOKEN,
        flags: F_OPERATOR,
        keyword_style: None,
        compound: Some(TokenType::LT_LT),
    };
    t[48] = TokenTypeInfo {
        lexeme: "-",
        name: "MINUS",
        precedence: 13,
        kind: MINUS_TOKEN,
        flags: F_OPERATOR | F_BINARY_OPERATOR | F_USER_DEFINABLE_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[49] = TokenTypeInfo {
        lexeme: "-=",
        name: "MINUS_EQ",
        precedence: 1,
        kind: MINUS_EQ_TOKEN,
        flags: F_OPERATOR,
        keyword_style: None,
        compound: Some(TokenType::MINUS),
    };
    t[50] = TokenTypeInfo {
        lexeme: "--",
        name: "MINUS_MINUS",
        precedence: 16,
        kind: MINUS_MINUS_TOKEN,
        flags: F_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[51] = TokenTypeInfo {
        lexeme: "{",
        name: "OPEN_CURLY_BRACKET",
        precedence: 0,
        kind: OPEN_CURLY_BRACKET_TOKEN,
        flags: 0,
        keyword_style: None,
        compound: None,
    };
    t[52] = TokenTypeInfo {
        lexeme: "(",
        name: "OPEN_PAREN",
        precedence: 17,
        kind: OPEN_PAREN_TOKEN,
        flags: 0,
        keyword_style: None,
        compound: None,
    };
    t[53] = TokenTypeInfo {
        lexeme: "[",
        name: "OPEN_SQUARE_BRACKET",
        precedence: 17,
        kind: OPEN_SQUARE_BRACKET_TOKEN,
        flags: 0,
        keyword_style: None,
        compound: None,
    };
    t[54] = TokenTypeInfo {
        lexeme: "%",
        name: "PERCENT",
        precedence: 14,
        kind: PERCENT_TOKEN,
        flags: F_OPERATOR | F_BINARY_OPERATOR | F_USER_DEFINABLE_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[55] = TokenTypeInfo {
        lexeme: "%=",
        name: "PERCENT_EQ",
        precedence: 1,
        kind: PERCENT_EQ_TOKEN,
        flags: F_OPERATOR,
        keyword_style: None,
        compound: Some(TokenType::PERCENT),
    };
    t[56] = TokenTypeInfo {
        lexeme: ".",
        name: "PERIOD",
        precedence: 17,
        kind: PERIOD_TOKEN,
        flags: 0,
        keyword_style: None,
        compound: None,
    };
    t[57] = TokenTypeInfo {
        lexeme: "..",
        name: "PERIOD_PERIOD",
        precedence: 2,
        kind: PERIOD_PERIOD_TOKEN,
        flags: F_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[58] = TokenTypeInfo {
        lexeme: "+",
        name: "PLUS",
        precedence: 13,
        kind: PLUS_TOKEN,
        flags: F_OPERATOR | F_BINARY_OPERATOR | F_USER_DEFINABLE_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[59] = TokenTypeInfo {
        lexeme: "+=",
        name: "PLUS_EQ",
        precedence: 1,
        kind: PLUS_EQ_TOKEN,
        flags: F_OPERATOR,
        keyword_style: None,
        compound: Some(TokenType::PLUS),
    };
    t[60] = TokenTypeInfo {
        lexeme: "++",
        name: "PLUS_PLUS",
        precedence: 16,
        kind: PLUS_PLUS_TOKEN,
        flags: F_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[61] = TokenTypeInfo {
        lexeme: "?",
        name: "QUESTION",
        precedence: 3,
        kind: QUESTION_TOKEN,
        flags: F_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[62] = TokenTypeInfo {
        lexeme: "?.",
        name: "QUESTION_PERIOD",
        precedence: 17,
        kind: QUESTION_PERIOD_TOKEN,
        flags: F_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[63] = TokenTypeInfo {
        lexeme: "??",
        name: "QUESTION_QUESTION",
        precedence: 4,
        kind: QUESTION_QUESTION_TOKEN,
        flags: F_OPERATOR | F_BINARY_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[64] = TokenTypeInfo {
        lexeme: "??=",
        name: "QUESTION_QUESTION_EQ",
        precedence: 1,
        kind: QUESTION_QUESTION_EQ_TOKEN,
        flags: F_OPERATOR,
        keyword_style: None,
        compound: Some(TokenType::QUESTION_QUESTION),
    };
    t[65] = TokenTypeInfo {
        lexeme: ";",
        name: "SEMICOLON",
        precedence: 0,
        kind: SEMICOLON_TOKEN,
        flags: 0,
        keyword_style: None,
        compound: None,
    };
    t[66] = TokenTypeInfo {
        lexeme: "/",
        name: "SLASH",
        precedence: 14,
        kind: SLASH_TOKEN,
        flags: F_OPERATOR | F_BINARY_OPERATOR | F_USER_DEFINABLE_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[67] = TokenTypeInfo {
        lexeme: "/=",
        name: "SLASH_EQ",
        precedence: 1,
        kind: SLASH_EQ_TOKEN,
        flags: F_OPERATOR,
        keyword_style: None,
        compound: Some(TokenType::SLASH),
    };
    t[68] = TokenTypeInfo {
        lexeme: "*",
        name: "STAR",
        precedence: 14,
        kind: STAR_TOKEN,
        flags: F_OPERATOR | F_BINARY_OPERATOR | F_USER_DEFINABLE_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[69] = TokenTypeInfo {
        lexeme: "*=",
        name: "STAR_EQ",
        precedence: 1,
        kind: STAR_EQ_TOKEN,
        flags: F_OPERATOR,
        keyword_style: None,
        compound: Some(TokenType::STAR),
    };
    t[70] = TokenTypeInfo {
        lexeme: "${",
        name: "STRING_INTERPOLATION_EXPRESSION",
        precedence: 0,
        kind: STRING_INTERPOLATION_TOKEN,
        flags: 0,
        keyword_style: None,
        compound: None,
    };
    t[71] = TokenTypeInfo {
        lexeme: "$",
        name: "STRING_INTERPOLATION_IDENTIFIER",
        precedence: 0,
        kind: STRING_INTERPOLATION_IDENTIFIER_TOKEN,
        flags: 0,
        keyword_style: None,
        compound: None,
    };
    t[72] = TokenTypeInfo {
        lexeme: "~",
        name: "TILDE",
        precedence: 15,
        kind: TILDE_TOKEN,
        flags: F_OPERATOR | F_USER_DEFINABLE_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[73] = TokenTypeInfo {
        lexeme: "~/",
        name: "TILDE_SLASH",
        precedence: 14,
        kind: TILDE_SLASH_TOKEN,
        flags: F_OPERATOR | F_BINARY_OPERATOR | F_USER_DEFINABLE_OPERATOR,
        keyword_style: None,
        compound: None,
    };
    t[74] = TokenTypeInfo {
        lexeme: "~/=",
        name: "TILDE_SLASH_EQ",
        precedence: 1,
        kind: TILDE_SLASH_EQ_TOKEN,
        flags: F_OPERATOR,
        keyword_style: None,
        compound: Some(TokenType::TILDE_SLASH),
    };
    t[75] = TokenTypeInfo {
        lexeme: "`",
        name: "BACKPING",
        precedence: 0,
        kind: BACKPING_TOKEN,
        flags: 0,
        keyword_style: None,
        compound: None,
    };
    t[76] = TokenTypeInfo {
        lexeme: "\\",
        name: "BACKSLASH",
        precedence: 0,
        kind: BACKSLASH_TOKEN,
        flags: 0,
        keyword_style: None,
        compound: None,
    };
    t[77] = TokenTypeInfo {
        lexeme: "...",
        name: "PERIOD_PERIOD_PERIOD",
        precedence: 0,
        kind: PERIOD_PERIOD_PERIOD_TOKEN,
        flags: 0,
        keyword_style: None,
        compound: None,
    };
    t[78] = TokenTypeInfo {
        lexeme: "...?",
        name: "PERIOD_PERIOD_PERIOD_QUESTION",
        precedence: 0,
        kind: PERIOD_PERIOD_PERIOD_QUESTION_TOKEN,
        flags: 0,
        keyword_style: None,
        compound: None,
    };
    t[79] = TokenTypeInfo {
        lexeme: "?..",
        name: "QUESTION_PERIOD_PERIOD",
        precedence: 2,
        kind: QUESTION_PERIOD_PERIOD_TOKEN,
        flags: 0,
        keyword_style: None,
        compound: None,
    };
    t[80] = TokenTypeInfo {
        lexeme: "malformed input",
        name: "BAD_INPUT",
        precedence: 0,
        kind: BAD_INPUT_TOKEN,
        flags: F_STRING_VALUE_NULL,
        keyword_style: None,
        compound: None,
    };
    t[81] = TokenTypeInfo {
        lexeme: "recovery",
        name: "RECOVERY",
        precedence: 0,
        kind: RECOVERY_TOKEN,
        flags: F_STRING_VALUE_NULL,
        keyword_style: None,
        compound: None,
    };
    t[82] = TokenTypeInfo {
        lexeme: "abstract",
        name: "ABSTRACT",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: F_MODIFIER,
        keyword_style: Some(KeywordStyle::BuiltIn),
        compound: None,
    };
    t[83] = TokenTypeInfo {
        lexeme: "as",
        name: "AS",
        precedence: 8,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::BuiltIn),
        compound: None,
    };
    t[84] = TokenTypeInfo {
        lexeme: "assert",
        name: "ASSERT",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[85] = TokenTypeInfo {
        lexeme: "async",
        name: "ASYNC",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Pseudo),
        compound: None,
    };
    t[86] = TokenTypeInfo {
        lexeme: "augment",
        name: "AUGMENT",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: F_MODIFIER,
        keyword_style: Some(KeywordStyle::BuiltIn),
        compound: None,
    };
    t[87] = TokenTypeInfo {
        lexeme: "await",
        name: "AWAIT",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Pseudo),
        compound: None,
    };
    t[88] = TokenTypeInfo {
        lexeme: "base",
        name: "BASE",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Pseudo),
        compound: None,
    };
    t[89] = TokenTypeInfo {
        lexeme: "break",
        name: "BREAK",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[90] = TokenTypeInfo {
        lexeme: "case",
        name: "CASE",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[91] = TokenTypeInfo {
        lexeme: "catch",
        name: "CATCH",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[92] = TokenTypeInfo {
        lexeme: "class",
        name: "CLASS",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: F_TOP_LEVEL_KEYWORD,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[93] = TokenTypeInfo {
        lexeme: "const",
        name: "CONST",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: F_MODIFIER,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[94] = TokenTypeInfo {
        lexeme: "continue",
        name: "CONTINUE",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[95] = TokenTypeInfo {
        lexeme: "covariant",
        name: "COVARIANT",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: F_MODIFIER,
        keyword_style: Some(KeywordStyle::BuiltIn),
        compound: None,
    };
    t[96] = TokenTypeInfo {
        lexeme: "default",
        name: "DEFAULT",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[97] = TokenTypeInfo {
        lexeme: "deferred",
        name: "DEFERRED",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::BuiltIn),
        compound: None,
    };
    t[98] = TokenTypeInfo {
        lexeme: "do",
        name: "DO",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[99] = TokenTypeInfo {
        lexeme: "dynamic",
        name: "DYNAMIC",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::BuiltIn),
        compound: None,
    };
    t[100] = TokenTypeInfo {
        lexeme: "else",
        name: "ELSE",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[101] = TokenTypeInfo {
        lexeme: "enum",
        name: "ENUM",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: F_TOP_LEVEL_KEYWORD,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[102] = TokenTypeInfo {
        lexeme: "export",
        name: "EXPORT",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: F_TOP_LEVEL_KEYWORD,
        keyword_style: Some(KeywordStyle::BuiltIn),
        compound: None,
    };
    t[103] = TokenTypeInfo {
        lexeme: "extends",
        name: "EXTENDS",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[104] = TokenTypeInfo {
        lexeme: "extension",
        name: "EXTENSION",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: F_TOP_LEVEL_KEYWORD,
        keyword_style: Some(KeywordStyle::BuiltIn),
        compound: None,
    };
    t[105] = TokenTypeInfo {
        lexeme: "external",
        name: "EXTERNAL",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: F_MODIFIER,
        keyword_style: Some(KeywordStyle::BuiltIn),
        compound: None,
    };
    t[106] = TokenTypeInfo {
        lexeme: "factory",
        name: "FACTORY",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::BuiltIn),
        compound: None,
    };
    t[107] = TokenTypeInfo {
        lexeme: "false",
        name: "FALSE",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[108] = TokenTypeInfo {
        lexeme: "final",
        name: "FINAL",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: F_MODIFIER,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[109] = TokenTypeInfo {
        lexeme: "finally",
        name: "FINALLY",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[110] = TokenTypeInfo {
        lexeme: "for",
        name: "FOR",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[111] = TokenTypeInfo {
        lexeme: "Function",
        name: "FUNCTION",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::BuiltIn),
        compound: None,
    };
    t[112] = TokenTypeInfo {
        lexeme: "get",
        name: "GET",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::BuiltIn),
        compound: None,
    };
    t[113] = TokenTypeInfo {
        lexeme: "hide",
        name: "HIDE",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Pseudo),
        compound: None,
    };
    t[114] = TokenTypeInfo {
        lexeme: "if",
        name: "IF",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[115] = TokenTypeInfo {
        lexeme: "implements",
        name: "IMPLEMENTS",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::BuiltIn),
        compound: None,
    };
    t[116] = TokenTypeInfo {
        lexeme: "import",
        name: "IMPORT",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: F_TOP_LEVEL_KEYWORD,
        keyword_style: Some(KeywordStyle::BuiltIn),
        compound: None,
    };
    t[117] = TokenTypeInfo {
        lexeme: "in",
        name: "IN",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[118] = TokenTypeInfo {
        lexeme: "inout",
        name: "INOUT",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Pseudo),
        compound: None,
    };
    t[119] = TokenTypeInfo {
        lexeme: "interface",
        name: "INTERFACE",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::BuiltIn),
        compound: None,
    };
    t[120] = TokenTypeInfo {
        lexeme: "is",
        name: "IS",
        precedence: 8,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[121] = TokenTypeInfo {
        lexeme: "late",
        name: "LATE",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: F_MODIFIER,
        keyword_style: Some(KeywordStyle::BuiltIn),
        compound: None,
    };
    t[122] = TokenTypeInfo {
        lexeme: "library",
        name: "LIBRARY",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: F_TOP_LEVEL_KEYWORD,
        keyword_style: Some(KeywordStyle::BuiltIn),
        compound: None,
    };
    t[123] = TokenTypeInfo {
        lexeme: "mixin",
        name: "MIXIN",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: F_TOP_LEVEL_KEYWORD,
        keyword_style: Some(KeywordStyle::BuiltIn),
        compound: None,
    };
    t[124] = TokenTypeInfo {
        lexeme: "native",
        name: "NATIVE",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Pseudo),
        compound: None,
    };
    t[125] = TokenTypeInfo {
        lexeme: "new",
        name: "NEW",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[126] = TokenTypeInfo {
        lexeme: "null",
        name: "NULL",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[127] = TokenTypeInfo {
        lexeme: "of",
        name: "OF",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Pseudo),
        compound: None,
    };
    t[128] = TokenTypeInfo {
        lexeme: "on",
        name: "ON",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Pseudo),
        compound: None,
    };
    t[129] = TokenTypeInfo {
        lexeme: "operator",
        name: "OPERATOR",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::BuiltIn),
        compound: None,
    };
    t[130] = TokenTypeInfo {
        lexeme: "out",
        name: "OUT",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Pseudo),
        compound: None,
    };
    t[131] = TokenTypeInfo {
        lexeme: "part",
        name: "PART",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: F_TOP_LEVEL_KEYWORD,
        keyword_style: Some(KeywordStyle::BuiltIn),
        compound: None,
    };
    t[132] = TokenTypeInfo {
        lexeme: "patch",
        name: "PATCH",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Pseudo),
        compound: None,
    };
    t[133] = TokenTypeInfo {
        lexeme: "required",
        name: "REQUIRED",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: F_MODIFIER,
        keyword_style: Some(KeywordStyle::BuiltIn),
        compound: None,
    };
    t[134] = TokenTypeInfo {
        lexeme: "rethrow",
        name: "RETHROW",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[135] = TokenTypeInfo {
        lexeme: "return",
        name: "RETURN",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[136] = TokenTypeInfo {
        lexeme: "sealed",
        name: "SEALED",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Pseudo),
        compound: None,
    };
    t[137] = TokenTypeInfo {
        lexeme: "set",
        name: "SET",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::BuiltIn),
        compound: None,
    };
    t[138] = TokenTypeInfo {
        lexeme: "show",
        name: "SHOW",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Pseudo),
        compound: None,
    };
    t[139] = TokenTypeInfo {
        lexeme: "source",
        name: "SOURCE",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Pseudo),
        compound: None,
    };
    t[140] = TokenTypeInfo {
        lexeme: "static",
        name: "STATIC",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: F_MODIFIER,
        keyword_style: Some(KeywordStyle::BuiltIn),
        compound: None,
    };
    t[141] = TokenTypeInfo {
        lexeme: "super",
        name: "SUPER",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[142] = TokenTypeInfo {
        lexeme: "switch",
        name: "SWITCH",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[143] = TokenTypeInfo {
        lexeme: "sync",
        name: "SYNC",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Pseudo),
        compound: None,
    };
    t[144] = TokenTypeInfo {
        lexeme: "this",
        name: "THIS",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[145] = TokenTypeInfo {
        lexeme: "throw",
        name: "THROW",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[146] = TokenTypeInfo {
        lexeme: "true",
        name: "TRUE",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[147] = TokenTypeInfo {
        lexeme: "try",
        name: "TRY",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[148] = TokenTypeInfo {
        lexeme: "typedef",
        name: "TYPEDEF",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: F_TOP_LEVEL_KEYWORD,
        keyword_style: Some(KeywordStyle::BuiltIn),
        compound: None,
    };
    t[149] = TokenTypeInfo {
        lexeme: "var",
        name: "VAR",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: F_MODIFIER,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[150] = TokenTypeInfo {
        lexeme: "void",
        name: "VOID",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[151] = TokenTypeInfo {
        lexeme: "when",
        name: "WHEN",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Pseudo),
        compound: None,
    };
    t[152] = TokenTypeInfo {
        lexeme: "while",
        name: "WHILE",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[153] = TokenTypeInfo {
        lexeme: "with",
        name: "WITH",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Reserved),
        compound: None,
    };
    t[154] = TokenTypeInfo {
        lexeme: "yield",
        name: "YIELD",
        precedence: 0,
        kind: KEYWORD_TOKEN,
        flags: 0,
        keyword_style: Some(KeywordStyle::Pseudo),
        compound: None,
    };
    t
};
