// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/literal_entry_info_impl.dart

//! The steps of [`LiteralEntryInfo`] for control flow and spread entries.
//! Each Dart class is a variant of [`LiteralEntryInfo`] (see
//! `literal_entry_info.rs`, which also has the `hasEntry` and
//! `ifConditionDelta` values per class); this file has the bodies of the
//! overridden `parse` and `computeNext` methods. The variants without own
//! state forward to these functions from `LiteralEntryInfo::parse` and
//! `LiteralEntryInfo::compute_next`.

use dartr_syntax::{Keyword, TokenId, TokenType, Tokens};

use crate::listener::Listener;
use crate::literal_entry_info::LiteralEntryInfo;
use crate::parser_impl::{ForPartsContext, Parser};

/// Dart (line 11): `ifCondition`.
///
/// [ifCondition] is the first step for parsing a literal entry
/// starting with `if` control flow.
pub const IF_CONDITION: LiteralEntryInfo = LiteralEntryInfo::IfCondition;

/// Dart (line 15): `spreadOperator`.
///
/// [spreadOperator] is the first step for parsing a literal entry
/// preceded by a '...' spread operator.
pub const SPREAD_OPERATOR: LiteralEntryInfo = LiteralEntryInfo::SpreadOperator;

/// Dart (line 21): `nullAwareEntry`.
///
/// [nullAwareEntry] is for parsing a null-aware element in a literal list or
/// set, preceded by the `?` null-aware marker, or a null-aware map entry in a
/// map literal, where either the key or the value is preceded by the `?`
/// null-aware marker.
pub const NULL_AWARE_ENTRY: LiteralEntryInfo = LiteralEntryInfo::NullAwareEntry;

/// Dart `new Nested(nestedStep, lastStep)`.
fn nested(nested_step: LiteralEntryInfo, last_step: LiteralEntryInfo) -> LiteralEntryInfo {
    LiteralEntryInfo::Nested {
        nested_step: Some(Box::new(nested_step)),
        last_step: Box::new(last_step),
    }
}

/// Dart `new ForCondition()`.
fn new_for_condition() -> LiteralEntryInfo {
    LiteralEntryInfo::ForCondition { in_style: false }
}

/// `next.isA(Keyword.FOR) || (next.isA(Keyword.AWAIT) && next.next!.isA(Keyword.FOR))`.
fn is_for_or_await_for(tokens: &Tokens, next: TokenId) -> bool {
    tokens.ty(next) == Keyword::FOR
        || (tokens.ty(next) == Keyword::AWAIT && tokens.ty(tokens.next(next)) == Keyword::FOR)
}

// -------------------------------------------------------------------------
// ForCondition: the first step when processing a `for` control flow
// collection entry.

/// Dart (line 30): `ForCondition.parse(Token token, Parser parser)`.
/// [in_style] is the Dart field `_inStyle`.
pub(crate) fn for_condition_parse<L: Listener>(
    in_style: &mut bool,
    mut token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let mut next = parser.next(token);
    let mut await_token: Option<TokenId> = None;
    if parser.is_a(next, Keyword::AWAIT) {
        token = next;
        await_token = Some(token);
        next = parser.next(token);
    }
    let for_token = next;
    debug_assert!(parser.is_a(for_token, Keyword::FOR));
    parser
        .listener
        .begin_for_control_flow(await_token, for_token);

    let mut for_parts_context = ForPartsContext::default();
    token = parser.parse_for_loop_parts_start(await_token, for_token, &mut for_parts_context);
    let pattern_keyword = for_parts_context.pattern_keyword;
    if let Some(pattern_keyword) = pattern_keyword {
        if parser.is_a(parser.next(token), TokenType::EQ) {
            // Process `for ( pattern = expression ; ... ; ... )`
            let equals = parser.next(token);
            token = parser.parse_expression(equals);
            parser
                .listener
                .handle_for_initializer_pattern_variable_assignment(pattern_keyword, equals);
            *in_style = false;
            return parser.parse_for_loop_parts_rest(token, for_token, await_token);
        } else {
            // Process `for ( pattern in expression )`
            // assert(token.next!.isA(Keyword.IN));
            *in_style = true;
            return parser.parse_for_in_loop_parts_rest(
                token,
                await_token,
                for_token,
                Some(pattern_keyword),
                /* identifier = */ None,
            );
        }
    }
    let identifier = parser.next(token);
    token = parser.parse_for_loop_parts_mid(token, await_token, for_token);

    if parser.is_a(parser.next(token), Keyword::IN)
        || parser.is_a(parser.next(token), TokenType::COLON)
    {
        // Process `for ( ... in ... )`
        *in_style = true;
        token = parser.parse_for_in_loop_parts_rest(
            token,
            await_token,
            for_token,
            /* patternKeyword = */ None,
            Some(identifier),
        );
    } else {
        // Process `for ( ... ; ... ; ... )`
        *in_style = false;
        token = parser.parse_for_loop_parts_rest(token, for_token, await_token);
    }
    token
}

