// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/parser_impl.dart (lines 10490-12597)

#![allow(unused_imports, unused_variables, unused_mut, unused_assignments, clippy::all)]

#[allow(unused_imports)]
use crate::type_info::{
    compute_method_type_arguments_mut, compute_type_mut, compute_type_param_or_arg_mut,
    compute_variable_pattern_type_mut,
};
use dartr_diagnostics::cfe::{CfeCode, CfeMessage};
use dartr_diagnostics::cfe_codes as diag;
use dartr_syntax::token_constants::*;
use dartr_syntax::{Keyword, TokenId, TokenType, Tokens};

use super::{AwaitOrYieldContext, ConstantPatternContext, ForPartsContext, Parser, PatternContext};
use crate::assert::Assert;
use crate::async_modifier::AsyncModifier;
use crate::block_kind::BlockKind;
use crate::constructor_reference_context::ConstructorReferenceContext;
use crate::declaration_kind::{DeclarationHeaderKind, DeclarationKind};
use crate::directive_context::DirectiveContext;
use crate::experimental_features::ExperimentalFlag;
use crate::formal_parameter_kind::FormalParameterKind;
use crate::identifier_context::{
    IdentifierContext, looks_like_expression_start, looks_like_pattern_start,
};
use crate::listener::Listener;
use crate::listener_stack::Layer;
use crate::literal_entry_info::{LiteralEntryInfo, looks_like_literal_entry};
use crate::loop_state::LoopState;
use crate::member_kind::MemberKind;
use crate::modifier_context::ModifierContext;
use crate::type_info::{
    ILLEGAL_PATTERN_IDENTIFIERS, NO_TYPE, NO_TYPE_PARAM_OR_ARG, TypeInfo, TypeParamOrArgInfo,
    compute_type, compute_type_param_or_arg, compute_variable_pattern_type,
};

// Dart precedence constants (scanner/token.dart) used in this file.
const EQUALITY_PRECEDENCE: i32 = 7;
const BITWISE_OR_PRECEDENCE: i32 = 9;
const POSTFIX_PRECEDENCE: i32 = 16;
const SELECTOR_PRECEDENCE: i32 = 17;

/// Dart `TokenType.isRelationalOperator`.
fn is_relational_operator(ty: TokenType) -> bool {
    ty == TokenType::LT || ty == TokenType::LT_EQ || ty == TokenType::GT || ty == TokenType::GT_EQ
}

/// Dart `TokenType.isEqualityOperator`.
fn is_equality_operator(ty: TokenType) -> bool {
    ty == TokenType::BANG_EQ || ty == TokenType::EQ_EQ
}

impl<L: Listener> Parser<L> {
    /// Dart (line 10490): `Token parseBlock(Token token, BlockKind blockKind)`
    ///
    /// ```
    /// block:
    ///   '{' statement* '}'
    /// ;
    /// ```
    pub fn parse_block(&mut self, token: TokenId, block_kind: BlockKind) -> TokenId {
        let mut token = self.ensure_block(token, Some(block_kind));
        let begin = token;
        self.listener.begin_block(begin, block_kind);
        let mut statement_count: i32 = 0;
        let mut start_token = self.next(token);
        while self.not_eof_or_type(TokenType::CLOSE_CURLY_BRACKET, start_token) {
            token = self.parse_statement(token);
            if self.next(token) == start_token {
                // No progress was made, so we report the current token as being invalid
                // and move forward.
                token = self.next(token);
                let message = diag::unexpected_token(self.lexeme(token));
                self.report_recoverable_error(token, message);
            }
            statement_count += 1;
            start_token = self.next(token);
        }
        token = self.next(token);
        // assert(token.isEof || token.isA(TokenType.CLOSE_CURLY_BRACKET));
        self.listener
            .end_block(statement_count, begin, token, block_kind);
        token
    }

    /// Dart (line 10515): `Token parseInvalidBlock(Token token)`
    pub fn parse_invalid_block(&mut self, token: TokenId) -> TokenId {
        let begin = self.next(token);
        // assert(begin.isA(TokenType.OPEN_CURLY_BRACKET));
        // Parse and report the invalid block, but suppress errors
        // because an error has already been reported by the caller.
        self.push_no_errors_listener();
        // The scanner ensures that `{` always has a closing `}`.
        let token = self.parse_block(token, BlockKind::Invalid);
        self.pop_layer();
        self.listener.handle_invalid_top_level_block(begin);
        token
    }

    /// Dart (line 10531): `bool looksLikeExpressionAfterAwaitOrYield( Token token, AwaitOrYieldContext context, )`
    ///
    /// Determine if the following tokens look like an expression and not a local
    /// variable or local function declaration.
    pub fn looks_like_expression_after_await_or_yield(
        &mut self,
        token: TokenId,
        context: AwaitOrYieldContext,
    ) -> bool {
        // TODO(srawlins): Consider parsing the potential expression once doing so
        //  does not modify the token stream. For now, use simple look ahead and
        //  ensure no false positives.

        let mut token = self.next(token);
        if self.is_identifier(token) {
            token = self.next(token);
            if self.is_a(token, TokenType::OPEN_PAREN) {
                token = self.next(self.end_group(token).unwrap());
                if self.is_a(token, TokenType::SEMICOLON)
                    || self.is_a(token, TokenType::PERIOD)
                    || self.is_a(token, TokenType::COMMA)
                    || self.is_a(token, TokenType::PERIOD_PERIOD)
                    || self.is_a(token, TokenType::QUESTION)
                    || self.is_a(token, TokenType::QUESTION_PERIOD)
                    || self.is_a(token, TokenType::CLOSE_PAREN)
                {
                    // E.g. (in a non-async function): `await f();`.
                    return true;
                } else if self.ty(token).is_binary_operator() {
                    // E.g. (in a non-async function):
                    // `await returnsFuture() + await returnsFuture()`.
                    return true;
                }
            } else if self.is_a(token, TokenType::PERIOD)
                || self.is_a(token, TokenType::CLOSE_PAREN)
                || self.is_a(token, TokenType::CLOSE_SQUARE_BRACKET)
            {
                // TODO(srawlins): Also consider when `token` is `;`. There is still not
                // good error recovery on `yield x;`. This would also require
                // modification to analyzer's
                // test_parseCompilationUnit_pseudo_asTypeName.

                // E.g. (in a non-async function): `if (await f) {}`.
                return true;
            } else if self.is_a(token, TokenType::COMMA)
                && context == AwaitOrYieldContext::UnaryExpression
            {
                // E.g. (in a non-async function): `xor(await f, await f, await f);`,
                // but not `await y, z` (`await` is a class here so it's declaring two
                // variables).
                return true;
            } else if self.ty(token).is_binary_operator() {
                // E.g. (in a non-async function): (first part of) `await f + await f;`,
                return true;
            } else if self.is_a(token, TokenType::SEMICOLON)
                && context == AwaitOrYieldContext::UnaryExpression
            {
                // E.g. (in a non-async function): (second part of) `await f + await f;`
                // but not `await f;` (`await` is a class here so it's a variable
                // declaration).
                return true;
            }
        } else if self.is_a(token, Keyword::NULL) {
            return true;
        }
        // TODO(srawlins): Consider other possibilities for `token` which would
        //  imply it looks like an expression, for example beginning with `<`, as
        //  part of a collection literal type argument list, `(`, other literals,
        //  etc. For example, there is still not good error recovery on
        //  `yield <int>[]`.

        false
    }

    /// Dart (line 10598): `bool looksLikeAwaitExpression(Token token, AwaitOrYieldContext context)`
    ///
    /// Determine if the following tokens look like an 'await' expression
    /// and not a local variable or local function declaration.
    pub fn looks_like_await_expression(
        &mut self,
        token: TokenId,
        context: AwaitOrYieldContext,
    ) -> bool {
        let token = self.next(token);
        // assert(token.isA(Keyword.AWAIT));

        self.looks_like_expression_after_await_or_yield(token, context)
    }

    /// Dart (line 10607): `bool looksLikeYieldStatement(Token token, AwaitOrYieldContext context)`
    ///
    /// Determine if the following tokens look like a 'yield' expression and not a
    /// local variable or local function declaration.
    pub fn looks_like_yield_statement(
        &mut self,
        token: TokenId,
        context: AwaitOrYieldContext,
    ) -> bool {
        let token = self.next(token);
        // assert(token.isA(Keyword.YIELD));

        self.looks_like_expression_after_await_or_yield(token, context)
    }

    /// Dart (line 10619): `Token parseAwaitExpression(Token token, bool allowCascades)`
    ///
    /// ```
    /// awaitExpression:
    ///   'await' unaryExpression
    /// ;
    /// ```
    pub fn parse_await_expression(&mut self, token: TokenId, allow_cascades: bool) -> TokenId {
        let await_token = self.next(token);
        // assert(awaitToken.isA(Keyword.AWAIT));
        self.listener.begin_await_expression(await_token);
        let token = self.parse_precedence_expression(
            await_token,
            POSTFIX_PRECEDENCE,
            allow_cascades,
            ConstantPatternContext::None,
        );
        if self.in_async() {
            self.listener.end_await_expression(await_token, token);
        } else {
            let error_message = diag::await_not_async();
            let error_code = error_message.code;
            self.report_recoverable_error(await_token, error_message);
            self.listener
                .end_invalid_await_expression(await_token, token, error_code);
        }
        token
    }

