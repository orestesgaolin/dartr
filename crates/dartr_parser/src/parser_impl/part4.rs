// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/parser_impl.dart (lines 6687-8450)

#![allow(unused_imports, unused_variables, unused_mut, clippy::all)]

#[allow(unused_imports)]
use crate::type_info::{
    compute_method_type_arguments_mut, compute_type_mut, compute_type_param_or_arg_mut,
    compute_variable_pattern_type_mut,
};
use dartr_diagnostics::cfe::{CfeCode, CfeMessage};
use dartr_diagnostics::cfe_codes as diag;
use dartr_syntax::token_constants::*;
use dartr_syntax::{Keyword, TokenId, TokenType, Tokens};

use super::{
    AwaitOrYieldContext, ConstantPatternContext, ForPartsContext, Parser, PatternContext,
    TOKEN_RECOVERY_REPLACEMENTS,
};
use crate::assert::Assert;
use crate::async_modifier::AsyncModifier;
use crate::block_kind::BlockKind;
use crate::constructor_reference_context::ConstructorReferenceContext;
use crate::declaration_kind::{DeclarationHeaderKind, DeclarationKind};
use crate::directive_context::DirectiveContext;
use crate::experimental_features::ExperimentalFlag;
use crate::formal_parameter_kind::FormalParameterKind;
use crate::identifier_context::IdentifierContext;
use crate::listener::Listener;
use crate::listener_stack::Layer;
use crate::literal_entry_info::LiteralEntryInfo;
use crate::loop_state::LoopState;
use crate::member_kind::MemberKind;
use crate::modifier_context::ModifierContext;
use crate::type_info::{
    NO_TYPE_PARAM_OR_ARG, TypeInfo, TypeParamOrArgInfo, compute_method_type_arguments,
    compute_type_param_or_arg,
};

// Precedence levels (Dart `scanner/token.dart`). Private copies: the
// skeleton has no shared definition of these constants.
const ASSIGNMENT_PRECEDENCE: i32 = 1;
const CASCADE_PRECEDENCE: i32 = 2;
const EQUALITY_PRECEDENCE: i32 = 7;
const RELATIONAL_PRECEDENCE: i32 = 8;
const MULTIPLICATIVE_PRECEDENCE: i32 = 14;
const PREFIX_PRECEDENCE: i32 = 15;
const POSTFIX_PRECEDENCE: i32 = 16;
const SELECTOR_PRECEDENCE: i32 = 17;

/// Dart `_tokenRecoveryReplacements[lexeme]` (`None` when the map does not
/// contain the key).
fn token_recovery_replacements(lexeme: &str) -> Option<&'static [TokenType]> {
    TOKEN_RECOVERY_REPLACEMENTS
        .iter()
        .find(|(key, _)| *key == lexeme)
        .map(|(_, value)| *value)
}

impl<L: Listener> Parser<L> {
    /// Dart (line 6687): `Token parseStatement(Token token)`
    pub fn parse_statement(&mut self, token: TokenId) -> TokenId {
        let depth = self.statement_depth;
        self.statement_depth += 1;
        if depth > 500 {
            // This happens for degenerate programs, for example, a lot of nested
            // if-statements. The language test deep_nesting2_negative_test, for
            // example, provokes this.
            return self.recover_from_stack_overflow(token);
        }
        let result = self.parse_statement_x(token);
        self.statement_depth -= 1;
        result
    }

    /// Dart (line 6699): `Token parseStatementX(Token token)`
    pub fn parse_statement_x(&mut self, token: TokenId) -> TokenId {
        let next = self.next(token);
        if self.kind(next) == IDENTIFIER_TOKEN {
            if self.is_a(self.next(next), TokenType::COLON) {
                return self.parse_labeled_statement(token);
            }
            return self.parse_expression_statement_or_declaration_after_modifiers(
                token, token, /* lateToken = */ None, /* varFinalOrConst = */ None,
                /* typeInfo = */ None, None,
            );
        }
        let value: Option<&'static str> = self.string_value(next);
        if value == Some("{") {
            // The scanner ensures that `{` always has a closing `}`.
            if self.is_patterns_feature_enabled
                && self.is_a(self.next(self.end_group(next).unwrap()), TokenType::EQ)
            {
                // Expression statement beginning with a pattern assignment
                return self.parse_expression_statement(token);
            } else {
                return self.parse_block(token, BlockKind::Statement);
            }
        } else if value == Some("return") {
            return self.parse_return_statement(token);
        } else if value == Some("var") || value == Some("final") {
            let var_or_final = next;
            if !self.is_modifier(self.next(var_or_final)) {
                return self.parse_expression_statement_or_declaration_after_modifiers(
                    var_or_final,
                    token,
                    /* lateToken = */ None,
                    Some(var_or_final),
                    /* typeInfo = */ None,
                    None,
                );
            }
            return self.parse_expression_statement_or_declaration(token, None);
        } else if value == Some("if") {
            return self.parse_if_statement(token);
        } else if value == Some("await") && self.is_a(self.next(next), Keyword::FOR) {
            return self.parse_for_statement(next, Some(next));
        } else if value == Some("for") {
            return self.parse_for_statement(token, /* awaitToken = */ None);
        } else if value == Some("rethrow") {
            return self.parse_rethrow_statement(token);
        } else if value == Some("while") {
            return self.parse_while_statement(token);
        } else if value == Some("do") {
            return self.parse_do_while_statement(token);
        } else if value == Some("try") {
            return self.parse_try_statement(token);
        } else if value == Some("switch") {
            return self.parse_switch_statement(token);
        } else if value == Some("break") {
            return self.parse_break_statement(token);
        } else if value == Some("continue") {
            return self.parse_continue_statement(token);
        } else if value == Some("assert") {
            return self.parse_assert_statement(token);
        } else if value == Some(";") {
            return self.parse_empty_statement(token);
        } else if value == Some("yield") {
            match self.async_state {
                AsyncModifier::Sync => {
                    if self.is_a(self.next(next), TokenType::COLON) {
                        return self.parse_labeled_statement(token);
                    }
                    if self.looks_like_yield_statement(token, AwaitOrYieldContext::Statement) {
                        // Recovery: looks like an expression preceded by `yield` but not
                        // inside an Async or AsyncStar context. parseYieldStatement will
                        // report the error.
                        return self.parse_yield_statement(token);
                    }
                    return self.parse_expression_statement_or_declaration(token, None);
                }
                AsyncModifier::SyncStar | AsyncModifier::AsyncStar => {
                    return self.parse_yield_statement(token);
                }
                AsyncModifier::Async => {
                    return self.parse_yield_statement(token);
                }
            }
        } else if value == Some("const") {
            return self.parse_expression_statement_or_const_declaration(token);
        } else if value == Some("await") {
            if self.in_plain_sync() {
                if !self.looks_like_await_expression(token, AwaitOrYieldContext::Statement) {
                    return self.parse_expression_statement_or_declaration(token, None);
                }
                // Recovery: looks like an expression preceded by `await`
                // but not inside an async context.
                // Fall through to parseExpressionStatement
                // and parseAwaitExpression will report the error.
            }
            return self.parse_expression_statement(token);
        } else if value == Some("set") && self.is_identifier(self.next(next)) {
            // Recovery: invalid use of `set`
            self.report_recoverable_error_with_token(next, diag::unexpected_token);
            return self.parse_statement_x(next);
        } else if self.is_identifier(next) {
            if self.is_a(self.next(next), TokenType::COLON) {
                return self.parse_labeled_statement(token);
            }
            return self.parse_expression_statement_or_declaration(token, None);
        } else {
            return self.parse_expression_statement_or_declaration(token, None);
        }
    }

    /// Dart (line 6814): `Token parseYieldStatement(Token token)`
    ///
    /// ```
    /// yieldStatement:
    ///   'yield' expression? ';'
    /// ;
    /// ```
    pub fn parse_yield_statement(&mut self, token: TokenId) -> TokenId {
        let mut token = self.next(token);
        let begin = token;
        debug_assert!(self.is_a(token, Keyword::YIELD));
        self.listener.begin_yield_statement(begin);
        let mut star_token: Option<TokenId> = None;
        if self.is_a(self.next(token), TokenType::STAR) {
            token = self.next(token);
            star_token = Some(token);
        }
        token = self.parse_expression(token);
        token = self.ensure_semicolon(token);
        if self.in_generator() {
            self.listener.end_yield_statement(begin, star_token, token);
        } else {
            let error_code: &'static CfeCode = &diag::YIELD_NOT_GENERATOR;
            self.report_recoverable_error(begin, diag::yield_not_generator());
            // TODO(srawlins): Add tests in analyzer to ensure the AstBuilder
            //  correctly handles invalid yields, and that the error message is
            //  correctly plumbed through.
            self.listener
                .end_invalid_yield_statement(begin, star_token, token, error_code);
        }
        token
    }