/// Dart (line 94): `ForCondition.computeNext(Token token)`.
pub(crate) fn for_condition_compute_next(
    in_style: bool,
    tokens: &Tokens,
    token: TokenId,
) -> LiteralEntryInfo {
    let next = tokens.next(token);
    let complete = || {
        if in_style {
            LiteralEntryInfo::ForInComplete
        } else {
            LiteralEntryInfo::ForComplete
        }
    };
    if is_for_or_await_for(tokens, next) {
        return nested(new_for_condition(), complete());
    } else if tokens.ty(next) == Keyword::IF {
        return nested(IF_CONDITION, complete());
    } else if tokens.ty(next) == TokenType::PERIOD_PERIOD_PERIOD
        || tokens.ty(next) == TokenType::PERIOD_PERIOD_PERIOD_QUESTION
    {
        return if in_style {
            LiteralEntryInfo::ForInSpread
        } else {
            LiteralEntryInfo::ForSpread
        };
    } else if tokens.ty(next) == TokenType::QUESTION {
        return nested(NULL_AWARE_ENTRY, complete());
    }
    if in_style {
        LiteralEntryInfo::ForInEntry
    } else {
        LiteralEntryInfo::ForEntry
    }
}

// ForSpread (line 122), ForInSpread (line 133), ForEntry (line 144) and
// ForInEntry (line 155) only override `computeNext` with a constant result:
// see `LiteralEntryInfo::compute_next`.

/// Dart (line 168): `ForComplete.parse(Token token, Parser parser)`.
pub(crate) fn for_complete_parse<L: Listener>(token: TokenId, parser: &mut Parser<L>) -> TokenId {
    parser.listener.end_for_control_flow(token);
    token
}

/// Dart (line 178): `ForInComplete.parse(Token token, Parser parser)`.
pub(crate) fn for_in_complete_parse<L: Listener>(
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    parser.listener.end_for_in_control_flow(token);
    token
}

// -------------------------------------------------------------------------
// IfCondition: the first step when processing an `if` control flow
// collection entry.

/// Dart (line 189): `IfCondition.parse(Token token, Parser parser)`.
pub(crate) fn if_condition_parse<L: Listener>(token: TokenId, parser: &mut Parser<L>) -> TokenId {
    let if_token = parser.next(token);
    debug_assert!(parser.is_a(if_token, Keyword::IF));
    parser.listener.begin_if_control_flow(if_token);
    let allow_case = parser.is_patterns_feature_enabled;
    let result = parser.ensure_parenthesized_condition(if_token, allow_case);
    parser.listener.handle_then_control_flow(result);
    result
}

/// Dart (line 202): `IfCondition.computeNext(Token token)`.
pub(crate) fn if_condition_compute_next(tokens: &Tokens, token: TokenId) -> LiteralEntryInfo {
    let next = tokens.next(token);
    if is_for_or_await_for(tokens, next) {
        return nested(new_for_condition(), LiteralEntryInfo::IfComplete);
    } else if tokens.ty(next) == Keyword::IF {
        return nested(IF_CONDITION, LiteralEntryInfo::IfComplete);
    } else if tokens.ty(next) == TokenType::PERIOD_PERIOD_PERIOD
        || tokens.ty(next) == TokenType::PERIOD_PERIOD_PERIOD_QUESTION
    {
        return LiteralEntryInfo::IfSpread;
    } else if tokens.ty(next) == TokenType::QUESTION {
        return nested(NULL_AWARE_ENTRY, LiteralEntryInfo::IfComplete);
    }
    LiteralEntryInfo::IfEntry
}