    /// Dart (line 10648): `Token parseThrowExpression(Token token, bool allowCascades)`
    ///
    /// ```
    /// throwExpression:
    ///   'throw' expression
    /// ;
    ///
    /// throwExpressionWithoutCascade:
    ///   'throw' expressionWithoutCascade
    /// ;
    /// ```
    pub fn parse_throw_expression(&mut self, token: TokenId, allow_cascades: bool) -> TokenId {
        let throw_token = self.next(token);
        // assert(throwToken.isA(Keyword.THROW));
        if self.is_a(self.next(throw_token), TokenType::SEMICOLON) {
            // TODO(danrubel): Find a better way to intercept the parseExpression
            // recovery to generate this error message rather than explicitly
            // checking the next token as we are doing here.
            let next = self.next(throw_token);
            self.report_recoverable_error(next, diag::missing_expression_in_throw());
            let next = self.next(throw_token);
            let offset = self.char_offset(next);
            let byte = self.tokens().byte_offset(next);
            let string = self.tokens_mut().push_synthetic_string(
                TokenType::STRING,
                "\"\"",
                offset,
                byte,
                /* _length = */ Some(0),
            );
            self.rewriter().insert_token(throw_token, string);
        }
        let token = if allow_cascades {
            self.parse_expression(throw_token)
        } else {
            self.parse_expression_without_cascade(throw_token)
        };
        self.listener.handle_throw_expression(throw_token, token);
        token
    }

    /// Dart (line 10678): `Token parseRethrowStatement(Token token)`
    ///
    /// ```
    /// rethrowStatement:
    ///   'rethrow' ';'
    /// ;
    /// ```
    pub fn parse_rethrow_statement(&mut self, token: TokenId) -> TokenId {
        let throw_token = self.next(token);
        // assert(throwToken.isA(Keyword.RETHROW));
        self.listener.begin_rethrow_statement(throw_token);
        let token = self.ensure_semicolon(throw_token);
        self.listener.end_rethrow_statement(throw_token, token);
        token
    }

    /// Dart (line 10705): `Token parseTryStatement(Token token)`
    ///
    /// ```
    /// tryStatement:
    ///   'try' block (onPart+ finallyPart? | finallyPart)
    /// ;
    ///
    /// onPart:
    ///   catchPart block |
    ///   'on' type catchPart? block
    /// ;
    ///
    /// catchPart:
    ///   'catch' '(' identifier (',' identifier)? ')'
    /// ;
    ///
    /// finallyPart:
    ///   'finally' block
    /// ;
    /// ```
    pub fn parse_try_statement(&mut self, token: TokenId) -> TokenId {
        let try_keyword = self.next(token);
        // assert(tryKeyword.isA(Keyword.TRY));
        self.listener.begin_try_statement(try_keyword);
        let mut last_consumed = self.parse_block(try_keyword, BlockKind::TryStatement);
        let mut token = self.next(last_consumed);
        let mut catch_count: i32 = 0;

        let mut value = self.string_value(token);
        while value == Some("catch") || value == Some("on") {
            let mut did_begin_catch_clause = false;
            let mut on_keyword: Option<TokenId> = None;
            if value == Some("on") {
                // 'on' type catchPart?
                // Note https://github.com/dart-lang/language/blob/master/accepted/future-releases/records/records-feature-specification.md#ambiguity-with-on-clauses
                // "Whenever on appears after a try block or after a preceding on clause
                // on a try block, we unconditionally parse it as an on clause".
                on_keyword = Some(token);
                let type_info = compute_type_mut(self.tokens_mut(),
                    token,
                    /* required = */ true,
                    false,
                    false,
                );
                self.listener.begin_catch_clause(token);
                did_begin_catch_clause = true;
                last_consumed = type_info.ensure_type_not_void(token, self);
                token = self.next(last_consumed);
                value = self.string_value(token);
            }
            let mut catch_keyword: Option<TokenId> = None;
            let mut comma: Option<TokenId> = None;
            if value == Some("catch") {
                if !did_begin_catch_clause {
                    self.listener.begin_catch_clause(token);
                    did_begin_catch_clause = true;
                }
                catch_keyword = Some(token);
                let catch_kw = token;

                let mut open_parens = self.next(catch_kw);
                if !self.is_a(open_parens, TokenType::OPEN_PAREN) {
                    self.report_recoverable_error(open_parens, diag::catch_syntax());
                    open_parens = self
                        .rewriter()
                        .insert_parens(catch_kw, /* includeIdentifier = */ true);
                }

                let mut exception_name = self.next(open_parens);
                if self.kind(exception_name) != IDENTIFIER_TOKEN {
                    exception_name =
                        IdentifierContext::CatchParameter.ensure_identifier(open_parens, self);
                }

                if self.is_a(self.next(exception_name), TokenType::CLOSE_PAREN) {
                    // OK: `catch (identifier)`.
                } else {
                    let c = self.next(exception_name);
                    comma = Some(c);
                    if !self.is_a(c, TokenType::COMMA) {
                        // Recovery
                        if !self.is_synthetic(exception_name) {
                            self.report_recoverable_error(c, diag::catch_syntax());
                        }

                        // TODO(danrubel): Consider inserting `on` clause if
                        // exceptionName is preceded by type and followed by a comma.
                        // Then this
                        //   } catch (E e, t) {
                        // will recover to
                        //   } on E catch (e, t) {
                        // with a detailed explanation for the user in the error
                        // indicating what they should do to fix the code.

                        // TODO(danrubel): Consider inserting synthetic identifier if
                        // exceptionName is a non-synthetic identifier followed by `.`.
                        // Then this
                        //   } catch (
                        //   e.f();
                        // will recover to
                        //   } catch (_s_) {}
                        //   e.f();
                        // rather than
                        //   } catch (e) {}
                        //   _s_.f();

                        let end_group = self.end_group(open_parens).unwrap();
                        if self.is_synthetic(end_group) {
                            // The scanner did not place the synthetic ')' correctly.
                            self.rewriter().move_synthetic(exception_name, end_group);
                            comma = None;
                        } else {
                            comma = Some(
                                self.rewriter()
                                    .insert_synthetic_token(exception_name, TokenType::COMMA),
                            );
                        }
                    }
                    if let Some(c) = comma {
                        let mut trace_name = self.next(c);
                        if self.kind(trace_name) != IDENTIFIER_TOKEN {
                            trace_name =
                                IdentifierContext::CatchParameter.ensure_identifier(c, self);
                        }
                        if !self.is_a(self.next(trace_name), TokenType::CLOSE_PAREN) {
                            // Recovery
                            if !self.is_synthetic(trace_name) {
                                let next = self.next(trace_name);
                                self.report_recoverable_error(
                                    next,
                                    diag::catch_syntax_extra_parameters(),
                                );
                            }
                            let end_group = self.end_group(open_parens).unwrap();
                            if self.is_synthetic(end_group) {
                                // The scanner did not place the synthetic ')' correctly.
                                self.rewriter().move_synthetic(trace_name, end_group);
                            }
                        }
                    }
                }
                last_consumed = self.parse_formal_parameters(catch_kw, MemberKind::Catch);
                token = self.next(last_consumed);
            }
            self.listener.end_catch_clause(token);
            last_consumed = self.parse_block(last_consumed, BlockKind::CatchClause);
            token = self.next(last_consumed);
            catch_count += 1;
            self.listener
                .handle_catch_block(on_keyword, catch_keyword, comma);
            value = self.string_value(token); // while condition
        }

        let mut finally_keyword: Option<TokenId> = None;
        if self.is_a(token, Keyword::FINALLY) {
            finally_keyword = Some(token);
            last_consumed = self.parse_block(token, BlockKind::FinallyClause);
            self.listener.handle_finally_block(token);
        } else {
            if catch_count == 0 {
                self.report_recoverable_error(try_keyword, diag::only_try());
            }
        }
        self.listener
            .end_try_statement(catch_count, try_keyword, finally_keyword, last_consumed);
        last_consumed
    }

    /// Dart (line 10856): `Token parseSwitchStatement(Token token)`
    ///
    /// ```
    /// switchStatement:
    ///   'switch' parenthesizedExpression switchBlock
    /// ;
    /// ```
    pub fn parse_switch_statement(&mut self, token: TokenId) -> TokenId {
        let switch_keyword = self.next(token);
        // assert(switchKeyword.isA(Keyword.SWITCH));
        self.listener.begin_switch_statement(switch_keyword);
        let mut token = self.ensure_parenthesized_condition(switch_keyword, false);
        let saved_loop_state = self.loop_state;
        if self.loop_state == LoopState::OutsideLoop {
            self.loop_state = LoopState::InsideSwitch;
        }
        token = self.parse_switch_block(token);
        self.loop_state = saved_loop_state;
        self.listener.end_switch_statement(switch_keyword, token);
        token
    }