    /// Dart (line 6842): `Token parseReturnStatement(Token token)`
    ///
    /// ```
    /// returnStatement:
    ///   'return' expression? ';'
    /// ;
    /// ```
    pub fn parse_return_statement(&mut self, token: TokenId) -> TokenId {
        let mut token = self.next(token);
        let begin = token;
        debug_assert!(self.is_a(token, Keyword::RETURN));
        self.listener.begin_return_statement(begin);
        let next = self.next(token);
        if self.is_a(next, TokenType::SEMICOLON) {
            self.listener
                .end_return_statement(/* hasExpression = */ false, begin, next);
            return next;
        }
        token = self.parse_expression(token);
        token = self.ensure_semicolon(token);
        self.listener
            .end_return_statement(/* hasExpression = */ true, begin, token);
        if self.in_generator() {
            self.listener
                .handle_invalid_statement(begin, diag::generator_returns_value());
        }
        token
    }

    /// Dart (line 6865): `Token parseLabel(Token token)`
    ///
    /// ```
    /// label:
    ///   identifier ':'
    /// ;
    /// ```
    pub fn parse_label(&mut self, token: TokenId) -> TokenId {
        debug_assert!(self.is_identifier(self.next(token)));
        let token = self.ensure_identifier(token, IdentifierContext::LabelDeclaration);
        let token = self.next(token);
        debug_assert!(self.is_a(token, TokenType::COLON));
        self.listener.handle_label(token);
        token
    }

    /// Dart (line 6878): `Token parseLabeledStatement(Token token)`
    ///
    /// ```
    /// statement:
    ///   label* nonLabelledStatement
    /// ;
    /// ```
    pub fn parse_labeled_statement(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        let mut next = self.next(token);
        debug_assert!(self.is_identifier(next));
        debug_assert!(self.is_a(self.next(next), TokenType::COLON));
        let mut label_count: i32 = 0;
        loop {
            token = self.parse_label(token);
            next = self.next(token);
            label_count += 1;
            if !(self.is_identifier(next) && self.is_a(self.next(next), TokenType::COLON)) {
                break;
            }
        }
        self.listener.begin_labeled_statement(next, label_count);
        token = self.parse_statement(token);
        self.listener.end_labeled_statement(label_count);
        token
    }

    /// Dart (line 6904): `Token parseExpressionStatement(Token token)`
    ///
    /// ```
    /// expressionStatement:
    ///   expression? ';'
    /// ;
    /// ```
    ///
    /// Note: This method can fail to make progress. If there is neither an
    /// expression nor a semi-colon, then a synthetic identifier and synthetic
    /// semicolon will be inserted before [token] and the semicolon will be
    /// returned.
    pub fn parse_expression_statement(&mut self, token: TokenId) -> TokenId {
        // TODO(brianwilkerson): If the next token is not the start of a valid
        // expression, then this method shouldn't report that we have an expression
        // statement.
        let begin_token = self.next(token);
        let mut token = self.parse_expression(token);
        token = self.ensure_semicolon(token);
        self.listener
            .handle_expression_statement(begin_token, token);
        token
    }

    /// Dart (line 6917): `Token parseExpression(Token token)`
    pub fn parse_expression(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        let depth = self.expression_depth;
        self.expression_depth += 1;
        if depth > 500 {
            // This happens in degenerate programs, for example, with a lot of nested
            // list literals. This is provoked by, for example, the language test
            // deep_nesting1_negative_test.
            let mut next = self.next(token);
            self.report_recoverable_error(next, diag::stack_overflow());

            // Recovery
            let end_group = self.end_group(next);
            if let Some(end_group) = end_group {
                while !self.is_eof(next) && next != end_group {
                    token = next;
                    next = self.next(token);
                }
            } else {
                while !(self.is_a(next, TokenType::CLOSE_PAREN)
                    || self.is_a(next, TokenType::CLOSE_SQUARE_BRACKET)
                    || self.is_a(next, TokenType::CLOSE_CURLY_BRACKET)
                    || self.is_a(next, TokenType::SEMICOLON))
                {
                    token = next;
                    next = self.next(token);
                }
            }
            if !self.is_eof(token) {
                token = self.rewriter().insert_synthetic_identifier(token, "");
                self.listener
                    .handle_identifier(token, IdentifierContext::Expression);
            }
        } else {
            token = self.parse_expression_impl(token, /* allowCascades = */ true);
        }
        self.expression_depth -= 1;
        token
    }

    /// Dart (line 6952): `Token parseExpressionWithoutCascade(Token token)`
    pub fn parse_expression_without_cascade(&mut self, token: TokenId) -> TokenId {
        self.parse_expression_impl(token, /* allowCascades = */ false)
    }

    /// Dart (line 6957): `Token _parseExpression(Token token, bool allowCascades)`
    pub fn parse_expression_impl(&mut self, token: TokenId, allow_cascades: bool) -> TokenId {
        if self.is_patterns_feature_enabled && self.looks_like_outer_pattern_equals(token) {
            return if allow_cascades {
                self.parse_pattern_assignment(token)
            } else {
                self.parse_pattern_assignment_impl(token, /* allowCascades = */ false)
            };
        }
        if self.is_a(self.next(token), Keyword::THROW) {
            self.parse_throw_expression(token, allow_cascades)
        } else {
            self.parse_precedence_expression(
                token,
                ASSIGNMENT_PRECEDENCE,
                allow_cascades,
                ConstantPatternContext::None,
            )
        }
    }

    /// Dart (line 6973): `bool canParseAsConditional(Token question)`
    pub fn can_parse_as_conditional(&mut self, question: TokenId) -> bool {
        // We want to check if we can parse, not send events and permanently change
        // the token stream. Set it up so we can do that.
        self.push_null_listener();
        let original_rewriter = self.begin_undoable_rewriter();

        let mut is_conditional = false;

        let after_expression1 = self.parse_expression_without_cascade(question);
        if !self.listener.null_listener_has_errors()
            && self.is_a(self.next(after_expression1), TokenType::COLON)
        {
            let colon = self.next(after_expression1);
            self.parse_expression_without_cascade(colon);
            if !self.listener.null_listener_has_errors() {
                // Now we know it's a conditional expression.
                is_conditional = true;
            }
        }

        // Undo all changes and reset.
        self.end_undoable_rewriter(original_rewriter);
        self.pop_null_listener();

        is_conditional
    }

    /// Dart (line 7003): `Token parseConditionalExpressionRest(Token token)`
    pub fn parse_conditional_expression_rest(&mut self, token: TokenId) -> TokenId {
        let mut token = self.next(token);
        let question = token;
        debug_assert!(self.is_a(question, TokenType::QUESTION));
        self.listener.begin_conditional_expression(token);
        token = self.parse_expression_without_cascade(token);
        let colon = self.ensure_colon(token);
        self.listener.handle_conditional_expression_colon();
        token = self.parse_expression_without_cascade(colon);
        self.listener
            .end_conditional_expression(question, colon, token);
        token
    }

    /// Dart (line 7021): `bool _isDotShorthand(Token token)`
    ///
    /// Returns `true` if [token] is a `.` and the next token after is an
    /// identifier or the `new` keyword, or if [token] is `const` followed by a
    /// `.` and an identifier or the `new` keyword after.
    ///
    /// This indicates the parsing of a dot shorthand e.g. `.parse(42)` or
    /// `const .parse(42)`.
    pub fn is_dot_shorthand(&mut self, token: TokenId) -> bool {
        let next = self.next(token);
        if self.is_a(token, TokenType::PERIOD)
            && (self.is_identifier(next) || self.is_a(next, Keyword::NEW))
        {
            return true;
        }
        if self.is_a(token, Keyword::CONST) && self.is_a(next, TokenType::PERIOD) {
            let period = next;
            let after_period = self.next(period);
            if self.is_identifier(after_period) || self.is_a(after_period, Keyword::NEW) {
                return true;
            }
        }
        false
    }