// IfSpread (line 221) and IfEntry (line 230) only override `computeNext`
// with a constant result: see `LiteralEntryInfo::compute_next`.

/// Dart (line 241): `IfComplete.parse(Token token, Parser parser)`.
pub(crate) fn if_complete_parse<L: Listener>(token: TokenId, parser: &mut Parser<L>) -> TokenId {
    if !parser.is_a(parser.next(token), Keyword::ELSE) {
        parser.listener.end_if_control_flow(token);
    }
    token
}

/// Dart (line 249): `IfComplete.computeNext(Token token)`.
pub(crate) fn if_complete_compute_next(
    tokens: &Tokens,
    token: TokenId,
) -> Option<LiteralEntryInfo> {
    if tokens.ty(tokens.next(token)) == Keyword::ELSE {
        Some(LiteralEntryInfo::IfElse)
    } else {
        None
    }
}

// IfElse: a step for parsing the `else` portion of an `if` control flow.

/// Dart (line 259): `IfElse.parse(Token token, Parser parser)`.
pub(crate) fn if_else_parse<L: Listener>(token: TokenId, parser: &mut Parser<L>) -> TokenId {
    let else_token = parser.next(token);
    debug_assert!(parser.is_a(else_token, Keyword::ELSE));
    parser.listener.handle_else_control_flow(else_token);
    else_token
}

/// Dart (line 267): `IfElse.computeNext(Token token)`.
pub(crate) fn if_else_compute_next(tokens: &Tokens, token: TokenId) -> LiteralEntryInfo {
    debug_assert!(tokens.ty(token) == Keyword::ELSE);
    let next = tokens.next(token);
    if is_for_or_await_for(tokens, next) {
        return nested(new_for_condition(), LiteralEntryInfo::IfElseComplete);
    } else if tokens.ty(next) == Keyword::IF {
        return nested(IF_CONDITION, LiteralEntryInfo::IfElseComplete);
    } else if tokens.ty(next) == TokenType::PERIOD_PERIOD_PERIOD
        || tokens.ty(next) == TokenType::PERIOD_PERIOD_PERIOD_QUESTION
    {
        return LiteralEntryInfo::ElseSpread;
    } else if tokens.ty(next) == TokenType::QUESTION {
        return nested(NULL_AWARE_ENTRY, LiteralEntryInfo::IfElseComplete);
    }
    LiteralEntryInfo::ElseEntry
}

// ElseSpread (line 285) and ElseEntry (line 294) only override
// `computeNext` with a constant result: see `LiteralEntryInfo::compute_next`.

/// Dart (line 307): `IfElseComplete.parse(Token token, Parser parser)`.
pub(crate) fn if_else_complete_parse<L: Listener>(
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    parser.listener.end_if_else_control_flow(token);
    token
}

// -------------------------------------------------------------------------

/// Dart (line 318): `SpreadOperator.parse(Token token, Parser parser)`.
/// The first step when processing a spread entry.
pub(crate) fn spread_operator_parse<L: Listener>(
    token: TokenId,
    parser: &mut Parser<L>,
) -> TokenId {
    let operator = parser.next(token);
    debug_assert!(
        parser.is_a(operator, TokenType::PERIOD_PERIOD_PERIOD)
            || parser.is_a(operator, TokenType::PERIOD_PERIOD_PERIOD_QUESTION)
    );
    let token = parser.parse_expression(operator);
    parser.listener.handle_spread_expression(operator);
    token
}

/// Dart (line 344): `Nested.computeNext(Token token)`.
///
/// Dart changes `nestedStep` and returns `this`; here the new value is built
/// from the moved fields.
pub(crate) fn nested_compute_next(
    nested_step: Option<Box<LiteralEntryInfo>>,
    last_step: Box<LiteralEntryInfo>,
    tokens: &Tokens,
    token: TokenId,
) -> LiteralEntryInfo {
    let nested_step = nested_step.unwrap().compute_next(tokens, token);
    if nested_step.is_some() {
        LiteralEntryInfo::Nested {
            nested_step: nested_step.map(Box::new),
            last_step,
        }
    } else {
        *last_step
    }
}

// NullAwareEntry (line 350) has no overrides (`hasEntry: true`).