    /// Dart (line 10876): `Token parseSwitchBlock(Token token)`
    ///
    /// ```
    /// switchBlock:
    ///   '{' switchCase* defaultCase? '}'
    /// ;
    /// ```
    pub fn parse_switch_block(&mut self, token: TokenId) -> TokenId {
        let mut token = self.ensure_block(token, Some(BlockKind::SwitchStatement));
        let begin_switch = token;
        self.listener.begin_switch_block(begin_switch);
        let mut case_count: i32 = 0;
        let mut default_keyword: Option<TokenId> = None;
        let mut colon_after_default: Option<TokenId> = None;
        while self.not_eof_or_type(TokenType::CLOSE_CURLY_BRACKET, self.next(token)) {
            let begin_case = self.next(token);
            let mut expression_count: i32 = 0;
            let mut label_count: i32 = 0;
            let mut peek = self.peek_past_labels(begin_case);
            loop {
                // Loop until we find something that can't be part of a switch case.
                let value = self.string_value(peek);
                if value == Some("default") {
                    while self.next(token) != peek {
                        token = self.parse_label(token);
                        label_count += 1;
                    }
                    if default_keyword.is_some() {
                        let next = self.next(token);
                        self.report_recoverable_error(next, diag::switch_has_multiple_defaults());
                    }
                    let dk = self.next(token);
                    default_keyword = Some(dk);
                    token = self.ensure_colon(dk);
                    colon_after_default = Some(token);
                    peek = self.next(token);
                    break;
                } else if value == Some("case") {
                    while self.next(token) != peek {
                        token = self.parse_label(token);
                        label_count += 1;
                    }
                    let case_keyword = self.next(token);
                    if default_keyword.is_some() {
                        self.report_recoverable_error(
                            case_keyword,
                            diag::switch_has_case_after_default(),
                        );
                    }
                    self.listener.begin_case_expression(case_keyword);
                    if self.is_patterns_feature_enabled {
                        token = self.parse_pattern(case_keyword, PatternContext::Matching, 1);
                    } else {
                        token = self.parse_expression(case_keyword);
                    }
                    let next = self.next(token);
                    let mut when: Option<TokenId> = None;
                    if self.is_a(next, Keyword::WHEN) {
                        token = next;
                        when = Some(next);
                        self.listener.begin_switch_case_when_clause(next);
                        token = self.parse_expression(token);
                        self.listener.end_switch_case_when_clause(token);
                    } else {
                        self.listener.handle_switch_case_no_when_clause(token);
                    }
                    token = self.ensure_colon(token);
                    self.listener.end_case_expression(case_keyword, when, token);
                    expression_count += 1;
                    peek = self.peek_past_labels(self.next(token));
                } else if expression_count > 0 {
                    break;
                } else {
                    // Recovery
                    self.report_recoverable_error(peek, diag::expected_token("case"));
                    let end_group = self.end_group(begin_switch).unwrap();
                    while self.next(token) != end_group {
                        token = self.next(token);
                    }
                    peek = self.peek_past_labels(self.next(token));
                    break;
                }
            }
            token = self.parse_statements_in_switch_case(
                token,
                peek,
                begin_case,
                label_count,
                expression_count,
                default_keyword,
                colon_after_default,
            );
            case_count += 1;
        }
        token = self.next(token);
        self.listener
            .end_switch_block(case_count, begin_switch, token);
        // assert(token.isEof || token.isA(TokenType.CLOSE_CURLY_BRACKET));
        token
    }

    /// Dart (line 10967): `Token peekPastLabels(Token token)`
    ///
    /// Peek after the following labels (if any). The following token
    /// is used to determine if the labels belong to a statement or a
    /// switch case.
    pub fn peek_past_labels(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        while self.is_identifier(token) && self.is_a(self.next(token), TokenType::COLON) {
            token = self.next(self.next(token));
        }
        token
    }

    /// Dart (line 10975): `Token parseStatementsInSwitchCase( Token token, Token peek, Token begin, int labelCount, int expressionCount, Token? defaultKeyword, Token? colonAfterDefault, )`
    ///
    /// Parse statements after a switch `case:` or `default:`.
    pub fn parse_statements_in_switch_case(
        &mut self,
        token: TokenId,
        peek: TokenId,
        begin: TokenId,
        label_count: i32,
        expression_count: i32,
        default_keyword: Option<TokenId>,
        colon_after_default: Option<TokenId>,
    ) -> TokenId {
        let mut token = token;
        let mut peek = peek;
        self.listener
            .begin_switch_case(label_count, expression_count, begin);
        // Finally zero or more statements.
        let mut statement_count: i32 = 0;
        while self.kind(self.next(token)) != EOF_TOKEN {
            let value = self.string_value(peek);
            if value == Some("case")
                || value == Some("default")
                || (value == Some("}") && self.next(token) == peek)
            {
                // A label just before "}" will be handled as a statement error.
                break;
            } else {
                let start_token = self.next(token);
                token = self.parse_statement(token);
                let next = self.next(token);
                if next == start_token {
                    // No progress was made, so we report the current token as being
                    // invalid and move forward.
                    let message = diag::unexpected_token(self.lexeme(next));
                    self.report_recoverable_error(next, message);
                    token = next;
                }
                statement_count += 1;
            }
            peek = self.peek_past_labels(self.next(token));
        }
        self.listener.end_switch_case(
            label_count,
            expression_count,
            default_keyword,
            colon_after_default,
            statement_count,
            begin,
            token,
        );
        token
    }

    /// Dart (line 11028): `Token parseBreakStatement(Token token)`
    ///
    /// ```
    /// breakStatement:
    ///   'break' identifier? ';'
    /// ;
    /// ```
    pub fn parse_break_statement(&mut self, token: TokenId) -> TokenId {
        let mut token = self.next(token);
        let break_keyword = token;
        // assert(breakKeyword.isA(Keyword.BREAK));
        let mut has_target = false;
        if self.is_identifier(self.next(token)) {
            token = self.ensure_identifier(token, IdentifierContext::LabelReference);
            has_target = true;
        } else if !self.is_break_allowed() {
            self.report_recoverable_error(break_keyword, diag::break_outside_of_loop());
        }
        token = self.ensure_semicolon(token);
        self.listener
            .handle_break_statement(has_target, break_keyword, token);
        token
    }

    /// Dart (line 11048): `Token parseAssert(Token token, Assert kind)`
    ///
    /// ```
    /// assertion:
    ///   'assert' '(' expression (',' expression)? ','? ')'
    /// ;
    /// ```
    pub fn parse_assert(&mut self, token: TokenId, kind: Assert) -> TokenId {
        let mut token = self.next(token);
        // assert(token.isA(Keyword.ASSERT));
        self.listener.begin_assert(token, kind);
        let assert_keyword = token;
        let mut left_parenthesis = self.next(token);
        if !self.is_a(left_parenthesis, TokenType::OPEN_PAREN) {
            // Recovery
            self.report_recoverable_error(left_parenthesis, diag::expected_but_got("("));
            left_parenthesis = self
                .rewriter()
                .insert_parens(token, /* includeIdentifier = */ true);
        }
        token = left_parenthesis;
        let mut comma_token: Option<TokenId> = None;
        let old = self.may_parse_function_expressions;
        self.may_parse_function_expressions = true;

        token = self.parse_expression(token);
        if self.is_a(self.next(token), TokenType::COMMA) {
            token = self.next(token);
            if !self.is_a(self.next(token), TokenType::CLOSE_PAREN) {
                comma_token = Some(token);
                token = self.parse_expression(token);
                if self.is_a(self.next(token), TokenType::COMMA) {
                    // Trailing comma is ignored.
                    token = self.next(token);
                }
            }
        }

        let end_group = self.end_group(left_parenthesis).unwrap();
        if self.next(token) == end_group {
            token = end_group;
        } else {
            // Recovery
            if self.is_synthetic(end_group) {
                // The scanner did not place the synthetic ')' correctly, so move it.
                token = self.rewriter().move_synthetic(token, end_group);
            } else {
                let next = self.next(token);
                self.report_recoverable_error_with_token(next, diag::unexpected_token);
                token = end_group;
            }
        }

        // assert(token.isA(TokenType.CLOSE_PAREN));
        self.may_parse_function_expressions = old;
        if kind == Assert::Expression {
            self.report_recoverable_error(assert_keyword, diag::assert_as_expression());
        } else if kind == Assert::Statement {
            self.ensure_semicolon(token);
        }
        self.listener
            .end_assert(assert_keyword, kind, left_parenthesis, comma_token, token);
        token
    }

    /// Dart (line 11113): `Token parseAssertStatement(Token token)`
    ///
    /// ```
    /// assertStatement:
    ///   assertion ';'
    /// ;
    /// ```
    pub fn parse_assert_statement(&mut self, token: TokenId) -> TokenId {
        // assert(token.next!.isA(Keyword.ASSERT));
        // parseAssert ensures that there is a trailing semicolon.
        let token = self.parse_assert(token, Assert::Statement);
        self.next(token)
    }

    /// Dart (line 11124): `Token parseContinueStatement(Token token)`
    ///
    /// ```
    /// continueStatement:
    ///   'continue' identifier? ';'
    /// ;
    /// ```
    pub fn parse_continue_statement(&mut self, token: TokenId) -> TokenId {
        let mut token = self.next(token);
        let continue_keyword = token;
        // assert(continueKeyword.isA(Keyword.CONTINUE));
        let mut has_target = false;
        if self.is_identifier(self.next(token)) {
            token = self.ensure_identifier(token, IdentifierContext::LabelReference);
            has_target = true;
            if !self.is_continue_with_label_allowed() {
                self.report_recoverable_error(continue_keyword, diag::continue_outside_of_loop());
            }
        } else if !self.is_continue_allowed() {
            let message = if self.loop_state == LoopState::InsideSwitch {
                diag::continue_without_label_in_case()
            } else {
                diag::continue_outside_of_loop()
            };
            self.report_recoverable_error(continue_keyword, message);
        }
        token = self.ensure_semicolon(token);
        self.listener
            .handle_continue_statement(has_target, continue_keyword, token);
        token
    }

    /// Dart (line 11152): `Token parseEmptyStatement(Token token)`
    ///
    /// ```
    /// emptyStatement:
    ///   ';'
    /// ;
    /// ```
    pub fn parse_empty_statement(&mut self, token: TokenId) -> TokenId {
        let token = self.next(token);
        // assert(token.isA(TokenType.SEMICOLON));
        self.listener.handle_empty_statement(token);
        token
    }