    /// Dart (line 7035): `Token parsePrecedenceExpression( Token token, int precedence, bool allowCascades, ConstantPatternContext constantPatternContext, )`
    pub fn parse_precedence_expression(
        &mut self,
        token: TokenId,
        precedence: i32,
        allow_cascades: bool,
        constant_pattern_context: ConstantPatternContext,
    ) -> TokenId {
        debug_assert!(precedence >= 1);
        debug_assert!(precedence <= SELECTOR_PRECEDENCE);

        let mut token = token;
        let next_token = self.next(token);
        let is_dot_shorthand = self.is_dot_shorthand(next_token);
        if !is_dot_shorthand {
            if self.is_a(next_token, Keyword::CONST)
                && self.is_a(self.next(next_token), TokenType::PERIOD)
            {
                // Recovery.
                // This is an incomplete dot shorthand like `C c = const .`.
                // This allows for better code completion, assuming the user wanted to
                // write a dot shorthand.
                let const_keyword = next_token;
                let dot = self.next(next_token);
                self.listener.begin_const_dot_shorthand(const_keyword);
                token = self.ensure_identifier(dot, IdentifierContext::ExpressionContinuation);
                self.listener.handle_dot_shorthand_head(dot);
                self.listener.end_const_dot_shorthand(const_keyword);
                self.listener.handle_dot_shorthand_context(dot);
                return token;
            }
            if self.is_a(next_token, TokenType::PERIOD) {
                // Recovery.
                // This is an incomplete dot shorthand like `var x = .`.
                // This allows for better code completion, assuming the user wanted to
                // write a dot shorthand.
                token =
                    self.ensure_identifier(next_token, IdentifierContext::ExpressionContinuation);
                self.listener.handle_dot_shorthand_head(next_token);
                self.listener.handle_dot_shorthand_context(next_token);
            } else {
                token =
                    self.parse_unary_expression(token, allow_cascades, constant_pattern_context);
            }
        }

        let mut bang_token = token;
        if self.is_a(self.next(token), TokenType::BANG) {
            bang_token = self.next(token);
        }
        let mut type_arg = compute_method_type_arguments_mut(self.tokens_mut(), bang_token);
        if type_arg != NO_TYPE_PARAM_OR_ARG {
            if self.is_a(bang_token, TokenType::BANG) {
                // For example `e!<int>()`, where [token] is before '<'.
                self.listener.handle_non_null_assert_expression(bang_token);
            }
            token = type_arg.parse_arguments(bang_token, self);
            if !self.is_a(self.next(token), TokenType::OPEN_PAREN) {
                // For example `e<a, b>;`, where [token] is before ';' or
                // `C<int>.new`, where [token] is before '.'.
                if constant_pattern_context != ConstantPatternContext::None {
                    let after_bang = self.next(bang_token);
                    self.report_recoverable_error(
                        after_bang,
                        diag::invalid_constant_pattern_generic(),
                    );
                }
                let after_bang = self.next(bang_token);
                self.listener.handle_type_argument_application(after_bang);
                type_arg = NO_TYPE_PARAM_OR_ARG;
            }
        }

        token = self.parse_dot_shorthand(
            token,
            next_token,
            is_dot_shorthand,
            allow_cascades,
            type_arg,
            constant_pattern_context,
        );

        self.parse_precedence_expression_loop(
            precedence,
            allow_cascades,
            type_arg,
            token,
            constant_pattern_context,
            false,
        )
    }

    /// Dart (line 7125): `Token _parseDotShorthand( Token token, Token nextToken, bool isDotShorthand, bool allowCascades, TypeParamOrArgInfo typeArg, ConstantPatternContext constantPatternContext, )`
    pub fn parse_dot_shorthand(
        &mut self,
        token: TokenId,
        next_token: TokenId,
        is_dot_shorthand: bool,
        allow_cascades: bool,
        type_arg: TypeParamOrArgInfo,
        constant_pattern_context: ConstantPatternContext,
    ) -> TokenId {
        if !is_dot_shorthand {
            return token;
        }

        let mut token = token;
        if self.is_a(next_token, Keyword::CONST) {
            let dot = self.next(next_token);
            self.listener.begin_const_dot_shorthand(next_token);
            token = self.parse_primary(
                dot,
                IdentifierContext::ExpressionContinuation,
                constant_pattern_context,
            );

            self.listener.handle_dot_shorthand_head(dot);
            self.listener.end_const_dot_shorthand(next_token);

            token = self.parse_precedence_expression_loop(
                SELECTOR_PRECEDENCE,
                allow_cascades,
                type_arg,
                token,
                constant_pattern_context,
                false,
            );

            self.listener.handle_dot_shorthand_context(dot);
        } else {
            let dot = self.next(token);
            token = self.parse_precedence_expression_loop(
                SELECTOR_PRECEDENCE,
                allow_cascades,
                type_arg,
                token,
                constant_pattern_context,
                /* isDotShorthand: */ true,
            );

            // With SELECTOR_PRECEDENCE, the `!` operator isn't parsed before
            // handling the dot shorthand context. We want to capture and handle the
            // null-assert before caching the context type.
            let next = self.next(token);
            if self.is_a(next, TokenType::BANG) {
                self.listener.handle_non_null_assert_expression(next);
                token = next;
            }

            // The entire shorthand is parsed at this point.
            self.listener.handle_dot_shorthand_context(dot);
        }
        token
    }

