// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/literal_entry_info.dart

//! Dart `LiteralEntryInfo` is a class hierarchy with some mutable instances
//! (`ForCondition`, `Nested`); here it is an enum that is moved through the
//! steps: Dart `info = info.computeNext(token)` is
//! `info = info.compute_next(tokens, token)`. Each Dart class (also the ones
//! of `literal_entry_info_impl.dart`) is one variant; the `parse` and
//! `computeNext` bodies of the impl classes are in
//! [`crate::literal_entry_info_impl`].

use dartr_syntax::{Keyword, TokenId, TokenType, Tokens};

use crate::identifier_context::looks_like_expression_start;
use crate::listener::Listener;
use crate::literal_entry_info_impl::{IF_CONDITION, NULL_AWARE_ENTRY, SPREAD_OPERATOR};
use crate::parser_impl::Parser;

/// Dart `LiteralEntryInfo` (line 35).
///
/// [LiteralEntryInfo] represents steps for processing an entry
/// in a literal list, map, or set. These steps will handle parsing
/// both control flow and spreadable operators, and indicate
/// when the client should parse the literal entry.
///
/// Clients should parse a single entry in a list, set, or map like this:
/// ```text
///    LiteralEntryInfo info = computeLiteralEntry(token);
///    while (info != null) {
///      if (info.hasEntry) {
///        ... parse expression (`:` expression)? ...
///        token = lastConsumedToken;
///      } else {
///        token = info.parse(token, parser);
///      }
///      info = info.computeNext(token);
///    }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LiteralEntryInfo {
    /// Dart `simpleEntry` (a plain `LiteralEntryInfo(hasEntry: true,
    /// ifConditionDelta: 0)`).
    Simple,
    /// Dart `ForCondition`; `in_style` is Dart `_inStyle`.
    ForCondition { in_style: bool },
    /// Dart `ForSpread` (a `SpreadOperator`).
    ForSpread,
    /// Dart `ForInSpread` (a `SpreadOperator`).
    ForInSpread,
    /// Dart `ForEntry`.
    ForEntry,
    /// Dart `ForInEntry`.
    ForInEntry,
    /// Dart `ForComplete`.
    ForComplete,
    /// Dart `ForInComplete`.
    ForInComplete,
    /// Dart `IfCondition` (`ifCondition`).
    IfCondition,
    /// Dart `IfSpread` (a `SpreadOperator`).
    IfSpread,
    /// Dart `IfEntry`.
    IfEntry,
    /// Dart `IfComplete`.
    IfComplete,
    /// Dart `IfElse`.
    IfElse,
    /// Dart `ElseSpread` (a `SpreadOperator`).
    ElseSpread,
    /// Dart `ElseEntry`.
    ElseEntry,
    /// Dart `IfElseComplete`.
    IfElseComplete,
    /// Dart `SpreadOperator` (`spreadOperator`).
    SpreadOperator,
    /// Dart `Nested`: `nested_step` is Dart `nestedStep` (never `None`
    /// while the value is in use, as in Dart), `last_step` Dart `lastStep`.
    Nested {
        nested_step: Option<Box<LiteralEntryInfo>>,
        last_step: Box<LiteralEntryInfo>,
    },
    /// Dart `NullAwareEntry` (`nullAwareEntry`).
    NullAwareEntry,
}

/// Dart (line 12): `simpleEntry`.
///
/// [simpleEntry] is the first step for parsing a literal entry
/// without any control flow or spread collection operator.
pub const SIMPLE_ENTRY: LiteralEntryInfo = LiteralEntryInfo::Simple;

impl LiteralEntryInfo {
    /// Dart `hasEntry`.
    ///
    /// `true` if an entry should be parsed by the caller
    /// or `false` if this object's [parse] method should be called.
    pub fn has_entry(&self) -> bool {
        use LiteralEntryInfo::*;
        match self {
            Simple => true,
            ForCondition { .. } => false,
            ForSpread | ForInSpread | IfSpread | ElseSpread | SpreadOperator => false,
            ForEntry | ForInEntry => true,
            ForComplete | ForInComplete => false,
            IfCondition => false,
            IfEntry => true,
            IfComplete => false,
            IfElse => false,
            ElseEntry => true,
            IfElseComplete => false,
            // Dart (impl line 338): `bool get hasEntry => nestedStep!.hasEntry;`
            Nested { nested_step, .. } => nested_step.as_ref().unwrap().has_entry(),
            NullAwareEntry => true,
        }
    }

    /// Dart `ifConditionDelta`.
    ///
    /// Used for recovery, this indicates
    /// +1 for an `if` condition and -1 for `else`.
    pub fn if_condition_delta(&self) -> i32 {
        use LiteralEntryInfo::*;
        match self {
            IfCondition => 1,
            IfElse => -1,
            // `Nested` does not delegate `ifConditionDelta` (it is 0).
            Simple
            | ForCondition { .. }
            | ForSpread
            | ForInSpread
            | ForEntry
            | ForInEntry
            | ForComplete
            | ForInComplete
            | IfSpread
            | IfEntry
            | IfComplete
            | ElseSpread
            | ElseEntry
            | IfElseComplete
            | SpreadOperator
            | Nested { .. }
            | NullAwareEntry => 0,
        }
    }