    /// Dart (line 11173): `Token parseInvalidOperatorDeclaration( Token beforeStart, Token? abstractToken, Token? augmentToken, Token? externalToken, Token? staticToken, Token? covariantToken, Token? lateToken, Token? varFinalOrConst, Token beforeType, DeclarationKind kind, String? enclosingDeclarationName, )`
    ///
    /// Recover from finding an operator declaration missing the `operator`
    /// keyword. The metadata for the member, if any, has already been parsed
    /// (and events have already been generated).
    pub fn parse_invalid_operator_declaration(
        &mut self,
        before_start: TokenId,
        abstract_token: Option<TokenId>,
        augment_token: Option<TokenId>,
        external_token: Option<TokenId>,
        static_token: Option<TokenId>,
        covariant_token: Option<TokenId>,
        late_token: Option<TokenId>,
        var_final_or_const: Option<TokenId>,
        before_type: TokenId,
        kind: DeclarationKind,
        enclosing_declaration_name: Option<&str>,
    ) -> TokenId {
        let mut type_info = compute_type_mut(self.tokens_mut(),
            before_start,
            /* required = */ false,
            /* inDeclaration = */ true,
            false,
        );
        let mut before_name = type_info.skip_type_mut(self.tokens_mut(), before_type);
        let mut next = self.next(before_name);

        if self.is_a(next, Keyword::OPERATOR) {
            next = self.next(next);
        } else {
            // The 'operator' keyword is missing, but we may or may not have a type
            // before the token that is the actual operator.
            let mut operator = next;
            if !self.is_operator(next) && self.is_operator(self.next(next)) {
                before_name = next;
                operator = self.next(next);
            }
            self.report_recoverable_error(operator, diag::missing_operator_keyword());
            self.rewriter()
                .insert_synthetic_keyword(before_name, Keyword::OPERATOR);

            // Having inserted the keyword the type now possibly compute differently.
            type_info = compute_type_mut(self.tokens_mut(),
                before_start,
                /* required = */ true,
                /* inDeclaration = */ true,
                false,
            );
            before_name = type_info.skip_type_mut(self.tokens_mut(), before_type);
            next = self.next(before_name);

            // The 'next' token can be the just-inserted 'operator' keyword.
            // If it is, change it so it points to the actual operator.
            if !self.is_operator(next)
                && self.is_operator(self.next(next))
                && self.string_value(next) == Some("operator")
            {
                next = self.next(next);
            }
        }

        // assert(
        //   (next.isOperator && next.endGroup == null) ||
        //       next.isA(TokenType.EQ_EQ_EQ) ||
        //       next.isA(TokenType.BANG_EQ_EQ),
        // );

        let name = self.next(before_name);
        let token = self.parse_method(
            before_start,
            abstract_token,
            augment_token,
            external_token,
            static_token,
            covariant_token,
            late_token,
            var_final_or_const,
            before_type,
            &type_info,
            /* getOrSet = */ None,
            /* newToken = */ None,
            name,
            kind,
            enclosing_declaration_name,
            /* nameIsRecovered = */ false,
        );
        self.listener.end_member();
        token
    }

    /// Dart (line 11256): `Token recoverFromInvalidMember( Token token, Token beforeStart, Token? abstractToken, Token? augmentToken, Token? externalToken, Token? staticToken, Token? covariantToken, Token? lateToken, Token? varFinalOrConst, Token beforeType, TypeInfo typeInfo, Token? getOrSet, Token? newToken, DeclarationKind kind, String? enclosingDeclarationName, )`
    ///
    /// Recover from finding an invalid class member. The metadata for the member,
    /// if any, has already been parsed (and events have already been generated).
    /// The member was expected to start with the token after [token].
    pub fn recover_from_invalid_member(
        &mut self,
        token: TokenId,
        before_start: TokenId,
        abstract_token: Option<TokenId>,
        augment_token: Option<TokenId>,
        external_token: Option<TokenId>,
        static_token: Option<TokenId>,
        covariant_token: Option<TokenId>,
        late_token: Option<TokenId>,
        var_final_or_const: Option<TokenId>,
        before_type: TokenId,
        type_info: &TypeInfo,
        get_or_set: Option<TokenId>,
        new_token: Option<TokenId>,
        kind: DeclarationKind,
        enclosing_declaration_name: Option<&str>,
    ) -> TokenId {
        let mut token = token;
        let next = self.next(token);
        let value = self.string_value(next);

        if value == Some("class") {
            return self.report_and_skip_class_in_class(next);
        } else if value == Some("enum") {
            return self.report_and_skip_enum_in_class(next);
        } else if value == Some("typedef") {
            return self.report_and_skip_typedef_in_class(next);
        } else if self.is_operator(next) && self.end_group(next).is_none() {
            return self.parse_invalid_operator_declaration(
                before_start,
                abstract_token,
                augment_token,
                external_token,
                static_token,
                covariant_token,
                late_token,
                var_final_or_const,
                before_type,
                kind,
                enclosing_declaration_name,
            );
        }

        if get_or_set.is_some() || value == Some("(") || value == Some("=>") || value == Some("{") {
            let name = self.next(token);
            token = self.parse_method(
                before_start,
                abstract_token,
                augment_token,
                external_token,
                static_token,
                covariant_token,
                late_token,
                var_final_or_const,
                before_type,
                type_info,
                get_or_set,
                new_token,
                name,
                kind,
                enclosing_declaration_name,
                /* nameIsRecovered = */ false,
            );
        } else if token == before_start {
            // TODO(danrubel): Provide a more specific error message for extra ';'.
            self.report_recoverable_error_with_token(next, diag::expected_class_member);
            self.listener.handle_invalid_member(next);
            if value != Some("}") {
                // Ensure we make progress.
                token = next;
            }
        } else {
            let name = self.next(token);
            token = self.parse_fields(
                before_start,
                abstract_token,
                augment_token,
                external_token,
                static_token,
                covariant_token,
                late_token,
                var_final_or_const,
                before_type,
                type_info,
                name,
                kind,
                enclosing_declaration_name,
                /* nameIsRecovered = */ false,
            );
        }

        self.listener.end_member();
        token
    }

    /// Dart (line 11353): `Token recoverFromStackOverflow(Token token)`
    ///
    /// Report that the nesting depth of the code being parsed is too large for
    /// the parser to safely handle. Return the next `}` or EOF.
    pub fn recover_from_stack_overflow(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        let mut next = self.next(token);
        self.report_recoverable_error(next, diag::stack_overflow());
        next = self
            .rewriter()
            .insert_synthetic_token(token, TokenType::SEMICOLON);
        self.listener.handle_empty_statement(next);

        while self.not_eof_or_type(TokenType::CLOSE_CURLY_BRACKET, next) {
            token = next;
            next = self.next(token);
        }
        token
    }

    /// Dart (line 11416): `Token parseInvalidTopLevelDeclaration(Token token)`
    pub fn parse_invalid_top_level_declaration(&mut self, token: TokenId) -> TokenId {
        let mut next = self.next(token);
        let template: fn(&str) -> CfeMessage = if self.is_a(next, TokenType::SEMICOLON) {
            diag::unexpected_token
        } else {
            diag::expected_declaration
        };
        self.report_recoverable_error_with_token(next, template);
        if self.is_a(next, TokenType::OPEN_CURLY_BRACKET) {
            next = self.parse_invalid_block(token);
        }
        self.listener.handle_invalid_top_level_declaration(next);
        next
    }

    /// Dart (line 11431): `Token reportAndSkipClassInClass(Token token)`
    pub fn report_and_skip_class_in_class(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        // assert(token.isA(Keyword.CLASS));
        self.report_recoverable_error(token, diag::class_in_class());
        self.listener.handle_invalid_member(token);
        let mut next = self.next(token);
        // If the declaration appears to be a valid class declaration
        // then skip the entire declaration so that we only generate the one
        // error (above) rather than a plethora of unhelpful errors.
        if self.is_identifier(next) {
            // skip class name
            token = next;
            next = self.next(token);
            // TODO(danrubel): consider parsing (skipping) the class header
            // with a recovery listener so that no events are generated
            if self.is_a(next, TokenType::OPEN_CURLY_BRACKET) {
                if let Some(end_group) = self.end_group(next) {
                    // skip class body
                    token = end_group;
                }
            }
        }
        self.listener.end_member();
        token
    }

    /// Dart (line 11454): `Token reportAndSkipEnumInClass(Token token)`
    pub fn report_and_skip_enum_in_class(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        // assert(token.isA(Keyword.ENUM));
        self.report_recoverable_error(token, diag::enum_in_class());
        self.listener.handle_invalid_member(token);
        let mut next = self.next(token);
        // If the declaration appears to be a valid enum declaration
        // then skip the entire declaration so that we only generate the one
        // error (above) rather than a plethora of unhelpful errors.
        if self.is_identifier(next) {
            // skip enum name
            token = next;
            next = self.next(token);
            if self.is_a(next, TokenType::OPEN_CURLY_BRACKET) {
                if let Some(end_group) = self.end_group(next) {
                    // TODO(danrubel): Consider replacing this `skip enum` functionality
                    // with something that can parse and resolve the declaration
                    // even though it is in a class context
                    token = end_group;
                }
            }
        }
        self.listener.end_member();
        token
    }

    /// Dart (line 11477): `Token reportAndSkipTypedefInClass(Token token)`
    pub fn report_and_skip_typedef_in_class(&mut self, token: TokenId) -> TokenId {
        // assert(token.isA(Keyword.TYPEDEF));
        self.report_recoverable_error(token, diag::typedef_in_class());
        self.listener.handle_invalid_member(token);
        // TODO(brianwilkerson): If the declaration appears to be a valid typedef
        // then skip the entire declaration so that we generate a single error
        // (above) rather than many unhelpful errors.
        self.listener.end_member();
        token
    }