    /// Dart (line 7182): `Token _parsePrecedenceExpressionLoop( int precedence, bool allowCascades, TypeParamOrArgInfo typeArg, Token token, ConstantPatternContext constantPatternContext,`
    /// `{bool isDotShorthand = false})`
    pub fn parse_precedence_expression_loop(
        &mut self,
        precedence: i32,
        allow_cascades: bool,
        type_arg: TypeParamOrArgInfo,
        token: TokenId,
        constant_pattern_context: ConstantPatternContext,
        is_dot_shorthand: bool,
    ) -> TokenId {
        let mut type_arg = type_arg;
        let mut token = token;
        let mut constant_pattern_context = constant_pattern_context;
        let mut is_dot_shorthand = is_dot_shorthand;
        let mut next = self.next(token);
        let mut type_ = self.ty(next);
        let mut token_level = self.compute_precedence(next, /* forPattern: */ false);

        if constant_pattern_context != ConstantPatternContext::None {
            // For error recovery we allow too much when parsing constant patterns,
            // so for the cases that shouldn't be parsed as expressions in this
            // context we return directly.
            if type_ == TokenType::BANG {
                if token_level == POSTFIX_PRECEDENCE {
                    // This is a suffixed ! which is a null assert pattern.
                    return token;
                } else if self.is_a(self.next(next), TokenType::QUESTION) {
                    // This is a suffixed !? which is a null assert pattern in a null
                    // check pattern.
                    return token;
                }
            } else if type_ == TokenType::AS {
                // This is a suffixed `as` which is a case pattern.
                return token;
            }
        }
        constant_pattern_context = self.check_for_invalid_constant_pattern_operator(
            constant_pattern_context,
            precedence,
            token,
            next,
            type_,
            token_level,
        );
        if token_level < precedence {
            if self.recover_at_precedence_level && !self.currently_recovering {
                // Attempt recovery
                if self.attempt_precedence_level_recovery(
                    token,
                    precedence,
                    /* currentLevel = */ -1,
                    allow_cascades,
                    type_arg,
                ) {
                    return self.parse_precedence_expression_loop(
                        precedence,
                        allow_cascades,
                        type_arg,
                        token,
                        ConstantPatternContext::None,
                        false,
                    );
                }
            }
            return token;
        }
        let mut level = token_level;
        let mut last_binary_expression_level: i32 = -1;
        let mut last_cascade: Option<TokenId> = None;
        loop {
            let mut operator = next;
            if token_level == CASCADE_PRECEDENCE {
                if !allow_cascades {
                    return token;
                } else if last_cascade.is_some()
                    && self.is_a(next, TokenType::QUESTION_PERIOD_PERIOD)
                {
                    self.report_recoverable_error(next, diag::null_aware_cascade_out_of_order());
                }
                last_cascade = Some(next);
                token = self.parse_cascade_expression(token);
            } else if token_level == ASSIGNMENT_PRECEDENCE {
                // Right associative, so we recurse at the same precedence
                // level.
                let mut next = self.next(token);
                if self.is_a(self.next(next), TokenType::GT_EQ) {
                    // Special case use of triple-shift in cases where it isn't
                    // enabled.
                    let after_next = self.next(next);
                    self.report_experiment_not_enabled(
                        ExperimentalFlag::TripleShift,
                        next,
                        after_next,
                    );
                    debug_assert!(next == operator);
                    next = self.rewriter().replace_next_tokens_with_synthetic_token(
                        token,
                        /* count = */ 2,
                        TokenType::GT_GT_GT_EQ,
                    );
                    operator = next;
                }
                token = self.parse_expression_impl(next, allow_cascades);
                self.listener.handle_assignment_expression(operator, token);
            } else if token_level == POSTFIX_PRECEDENCE {
                if type_ == TokenType::PLUS_PLUS || type_ == TokenType::MINUS_MINUS {
                    let operator_token = self.next(token);
                    self.listener
                        .handle_unary_postfix_assignment_expression(operator_token);
                    token = next;
                } else if type_ == TokenType::BANG {
                    self.listener.handle_non_null_assert_expression(next);
                    token = next;
                }
            } else if token_level == SELECTOR_PRECEDENCE {
                if type_ == TokenType::PERIOD || type_ == TokenType::QUESTION_PERIOD {
                    let dot = self.next(token);
                    let after_dot = self.next(dot);
                    // TODO(eernst): Call reportExperimentNotEnabled to guide user when
                    // `_beginsAnonymousMethod(afterDot)`, but experiment disabled.
                    if self.is_anonymous_methods_feature_enabled
                        && self.begins_anonymous_method(after_dot)
                    {
                        token = self.parse_anonymous_method(dot, after_dot);
                    } else {
                        // Left associative, so we recurse at the next higher precedence
                        // level. However, SELECTOR_PRECEDENCE is the highest level, so we
                        // should just call [parseUnaryExpression] directly. However, a
                        // unary expression isn't legal after a period, so we call
                        // [parsePrimary] instead.
                        token = self.parse_primary(
                            dot,
                            IdentifierContext::ExpressionContinuation,
                            constant_pattern_context,
                        );
                        if is_dot_shorthand {
                            self.listener.handle_dot_shorthand_head(dot);
                            is_dot_shorthand = false;
                        } else {
                            self.listener.handle_dot_access(
                                operator,
                                token,
                                /* isNullAware = */ type_ == TokenType::QUESTION_PERIOD,
                            );
                        }
                        let mut bang_token = token;
                        if self.is_a(self.next(token), TokenType::BANG) {
                            bang_token = self.next(token);
                        }
                        type_arg = compute_method_type_arguments_mut(self.tokens_mut(), bang_token);
                        if type_arg != NO_TYPE_PARAM_OR_ARG {
                            // For example e.f<T>(c), where token is before '<'.
                            if self.is_a(bang_token, TokenType::BANG) {
                                self.listener.handle_non_null_assert_expression(bang_token);
                            }
                            token = type_arg.parse_arguments(bang_token, self);
                            if !self.is_a(self.next(token), TokenType::OPEN_PAREN) {
                                if constant_pattern_context != ConstantPatternContext::None {
                                    let after_bang = self.next(bang_token);
                                    self.report_recoverable_error(
                                        after_bang,
                                        diag::invalid_constant_pattern_generic(),
                                    );
                                }
                                let after_bang = self.next(bang_token);
                                self.listener.handle_type_argument_application(after_bang);
                                type_arg = NO_TYPE_PARAM_OR_ARG;
                            }
                        }
                    }
                } else if type_ == TokenType::OPEN_PAREN || type_ == TokenType::OPEN_SQUARE_BRACKET
                {
                    token = self.parse_argument_or_index_star(
                        token, type_arg, /* checkedNullAware = */ false,
                    );
                } else if type_ == TokenType::QUESTION {
                    // We have determined selector precedence so this is a null-aware
                    // bracket operator.
                    token = self.parse_argument_or_index_star(
                        token, type_arg, /* checkedNullAware = */ true,
                    );
                } else if type_ == TokenType::INDEX {
                    self.rewrite_square_brackets(token);
                    token = self.parse_argument_or_index_star(
                        token,
                        NO_TYPE_PARAM_OR_ARG,
                        /* checkedNullAware = */ false,
                    );
                } else if type_ == TokenType::BANG {
                    let bang = self.next(token);
                    self.listener.handle_non_null_assert_expression(bang);
                    token = next;
                } else {
                    // Recovery
                    let unexpected = self.next(token);
                    self.report_recoverable_error_with_token(unexpected, diag::unexpected_token);
                    token = next;
                }
            } else if type_ == TokenType::IS {
                token = self.parse_is_operator_rest(token);
            } else if type_ == TokenType::AS {
                token = self.parse_as_operator_rest(token);
            } else if type_ == TokenType::QUESTION {
                token = self.parse_conditional_expression_rest(token);
            } else {
                if level == EQUALITY_PRECEDENCE || level == RELATIONAL_PRECEDENCE {
                    // We don't allow (a == b == c) or (a < b < c).
                    if last_binary_expression_level == level {
                        // Report an error, then continue parsing as if it is legal.
                        self.report_recoverable_error(
                            next,
                            diag::equality_cannot_be_equality_operand(),
                        );
                    } else {
                        // Set a flag to catch subsequent binary expressions of this type.
                        last_binary_expression_level = level;
                    }
                }
                if self.is_a(next, TokenType::GT_GT)
                    && self.end(next) == self.char_offset(self.next(next))
                {
                    if self.is_a(self.next(next), TokenType::GT) {
                        // Special case use of triple-shift in cases where it isn't
                        // enabled.
                        let after_next = self.next(next);
                        self.report_experiment_not_enabled(
                            ExperimentalFlag::TripleShift,
                            next,
                            after_next,
                        );
                        debug_assert!(next == operator);
                        next = self.rewriter().replace_next_tokens_with_synthetic_token(
                            token,
                            /* count = */ 2,
                            TokenType::GT_GT_GT,
                        );
                        operator = next;
                    }
                }
                self.listener.begin_binary_expression(next);
                // Left associative, so we recurse at the next higher
                // precedence level.
                let after_token = self.next(token);
                token = self.parse_precedence_expression(
                    after_token,
                    level + 1,
                    allow_cascades,
                    ConstantPatternContext::None,
                );
                self.listener.end_binary_expression(operator, token);
            }
            next = self.next(token);
            type_ = self.ty(next);
            token_level = self.compute_precedence(next, /* forPattern: */ false);
            if constant_pattern_context != ConstantPatternContext::None {
                // For error recovery we allow too much when parsing constant
                // patterns, so for the cases that shouldn't be parsed as expressions
                // in this context we break out of the parsing loop directly.
                if type_ == TokenType::BANG {
                    if token_level == POSTFIX_PRECEDENCE {
                        // This is a suffixed ! which is a null assert pattern.
                        return token;
                    } else if self.is_a(self.next(next), TokenType::QUESTION) {
                        // This is a suffixed !? which is a null assert pattern in a null
                        // check pattern.
                        return token;
                    }
                } else if type_ == TokenType::AS {
                    // This is a suffixed `as` which is a case pattern.
                    return token;
                }
            }

            constant_pattern_context = self.check_for_invalid_constant_pattern_operator(
                constant_pattern_context,
                precedence,
                token,
                next,
                type_,
                token_level,
            );

            if self.recover_at_precedence_level && !self.currently_recovering {
                // Attempt recovery
                if self.attempt_precedence_level_recovery(
                    token,
                    precedence,
                    level,
                    allow_cascades,
                    type_arg,
                ) {
                    // Recovered - try again at same level with the replacement token.
                    next = self.next(token);
                    type_ = self.ty(next);
                    token_level = self.compute_precedence(next, /* forPattern: */ false);
                }
            }

            if token_level <= level {
                if token_level < precedence {
                    break;
                }
                level = token_level;
            } else {
                break;
            }
        }

        token
    }

    /// Dart (line 7478): `ConstantPatternContext _checkForInvalidConstantPatternOperator( ConstantPatternContext constantPatternContext, int precedence, Token token, Token next, TokenType type, int tokenLevel, )`
    pub fn check_for_invalid_constant_pattern_operator(
        &mut self,
        constant_pattern_context: ConstantPatternContext,
        precedence: i32,
        token: TokenId,
        next: TokenId,
        type_: TokenType,
        token_level: i32,
    ) -> ConstantPatternContext {
        if constant_pattern_context != ConstantPatternContext::None
            && precedence <= token_level
            && token_level < SELECTOR_PRECEDENCE
        {
            // If we are parsing a constant pattern, only [SELECTOR_PRECEDENCE] is
            // supported but we allow for parsing [EQUALITY_PRECEDENCE] and higher
            // for better error recovery.
            if constant_pattern_context == ConstantPatternContext::Explicit {
                self.report_recoverable_error(token, diag::invalid_constant_pattern_const_prefix());
            } else if token_level <= MULTIPLICATIVE_PRECEDENCE {
                self.report_recoverable_error(
                    next,
                    diag::invalid_constant_pattern_binary(type_.lexeme()),
                );
            } else {
                // These are prefix or postfix ++/-- and will not be constant
                // expressions, anyway.
                debug_assert!(
                    token_level == POSTFIX_PRECEDENCE || token_level == PREFIX_PRECEDENCE,
                    "Unexpected precedence level for {type_:?}: {token_level}"
                );
            }
            // Avoid additional constant pattern errors.
            return ConstantPatternContext::None;
        }
        constant_pattern_context
    }

    /// Dart (line 7524): `bool _beginsAnonymousMethod(Token token)`
    ///
    /// Can the next input be an anonymous method?
    ///
    /// Used during `_parsePrecedenceExpressionLoop` and
    /// `parseCascadeExpression`.
    ///
    /// Should only be invoked in a situation where the input before
    /// [token] is a period or two periods, or a question mark followed by
    /// one or two periods. Returns true if [token] shows that the next
    /// construct to parse can only be an anonymous method.
    pub fn begins_anonymous_method(&mut self, token: TokenId) -> bool {
        if self.is_a(token, TokenType::OPEN_CURLY_BRACKET) || self.is_a(token, TokenType::FUNCTION)
        {
            return true;
        }
        if self.is_a(token, TokenType::OPEN_PAREN) {
            let matching_parenthesis = self.end_group(token);
            if let Some(matching_parenthesis) = matching_parenthesis {
                // Dart `matchingParenthesis.next` (nullable): the arena always
                // links a next token for a token that is in the stream.
                let after_match = self.next(matching_parenthesis);
                if self.is_a(after_match, TokenType::OPEN_CURLY_BRACKET)
                    || self.is_a(after_match, TokenType::FUNCTION)
                {
                    return true;
                }
            }
        }
        false
    }