    /// Dart (line 50): `Token parse(Token token, Parser parser)`.
    ///
    /// Parse the control flow and spread collection aspects of this entry.
    pub fn parse<L: Listener>(&mut self, token: TokenId, parser: &mut Parser<L>) -> TokenId {
        use LiteralEntryInfo::*;
        match self {
            ForCondition { in_style } => {
                crate::literal_entry_info_impl::for_condition_parse(in_style, token, parser)
            }
            // `ForSpread`, `ForInSpread`, `IfSpread` and `ElseSpread` extend
            // `SpreadOperator` and inherit its `parse`.
            ForSpread | ForInSpread | IfSpread | ElseSpread | SpreadOperator => {
                crate::literal_entry_info_impl::spread_operator_parse(token, parser)
            }
            ForComplete => crate::literal_entry_info_impl::for_complete_parse(token, parser),
            ForInComplete => crate::literal_entry_info_impl::for_in_complete_parse(token, parser),
            IfCondition => crate::literal_entry_info_impl::if_condition_parse(token, parser),
            IfComplete => crate::literal_entry_info_impl::if_complete_parse(token, parser),
            IfElse => crate::literal_entry_info_impl::if_else_parse(token, parser),
            IfElseComplete => crate::literal_entry_info_impl::if_else_complete_parse(token, parser),
            // Dart (impl line 341): `nestedStep!.parse(token, parser)`.
            Nested { nested_step, .. } => nested_step.as_mut().unwrap().parse(token, parser),
            Simple | ForEntry | ForInEntry | IfEntry | ElseEntry | NullAwareEntry => {
                if self.has_entry() {
                    panic!("Internal Error: should not call parse");
                } else {
                    panic!("Internal Error: {:?} should implement parse", self);
                }
            }
        }
    }

    /// Dart (line 57): `LiteralEntryInfo? computeNext(Token token)`.
    ///
    /// Returns the next step when parsing an entry or `null` if none.
    pub fn compute_next(self, tokens: &Tokens, token: TokenId) -> Option<LiteralEntryInfo> {
        use LiteralEntryInfo::*;
        match self {
            ForCondition { in_style } => Some(
                crate::literal_entry_info_impl::for_condition_compute_next(in_style, tokens, token),
            ),
            ForSpread => Some(ForComplete),
            ForInSpread => Some(ForInComplete),
            ForEntry => Some(ForComplete),
            ForInEntry => Some(ForInComplete),
            IfCondition => Some(crate::literal_entry_info_impl::if_condition_compute_next(
                tokens, token,
            )),
            IfSpread => Some(IfComplete),
            IfEntry => Some(IfComplete),
            IfComplete => crate::literal_entry_info_impl::if_complete_compute_next(tokens, token),
            IfElse => Some(crate::literal_entry_info_impl::if_else_compute_next(
                tokens, token,
            )),
            ElseSpread => Some(IfElseComplete),
            ElseEntry => Some(IfElseComplete),
            Nested {
                nested_step,
                last_step,
            } => Some(crate::literal_entry_info_impl::nested_compute_next(
                nested_step,
                last_step,
                tokens,
                token,
            )),
            // The base class `computeNext` returns `null`.
            Simple | ForComplete | ForInComplete | IfElseComplete | SpreadOperator
            | NullAwareEntry => None,
        }
    }
}

/// Dart (line 61): `LiteralEntryInfo computeLiteralEntry(Token token)`.
///
/// Compute the [LiteralEntryInfo] for the literal list, map, or set entry.
pub fn compute_literal_entry(tokens: &Tokens, token: TokenId) -> LiteralEntryInfo {
    let next = tokens.next(token);
    let next_ty = tokens.ty(next);
    if next_ty == Keyword::IF {
        return IF_CONDITION;
    } else if next_ty == Keyword::FOR
        || (next_ty == Keyword::AWAIT && tokens.ty(tokens.next(next)) == Keyword::FOR)
    {
        return LiteralEntryInfo::ForCondition { in_style: false };
    } else if next_ty == TokenType::PERIOD_PERIOD_PERIOD
        || next_ty == TokenType::PERIOD_PERIOD_PERIOD_QUESTION
    {
        return SPREAD_OPERATOR;
    } else if next_ty == TokenType::QUESTION {
        return NULL_AWARE_ENTRY;
    }
    SIMPLE_ENTRY
}

/// Dart (line 79): `bool looksLikeLiteralEntry(Token token)`.
///
/// Return `true` if the given [token] should be treated like the start of
/// a literal entry in a list, set, or map for the purposes of recovery.
pub fn looks_like_literal_entry(tokens: &Tokens, token: TokenId) -> bool {
    let ty = tokens.ty(token);
    looks_like_expression_start(tokens, token)
        || ty == TokenType::PERIOD_PERIOD_PERIOD
        || ty == TokenType::PERIOD_PERIOD_PERIOD_QUESTION
        || ty == Keyword::IF
        || ty == Keyword::FOR
        || (ty == Keyword::AWAIT && tokens.ty(tokens.next(token)) == Keyword::FOR)
}