    /// Dart (line 11549): `Token parsePattern( Token token, PatternContext patternContext,`
    /// `{int precedence = 1})`
    ///
    /// pattern               ::= logicalOrPattern
    /// logicalOrPattern      ::= logicalOrPattern ( '|' logicalAndPattern )?
    /// logicalAndPattern     ::= logicalAndPattern ( '&' relationalPattern )?
    /// relationalPattern     ::= ( equalityOperator | relationalOperator)
    ///                               relationalExpression
    ///                         | unaryPattern
    /// unaryPattern          ::= castPattern
    ///                         | nullCheckPattern
    ///                         | nullAssertPattern
    ///                         | primaryPattern
    /// castPattern ::= primaryPattern 'as' type
    /// nullAssertPattern ::= primaryPattern '!'
    /// nullCheckPattern ::= primaryPattern '?'
    ///
    /// [patternContext] indicates whether the pattern is refutable or
    /// irrefutable, and whether it occurs as part of a patternAssignment.
    pub fn parse_pattern(
        &mut self,
        token: TokenId,
        pattern_context: PatternContext,
        precedence: i32,
    ) -> TokenId {
        debug_assert!(precedence >= 1);
        debug_assert!(precedence <= SELECTOR_PRECEDENCE);
        self.listener.begin_pattern(token);
        let start = self.next(token);
        let mut token = self.parse_primary_pattern(token, pattern_context);
        loop {
            let next = self.next(token);
            let token_level = self.compute_precedence(next, /* forPattern = */ true);
            if token_level < precedence {
                self.listener.end_pattern(token);
                return token;
            }
            let case = match self.lexeme(next) {
                "as" => 0,
                "!" => 1,
                "?" => 2,
                "&&" | "||" => 3,
                _ => 4,
            };
            match case {
                // castPattern ::= primaryPattern 'as' type
                0 => {
                    if !self.is_last_pattern_allowed_inside_unary_pattern {
                        self.report_recoverable_error_with_end(
                            start,
                            token,
                            diag::invalid_inside_unary_pattern(),
                        );
                    }
                    token = next;
                    let operator = next;
                    self.listener.begin_as_operator_type(token);
                    let type_info = self.compute_type_after_is_or_as(token);
                    token = type_info.ensure_type_not_void(token, self);
                    self.listener.end_as_operator_type(operator);
                    self.listener.handle_cast_pattern(operator);
                }
                1 => {
                    if !self.is_last_pattern_allowed_inside_unary_pattern {
                        self.report_recoverable_error_with_end(
                            start,
                            token,
                            diag::invalid_inside_unary_pattern(),
                        );
                    }
                    // nullAssertPattern ::= primaryPattern '!'
                    self.listener.handle_null_assert_pattern(next);
                    token = next;
                }
                2 => {
                    if !self.is_last_pattern_allowed_inside_unary_pattern {
                        self.report_recoverable_error_with_end(
                            start,
                            token,
                            diag::invalid_inside_unary_pattern(),
                        );
                    }
                    // nullCheckPattern ::= primaryPattern '?'
                    self.listener.handle_null_check_pattern(next);
                    token = next;
                }
                3 => {
                    self.listener.begin_binary_pattern(next);
                    // Left associative so we parse the RHS one precedence level higher
                    token = self.parse_pattern(next, pattern_context, token_level + 1);
                    // Note that "next" here is before "token" and the one that had either
                    // "&&" or "||".
                    self.listener.end_binary_pattern(next);
                }
                _ => {
                    // Some other operator that doesn't belong in a pattern
                    self.listener.end_pattern(token);
                    return token;
                }
            }
            // None of the pattern types handled by the switch above are valid inside
            // a unary pattern.
            self.is_last_pattern_allowed_inside_unary_pattern = false;
        }
    }

    /// Dart (line 11658): `Token parsePrimaryPattern(Token token, PatternContext patternContext)`
    ///
    /// primaryPattern        ::= constantPattern
    ///                         | variablePattern
    ///                         | parenthesizedPattern
    ///                         | listPattern
    ///                         | mapPattern
    ///                         | recordPattern
    ///                         | objectPattern
    /// listPattern ::= typeArguments? '[' patterns? ']'
    /// mapPattern        ::= typeArguments? '{' mapPatternEntries? '}'
    /// mapPatternEntries ::= mapPatternEntry ( ',' mapPatternEntry )* ','?
    /// mapPatternEntry   ::= expression ':' pattern
    /// variablePattern ::= ( 'var' | 'final' | 'final'? type )? identifier
    /// parenthesizedPattern  ::= '(' pattern ')'
    /// recordPattern         ::= '(' patternFields? ')'
    /// patternFields         ::= patternField ( ',' patternField )* ','?
    /// patternField          ::= ( identifier? ':' )? pattern
    /// constantPattern ::= booleanLiteral
    ///                   | nullLiteral
    ///                   | numericLiteral
    ///                   | stringLiteral
    ///                   | identifier
    ///                   | qualifiedName
    ///                   | constObjectExpression
    ///                   | 'const' typeArguments? '[' elements? ']'
    ///                   | 'const' typeArguments? '{' elements? '}'
    ///                   | 'const' '(' expression ')'
    /// objectPattern ::= typeName typeArguments? '(' patternFields? ')'
    pub fn parse_primary_pattern(
        &mut self,
        token: TokenId,
        pattern_context: PatternContext,
    ) -> TokenId {
        let mut token = token;
        let start = token;
        let type_arg =
            compute_type_param_or_arg_mut(self.tokens_mut(), token, /* inDeclaration = */ true, false);
        let mut next = { let skipped = type_arg.skip_mut(self.tokens_mut(), token); self.next(skipped) };
        let case = match self.lexeme(next) {
            "[]" | "[" => 0,
            "{" => 1,
            _ => 2,
        };
        match case {
            0 => {
                // listPattern ::= typeArguments? '[' patterns? ']'
                token = type_arg.parse_arguments(token, self);
                token = self.parse_list_pattern_suffix(token, pattern_context);
                // A list pattern is a valid form of outerPattern, so verify that
                // skipOuterPattern would have skipped this pattern properly.
                // assert(
                //   identical(inhibitPrinting(() => skipOuterPattern(start)), token),
                // );
                self.is_last_pattern_allowed_inside_unary_pattern = true;
                return token;
            }
            1 => {
                // mapPattern        ::= typeArguments? '{' mapPatternEntries? '}'
                // mapPatternEntries ::= mapPatternEntry ( ',' mapPatternEntry )* ','?
                // mapPatternEntry   ::= expression ':' pattern
                token = type_arg.parse_arguments(token, self);
                token = self.parse_map_pattern_suffix(token, pattern_context);
                // A map pattern is a valid form of outerPattern, so verify that
                // skipOuterPattern would have skipped this pattern properly.
                // assert(
                //   identical(inhibitPrinting(() => skipOuterPattern(start)), token),
                // );
                self.is_last_pattern_allowed_inside_unary_pattern = true;
                return token;
            }
            _ => {}
        }
        // Whatever was after the optional type arguments didn't parse as a pattern
        // that can start with type arguments, so back up and reparse assuming that
        // we weren't looking at type arguments after all.
        next = self.next(token);
        let case = match self.lexeme(next) {
            "var" | "final" => 0,
            "(" => 1,
            "const" => 2,
            _ => 3,
        };
        match case {
            0 => {
                // variablePattern ::= ( 'var' | 'final' | 'final'? type )? identifier
                self.is_last_pattern_allowed_inside_unary_pattern = true;
                return self.parse_variable_pattern(token, pattern_context, &NO_TYPE);
            }
            1 => {
                // "(" could start a record type (which has to be followed by an
                // identifier (or ? identifier) though), e.g. `(int, int) foo`
                // or `(int, int)? bar`.
                let after_end_group = self.next(self.end_group(next).unwrap());
                if self.is_identifier(after_end_group)
                    || (self.is_a(after_end_group, TokenType::QUESTION)
                        && self.is_identifier(self.next(after_end_group)))
                {
                    let type_info = compute_variable_pattern_type_mut(self.tokens_mut(),
                        token,
                        /* required = */ true,
                    );
                    if type_info.is_record_type() && !type_info.recovered() {
                        self.is_last_pattern_allowed_inside_unary_pattern = true;
                        return self.parse_variable_pattern(token, pattern_context, &type_info);
                    }
                }
                // parenthesizedPattern  ::= '(' pattern ')'
                // recordPattern         ::= '(' patternFields? ')'
                // patternFields         ::= patternField ( ',' patternField )* ','?
                // patternField          ::= ( identifier? ':' )? pattern
                let next_next = self.next(next);
                if self.is_a(next_next, TokenType::CLOSE_PAREN) {
                    self.listener.handle_record_pattern(next, /* count = */ 0);
                    token = next_next;
                } else {
                    token =
                        self.parse_parenthesized_pattern_or_record_pattern(token, pattern_context);
                }
                // A record or parenthesized pattern is a valid form of outerPattern, so
                // verify that skipOuterPattern would have skipped this pattern
                // properly.
                // assert(
                //   identical(inhibitPrinting(() => skipOuterPattern(start)), token),
                // );
                self.is_last_pattern_allowed_inside_unary_pattern = true;
                return token;
            }
            2 => {
                // constantPattern ::= booleanLiteral
                //                   | nullLiteral
                //                   | '-'? numericLiteral
                //                   | stringLiteral
                //                   | identifier
                //                   | qualifiedName
                //                   | constObjectExpression
                //                   | 'const' typeArguments? '[' elements? ']'
                //                   | 'const' typeArguments? '{' elements? '}'
                //                   | 'const' '(' expression ')'
                let const_ = next;
                self.listener.begin_constant_pattern(Some(const_));
                // The supported precedence is [SELECTOR_PRECEDENCE] but for better
                // error recovery we allow for parsing [EQUALITY_PRECEDENCE] and higher,
                // and report an error in [_parsePrecedenceExpressionLoop] instead.
                token = self.parse_precedence_expression(
                    const_,
                    EQUALITY_PRECEDENCE,
                    /* allowCascades = */ false,
                    ConstantPatternContext::Explicit,
                );
                self.listener.end_constant_pattern(Some(const_));
                self.is_last_pattern_allowed_inside_unary_pattern = true;
                return token;
            }
            _ => {}
        }
        let ty = self.ty(next);
        if is_relational_operator(ty) || is_equality_operator(ty) {
            // TODO(paulberry): maybe handle other operators for error recovery?
            let operator = next;
            token = self.parse_precedence_expression(
                next,
                BITWISE_OR_PRECEDENCE,
                /* allowCascades = */ false,
                ConstantPatternContext::None,
            );
            self.listener.handle_relational_pattern(operator);
            self.is_last_pattern_allowed_inside_unary_pattern = false;
            return token;
        }
        let type_info = compute_variable_pattern_type_mut(self.tokens_mut(), token, false);
        if type_info != NO_TYPE {
            self.is_last_pattern_allowed_inside_unary_pattern = true;
            return self.parse_variable_pattern(token, pattern_context, &type_info);
        }
        // objectPattern ::= typeName typeArguments? '(' patternFields? ')'
        if self.is_identifier(next) {
            let before_first_identifier = token;
            token = next;
            let first_identifier = next;
            next = self.next(token);
            let mut dot: Option<TokenId> = None;
            let mut second_identifier: Option<TokenId> = None;
            if self.is_a(next, TokenType::PERIOD) {
                token = next;
                dot = Some(next);
                next = self.next(token);
                if self.is_identifier(next) {
                    token = next;
                    second_identifier = Some(next);
                } else {
                    second_identifier = Some(
                        IdentifierContext::ExpressionContinuation.ensure_identifier(token, self),
                    );
                }
            }
            let potential_type_arg = compute_type_param_or_arg_mut(self.tokens_mut(), token, false, false);
            let after_token = { let skipped = potential_type_arg.skip_mut(self.tokens_mut(), token); self.next(skipped) };
            if self.is_a(after_token, TokenType::OPEN_PAREN) && !potential_type_arg.recovered() {
                let type_arg = potential_type_arg;
                token = type_arg.parse_arguments(token, self);
                token = self.parse_object_pattern_rest(token, pattern_context);
                self.listener
                    .handle_object_pattern(first_identifier, dot, second_identifier);
                // An object pattern is a valid form of outerPattern, so verify that
                // skipOuterPattern would have skipped this pattern properly.
                // assert(
                //   identical(inhibitPrinting(() => skipOuterPattern(start)), token),
                // );
                self.is_last_pattern_allowed_inside_unary_pattern = true;
                return token;
            } else if dot.is_none() {
                // It's a single identifier.  If it's a wildcard pattern or we're in an
                // irrefutable context, parse it as a variable pattern.
                let name = self.lexeme(first_identifier);
                if !pattern_context.is_refutable() || name == "_" {
                    // It's a wildcard pattern with no preceding type, so parse it as a
                    // variable pattern.
                    self.is_last_pattern_allowed_inside_unary_pattern = true;
                    return self.parse_variable_pattern(
                        before_first_identifier,
                        pattern_context,
                        &type_info,
                    );
                } else if ILLEGAL_PATTERN_IDENTIFIERS.contains(&name) {
                    let message = diag::illegal_pattern_identifier_name(name);
                    self.report_recoverable_error(first_identifier, message);
                }
            }
            // It's not an object pattern so parse it as an expression.
            token = before_first_identifier;
        }
        self.listener
            .begin_constant_pattern(/* constKeyword = */ None);
        // The supported precedence is [SELECTOR_PRECEDENCE] but for better
        // error recovery we allow for parsing [EQUALITY_PRECEDENCE] and higher,
        // and report an error in [_parsePrecedenceExpressionLoop] instead.
        token = self.parse_precedence_expression(
            token,
            EQUALITY_PRECEDENCE,
            /* allowCascades = */ false,
            ConstantPatternContext::Implicit,
        );
        self.listener
            .end_constant_pattern(/* constKeyword = */ None);
        self.is_last_pattern_allowed_inside_unary_pattern = true;
        token
    }