    /// Dart (line 7550): `Token _parseAnonymousMethod(Token punctuation, Token afterPunctuation)`
    ///
    /// Parse an anonymous method.
    ///
    /// Used during `_parsePrecedenceExpressionLoop` and
    /// `parseCascadeExpression`.
    ///
    /// Should only be invoked in a situation where
    /// `_beginsAnonymousMethod(afterPunctuation)` has returned true.
    pub fn parse_anonymous_method(
        &mut self,
        punctuation: TokenId,
        after_punctuation: TokenId,
    ) -> TokenId {
        let mut current_token: TokenId;
        self.listener.begin_anonymous_method_invocation(punctuation);
        if self.is_a(after_punctuation, TokenType::OPEN_PAREN) {
            current_token = self.parse_formal_parameters(punctuation, MemberKind::AnonymousMethod);
        } else {
            self.listener.handle_implicit_formal_parameters(punctuation);
            current_token = punctuation;
        }
        let after_parameters = self.next(current_token);
        let mut function_definition: Option<TokenId> = None;
        let is_expression: bool;
        if self.is_a(after_parameters, TokenType::OPEN_CURLY_BRACKET) {
            is_expression = false;
            current_token = self.parse_block(current_token, BlockKind::FunctionBody);
        } else if self.is_a(after_parameters, TokenType::FUNCTION) {
            is_expression = true;
            function_definition = Some(after_parameters);
            current_token = self.parse_expression_without_cascade(after_parameters);
        } else {
            self.report_recoverable_error(after_parameters, diag::expected_but_got2("{", "=>"));
            let synthetic = self
                .rewriter()
                .insert_synthetic_token(current_token, TokenType::FUNCTION);
            function_definition = Some(synthetic);
            is_expression = true;
            current_token = self.parse_expression_without_cascade(synthetic);
        }
        self.listener.end_anonymous_method_invocation(
            punctuation,
            function_definition,
            current_token,
            is_expression,
        );
        current_token
    }

    /// Dart (line 7594): `bool _attemptPrecedenceLevelRecovery( Token token, int precedence, int currentLevel, bool allowCascades, TypeParamOrArgInfo typeArg, )`
    ///
    /// Attempt a recovery where [token].next is replaced.
    pub fn attempt_precedence_level_recovery(
        &mut self,
        token: TokenId,
        precedence: i32,
        current_level: i32,
        allow_cascades: bool,
        type_arg: TypeParamOrArgInfo,
    ) -> bool {
        // Attempt recovery.
        self.recover_at_precedence_level = false;
        let replacements = match token_recovery_replacements(self.lexeme(self.next(token))) {
            Some(replacements) => replacements,
            None => {
                // This shouldn't happen. But if it does we don't want to crash.
                // assert(false, "Faulty logic for _recoverAtPrecedenceLevel");
                return false;
            }
        };
        for i in 0..replacements.len() {
            let replacement = replacements[i];

            if current_level >= 0 {
                // Check that the new precedence and currentLevel would have accepted
                // this replacement here.
                let new_level = replacement.precedence() as i32;
                // The loop it would normally have gone through is something like
                // for (; ; --level) {
                //   while (identical(tokenLevel, level)) {
                //   }
                // }
                // So if the new tokens level <= the "old" (current) level, [level] (in
                // the above code snippet) would get down to it and accept it.
                // But if the new tokens level > the "old" (current) level, normally we
                // would never get to it - so we shouldn't here either.
                // As the loop starts by taking the first tokens tokenLevel as level,
                // recursing below won't weed that out so we need to do it here.
                if new_level > current_level {
                    continue;
                }
            }

            self.currently_recovering = true;
            self.push_null_listener();
            let original_rewriter = self.begin_undoable_rewriter();
            self.rewriter()
                .replace_next_token_with_synthetic_token(token, replacement);
            let mut accept_recovery = false;
            let after_expression = self.parse_precedence_expression_loop(
                precedence,
                allow_cascades,
                type_arg,
                token,
                ConstantPatternContext::None,
                false,
            );
            let after_expression_next = self.next(after_expression);

            if !self.listener.null_listener_has_errors()
                && token != after_expression
                && (self.is_a(after_expression_next, TokenType::SEMICOLON)
                    || self.is_a(after_expression_next, TokenType::COMMA)
                    || self.is_a(after_expression_next, TokenType::CLOSE_PAREN)
                    || self.is_a(after_expression_next, TokenType::OPEN_CURLY_BRACKET)
                    || self.is_a(after_expression_next, TokenType::CLOSE_CURLY_BRACKET)
                    || self.is_a(after_expression_next, TokenType::BAR)
                    || self.is_a(after_expression_next, TokenType::BAR_BAR)
                    || self.is_a(after_expression_next, TokenType::AMPERSAND)
                    || self.is_a(after_expression_next, TokenType::AMPERSAND_AMPERSAND)
                    || self.is_a(after_expression_next, TokenType::EOF)
                    || (self.is_a(after_expression_next, TokenType::IDENTIFIER)
                        && token_recovery_replacements(self.lexeme(after_expression_next))
                            .is_some()))
            {
                // Seems good!
                accept_recovery = true;
            }

            // Undo all changes and reset.
            self.currently_recovering = false;
            self.recover_at_precedence_level = false;
            self.end_undoable_rewriter(original_rewriter);
            self.pop_null_listener();

            if accept_recovery {
                // Report and redo recovery.
                let next = self.next(token);
                let message =
                    diag::binary_operator_written_out(self.lexeme(next), replacement.lexeme());
                self.report_recoverable_error(next, message);
                self.rewriter()
                    .replace_next_token_with_synthetic_token(token, replacement);
                return true;
            }
        }

        false
    }

    /// Dart (line 7707): `int _computePrecedence(Token token, {required bool forPattern})`
    ///
    /// Computes the precedence of [token].  [forPattern] indicates whether a
    /// pattern is being parsed (this changes the precedence of a few operators).
    pub fn compute_precedence(&mut self, token: TokenId, for_pattern: bool) -> i32 {
        let type_ = self.ty(token);
        if type_ == TokenType::BANG {
            // The '!' has prefix precedence but here it's being used as a
            // postfix operator to assert the expression has a non-null value.
            let next_type = self.ty(self.next(token));
            if next_type == TokenType::PERIOD
                || next_type == TokenType::QUESTION
                || next_type == TokenType::OPEN_PAREN
                || next_type == TokenType::OPEN_SQUARE_BRACKET
                || next_type == TokenType::QUESTION_PERIOD
            {
                return SELECTOR_PRECEDENCE;
            }
            return POSTFIX_PRECEDENCE;
        } else if type_ == TokenType::GT_GT {
            // ">>" followed by ">=" (without space between tokens) should for
            // recovery be seen as ">>>=".
            let next = self.next(token);
            let next_type = self.ty(next);
            if next_type == TokenType::GT_EQ && self.end(token) == self.offset(next) {
                return TokenType::GT_GT_GT_EQ.precedence() as i32;
            }
        } else if type_ == TokenType::QUESTION {
            if for_pattern {
                // The '?' has conditional precedence but here it's being used as a
                // postfix operator as part of a pattern, so it should have selector
                // precedence.
                return SELECTOR_PRECEDENCE;
            } else if self.is_a(self.next(token), TokenType::OPEN_SQUARE_BRACKET) {
                // "?[" can be a null-aware bracket or a conditional. If it's a
                // null-aware bracket it has selector precedence.
                let is_conditional = self.can_parse_as_conditional(token);
                if !is_conditional {
                    return SELECTOR_PRECEDENCE;
                }
            }
        } else if type_ == TokenType::IDENTIFIER {
            // An identifier at this point is not right. So some recovery is going to
            // happen soon. The question is, if we can do a better recovery here.
            if !for_pattern
                && !self.currently_recovering
                && token_recovery_replacements(self.lexeme(token)).is_some()
            {
                self.recover_at_precedence_level = true;
            }
        }

        type_.precedence() as i32
    }