    /// Dart (line 11869): `Token parseVariablePattern( Token token, PatternContext patternContext,`
    /// `{TypeInfo typeInfo = noType})`
    ///
    /// Parses variable pattern, or an identifier pattern that represents a
    /// variable, starting after [token].  [typeInfo] is information about the
    /// type appearing after [token], if any.
    ///
    /// variablePattern   ::= ( 'var' | 'final' | 'final'? type ) identifier
    /// identifierPattern ::= identifier
    pub fn parse_variable_pattern(
        &mut self,
        token: TokenId,
        pattern_context: PatternContext,
        type_info: &TypeInfo,
    ) -> TokenId {
        let mut token = token;
        let mut type_info = type_info.clone();
        let mut is_bare_identifier = false;
        let mut keyword: Option<TokenId> = None;
        if type_info != NO_TYPE {
            token = type_info.parse_type(token, self);
        } else {
            let next = self.next(token);
            if self.is_a(next, Keyword::VAR) || self.is_a(next, Keyword::FINAL) {
                token = next;
                keyword = Some(next);
                let next_is_paren = self.is_a(self.next(token), TokenType::OPEN_PAREN);
                type_info = compute_variable_pattern_type_mut(self.tokens_mut(), token, next_is_paren);
                token = type_info.parse_type(token, self);
            } else {
                is_bare_identifier = true;
            }
        }
        let next = self.next(token);
        if self.is_identifier(next) {
            token = next;
        } else {
            // Recovery
            token = self.insert_synthetic_identifier(
                token,
                IdentifierContext::LocalVariableDeclaration,
                None,
                None,
            );
        }
        let variable_name = self.lexeme(token).to_string();
        match pattern_context {
            PatternContext::Declaration => {
                // It is a compile-time error if a variable pattern in a declaration
                // context is marked with var or final.
                if let Some(keyword) = keyword {
                    self.report_recoverable_error(
                        keyword,
                        diag::variable_pattern_keyword_in_declaration_context(),
                    );
                }
            }
            PatternContext::Matching => {
                // All forms of variable patterns are valid in a matching context.  But
                // we do need to check for redundant `var`.
                if type_info != NO_TYPE {
                    if let Some(keyword) = keyword {
                        if self.is_a(keyword, Keyword::VAR) {
                            self.report_recoverable_error(keyword, diag::type_after_var());
                        }
                    }
                }
            }
            PatternContext::Assignment => {
                // It is a compile-time error if a variable pattern appears in an
                // assignment context.  However the spec doesn't consider a bare
                // identifier to be a variable pattern (it's an "identifier pattern").
                if !is_bare_identifier {
                    let message =
                        diag::pattern_assignment_declares_variable(if variable_name.is_empty() {
                            "(unnamed)"
                        } else {
                            &variable_name
                        });
                    self.report_recoverable_error(token, message);
                }
            }
        }
        let in_assignment_pattern = pattern_context == PatternContext::Assignment;
        if variable_name == "_" {
            if is_bare_identifier {
                self.listener.handle_no_type(token);
            }
            self.listener.handle_wildcard_pattern(keyword, token);
        } else if in_assignment_pattern && is_bare_identifier {
            if ILLEGAL_PATTERN_IDENTIFIERS.contains(&variable_name.as_str()) {
                let message = diag::illegal_pattern_assignment_variable_name(self.lexeme(token));
                self.report_recoverable_error(token, message);
            }
            self.listener.handle_assigned_variable_pattern(token);
        } else {
            if ILLEGAL_PATTERN_IDENTIFIERS.contains(&variable_name.as_str()) {
                let message = diag::illegal_pattern_variable_name(self.lexeme(token));
                self.report_recoverable_error(token, message);
            }
            if is_bare_identifier {
                self.listener.handle_no_type(token);
            }
            self.listener
                .handle_declared_variable_pattern(keyword, token, in_assignment_pattern);
        }
        token
    }

    /// Dart (line 11971): `Token parseListPatternSuffix(Token token, PatternContext patternContext)`
    ///
    /// This method parses the portion of a list pattern starting with the left
    /// bracket.
    ///
    /// listPattern ::= typeArguments? '[' patterns? ']'
    pub fn parse_list_pattern_suffix(
        &mut self,
        token: TokenId,
        pattern_context: PatternContext,
    ) -> TokenId {
        let before_token = token;
        let mut token = self.next(token);
        let begin_token = token;
        // assert(
        //   token.isA(TokenType.OPEN_SQUARE_BRACKET) || token.isA(TokenType.INDEX),
        // );
        let mut count: i32 = 0;
        if self.is_a(token, TokenType::INDEX) {
            token = self.rewrite_square_brackets(before_token);
            token = self.next(token);
            let next = self.next(token);
            self.listener
                .handle_list_pattern(/* count = */ 0, token, next);
            return self.next(token);
        }
        let old = self.may_parse_function_expressions;
        self.may_parse_function_expressions = true;
        loop {
            let mut next = self.next(token);
            if self.is_a(next, TokenType::CLOSE_SQUARE_BRACKET) {
                token = next;
                break;
            }
            if self.is_a(next, TokenType::PERIOD_PERIOD_PERIOD) {
                let dots = next;
                token = next;
                next = self.next(token);
                let has_sub_pattern = looks_like_pattern_start(self.tokens(), next);
                if has_sub_pattern {
                    token = self.parse_pattern(token, pattern_context, 1);
                }
                self.listener.handle_rest_pattern(dots, has_sub_pattern);
            } else {
                token = self.parse_pattern(token, pattern_context, 1);
                if next == self.next(token) {
                    // No tokens were consumed (though it's possible that a synthetic
                    // token was inserted). If this happens, go ahead and skip the next
                    // token to ensure that progress is made.
                    token = self.next(token);
                }
            }
            next = self.next(token);
            count += 1;
            if !self.is_a(next, TokenType::COMMA) {
                if self.is_a(next, TokenType::CLOSE_SQUARE_BRACKET) {
                    token = next;
                    break;
                }

                // Recovery
                if !looks_like_literal_entry(self.tokens(), next) {
                    let end_group = self.end_group(begin_token).unwrap();
                    if self.is_synthetic(end_group) {
                        // The scanner has already reported an error,
                        // but inserted `]` in the wrong place.
                        token = self.rewriter().move_synthetic(token, end_group);
                    } else {
                        // Report an error and jump to the end of the list.
                        self.report_recoverable_error(next, diag::expected_but_got("]"));
                        token = end_group;
                    }
                    break;
                }
                // This looks like the start of an expression.
                // Report an error, insert the comma, and continue parsing.
                let offset = self.offset(next);
                let byte = self.tokens().byte_offset(next);
                let comma = self
                    .tokens_mut()
                    .push_synthetic(TokenType::COMMA, offset, byte);
                let message = diag::expected_but_got(",");
                next = self.rewrite_and_recover(token, message, comma);
            }
            token = next;
        }
        self.listener.handle_list_pattern(count, begin_token, token);
        self.may_parse_function_expressions = old;
        token
    }