    /// Dart (line 7756): `Token parseCascadeExpression(Token token)`
    pub fn parse_cascade_expression(&mut self, token: TokenId) -> TokenId {
        let mut token = self.next(token);
        let cascade_operator = token;
        debug_assert!(
            self.is_a(cascade_operator, TokenType::PERIOD_PERIOD)
                || self.is_a(cascade_operator, TokenType::QUESTION_PERIOD_PERIOD)
        );
        self.listener.begin_cascade(cascade_operator);
        let after_dots = self.next(token);
        if self.is_a(after_dots, TokenType::OPEN_SQUARE_BRACKET) {
            token = self.parse_argument_or_index_star(
                token,
                NO_TYPE_PARAM_OR_ARG,
                /* checkedNullAware = */ false,
            );
        } else if self.is_anonymous_methods_feature_enabled
            && self.begins_anonymous_method(after_dots)
        {
            token = self.parse_anonymous_method(cascade_operator, after_dots);
        } else {
            // TODO(eernst): Call `reportExperimentNotEnabled` to guide user when
            // `_beginsAnonymousMethod(afterDots)`.
            token = self.parse_send(
                token,
                IdentifierContext::ExpressionContinuation,
                ConstantPatternContext::None,
            );
            let is_null_aware = self.is_a(cascade_operator, TokenType::QUESTION_PERIOD_PERIOD);
            self.listener.handle_cascade_access(
                cascade_operator,
                token,
                /* isNullAware = */ is_null_aware,
            );
        }
        let mut next = self.next(token);
        let mut mark: TokenId;
        loop {
            mark = token;
            if self.is_a(next, TokenType::PERIOD) || self.is_a(next, TokenType::QUESTION_PERIOD) {
                let is_null_aware = self.is_a(next, TokenType::QUESTION_PERIOD);
                let period = next;
                let after_period = self.next(period);
                // TODO(eernst): Call `reportExperimentNotEnabled` to guide user when
                // there is a match except for the experiment being disabled.
                if self.is_anonymous_methods_feature_enabled
                    && self.begins_anonymous_method(after_period)
                {
                    token = self.parse_anonymous_method(period, after_period);
                } else {
                    token = self.parse_send(
                        next,
                        IdentifierContext::ExpressionContinuation,
                        ConstantPatternContext::None,
                    );
                    next = self.next(token);
                    self.listener
                        .handle_dot_access(period, token, is_null_aware);
                }
            } else if self.is_a(next, TokenType::BANG) {
                self.listener.handle_non_null_assert_expression(next);
                token = next;
                next = self.next(token);
            }
            let mut type_arg = compute_method_type_arguments_mut(self.tokens_mut(), token);
            if type_arg != NO_TYPE_PARAM_OR_ARG {
                // For example a(b)..<T>(c), where token is '<'.
                token = type_arg.parse_arguments(token, self);
                next = self.next(token);
                if !self.is_a(next, TokenType::OPEN_PAREN) {
                    let after = self.next(token);
                    self.listener.handle_type_argument_application(after);
                    type_arg = NO_TYPE_PARAM_OR_ARG;
                }
            }
            let next_type = self.ty(next);
            if next_type == TokenType::INDEX {
                // If we don't split the '[]' here we will stop parsing it as a cascade
                // and either split it later (parsing it wrong) or inserting ; before it
                // (also wrong).
                // See also https://github.com/dart-lang/sdk/issues/42267.
                self.rewrite_square_brackets(token);
            }
            token = self
                .parse_argument_or_index_star(token, type_arg, /* checkedNullAware = */ false);
            next = self.next(token);
            if mark == token {
                break;
            }
        }

        if self.ty(next).precedence() as i32 == ASSIGNMENT_PRECEDENCE {
            let assignment = next;
            token = self.parse_expression_without_cascade(next);
            self.listener
                .handle_assignment_expression(assignment, token);
        }
        self.listener.end_cascade();
        token
    }

    /// Dart (line 7851): `Token parseUnaryExpression( Token token, bool allowCascades, ConstantPatternContext constantPatternContext, )`
    pub fn parse_unary_expression(
        &mut self,
        token: TokenId,
        allow_cascades: bool,
        constant_pattern_context: ConstantPatternContext,
    ) -> TokenId {
        let mut token = token;
        let mut constant_pattern_context = constant_pattern_context;
        let value: Option<&'static str> = self.string_value(self.next(token));
        // Prefix:
        if value == Some("await") {
            if self.in_plain_sync() {
                if !self.looks_like_await_expression(token, AwaitOrYieldContext::UnaryExpression) {
                    return self.parse_primary(
                        token,
                        IdentifierContext::Expression,
                        ConstantPatternContext::None,
                    );
                }
                // Recovery: Looks like an expression preceded by `await`.
                // Fall through and let parseAwaitExpression report the error.
            }
            return self.parse_await_expression(token, allow_cascades);
        } else if value == Some("+") {
            // Dart no longer allows prefix-plus.
            let next = self.next(token);
            let offset = self.offset(next);
            let byte_offset = self.tokens().byte_offset(next);
            let synthetic = self.tokens_mut().push_synthetic_string(
                TokenType::IDENTIFIER,
                "",
                offset,
                byte_offset,
                None,
            );
            self.rewrite_and_recover(
                token,
                // TODO(danrubel): Consider reporting "missing identifier" instead.
                diag::unsupported_prefix_plus(),
                synthetic,
            );
            return self.parse_primary(
                token,
                IdentifierContext::Expression,
                constant_pattern_context,
            );
        } else if value == Some("!") || value == Some("~") {
            let operator = self.next(token);
            if constant_pattern_context != ConstantPatternContext::None {
                self.report_recoverable_error(
                    operator,
                    diag::invalid_constant_pattern_unary(value.unwrap()),
                );
            }
            // Right associative, so we recurse at the same precedence
            // level.
            token = self.parse_precedence_expression(
                operator,
                POSTFIX_PRECEDENCE,
                allow_cascades,
                ConstantPatternContext::None,
            );
            self.listener.handle_unary_prefix_expression(operator);
            return token;
        } else if value == Some("-") {
            let operator = self.next(token);
            if constant_pattern_context == ConstantPatternContext::Explicit {
                self.report_recoverable_error(
                    operator,
                    diag::invalid_constant_pattern_const_prefix(),
                );
                // Avoid subsequent errors.
                constant_pattern_context = ConstantPatternContext::None;
            }
            // Right associative, so we recurse at the same precedence
            // level.
            token = self.parse_precedence_expression(
                operator,
                POSTFIX_PRECEDENCE,
                allow_cascades,
                if constant_pattern_context != ConstantPatternContext::None {
                    ConstantPatternContext::NumericLiteralOnly
                } else {
                    ConstantPatternContext::None
                },
            );
            self.listener.handle_unary_prefix_expression(operator);
            return token;
        } else if value == Some("++") || value == Some("--") {
            // TODO(ahe): Validate this is used correctly.
            let operator = self.next(token);
            // Right associative, so we recurse at the same precedence
            // level.
            token = self.parse_precedence_expression(
                operator,
                POSTFIX_PRECEDENCE,
                allow_cascades,
                ConstantPatternContext::None,
            );
            self.listener
                .handle_unary_prefix_assignment_expression(operator);
            return token;
        } else if self.use_implicit_creation_expression && self.is_identifier(self.next(token)) {
            let mut identifier = self.next(token);
            if self.is_a(self.next(identifier), TokenType::PERIOD) {
                identifier = self.next(self.next(identifier));
            }
            if self.is_identifier(identifier) {
                // Looking at `identifier ('.' identifier)?`.
                if self.is_a(self.next(identifier), TokenType::LT) {
                    let type_arg =
                        compute_type_param_or_arg_mut(self.tokens_mut(), identifier, false, false);
                    if type_arg != NO_TYPE_PARAM_OR_ARG {
                        let end_type_arguments = type_arg.skip_mut(self.tokens_mut(), identifier);
                        let after_type_arguments = self.next(end_type_arguments);
                        if self.is_a(after_type_arguments, TokenType::PERIOD) {
                            let after_period = self.next(after_type_arguments);
                            if self.is_new_or_identifier(after_period)
                                && self.is_a(self.next(after_period), TokenType::OPEN_PAREN)
                            {
                                let open_angle_bracket = self.next(identifier);
                                return self.parse_implicit_creation_expression(
                                    token,
                                    open_angle_bracket,
                                    type_arg,
                                );
                            }
                        }
                    }
                }
            }
        }
        self.parse_primary(
            token,
            IdentifierContext::Expression,
            constant_pattern_context,
        )
    }