    /// Dart (line 12054): `Token parseMapPatternSuffix(Token token, PatternContext patternContext)`
    ///
    /// This method parses the portion of a map pattern starting with the left
    /// curly brace.
    ///
    /// mapPattern        ::= typeArguments? '{' mapPatternEntries? '}'
    /// mapPatternEntries ::= mapPatternEntry ( ',' mapPatternEntry )* ','?
    /// mapPatternEntry   ::= expression ':' pattern
    pub fn parse_map_pattern_suffix(
        &mut self,
        token: TokenId,
        pattern_context: PatternContext,
    ) -> TokenId {
        let mut token = self.next(token);
        let left_brace = token;
        // assert(leftBrace.isA(TokenType.OPEN_CURLY_BRACKET));
        let mut next = self.next(token);
        if self.is_a(next, TokenType::CLOSE_CURLY_BRACKET) {
            self.listener
                .handle_map_pattern(/* count = */ 0, left_brace, next);
            return next;
        }

        let old = self.may_parse_function_expressions;
        self.may_parse_function_expressions = true;
        let mut count: i32 = 0;
        loop {
            if self.is_a(next, TokenType::PERIOD_PERIOD_PERIOD) {
                let dots = next;
                token = next;
                next = self.next(token);
                let has_sub_pattern = looks_like_pattern_start(self.tokens(), next);
                if has_sub_pattern {
                    token = self.parse_pattern(token, pattern_context, 1);
                }
                self.listener.handle_rest_pattern(dots, has_sub_pattern);
            } else {
                token = self.parse_expression(token);
                let mut colon = self.next(token);
                if !self.is_a(colon, TokenType::COLON) {
                    // Recover from a missing colon by inserting one.
                    let offset = self.char_offset(next);
                    let byte = self.tokens().byte_offset(next);
                    let new_colon =
                        self.tokens_mut()
                            .push_synthetic(TokenType::COLON, offset, byte);
                    colon = self.rewrite_and_recover(token, diag::expected_but_got(":"), new_colon);
                }
                token = self.parse_pattern(colon, pattern_context, 1);
                if next == self.next(token) {
                    // No tokens were consumed (though it's possible that a synthetic
                    // token was inserted). If this happens, go ahead and skip the next
                    // token to ensure that progress is made.
                    token = self.next(token);
                }
                let after = self.next(token);
                self.listener.handle_map_pattern_entry(colon, after);
            }
            count += 1;
            next = self.next(token);

            let mut comma: Option<TokenId> = None;
            if self.is_a(next, TokenType::COMMA) {
                token = next;
                comma = Some(next);
                next = self.next(token);
            }
            if self.is_a(next, TokenType::CLOSE_CURLY_BRACKET) {
                break;
            }

            if comma.is_none() {
                // Recovery
                if looks_like_literal_entry(self.tokens(), next) {
                    // If this looks like the start of an expression,
                    // then report an error, insert the comma, and continue parsing.
                    let offset = self.offset(next);
                    let byte = self.tokens().byte_offset(next);
                    let comma = self
                        .tokens_mut()
                        .push_synthetic(TokenType::COMMA, offset, byte);
                    let message = diag::expected_but_got(",");
                    token = self.rewrite_and_recover(token, message, comma);
                } else {
                    self.report_recoverable_error(next, diag::expected_but_got("}"));
                    // Scanner guarantees a closing curly bracket
                    next = self.end_group(left_brace).unwrap();
                    break;
                }
            }
        }
        self.may_parse_function_expressions = old;
        self.listener.handle_map_pattern(count, left_brace, next);
        next
    }

    /// Dart (line 12143): `Token parseParenthesizedPatternOrRecordPattern( Token token, PatternContext patternContext, )`
    ///
    /// Parses either a parenthesizedPattern or a recordPattern.
    ///
    /// parenthesizedPattern  ::= '(' pattern ')'
    /// recordPattern         ::= '(' patternFields? ')'
    /// patternFields         ::= patternField ( ',' patternField )* ','?
    /// patternField          ::= ( identifier? ':' )? pattern
    pub fn parse_parenthesized_pattern_or_record_pattern(
        &mut self,
        token: TokenId,
        pattern_context: PatternContext,
    ) -> TokenId {
        let begin = self.next(token);
        // assert(begin.isA(TokenType.OPEN_PAREN));
        let old = self.may_parse_function_expressions;
        self.may_parse_function_expressions = true;

        let mut token = begin;
        let mut count: i32 = 0;
        let mut was_record = false;
        let mut was_valid_record = false;
        loop {
            let mut next = self.next(token);
            if (count > 0 || was_record) && self.is_a(next, TokenType::CLOSE_PAREN) {
                break;
            }
            let mut colon: Option<TokenId> = None;
            if self.is_a(next, TokenType::COLON) {
                was_record = true;
                was_valid_record = true;
                self.listener.handle_no_name(token);
                token = next;
                colon = Some(next);
            } else if !self.is_a(next, TokenType::OPEN_PAREN)
                && self.is_a(self.next(next), TokenType::COLON)
            {
                // We don't allow `next` to be `(` here because
                // `((:a, :b), :c, :d)` (and similar) is fine.
                // Record with named expression.
                was_record = true;
                token = self.ensure_identifier(token, IdentifierContext::NamedRecordFieldReference);
                token = self.next(token);
                colon = Some(token);
                was_valid_record = true;
            }
            token = self.parse_pattern(token, pattern_context, 1);
            next = self.next(token);
            if was_record || colon.is_some() {
                self.listener.handle_pattern_field(colon);
            }
            count += 1;
            if !self.is_a(next, TokenType::COMMA) {
                break;
            } else {
                // It is a comma, i.e. it's a record.
                if !was_record && colon.is_none() {
                    self.listener.handle_pattern_field(colon);
                }
                was_record = true;
                was_valid_record = true;
            }
            token = next;
        }
        token = self.ensure_close_paren(token, begin);
        // assert(token.isA(TokenType.CLOSE_PAREN));

        // assert(wasRecord || count <= 1);

        if was_record {
            if count == 1 && !was_valid_record {
                self.report_recoverable_error(
                    token,
                    diag::record_literal_one_positional_field_no_trailing_comma(),
                );
            }
            self.listener.handle_record_pattern(begin, count);
        } else {
            self.listener.handle_parenthesized_pattern(begin);
        }

        self.may_parse_function_expressions = old;
        token
    }

    /// Dart (line 12223): `Token parseObjectPatternRest(Token token, PatternContext patternContext)`
    ///
    /// Parses the rest of an objectPattern, where [token] is the token before the
    /// `(`.
    ///
    /// objectPattern ::= typeName typeArguments? '(' patternFields? ')'
    pub fn parse_object_pattern_rest(
        &mut self,
        token: TokenId,
        pattern_context: PatternContext,
    ) -> TokenId {
        let mut token = self.next(token);
        let begin = token;
        // assert(begin.isA(TokenType.OPEN_PAREN));
        let mut argument_count: i32 = 0;
        let old = self.may_parse_function_expressions;
        self.may_parse_function_expressions = true;
        loop {
            let mut next = self.next(token);
            if self.is_a(next, TokenType::CLOSE_PAREN) {
                token = next;
                break;
            }
            let mut colon: Option<TokenId> = None;
            if self.is_a(next, TokenType::COLON) {
                self.listener.handle_no_name(token);
                token = next;
                colon = Some(next);
            } else if self.is_a(self.next(next), TokenType::COLON) {
                // This is different from `parseParenthesizedPatternOrRecordPattern`
                // because this isn't valid because of the missing name:
                // `var Point((:x, :y), :z) = Point((x: 1, y: 2), 3);`
                token = self.ensure_identifier(token, IdentifierContext::NamedArgumentReference);
                token = self.next(token);
                colon = Some(token);
            }
            token = self.parse_pattern(token, pattern_context, 1);
            next = self.next(token);
            self.listener.handle_pattern_field(colon);
            argument_count += 1;
            if !self.is_a(next, TokenType::COMMA) {
                if self.is_a(next, TokenType::CLOSE_PAREN) {
                    token = next;
                    break;
                }
                // Recovery
                if looks_like_expression_start(self.tokens(), next) {
                    // If this looks like the start of an expression,
                    // then report an error, insert the comma, and continue parsing.
                    let offset = self.offset(next);
                    let byte = self.tokens().byte_offset(next);
                    let comma = self
                        .tokens_mut()
                        .push_synthetic(TokenType::COMMA, offset, byte);
                    next = self.rewrite_and_recover(token, diag::expected_but_got(","), comma);
                } else {
                    token = self.ensure_close_paren(token, begin);
                    break;
                }
            }
            token = next;
        }
        // assert(token.isA(TokenType.CLOSE_PAREN));
        self.may_parse_function_expressions = old;
        self.listener
            .handle_object_pattern_fields(argument_count, begin, token);
        token
    }

    /// Dart (line 12286): `bool looksLikeOuterPatternEquals(Token token)`
    ///
    /// Returns `true` if the given [token] looks like an outer pattern followed
    /// by `=`.  This occurs in the following grammar productions:
    ///
    /// patternVariableDeclaration ::= ( 'final' | 'var' ) outerPattern '='
    ///                                expression
    /// patternAssignment ::= outerPattern '=' expression
    pub fn looks_like_outer_pattern_equals(&mut self, token: TokenId) -> bool {
        let after_outer_pattern = self.skip_outer_pattern(token);
        let Some(after_outer_pattern) = after_outer_pattern else {
            return false;
        };
        self.is_a(self.next(after_outer_pattern), TokenType::EQ)
    }

    /// Dart (line 12300): `Token? skipOuterPattern(Token token)`
    ///
    /// Tries to advance beyond an "outer pattern" starting from [token].  If the
    /// next construct after [token] is not an outer pattern, returns `null`.
    ///
    /// outerPattern ::= parenthesizedPattern
    ///                | listPattern
    ///                | mapPattern
    ///                | recordPattern
    ///                | objectPattern
    pub fn skip_outer_pattern(&mut self, token: TokenId) -> Option<TokenId> {
        let mut token = token;
        let mut next = self.next(token);
        if self.is_identifier(next) {
            token = next;
            next = self.next(token);
            if !self.is_a(next, TokenType::PERIOD) {
                return self.skip_object_pattern_rest(token);
            }
            token = next;
            next = self.next(token);
            if self.is_identifier(next) {
                return self.skip_object_pattern_rest(next);
            } else {
                // IDENTIFIER `.` NON-IDENTIFIER (not a pattern)
                return None;
            }
        }
        let type_param_or_arg = compute_type_param_or_arg_mut(self.tokens_mut(), token, false, false);
        token = type_param_or_arg.skip_mut(self.tokens_mut(), token);
        next = self.next(token);
        if self.is_a(next, TokenType::INDEX) {
            // Empty list pattern
            return Some(next);
        }
        if self.is_a(next, TokenType::OPEN_SQUARE_BRACKET)
            || self.is_a(next, TokenType::OPEN_CURLY_BRACKET)
        {
            // List or map pattern
            return self.end_group(next);
        }
        if type_param_or_arg == NO_TYPE_PARAM_OR_ARG && self.is_a(next, TokenType::OPEN_PAREN) {
            // Record or parenthesized pattern
            return self.end_group(next);
        }
        // Not an outer pattern
        None
    }

    /// Dart (line 12342): `Token? skipObjectPatternRest(Token token)`
    ///
    /// Tries to advance through an object pattern, where [token] is the last
    /// token of the object pattern's type name.  If the tokens following
    /// [token] don't look like the rest of an object pattern, returns `null`.
    ///
    /// objectPattern ::= typeName typeArguments? '(' patternFields? ')'
    pub fn skip_object_pattern_rest(&mut self, token: TokenId) -> Option<TokenId> {
        let type_param_or_arg = compute_type_param_or_arg_mut(self.tokens_mut(), token, false, false);
        let token = type_param_or_arg.skip_mut(self.tokens_mut(), token);
        let next = self.tokens().next(token).get()?;
        if !self.is_a(next, TokenType::OPEN_PAREN) {
            return None;
        }
        self.end_group(next)
    }

    /// Dart (line 12353): `Token parsePatternVariableDeclarationStatement( Token keyword, Token start, Token varOrFinal, )`
    ///
    /// patternVariableDeclaration ::= ( 'final' | 'var' ) outerPattern '='
    ///                                expression
    pub fn parse_pattern_variable_declaration_statement(
        &mut self,
        keyword: TokenId,
        start: TokenId,
        var_or_final: TokenId,
    ) -> TokenId {
        let mut token = self.parse_pattern(keyword, PatternContext::Declaration, 1);
        let equals = self.next(token);
        // Caller should have assured that the pattern was followed by an `=`.
        // assert(equals.isA(TokenType.EQ));
        token = self.parse_expression(equals);
        let semicolon = self.ensure_semicolon(token);
        self.listener
            .handle_pattern_variable_declaration_statement(keyword, equals, semicolon);
        semicolon
    }

    /// Dart (line 12373): `Token parsePatternAssignment(Token token)`
    ///
    /// patternAssignment ::= outerPattern '=' expression
    pub fn parse_pattern_assignment(&mut self, token: TokenId) -> TokenId {
        self.parse_pattern_assignment_impl(token, /* allowCascades = */ true)
    }

    /// Dart (line 12377): `Token _parsePatternAssignment(Token token, bool allowCascades)`
    pub fn parse_pattern_assignment_impl(
        &mut self,
        token: TokenId,
        allow_cascades: bool,
    ) -> TokenId {
        let mut token = self.parse_pattern(token, PatternContext::Assignment, 1);
        let equals = self.next(token);
        // Caller should have assured that the pattern was followed by an `=`.
        // assert(equals.isA(TokenType.EQ));
        token = if allow_cascades {
            self.parse_expression(equals)
        } else {
            self.parse_expression_without_cascade(equals)
        };
        self.listener.handle_pattern_assignment(equals);
        token
    }

    /// Dart (line 12393): `Token parseSwitchExpression(Token token)`
    ///
    /// switchExpression    ::= 'switch' '(' expression ')' '{'
    ///                         switchExpressionCase ( ',' switchExpressionCase )*
    ///                             ','? '}'
    /// switchExpressionCase    ::= guardedPattern '=>' expression
    pub fn parse_switch_expression(&mut self, token: TokenId) -> TokenId {
        let switch_keyword = self.next(token);
        // assert(switchKeyword.isA(Keyword.SWITCH));
        let old = self.may_parse_function_expressions;
        self.may_parse_function_expressions = true;
        self.listener.begin_switch_expression(switch_keyword);
        let mut token = self.ensure_parenthesized_condition(switch_keyword, false);
        token = self.ensure_block(token, Some(BlockKind::SwitchExpression));
        let begin_switch = token;
        self.listener.begin_switch_expression_block(begin_switch);
        let mut next = self.next(token);
        let mut case_count: i32 = 0;
        if !self.is_a(next, TokenType::CLOSE_CURLY_BRACKET) {
            self.may_parse_function_expressions = false;
            loop {
                self.listener.begin_switch_expression_case();
                next = self.next(token);
                let begin_token = next;
                if self.is_a(next, Keyword::DEFAULT) {
                    self.report_recoverable_error(next, diag::default_in_switch_expression());
                    self.listener.handle_no_type(next);
                    self.listener.handle_wildcard_pattern(None, next);
                    token = next;
                } else {
                    if self.is_a(next, Keyword::CASE) {
                        let message = diag::unexpected_token(self.lexeme(next));
                        self.report_recoverable_error(next, message);
                        token = next;
                    }
                    token = self.parse_pattern(token, PatternContext::Matching, 1);
                }
                self.listener.handle_switch_expression_case_pattern(token);
                let mut when: Option<TokenId> = None;
                next = self.next(token);
                if self.is_a(next, Keyword::WHEN) {
                    token = next;
                    when = Some(next);
                    token = self.parse_expression(token);
                }
                let arrow: TokenId;
                if self.is_a(next, TokenType::COLON) {
                    // User accidentally used `:` instead of `=>`
                    arrow = next;
                    self.report_recoverable_error(arrow, diag::expected_but_got("=>"));
                } else {
                    arrow = self.ensure_function_arrow(token);
                }
                token = arrow;
                self.may_parse_function_expressions = true;
                token = self.parse_expression(token);
                self.may_parse_function_expressions = false;
                self.listener
                    .end_switch_expression_case(begin_token, when, arrow, token);
                case_count += 1;
                next = self.next(token);

                let mut comma: Option<TokenId> = None;
                if self.is_a(next, TokenType::COMMA) {
                    token = next;
                    comma = Some(next);
                    next = self.next(token);
                } else if self.is_a(next, TokenType::SEMICOLON) {
                    // User accidentally used `;` instead of `,`
                    self.report_recoverable_error(next, diag::expected_but_got(","));
                    token = next;
                    comma = Some(next);
                    next = self.next(token);
                }
                if self.is_a(next, TokenType::CLOSE_CURLY_BRACKET) {
                    break;
                }

                if comma.is_none() {
                    // Recovery
                    if looks_like_pattern_start(self.tokens(), next) {
                        // If this looks like the start of a pattern, then report an error,
                        // insert the comma, and continue parsing.
                        let offset = self.offset(next);
                        let byte = self.tokens().byte_offset(next);
                        let comma =
                            self.tokens_mut()
                                .push_synthetic(TokenType::COMMA, offset, byte);
                        let message = diag::expected_but_got(",");
                        token = self.rewrite_and_recover(token, message, comma);
                    } else {
                        // Scanner guarantees a closing curly bracket
                        let closing_bracket = self.end_group(begin_switch).unwrap();
                        comma = self.find_next_comma_or_semicolon(next, closing_bracket);
                        if let Some(comma) = comma {
                            // Note: `findNextCommaOrSemicolon` might have found a `;` instead
                            // of a `,`, but if it did, there's need to report an additional
                            // error.
                            self.report_recoverable_error(next, diag::expected_but_got(","));
                            token = comma;
                            next = self.next(token);
                        } else {
                            self.report_recoverable_error(next, diag::expected_but_got("}"));
                            next = closing_bracket;
                            break;
                        }
                    }
                }
            }
        }
        self.listener
            .end_switch_expression_block(case_count, begin_switch, next);
        self.may_parse_function_expressions = old;
        token = next;
        // assert(token.isEof || token.isA(TokenType.CLOSE_CURLY_BRACKET));
        self.listener.end_switch_expression(switch_keyword, token);
        token
    }

    /// Dart (line 12518): `Token? findNextCommaOrSemicolon(Token token, Token limit)`
    ///
    /// Finds and returns the next `,` or `;` token, starting at [token], but not
    /// searching beyond [limit].  If a begin token is encountered, the search
    /// proceeds after its matching end token, so the returned token (if any) will
    /// not be any more deeply nested than the starting point.
    pub fn find_next_comma_or_semicolon(
        &mut self,
        token: TokenId,
        limit: TokenId,
    ) -> Option<TokenId> {
        let mut token = token;
        loop {
            if self.is_eof(token) || token == limit {
                return None;
            }
            if self.is_a(token, TokenType::COMMA) || self.is_a(token, TokenType::SEMICOLON) {
                return Some(token);
            }
            token = match self.end_group(token) {
                Some(end_group) => end_group,
                None => self.next(token),
            };
        }
    }
}