    /// Dart (line 7974): `Token parseArgumentOrIndexStar( Token token, TypeParamOrArgInfo typeArg, bool checkedNullAware, )`
    pub fn parse_argument_or_index_star(
        &mut self,
        token: TokenId,
        type_arg: TypeParamOrArgInfo,
        checked_null_aware: bool,
    ) -> TokenId {
        let mut token = token;
        let mut type_arg = type_arg;
        let mut next = self.next(token);
        let begin_token = next;
        loop {
            let mut potential_null_aware = self.is_a(next, TokenType::QUESTION)
                && self.is_a(self.next(next), TokenType::OPEN_SQUARE_BRACKET);
            if potential_null_aware && !checked_null_aware {
                // While it's a potential null aware index it hasn't been checked.
                // It might be a conditional expression.
                debug_assert!(self.is_a(next, TokenType::QUESTION));
                let is_conditional = self.can_parse_as_conditional(next);
                if is_conditional {
                    potential_null_aware = false;
                }
            }

            if self.is_a(next, TokenType::OPEN_SQUARE_BRACKET) || potential_null_aware {
                debug_assert!(type_arg == NO_TYPE_PARAM_OR_ARG);
                let mut open_square_bracket = next;
                let mut question: Option<TokenId> = None;
                if self.is_a(next, TokenType::QUESTION) {
                    question = Some(next);
                    next = self.next(next);
                    open_square_bracket = next;
                    debug_assert!(self.is_a(open_square_bracket, TokenType::OPEN_SQUARE_BRACKET));
                }
                let old = self.may_parse_function_expressions;
                self.may_parse_function_expressions = true;
                token = self.parse_expression(next);
                next = self.next(token);
                self.may_parse_function_expressions = old;
                if !self.is_a(next, TokenType::CLOSE_SQUARE_BRACKET) {
                    // Recovery
                    self.report_recoverable_error(next, diag::expected_but_got("]"));
                    // Scanner ensures a closing ']'
                    let end_group = self.end_group(open_square_bracket).unwrap();
                    if self.is_synthetic(end_group) {
                        // Scanner inserted closing ']' in the wrong place, so move it.
                        next = self.rewriter().move_synthetic(token, end_group);
                    } else {
                        // Skip over unexpected tokens to where the user placed the `]`.
                        next = end_group;
                    }
                }
                self.listener
                    .handle_indexed_expression(question, open_square_bracket, next);
                token = next;
                let mut bang_token = token;
                if self.is_a(self.next(token), TokenType::BANG) {
                    bang_token = self.next(token);
                }
                type_arg = compute_method_type_arguments_mut(self.tokens_mut(), bang_token);
                if type_arg != NO_TYPE_PARAM_OR_ARG {
                    // For example a[b]<T>(c), where token is before '<'.
                    if self.is_a(bang_token, TokenType::BANG) {
                        self.listener.handle_non_null_assert_expression(bang_token);
                    }
                    token = type_arg.parse_arguments(bang_token, self);
                    if !self.is_a(self.next(token), TokenType::OPEN_PAREN) {
                        let after_bang = self.next(bang_token);
                        self.listener.handle_type_argument_application(after_bang);
                        type_arg = NO_TYPE_PARAM_OR_ARG;
                    }
                }
                next = self.next(token);
            } else if self.is_a(next, TokenType::OPEN_PAREN) {
                if type_arg == NO_TYPE_PARAM_OR_ARG {
                    self.listener.handle_no_type_arguments(next);
                }
                token = self.parse_arguments(token);
                self.listener.handle_send(begin_token, token);
                let mut bang_token = token;
                if self.is_a(self.next(token), TokenType::BANG) {
                    bang_token = self.next(token);
                }
                type_arg = compute_method_type_arguments_mut(self.tokens_mut(), bang_token);
                if type_arg != NO_TYPE_PARAM_OR_ARG {
                    // For example a(b)<T>(c), where token is before '<'.
                    if self.is_a(bang_token, TokenType::BANG) {
                        self.listener.handle_non_null_assert_expression(bang_token);
                    }
                    token = type_arg.parse_arguments(bang_token, self);
                    if !self.is_a(self.next(token), TokenType::OPEN_PAREN) {
                        let after_bang = self.next(bang_token);
                        self.listener.handle_type_argument_application(after_bang);
                        type_arg = NO_TYPE_PARAM_OR_ARG;
                    }
                }
                next = self.next(token);
            } else {
                break;
            }
        }
        token
    }

    /// Dart (line 8073): `Token parsePrimary( Token token, IdentifierContext context, ConstantPatternContext constantPatternContext, )`
    pub fn parse_primary(
        &mut self,
        token: TokenId,
        context: IdentifierContext,
        constant_pattern_context: ConstantPatternContext,
    ) -> TokenId {
        let mut token = token;
        self.try_rewrite_new_to_identifier(token, context);
        let next = self.next(token);
        let kind = self.kind(next);
        if kind == IDENTIFIER_TOKEN {
            if constant_pattern_context == ConstantPatternContext::NumericLiteralOnly {
                self.report_recoverable_error(next, diag::invalid_constant_pattern_negation());
                // Avoid subsequent errors.
                // Dart: `constantPatternContext == ConstantPatternContext.none;`
                // (a comparison without effect, so the context is not changed).
            }
            return self.parse_send_or_function_literal(token, context, constant_pattern_context);
        } else if kind == INT_TOKEN || kind == HEXADECIMAL_TOKEN {
            if constant_pattern_context == ConstantPatternContext::Explicit {
                self.report_recoverable_error(next, diag::invalid_constant_pattern_const_prefix());
            }
            if self.ty(next) == TokenType::INT_WITH_SEPARATORS
                || self.ty(next) == TokenType::HEXADECIMAL_WITH_SEPARATORS
            {
                return self.parse_literal_int_with_separators(token);
            } else {
                return self.parse_literal_int(token);
            }
        } else if kind == DOUBLE_TOKEN {
            if constant_pattern_context == ConstantPatternContext::Explicit {
                self.report_recoverable_error(next, diag::invalid_constant_pattern_const_prefix());
            }
            if self.ty(next) == TokenType::DOUBLE_WITH_SEPARATORS {
                return self.parse_literal_double_with_separators(token);
            } else {
                return self.parse_literal_double(token);
            }
        } else if kind == STRING_TOKEN {
            if constant_pattern_context == ConstantPatternContext::Explicit {
                self.report_recoverable_error(next, diag::invalid_constant_pattern_const_prefix());
            } else if constant_pattern_context == ConstantPatternContext::NumericLiteralOnly {
                self.report_recoverable_error(next, diag::invalid_constant_pattern_negation());
            }
            return self.parse_literal_string(token);
        } else if kind == HASH_TOKEN {
            if constant_pattern_context == ConstantPatternContext::Explicit {
                self.report_recoverable_error(next, diag::invalid_constant_pattern_const_prefix());
            } else if constant_pattern_context == ConstantPatternContext::NumericLiteralOnly {
                self.report_recoverable_error(next, diag::invalid_constant_pattern_negation());
            }
            return self.parse_literal_symbol(token);
        } else if kind == KEYWORD_TOKEN {
            let value: Option<&'static str> = self.string_value(next);
            if value == Some("true") || value == Some("false") {
                if constant_pattern_context == ConstantPatternContext::Explicit {
                    self.report_recoverable_error(
                        next,
                        diag::invalid_constant_pattern_const_prefix(),
                    );
                } else if constant_pattern_context == ConstantPatternContext::NumericLiteralOnly {
                    self.report_recoverable_error(next, diag::invalid_constant_pattern_negation());
                }
                return self.parse_literal_bool(token);
            } else if value == Some("null") {
                if constant_pattern_context == ConstantPatternContext::Explicit {
                    self.report_recoverable_error(
                        next,
                        diag::invalid_constant_pattern_const_prefix(),
                    );
                } else if constant_pattern_context == ConstantPatternContext::NumericLiteralOnly {
                    self.report_recoverable_error(next, diag::invalid_constant_pattern_negation());
                }
                return self.parse_literal_null(token);
            } else if value == Some("this") {
                return self.parse_this_expression(token, context);
            } else if value == Some("super") {
                return self.parse_super_expression(token, context);
            } else if value == Some("new") {
                return self.parse_new_expression(token);
            } else if value == Some("const") {
                if constant_pattern_context == ConstantPatternContext::Explicit {
                    self.report_recoverable_error(
                        next,
                        diag::invalid_constant_pattern_duplicate_const(),
                    );
                }
                return self.parse_const_expression(token);
            } else if value == Some("void") {
                return self.parse_send_or_function_literal(
                    token,
                    context,
                    constant_pattern_context,
                );
            } else if !self.in_plain_sync() && (value == Some("yield") || value == Some("async")) {
                // Fall through to the recovery code.
            } else if value == Some("assert") {
                return self.parse_assert(token, Assert::Expression);
            } else if self.is_patterns_feature_enabled && value == Some("switch") {
                return self.parse_switch_expression(token);
            } else if self.is_identifier(next) {
                if constant_pattern_context == ConstantPatternContext::NumericLiteralOnly {
                    self.report_recoverable_error(next, diag::invalid_constant_pattern_negation());
                    // Avoid subsequent errors.
                    // Dart: `constantPatternContext == ConstantPatternContext.none;`
                    // (a comparison without effect, so the context is not changed).
                }
                return self.parse_send_or_function_literal(
                    token,
                    context,
                    constant_pattern_context,
                );
            } else if value == Some("return") {
                // Recovery
                token = self.next(token);
                self.report_recoverable_error_with_token(token, diag::unexpected_token);
                return self.parse_primary(token, context, ConstantPatternContext::None);
            } else {
                // Fall through to the recovery code.
            }
        } else if kind == OPEN_PAREN_TOKEN {
            return self.parse_parenthesized_expression_function_literal_or_record_literal(
                token,
                constant_pattern_context,
            );
        } else if kind == OPEN_SQUARE_BRACKET_TOKEN || self.is_a(self.next(token), TokenType::INDEX)
        {
            let next = self.next(token);
            self.listener.handle_no_type_arguments(next);
            return self.parse_literal_list_suffix(token, /* constKeyword = */ None);
        } else if kind == OPEN_CURLY_BRACKET_TOKEN {
            let next = self.next(token);
            self.listener.handle_no_type_arguments(next);
            return self.parse_literal_set_or_map_suffix(token, /* constKeyword = */ None);
        } else if kind == LT_TOKEN {
            return self
                .parse_literal_list_set_map_or_function(token, /* constKeyword = */ None);
        } else {
            // Fall through to the recovery code.
        }
        //
        // Recovery code.
        //
        self.parse_send(token, context, constant_pattern_context)
    }

    /// Dart (line 8211): `Token parseParenthesizedExpressionFunctionLiteralOrRecordLiteral( Token token, ConstantPatternContext constantPatternContext, )`
    pub fn parse_parenthesized_expression_function_literal_or_record_literal(
        &mut self,
        token: TokenId,
        constant_pattern_context: ConstantPatternContext,
    ) -> TokenId {
        let next = self.next(token);
        debug_assert!(self.is_a(next, TokenType::OPEN_PAREN));

        if self.may_parse_function_expressions {
            let next_token = self.next(self.end_group(next).unwrap());
            let mut kind = self.kind(next_token);
            if kind == FUNCTION_TOKEN || kind == OPEN_CURLY_BRACKET_TOKEN {
                self.listener.handle_no_type_variables(next);
                return self.parse_function_expression(token);
            } else if kind == KEYWORD_TOKEN || kind == IDENTIFIER_TOKEN {
                if self.is_a(next_token, Keyword::ASYNC) || self.is_a(next_token, Keyword::SYNC) {
                    self.listener.handle_no_type_variables(next);
                    return self.parse_function_expression(token);
                }
                // Recovery
                // If there is a stray simple identifier in the function expression
                // because the user is typing (e.g. `() asy {}`) then continue parsing
                // and allow parseFunctionExpression to report an unexpected token.
                kind = self.kind(self.next(next_token));
                if kind == FUNCTION_TOKEN || kind == OPEN_CURLY_BRACKET_TOKEN {
                    self.listener.handle_no_type_variables(next);
                    return self.parse_function_expression(token);
                }
            }
        }
        let old = self.may_parse_function_expressions;
        self.may_parse_function_expressions = true;
        let token = self.parse_parenthesized_expression_or_record_literal(
            token,
            /* constKeywordForRecord = */ None,
            constant_pattern_context,
        );
        self.may_parse_function_expressions = old;
        token
    }

    /// Dart (line 8258): `Token ensureParenthesizedCondition(Token token, {required bool allowCase})`
    ///
    /// Parses an expression inside parentheses that represents the condition part
    /// of an if-statement, if-element, do-while statement, or while statement, or
    /// the scrutinee part of a switch statement.  [token] is the token before
    /// where the `(` is expected.
    ///
    /// [allowCase] indicates whether the condition may optionally be followed
    /// by a caseHead.
    pub fn ensure_parenthesized_condition(&mut self, token: TokenId, allow_case: bool) -> TokenId {
        let mut open_paren = self.next(token);
        if !self.is_a(open_paren, TokenType::OPEN_PAREN) {
            // Recover
            self.report_recoverable_error(open_paren, diag::expected_token("("));
            open_paren = self
                .rewriter()
                .insert_parens(token, /* includeIdentifier = */ false);
        }
        self.parse_expression_in_parenthesis_rest(open_paren, allow_case)
    }

    /// Dart (line 8275): `Token parseParenthesizedExpressionOrRecordLiteral( Token token, Token? constKeywordForRecord, ConstantPatternContext constantPatternContext, )`
    ///
    /// Parse either a parenthesized expression or a record literal.
    /// If [constKeywordForRecord] is non-null it is forced to be a record
    /// literal and an error will be issued if there is no trailing comma.
    pub fn parse_parenthesized_expression_or_record_literal(
        &mut self,
        token: TokenId,
        const_keyword_for_record: Option<TokenId>,
        constant_pattern_context: ConstantPatternContext,
    ) -> TokenId {
        let begin = self.next(token);
        debug_assert!(self.is_a(begin, TokenType::OPEN_PAREN));
        self.listener
            .begin_parenthesized_expression_or_record_literal(begin);

        // For parsing of parenthesized expression we need parity with
        // parseExpressionInParenthesisRest used in ensureParenthesizedCondition.

        let mut token = begin;
        let mut count: i32 = 0;
        let mut was_record = const_keyword_for_record.is_some();
        let mut was_valid_record = false;
        let mut illegal_trailing_comma: Option<TokenId> = None;
        loop {
            let mut next = self.next(token);
            if self.is_a(next, TokenType::CLOSE_PAREN) {
                if count == 0 {
                    was_record = true;
                }
                break;
            } else if count == 0
                && self.is_a(next, TokenType::COMMA)
                && self.is_a(self.next(next), TokenType::CLOSE_PAREN)
            {
                illegal_trailing_comma = Some(next);
                was_record = true;
                token = next;
                break;
            }
            let mut colon: Option<TokenId> = None;
            if self.is_a(self.next(next), TokenType::COLON) /* recovery */
                || self.is_a(next, TokenType::COLON)
            {
                // Record with named expression.
                was_record = true;
                token = self.ensure_identifier(token, IdentifierContext::NamedRecordFieldReference);
                token = self.next(token);
                colon = Some(token);
                was_valid_record = true;
            }
            let begin_expression_token = token;
            token = self.parse_expression(token);
            next = self.next(token);
            if let Some(colon) = colon {
                self.listener.handle_named_record_field(colon);
            } else {
                let first = self.next(begin_expression_token);
                self.listener.handle_positional_record_field(first);
            }
            count += 1;
            if !self.is_a(next, TokenType::COMMA) {
                // TODO(jensj): Possible more specific recovery.
                break;
            } else {
                // It is a comma, i.e. it's a record.
                was_record = true;
                was_valid_record = true;
            }
            token = next;
        }
        token = self.ensure_close_paren(token, begin);
        debug_assert!(self.is_a(token, TokenType::CLOSE_PAREN));

        debug_assert!(was_record || count <= 1);

        if was_record {
            if count == 0 && illegal_trailing_comma.is_some() {
                // Empty record literal with a comma `(,)`.
                self.report_recoverable_error(
                    illegal_trailing_comma.unwrap(),
                    diag::record_literal_zero_fields_with_trailing_comma(),
                );
            } else if count == 1 && !was_valid_record {
                self.report_recoverable_error(
                    token,
                    diag::record_literal_one_positional_field_no_trailing_comma(),
                );
            } else if count == 0 && constant_pattern_context != ConstantPatternContext::None {
                self.report_recoverable_error(
                    begin,
                    diag::invalid_constant_pattern_empty_record_literal(),
                );
            }
            self.listener
                .end_record_literal(begin, count, const_keyword_for_record);
        } else {
            self.listener.end_parenthesized_expression(begin);
        }

        token
    }

    /// Dart (line 8376): `Token parseExpressionInParenthesisRest( Token token,`
    /// `{required bool allowCase})`
    ///
    /// Parses an expression inside parentheses that represents the condition part
    /// of an if-statement, if-element, do-while statement, or while statement, or
    /// the scrutinee part of a switch statement.  [token] is the `(` token.
    ///
    /// [allowCase] indicates whether the condition may optionally be followed by
    /// a caseHead.
    pub fn parse_expression_in_parenthesis_rest(
        &mut self,
        token: TokenId,
        allow_case: bool,
    ) -> TokenId {
        debug_assert!(self.is_a(token, TokenType::OPEN_PAREN));
        let begin = token;
        let mut token = self.parse_expression(token);
        let mut next = self.next(token);
        if self.is_patterns_feature_enabled && self.is_a(next, Keyword::CASE) {
            token = next;
            let case_ = token;
            token = self.parse_pattern(token, PatternContext::Matching, /* precedence = */ 1);
            next = self.next(token);
            let mut when: Option<TokenId> = None;
            if self.is_a(next, Keyword::WHEN) {
                token = next;
                when = Some(next);
                self.listener.begin_pattern_guard(next);
                token = self.parse_expression(token);
                self.listener.end_pattern_guard(next);
            }
            token = self.ensure_close_paren(token, begin);
            self.listener
                .handle_parenthesized_condition(begin, Some(case_), when);
        } else {
            token = self.ensure_close_paren(token, begin);
            self.listener.handle_parenthesized_condition(
                begin, /* case_ = */ None, /* when = */ None,
            );
        }
        debug_assert!(self.is_a(token, TokenType::CLOSE_PAREN));
        token
    }

    /// Dart (line 8409): `Token parseThisExpression(Token token, IdentifierContext context)`
    pub fn parse_this_expression(&mut self, token: TokenId, context: IdentifierContext) -> TokenId {
        let mut token = self.next(token);
        let this_token = token;
        debug_assert!(self.is_a(this_token, Keyword::THIS));
        self.listener.handle_this_expression(this_token, context);
        let next = self.next(token);
        if self.is_a(next, TokenType::OPEN_PAREN) {
            // Constructor forwarding.
            self.listener.handle_no_type_arguments(next);
            token = self.parse_arguments(token);
            self.listener.handle_send(this_token, token);
        }
        token
    }

    /// Dart (line 8423): `Token parseSuperExpression(Token token, IdentifierContext context)`
    pub fn parse_super_expression(
        &mut self,
        token: TokenId,
        context: IdentifierContext,
    ) -> TokenId {
        let mut token = self.next(token);
        let super_token = token;
        debug_assert!(self.is_a(token, Keyword::SUPER));
        self.listener.handle_super_expression(super_token, context);
        let next = self.next(token);
        if self.is_a(next, TokenType::OPEN_PAREN) {
            // Super constructor.
            self.listener.handle_no_type_arguments(next);
            token = self.parse_arguments(token);
            self.listener.handle_send(super_token, token);
        } else if self.is_a(next, TokenType::QUESTION_PERIOD) {
            self.report_recoverable_error(next, diag::super_null_aware());
        }
        token
    }
}
