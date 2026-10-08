// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/parser_impl.dart (lines 8451-10489)

#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    unused_assignments,
    clippy::all
)]

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
use crate::identifier_context::{IdentifierContext, looks_like_expression_start};
use crate::listener::Listener;
use crate::listener_stack::Layer;
use crate::literal_entry_info::{
    LiteralEntryInfo, SIMPLE_ENTRY, compute_literal_entry, looks_like_literal_entry,
};
use crate::loop_state::LoopState;
use crate::member_kind::MemberKind;
use crate::modifier_context::{ModifierContext, is_modifier};
use crate::type_info::{
    NO_TYPE, NO_TYPE_PARAM_OR_ARG, TypeInfo, TypeParamOrArgInfo, compute_type,
    compute_type_param_or_arg,
};
use crate::type_info_impl::looks_like_name;

impl<L: Listener> Parser<L> {
    /// Dart (line 8451): `Token parseLiteralListSuffix(Token token, Token? constKeyword)`
    ///
    /// This method parses the portion of a list literal starting with the left
    /// square bracket.
    ///
    /// ```
    /// listLiteral:
    ///   'const'? typeArguments? '[' (elementList ','?)? ']'
    /// ;
    /// ```
    ///
    /// Provide a [constKeyword] if the literal is preceded by 'const', or `null`
    /// if not. This is a suffix parser because it is assumed that type arguments
    /// have been parsed, or `listener.handleNoTypeArguments` has been executed.
    pub fn parse_literal_list_suffix(
        &mut self,
        token: TokenId,
        const_keyword: Option<TokenId>,
    ) -> TokenId {
        let mut token = token;
        let before_token = token;
        token = self.next(token);
        let begin_token = token;
        debug_assert!(
            self.is_a(token, TokenType::OPEN_SQUARE_BRACKET) || self.is_a(token, TokenType::INDEX)
        );
        let mut count: i32 = 0;
        if self.is_a(token, TokenType::INDEX) {
            let rewritten = self.rewrite_square_brackets(before_token);
            token = self.next(rewritten);
            let after = self.next(token);
            self.listener
                .handle_literal_list(/* count = */ 0, token, const_keyword, after);
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
            let mut if_count: i32 = 0;
            let mut info = self.compute_literal_entry(token);
            while let Some(mut current) = info {
                next = self.next(token);
                if current.has_entry() {
                    if self.is_a(next, TokenType::QUESTION) {
                        let null_aware_token = next;
                        token = next;
                        token = self.parse_expression(token);
                        self.listener.handle_null_aware_element(null_aware_token);
                    } else {
                        token = self.parse_expression(token);
                    }
                } else {
                    token = current.parse(token, self);
                }
                if_count += current.if_condition_delta();
                info = self.next_literal_entry(current, token);
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
                let message = if if_count > 0 {
                    diag::expected_else_or_comma()
                } else {
                    diag::expected_but_got(",")
                };
                next = self.rewrite_and_recover(token, message, comma);
            }
            token = next;
        }
        self.may_parse_function_expressions = old;
        self.listener
            .handle_literal_list(count, begin_token, const_keyword, token);
        token
    }

    /// Dart (line 8536): `Token parseLiteralSetOrMapSuffix(Token token, Token? constKeyword)`
    ///
    /// This method parses the portion of a set or map literal that starts with
    /// the left curly brace when there are no leading type arguments.
    pub fn parse_literal_set_or_map_suffix(
        &mut self,
        token: TokenId,
        const_keyword: Option<TokenId>,
    ) -> TokenId {
        let mut token = self.next(token);
        let left_brace = token;
        debug_assert!(self.is_a(left_brace, TokenType::OPEN_CURLY_BRACKET));
        let mut next = self.next(token);
        if self.is_a(next, TokenType::CLOSE_CURLY_BRACKET) {
            self.listener.handle_literal_set_or_map(
                /* count = */ 0,
                left_brace,
                const_keyword,
                next,
                /* hasSetEntry = */ false,
            );
            return next;
        }

        let old = self.may_parse_function_expressions;
        self.may_parse_function_expressions = true;
        let mut count: i32 = 0;
        // TODO(danrubel): hasSetEntry parameter exists for replicating existing
        // behavior and will be removed once unified collection has been enabled
        let mut has_set_entry: Option<bool> = None;

        loop {
            let mut if_count: i32 = 0;
            let mut info = self.compute_literal_entry(token);
            if info == Some(SIMPLE_ENTRY) {
                // TODO(danrubel): Remove this section and use the while loop below
                // once hasSetEntry is no longer needed.
                token = self.parse_expression(token);
                let is_map_entry = self.is_a(self.next(token), TokenType::COLON);
                if has_set_entry.is_none() {
                    has_set_entry = Some(!is_map_entry);
                }
                if is_map_entry {
                    let colon = self.next(token);
                    token = colon;
                    let next = self.next(token);
                    let mut null_aware_value_token: Option<TokenId> = None;
                    if self.is_a(next, TokenType::QUESTION_PERIOD) {
                        token = self.split_following_question_period(token);
                        null_aware_value_token = Some(token);
                    } else if self.is_a(next, TokenType::QUESTION) {
                        token = next;
                        null_aware_value_token = Some(next);
                    }

                    token = self.parse_expression(token);
                    let end = self.next(token);
                    self.listener.handle_literal_map_entry(
                        colon,
                        end,
                        None,
                        null_aware_value_token,
                    );
                }
            } else {
                while let Some(mut current) = info {
                    if current.has_entry() {
                        let mut null_aware_key_token: Option<TokenId> = None;
                        let next = self.next(token);
                        if self.is_a(next, TokenType::QUESTION) {
                            // Null-aware key, for example:
                            //   <double, Symbol>{ if (b) ?x: y }
                            //   <double, Symbol>{ if (b) ?x: ?y }
                            null_aware_key_token = Some(next);
                            token = next;
                        }
                        token = self.parse_expression(token);

                        if self.is_a(self.next(token), TokenType::COLON) {
                            let colon = self.next(token);
                            token = colon;

                            let mut null_aware_value_token: Option<TokenId> = None;
                            let next = self.next(token);
                            if self.is_a(next, TokenType::QUESTION_PERIOD) {
                                token = self.split_following_question_period(token);
                                null_aware_value_token = Some(token);
                            } else if self.is_a(next, TokenType::QUESTION) {
                                token = next;
                                null_aware_value_token = Some(next);
                            }

                            token = self.parse_expression(token);
                            let end = self.next(token);
                            self.listener.handle_literal_map_entry(
                                colon,
                                end,
                                null_aware_key_token,
                                null_aware_value_token,
                            );
                        } else if let Some(null_aware_key_token) = null_aware_key_token {
                            // Null-aware element. For example:
                            //   <String>{ if (b) ?x }
                            self.listener
                                .handle_null_aware_element(null_aware_key_token);
                        }
                    } else {
                        token = current.parse(token, self);
                    }
                    if_count += current.if_condition_delta();
                    info = self.next_literal_entry(current, token);
                }
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
                self.listener.handle_literal_set_or_map(
                    count,
                    left_brace,
                    const_keyword,
                    next,
                    has_set_entry.unwrap_or(false),
                );
                self.may_parse_function_expressions = old;
                return next;
            }

            if comma.is_none() {
                // Recovery
                if looks_like_literal_entry(self.tokens(), next) {
                    // If this looks like the start of an expression,
                    // then report an error, insert the comma, and continue parsing.
                    // TODO(danrubel): Consider better error message
                    let offset = self.offset(next);
                    let byte = self.tokens().byte_offset(next);
                    let comma = self
                        .tokens_mut()
                        .push_synthetic(TokenType::COMMA, offset, byte);
                    let message = if if_count > 0 {
                        diag::expected_else_or_comma()
                    } else {
                        diag::expected_but_got(",")
                    };
                    token = self.rewrite_and_recover(token, message, comma);
                } else {
                    self.report_recoverable_error(next, diag::expected_but_got("}"));
                    // Scanner guarantees a closing curly bracket
                    next = self.end_group(left_brace).unwrap();
                    self.listener.handle_literal_set_or_map(
                        count,
                        left_brace,
                        const_keyword,
                        next,
                        has_set_entry.unwrap_or(false),
                    );
                    self.may_parse_function_expressions = old;
                    return next;
                }
            }
        }
    }

    /// Dart (line 8687): `LiteralEntryInfo? _computeLiteralEntry(Token token)`
    pub fn compute_literal_entry(&mut self, token: TokenId) -> Option<LiteralEntryInfo> {
        if self.is_a(self.next(token), TokenType::QUESTION_PERIOD) {
            self.split_following_question_period(token);
        }
        Some(compute_literal_entry(self.tokens(), token))
    }

    /// Dart (line 8694): `LiteralEntryInfo? _nextLiteralEntry(LiteralEntryInfo info, Token token)`
    pub fn next_literal_entry(
        &mut self,
        info: LiteralEntryInfo,
        token: TokenId,
    ) -> Option<LiteralEntryInfo> {
        if self.is_a(self.next(token), TokenType::QUESTION_PERIOD) {
            self.split_following_question_period(token);
        }
        info.compute_next(self.tokens(), token)
    }

    /// Dart (line 8701): `Token _splitFollowingQuestionPeriod(Token token)`
    pub fn split_following_question_period(&mut self, token: TokenId) -> TokenId {
        let next = self.next(token);
        debug_assert!(self.is_a(next, TokenType::QUESTION_PERIOD));
        let offset = self.char_offset(next);
        let byte = self.tokens().byte_offset(next);
        let question =
            self.tokens_mut()
                .push_simple(TokenType::QUESTION, offset, byte, TokenId::NONE);
        let new_next = self.rewriter().replace_token_following(token, question);
        let period =
            self.tokens_mut()
                .push_simple(TokenType::PERIOD, offset + 1, byte + 1, TokenId::NONE);
        self.rewriter().insert_token(new_next, period);
        new_next
    }

    /// Dart (line 8717): `Token parseLiteralFunctionSuffix(Token token)`
    ///
    /// formalParameterList functionBody.
    ///
    /// This is a suffix parser because it is assumed that type arguments have
    /// been parsed, or `listener.handleNoTypeArguments(..)` has been executed.
    pub fn parse_literal_function_suffix(&mut self, token: TokenId) -> TokenId {
        debug_assert!(self.is_a(self.next(token), TokenType::OPEN_PAREN));
        // Scanner ensures `(` has matching `)`.
        let next = self.next(self.end_group(self.next(token)).unwrap());
        let kind = self.kind(next);
        if kind != FUNCTION_TOKEN
            && kind != OPEN_CURLY_BRACKET_TOKEN
            && (kind != KEYWORD_TOKEN
                || !self.is_a(next, Keyword::ASYNC) && !self.is_a(next, Keyword::SYNC))
        {
            self.report_recoverable_error_with_token(next, diag::unexpected_token);
        }
        self.parse_function_expression(token)
    }

    /// Dart (line 8740): `Token parseLiteralListSetMapOrFunction(Token start, Token? constKeyword)`
    ///
    /// genericListLiteral | genericMapLiteral | genericFunctionLiteral.
    ///
    /// Where
    ///   genericListLiteral ::= typeArguments '[' (expressionList ','?)? ']'
    ///   genericMapLiteral ::=
    ///       typeArguments '{' (mapLiteralEntry (',' mapLiteralEntry)* ','?)? '}'
    ///   genericFunctionLiteral ::=
    ///       typeParameters formalParameterList functionBody
    /// Provide token for [constKeyword] if preceded by 'const', null if not.
    pub fn parse_literal_list_set_map_or_function(
        &mut self,
        start: TokenId,
        const_keyword: Option<TokenId>,
    ) -> TokenId {
        debug_assert!(self.is_a(self.next(start), TokenType::LT));
        let type_param_or_arg =
            compute_type_param_or_arg_mut(self.tokens_mut(), start, /* inDeclaration = */ true, false);
        let mut token = type_param_or_arg.skip_mut(self.tokens_mut(), start);
        if self.is_a(self.next(token), TokenType::OPEN_PAREN) {
            if let Some(const_keyword) = const_keyword {
                self.report_recoverable_error_with_token(const_keyword, diag::unexpected_token);
            }
            token = type_param_or_arg.parse_variables(start, self);
            return self.parse_literal_function_suffix(token);
        }
        // Note that parseArguments can rewrite the token stream!
        token = type_param_or_arg.parse_arguments(start, self);
        let next = self.next(token);
        if self.is_a(next, TokenType::OPEN_CURLY_BRACKET) {
            if type_param_or_arg.type_argument_count() > 2 {
                let start_next = self.next(start);
                self.report_recoverable_error_with_end(
                    start_next,
                    token,
                    diag::set_or_map_literal_too_many_type_arguments(),
                );
            }
            return self.parse_literal_set_or_map_suffix(token, const_keyword);
        }
        if !self.is_a(next, TokenType::OPEN_SQUARE_BRACKET) && !self.is_a(next, TokenType::INDEX) {
            // TODO(danrubel): Improve this error message.
            self.report_recoverable_error(next, diag::expected_but_got("["));
            self.rewriter()
                .insert_synthetic_token(token, TokenType::INDEX);
        }
        self.parse_literal_list_suffix(token, const_keyword)
    }

    /// Dart (line 8787): `Token parseMapLiteralEntry(Token token)`
    ///
    /// ```
    /// mapLiteralEntry:
    ///   expression ':' expression |
    ///   'if' '(' expression ')' mapLiteralEntry ( 'else' mapLiteralEntry )? |
    ///   'await'? 'for' '(' forLoopParts ')' mapLiteralEntry |
    ///   ( '...' | '...?' ) expression
    /// ;
    /// ```
    pub fn parse_map_literal_entry(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        // Assume the listener rejects non-string keys.
        // TODO(brianwilkerson): Change the assumption above by moving error
        // checking into the parser, making it possible to recover.
        let mut info = Some(compute_literal_entry(self.tokens(), token));
        while let Some(mut current) = info {
            if current.has_entry() {
                token = self.parse_expression(token);
                let colon = self.ensure_colon(token);
                token = self.parse_expression(colon);
                // TODO remove unused 2nd parameter
                let end = self.next(token);
                self.listener
                    .handle_literal_map_entry(colon, end, None, None);
            } else {
                token = current.parse(token, self);
            }
            info = current.compute_next(self.tokens(), token);
        }
        token
    }

    /// Dart (line 8807): `Token parseSendOrFunctionLiteral( Token token, IdentifierContext context, ConstantPatternContext constantPatternContext, )`
    pub fn parse_send_or_function_literal(
        &mut self,
        token: TokenId,
        context: IdentifierContext,
        constant_pattern_context: ConstantPatternContext,
    ) -> TokenId {
        if !self.may_parse_function_expressions || context.is_continuation() {
            // "Inside" a continuation we can't have a function literal.
            return self.parse_send(token, context, constant_pattern_context);
        }
        let type_info = compute_type_mut(self.tokens_mut(),
            token,
            /* required = */ false,
            false,
            false,
        );

        let before_name = type_info.skip_type_mut(self.tokens_mut(), token);
        let name = self.next(before_name);
        if self.is_identifier(name) {
            let type_param = compute_type_param_or_arg_mut(self.tokens_mut(), name, false, false);
            let next = { let skipped = type_param.skip_mut(self.tokens_mut(), name); self.next(skipped) };
            if self.is_a(next, TokenType::OPEN_PAREN) {
                let after = self.next(self.end_group(next).unwrap());
                if self.looks_like_function_body(after) {
                    return self.parse_function_literal(
                        token,
                        before_name,
                        name,
                        &type_info,
                        type_param,
                        context,
                    );
                }
            }
        }

        self.parse_send(token, context, constant_pattern_context)
    }

    /// Dart (line 8840): `Token ensureArguments(Token token)`
    pub fn ensure_arguments(&mut self, token: TokenId) -> TokenId {
        let mut next = self.next(token);
        if !self.is_a(next, TokenType::OPEN_PAREN) {
            self.report_recoverable_error(token, diag::expected_after_but_got("("));
            next = self
                .rewriter()
                .insert_parens(token, /* includeIdentifier = */ false);
        }
        self.parse_arguments_rest(next)
    }

    /// Dart (line 8852): `Token parseConstructorInvocationArguments(Token token)`
    pub fn parse_constructor_invocation_arguments(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        let mut next = self.next(token);
        if !self.is_a(next, TokenType::OPEN_PAREN) {
            // Recovery: Check for invalid type parameters
            let type_arg = compute_type_param_or_arg_mut(self.tokens_mut(), token, false, false);
            if type_arg == NO_TYPE_PARAM_OR_ARG {
                self.report_recoverable_error(token, diag::expected_after_but_got("("));
            } else {
                self.report_recoverable_error(token, diag::constructor_with_type_arguments());
                token = type_arg.parse_arguments(token, self);
                self.listener.handle_invalid_type_arguments(token);
                next = self.next(token);
            }
            if !self.is_a(next, TokenType::OPEN_PAREN) {
                next = self
                    .rewriter()
                    .insert_parens(token, /* includeIdentifier = */ false);
            }
        }
        self.parse_arguments_rest(next)
    }

    /// Dart (line 8880): `Token parseNewExpression(Token token)`
    ///
    /// ```
    /// newExpression:
    ///   'new' type ('.' identifier)? arguments
    /// ;
    /// ```
    pub fn parse_new_expression(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        let new_keyword = self.next(token);
        debug_assert!(self.is_a(new_keyword, Keyword::NEW));

        let mut potential_type_arg: Option<TypeParamOrArgInfo> = None;

        let next = self.next(new_keyword);

        if self.kind(next) == IDENTIFIER_TOKEN {
            let identifier = next;
            let value = self.lexeme(identifier).to_string();
            if (value == "Map" || value == "Set")
                && !self.is_a(self.next(identifier), TokenType::PERIOD)
            {
                let arg = compute_type_param_or_arg_mut(self.tokens_mut(), identifier, false, false);
                potential_type_arg = Some(arg);
                let after_token = { let skipped = arg.skip_mut(self.tokens_mut(), identifier); self.next(skipped) };
                if self.is_a(after_token, TokenType::OPEN_CURLY_BRACKET) {
                    // Recover by ignoring both the `new` and the `Map`/`Set` and parse as
                    // a literal map/set.
                    let message = diag::literal_with_class_and_new(
                        &value.to_lowercase(),
                        self.lexeme(identifier),
                    );
                    self.report_recoverable_error_with_end(new_keyword, identifier, message);
                    return self.parse_primary(
                        identifier,
                        IdentifierContext::Expression,
                        ConstantPatternContext::None,
                    );
                }
            } else if value == "List" && !self.is_a(self.next(identifier), TokenType::PERIOD) {
                let arg = compute_type_param_or_arg_mut(self.tokens_mut(), identifier, false, false);
                potential_type_arg = Some(arg);
                let after_token = { let skipped = arg.skip_mut(self.tokens_mut(), identifier); self.next(skipped) };
                if self.is_a(after_token, TokenType::OPEN_SQUARE_BRACKET)
                    || self.is_a(after_token, TokenType::INDEX)
                {
                    // Recover by ignoring both the `new` and the `List` and parse as
                    // a literal list.
                    let message = diag::literal_with_class_and_new(
                        &value.to_lowercase(),
                        self.lexeme(identifier),
                    );
                    self.report_recoverable_error_with_end(new_keyword, identifier, message);
                    return self.parse_primary(
                        identifier,
                        IdentifierContext::Expression,
                        ConstantPatternContext::None,
                    );
                }
            }
        } else {
            // This is probably an error. "Normal" recovery will happen in
            // parseConstructorReference.
            // Do special recovery for literal maps/set/list erroneously prepended
            // with 'new'.
            let not_identifier = next;
            let value = self.lexeme(not_identifier).to_string();
            if value == "<" {
                let arg = compute_type_param_or_arg_mut(self.tokens_mut(), new_keyword, false, false);
                potential_type_arg = Some(arg);
                let after_token = { let skipped = arg.skip_mut(self.tokens_mut(), new_keyword); self.next(skipped) };
                if self.is_a(after_token, TokenType::OPEN_CURLY_BRACKET)
                    || self.is_a(after_token, TokenType::OPEN_SQUARE_BRACKET)
                    || self.is_a(after_token, TokenType::INDEX)
                {
                    // Recover by ignoring the `new` and parse as a literal map/set/list.
                    self.report_recoverable_error(new_keyword, diag::literal_with_new());
                    return self.parse_primary(
                        new_keyword,
                        IdentifierContext::Expression,
                        ConstantPatternContext::None,
                    );
                }
            } else if value == "{" || value == "[" || value == "[]" {
                // Recover by ignoring the `new` and parse as a literal map/set/list.
                self.report_recoverable_error(new_keyword, diag::literal_with_new());
                return self.parse_primary(
                    new_keyword,
                    IdentifierContext::Expression,
                    ConstantPatternContext::None,
                );
            }
        }

        self.listener.begin_new_expression(new_keyword);
        token = self.parse_constructor_reference(
            new_keyword,
            ConstructorReferenceContext::New,
            /* typeArg = */ potential_type_arg,
        );
        token = self.parse_constructor_invocation_arguments(token);
        self.listener.end_new_expression(new_keyword);
        token
    }

    /// Dart (line 8977): `Token parseImplicitCreationExpression( Token token, Token openAngleBracket, TypeParamOrArgInfo typeArg, )`
    pub fn parse_implicit_creation_expression(
        &mut self,
        token: TokenId,
        open_angle_bracket: TokenId,
        type_arg: TypeParamOrArgInfo,
    ) -> TokenId {
        let mut token = token;
        let begin = self.next(token); // This is the class name.
        self.listener.begin_implicit_creation_expression(begin);
        token = self.parse_constructor_reference(
            token,
            ConstructorReferenceContext::Implicit,
            /* typeArg = */ Some(type_arg),
        );
        token = self.parse_constructor_invocation_arguments(token);
        self.listener
            .end_implicit_creation_expression(begin, open_angle_bracket);
        token
    }

    /// Dart (line 9011): `Token parseConstExpression(Token token)`
    ///
    /// This method parses a list or map literal that is known to start with the
    /// keyword 'const'.
    ///
    /// ```
    /// listLiteral:
    ///   'const'? typeArguments? '[' (expressionList ','?)? ']'
    /// ;
    ///
    /// mapLiteral:
    ///   'const'? typeArguments?
    ///     '{' (mapLiteralEntry (',' mapLiteralEntry)* ','?)? '}'
    /// ;
    ///
    /// mapLiteralEntry:
    ///   expression ':' expression
    /// ;
    /// ```
    pub fn parse_const_expression(&mut self, token: TokenId) -> TokenId {
        let mut token = self.next(token);
        let const_keyword = token;
        debug_assert!(self.is_a(const_keyword, Keyword::CONST));
        let next = self.next(token);
        let value = self.string_value(next);
        if value == Some("[") || value == Some("[]") {
            self.listener.begin_const_literal(next);
            self.listener.handle_no_type_arguments(next);
            token = self.parse_literal_list_suffix(token, Some(const_keyword));
            self.listener.end_const_literal(token);
            return token;
        }
        if value == Some("(") {
            // Const record literal.
            self.listener.begin_const_literal(next);
            token = self.parse_parenthesized_expression_or_record_literal(
                token,
                Some(const_keyword),
                ConstantPatternContext::None,
            );
            self.listener.end_const_literal(token);
            return token;
        }
        if value == Some("{") {
            self.listener.begin_const_literal(next);
            self.listener.handle_no_type_arguments(next);
            token = self.parse_literal_set_or_map_suffix(token, Some(const_keyword));
            self.listener.end_const_literal(token);
            return token;
        }
        if value == Some("<") {
            self.listener.begin_const_literal(next);
            token = self.parse_literal_list_set_map_or_function(token, Some(const_keyword));
            self.listener.end_const_literal(token);
            return token;
        }
        let lexeme = self.lexeme(next).to_string();
        let next_next = self.next(next);
        let mut potential_type_arg: Option<TypeParamOrArgInfo> = None;
        if (lexeme == "Map" || lexeme == "Set") && !self.is_a(next_next, TokenType::PERIOD) {
            // Special-case-recovery for `const Map<..>?{}` and `const Set<..>?{}`.
            let arg = compute_type_param_or_arg_mut(self.tokens_mut(), next, false, false);
            potential_type_arg = Some(arg);
            let after_token = { let skipped = arg.skip_mut(self.tokens_mut(), next); self.next(skipped) };
            if self.is_a(after_token, TokenType::OPEN_CURLY_BRACKET) {
                let next_value = self.string_value(next_next);
                if next_value == Some("{") {
                    // Recover by ignoring the `Map`/`Set` and parse as a literal map/set.
                    let message =
                        diag::literal_with_class(&lexeme.to_lowercase(), self.lexeme(next));
                    self.report_recoverable_error(next, message);
                    self.listener.begin_const_literal(next_next);
                    self.listener.handle_no_type_arguments(next_next);
                    token = self.parse_literal_set_or_map_suffix(next, Some(const_keyword));
                    self.listener.end_const_literal(token);
                    return token;
                }
                if next_value == Some("<") {
                    // Recover by ignoring the `Map`/`Set` and parse as a literal map/set.
                    let message =
                        diag::literal_with_class(&lexeme.to_lowercase(), self.lexeme(next));
                    self.report_recoverable_error(next, message);

                    self.listener.begin_const_literal(next_next);
                    token = self.parse_literal_list_set_map_or_function(next, Some(const_keyword));
                    self.listener.end_const_literal(token);
                    return token;
                }
                // assert(false, "Expected either { or < but found neither.");
            }
        } else if lexeme == "List" && !self.is_a(next_next, TokenType::PERIOD) {
            // Special-case-recovery for `const List<..>?[` and `const List<..>?[]`.
            let arg = compute_type_param_or_arg_mut(self.tokens_mut(), next, false, false);
            potential_type_arg = Some(arg);
            let after_token = { let skipped = arg.skip_mut(self.tokens_mut(), next); self.next(skipped) };
            if self.is_a(after_token, TokenType::OPEN_SQUARE_BRACKET)
                || self.is_a(after_token, TokenType::INDEX)
            {
                let next_value = self.string_value(next_next);
                if next_value == Some("[") || next_value == Some("[]") {
                    // Recover by ignoring the `List` and parse as a literal list.
                    let message =
                        diag::literal_with_class(&lexeme.to_lowercase(), self.lexeme(next));
                    self.report_recoverable_error(next, message);
                    self.listener.begin_const_literal(next_next);
                    self.listener.handle_no_type_arguments(next_next);
                    token = self.parse_literal_list_suffix(next, Some(const_keyword));
                    self.listener.end_const_literal(token);
                    return token;
                }
                if next_value == Some("<") {
                    // Recover by ignoring the `List` and parse as a literal list.
                    let message =
                        diag::literal_with_class(&lexeme.to_lowercase(), self.lexeme(next));
                    self.report_recoverable_error(next, message);
                    self.listener.begin_const_literal(next_next);
                    token = self.parse_literal_list_set_map_or_function(next, Some(const_keyword));
                    self.listener.end_const_literal(token);
                    return token;
                }
                // assert(false, "Expected either [, [] or < but found neither.");
            }
        }

        self.listener.begin_const_expression(const_keyword);
        token = self.parse_constructor_reference(
            token,
            ConstructorReferenceContext::Const,
            /* typeArg = */ potential_type_arg,
        );
        token = self.parse_constructor_invocation_arguments(token);
        self.listener.end_const_expression(const_keyword);
        token
    }

    /// Dart (line 9145): `Token parseLiteralInt(Token token)`
    ///
    /// ```
    /// intLiteral:
    ///   integer
    /// ;
    /// ```
    pub fn parse_literal_int(&mut self, token: TokenId) -> TokenId {
        let token = self.next(token);
        debug_assert!(self.kind(token) == INT_TOKEN || self.kind(token) == HEXADECIMAL_TOKEN);
        self.listener.handle_literal_int(token);
        token
    }

    /// Dart (line 9155): `Token parseLiteralIntWithSeparators(Token token)`
    pub fn parse_literal_int_with_separators(&mut self, token: TokenId) -> TokenId {
        let token = self.next(token);
        debug_assert!(self.kind(token) == INT_TOKEN || self.kind(token) == HEXADECIMAL_TOKEN);
        self.listener.handle_literal_int_with_separators(token);
        token
    }

    /// Dart (line 9170): `Token parseLiteralDouble(Token token)`
    ///
    /// ```
    /// doubleLiteral:
    ///   double
    /// ;
    /// ```
    pub fn parse_literal_double(&mut self, token: TokenId) -> TokenId {
        let token = self.next(token);
        debug_assert!(self.kind(token) == DOUBLE_TOKEN);
        self.listener.handle_literal_double(token);
        token
    }

    /// Dart (line 9177): `Token parseLiteralDoubleWithSeparators(Token token)`
    pub fn parse_literal_double_with_separators(&mut self, token: TokenId) -> TokenId {
        let token = self.next(token);
        debug_assert!(self.kind(token) == DOUBLE_TOKEN);
        self.listener.handle_literal_double_with_separators(token);
        token
    }

    /// Dart (line 9189): `Token parseLiteralString(Token token)`
    ///
    /// ```
    /// stringLiteral:
    ///   (multilineString | singleLineString)+
    /// ;
    /// ```
    pub fn parse_literal_string(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        let start_token = token;
        debug_assert!(self.kind(self.next(token)) == STRING_TOKEN);
        let old = self.may_parse_function_expressions;
        self.may_parse_function_expressions = true;
        token = self.parse_single_literal_string(token);
        let mut count: i32 = 1;
        while self.kind(self.next(token)) == STRING_TOKEN {
            token = self.parse_single_literal_string(token);
            count += 1;
        }
        if count > 1 {
            self.listener
                .handle_adjacent_string_literals(start_token, count);
        }
        self.may_parse_function_expressions = old;
        token
    }

    /// Dart (line 9212): `Token parseLiteralSymbol(Token token)`
    ///
    /// ```
    /// symbolLiteral:
    ///   '#' (operator | (identifier ('.' identifier)*))
    /// ;
    /// ```
    pub fn parse_literal_symbol(&mut self, token: TokenId) -> TokenId {
        let mut token = self.next(token);
        let hash_token = token;
        debug_assert!(self.is_a(hash_token, TokenType::HASH));
        self.listener.begin_literal_symbol(hash_token);
        let next = self.next(token);
        if self.is_user_definable_operator(next) {
            self.listener.handle_operator(next);
            self.listener
                .end_literal_symbol(hash_token, /* identifierCount = */ 1);
            next
        } else if self.is_a(next, Keyword::VOID) {
            self.listener.handle_symbol_void(next);
            self.listener
                .end_literal_symbol(hash_token, /* identifierCount = */ 1);
            next
        } else {
            let mut count: i32 = 1;
            token = self.ensure_identifier(token, IdentifierContext::LiteralSymbol);
            while self.is_a(self.next(token), TokenType::PERIOD) {
                count += 1;
                let next = self.next(token);
                token = self.ensure_identifier(next, IdentifierContext::LiteralSymbolContinuation);
            }
            self.listener.end_literal_symbol(hash_token, count);
            token
        }
    }

    /// Dart (line 9240): `Token parseSingleLiteralString(Token token)`
    pub fn parse_single_literal_string(&mut self, token: TokenId) -> TokenId {
        let mut token = self.next(token);
        debug_assert!(self.kind(token) == STRING_TOKEN);
        self.listener.begin_literal_string(token);
        // Parsing the prefix, for instance 'x of 'x${id}y${id}z'
        let mut interpolation_count: i32 = 0;
        let mut next = self.next(token);
        let mut kind = self.kind(next);
        while kind != EOF_TOKEN {
            if kind == STRING_INTERPOLATION_TOKEN {
                // Parsing ${expression}.
                let after = self.parse_expression(next);
                token = self.next(after);
                if !self.is_a(token, TokenType::CLOSE_CURLY_BRACKET) {
                    self.report_recoverable_error(token, diag::expected_but_got("}"));
                    token = self.end_group(next).unwrap();
                }
                self.listener
                    .handle_interpolation_expression(next, Some(token));
            } else if kind == STRING_INTERPOLATION_IDENTIFIER_TOKEN {
                // Parsing $identifier.
                token = self.parse_identifier_expression(next);
                self.listener
                    .handle_interpolation_expression(next, /* rightBracket = */ None);
            } else {
                break;
            }
            interpolation_count += 1;
            // Parsing the infix/suffix, for instance y and z' of 'x${id}y${id}z'
            token = self.parse_string_part(token);
            next = self.next(token);
            kind = self.kind(next);
        }
        self.listener.end_literal_string(interpolation_count, token);
        token
    }

    /// Dart (line 9277): `Token parseIdentifierExpression(Token token)`
    pub fn parse_identifier_expression(&mut self, token: TokenId) -> TokenId {
        let next = self.next(token);
        if self.kind(next) == KEYWORD_TOKEN && self.string_value(next) == Some("this") {
            self.listener
                .handle_this_expression(next, IdentifierContext::Expression);
            next
        } else {
            self.parse_send(
                token,
                IdentifierContext::Expression,
                ConstantPatternContext::None,
            )
        }
    }

    /// Dart (line 9297): `Token parseLiteralBool(Token token)`
    ///
    /// ```
    /// booleanLiteral:
    ///   'true' |
    ///   'false'
    /// ;
    /// ```
    pub fn parse_literal_bool(&mut self, token: TokenId) -> TokenId {
        let token = self.next(token);
        debug_assert!(self.is_a(token, Keyword::FALSE) || self.is_a(token, Keyword::TRUE));
        self.listener.handle_literal_bool(token);
        token
    }

    /// Dart (line 9309): `Token parseLiteralNull(Token token)`
    ///
    /// ```
    /// nullLiteral:
    ///   'null'
    /// ;
    /// ```
    pub fn parse_literal_null(&mut self, token: TokenId) -> TokenId {
        let token = self.next(token);
        debug_assert!(self.is_a(token, Keyword::NULL));
        self.listener.handle_literal_null(token);
        token
    }

    /// Dart (line 9316): `Token parseSend( Token token, IdentifierContext context, ConstantPatternContext constantPatternContext, )`
    pub fn parse_send(
        &mut self,
        token: TokenId,
        context: IdentifierContext,
        constant_pattern_context: ConstantPatternContext,
    ) -> TokenId {
        let mut token = token;
        let mut constant_pattern_context = constant_pattern_context;
        // Least-costly recovery of `Map<...>?{`, `Set<...>?{`, `List<...>[` and
        // `List<...>?[]`.
        // Note that we have to "peek" into the identifier because we don't want to
        // send an `handleIdentifier` if we end up recovering.
        let mut potential_type_arg: Option<TypeParamOrArgInfo> = None;
        let mut after_token: Option<TokenId> = None;
        let next = self.next(token);
        if self.kind(next) == IDENTIFIER_TOKEN {
            let identifier = next;
            let arg = compute_type_param_or_arg_mut(self.tokens_mut(), identifier, false, false);
            potential_type_arg = Some(arg);
            let after = { let skipped = arg.skip_mut(self.tokens_mut(), identifier); self.next(skipped) };
            after_token = Some(after);
            if self.is_a(after, TokenType::OPEN_CURLY_BRACKET) {
                let value = self.lexeme(identifier).to_string();
                if value == "Map" || value == "Set" {
                    // Recover by ignoring the `Map`/`Set` and parse as a literal map/set.
                    let message =
                        diag::literal_with_class(&value.to_lowercase(), self.lexeme(identifier));
                    self.report_recoverable_error(identifier, message);
                    return self.parse_primary(identifier, context, ConstantPatternContext::None);
                }
            } else if (arg != NO_TYPE_PARAM_OR_ARG
                && self.is_a(after, TokenType::OPEN_SQUARE_BRACKET))
                || self.is_a(after, TokenType::INDEX)
            {
                let value = self.lexeme(identifier).to_string();
                if value == "List" {
                    // Recover by ignoring the `List` and parse as a literal List.
                    // Note that we here require the `<...>` for `[` as `List[` would be
                    // an indexed expression. `List[]` wouldn't though, so we don't
                    // require it there.
                    let message =
                        diag::literal_with_class(&value.to_lowercase(), self.lexeme(identifier));
                    self.report_recoverable_error(identifier, message);
                    return self.parse_primary(identifier, context, ConstantPatternContext::None);
                }
            }
        }

        token = self.ensure_identifier(token, context);
        let begin_token = token;
        // Notice that we don't parse the bang (!) here as we do in many other
        // instances where we call computeMethodTypeArguments.
        // The reason is, that on a method call like "e.f!<int>()" we need the
        // "e.f" to become a "single unit" before processing the bang (!),
        // the type arguments and the arguments.
        // By not handling bang here we don't parse any of it, and the parser will
        // parse it correctly in a different recursion step.

        // Special-case [computeMethodTypeArguments] to re-use potentialTypeArg if
        // already computed.
        let potential_type_arg = match potential_type_arg {
            Some(arg) => arg,
            None => compute_type_param_or_arg_mut(self.tokens_mut(), token, false, false),
        };
        let after_token = match after_token {
            Some(after) => after,
            None => { let skipped = potential_type_arg.skip_mut(self.tokens_mut(), token); self.next(skipped) },
        };
        let type_arg =
            if self.is_a(after_token, TokenType::OPEN_PAREN) && !potential_type_arg.recovered() {
                potential_type_arg
            } else {
                NO_TYPE_PARAM_OR_ARG
            };

        if type_arg != NO_TYPE_PARAM_OR_ARG {
            token = type_arg.parse_arguments(token, self);
        } else {
            let next = self.next(token);
            self.listener.handle_no_type_arguments(next);
        }
        if constant_pattern_context == ConstantPatternContext::Explicit
            && !(self.is_a(self.next(token), TokenType::PERIOD)
                || self.is_a(self.next(token), TokenType::OPEN_PAREN)
                || self.is_a(self.next(token), TokenType::LT))
        {
            // For '.', '(' and '<' we might end up with a valid constant pattern,
            // i.e. a const constructor invocation, so we only report an error here
            // otherwise.
            self.report_recoverable_error(token, diag::invalid_constant_pattern_const_prefix());
            // Avoid subsequent errors.
            constant_pattern_context = ConstantPatternContext::None;
        }
        token = self.parse_arguments_opt(token);
        self.listener.handle_send(begin_token, token);
        token
    }

    /// Dart (line 9407): `Token skipArgumentsOpt(Token token)`
    pub fn skip_arguments_opt(&mut self, token: TokenId) -> TokenId {
        let next = self.next(token);
        self.listener.handle_no_arguments(next);
        if self.is_a(next, TokenType::OPEN_PAREN) {
            self.end_group(next).unwrap()
        } else {
            token
        }
    }

    /// Dart (line 9421): `Token parseArgumentsOptMetadata(Token token, bool hasTypeArguments)`
    ///
    /// Parse optional arguments specifically for metadata as metadata arguments
    /// has to follow the previous token without space.
    /// See also
    /// https://github.com/dart-lang/language/blob/master/accepted/future-releases/records/records-feature-specification.md#ambiguity-with-metadata-annotations
    pub fn parse_arguments_opt_metadata(
        &mut self,
        token: TokenId,
        has_type_arguments: bool,
    ) -> TokenId {
        let next = self.next(token);
        if !self.is_a(next, TokenType::OPEN_PAREN) {
            self.listener.handle_no_arguments(next);
            token
        } else if self.end(token) == self.char_offset(next) {
            self.parse_arguments(token)
        } else {
            // There is a '(', but it's not technically arguments to the metadata.
            // Decide if we should recover as if it is. This should only be done
            // if we know that it isn't a record type.
            if has_type_arguments {
                // Arguments are required, so parse as arguments anyway.
                self.report_recoverable_error(next, diag::metadata_space_before_parenthesis());
                return self.parse_arguments(token);
            }
            let start_paren = next;
            let end_paren = self.end_group(start_paren).unwrap();
            let after_paren = self.next(end_paren);
            let value = self.string_value(after_paren);
            if value == Some("class") || value == Some("enum") {
                // The 'class' and 'enum' keywords are reserved keywords and recovery
                // should be safe. Other keywords aren't reserved and needs more
                // lookahead to determine if recovery here would be good.
                //For now we don't.
                self.report_recoverable_error(next, diag::metadata_space_before_parenthesis());
                return self.parse_arguments(token);
            }

            self.listener.handle_no_arguments(next);
            token
        }
    }

    /// Dart (line 9455): `Token parseArgumentsOpt(Token token)`
    pub fn parse_arguments_opt(&mut self, token: TokenId) -> TokenId {
        let next = self.next(token);
        if !self.is_a(next, TokenType::OPEN_PAREN) {
            self.listener.handle_no_arguments(next);
            token
        } else {
            self.parse_arguments(token)
        }
    }

    /// Dart (line 9479): `Token parseArguments(Token token)`
    ///
    /// ```
    /// arguments:
    ///   '(' (argumentList ','?)? ')'
    /// ;
    ///
    /// argumentList:
    ///   namedArgument (',' namedArgument)* |
    ///   expressionList (',' namedArgument)*
    /// ;
    ///
    /// namedArgument:
    ///   label expression
    /// ;
    /// ```
    pub fn parse_arguments(&mut self, token: TokenId) -> TokenId {
        let next = self.next(token);
        self.parse_arguments_rest(next)
    }

    /// Dart (line 9484): `Token parseArgumentsRest(Token token)`
    ///
    /// Parses the rest of an arguments list, where [token] is the `(`.
    pub fn parse_arguments_rest(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        let begin = token;
        debug_assert!(self.is_a(begin, TokenType::OPEN_PAREN));
        self.listener.begin_arguments(begin);
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
            if self.is_a(self.next(next), TokenType::COLON) || /* recovery */
                self.is_a(next, TokenType::COLON)
            {
                let identifier =
                    self.ensure_identifier(token, IdentifierContext::NamedArgumentReference);
                token = self.next(identifier);
                colon = Some(token);
            }
            let mut expression_handled = false;
            let begin_expression_token = token;

            // For increased performance we'd prefer to shortcut common cases, but if
            // a subclass of the parser has a special implementation of
            // [parseExpression] (say, wanting to skip expressions) we can't do that.
            if self.allowed_to_shortcut_parse_expression() {
                let next1 = self.next(token);
                // TODO(jensj): Possibly also for STRING CLOSE_PAREN / STRING COMMA?
                if self.is_a(next1, TokenType::IDENTIFIER) {
                    let next2 = self.next(next1);
                    if self.is_a(next2, TokenType::COMMA)
                        || self.is_a(next2, TokenType::CLOSE_PAREN)
                    {
                        // Shortcut common cases:
                        // "IDENTIFIER COMMA" and "IDENTIFIER CLOSE_PAREN"
                        self.listener
                            .handle_identifier(next1, IdentifierContext::Expression);
                        self.listener.handle_no_type_arguments(next2);
                        self.listener.handle_no_arguments(next2);
                        self.listener.handle_send(next1, next1);
                        token = next1;
                        expression_handled = true;
                    } else if self.is_a(next2, TokenType::PERIOD) {
                        let next3 = self.next(next2);
                        if self.is_a(next3, TokenType::IDENTIFIER) {
                            let next4 = self.next(next3);
                            if self.is_a(next4, TokenType::COMMA)
                                || self.is_a(next4, TokenType::CLOSE_PAREN)
                            {
                                // Shortcut common cases:
                                // "IDENTIFIER DOT IDENTIFIER COMMA" and
                                // "IDENTIFIER DOT IDENTIFIER CLOSE_PAREN"
                                self.listener
                                    .handle_identifier(next1, IdentifierContext::Expression);
                                self.listener.handle_no_type_arguments(next2);
                                self.listener.handle_no_arguments(next2);
                                self.listener.handle_send(next1, next1);
                                self.listener.handle_identifier(
                                    next3,
                                    IdentifierContext::ExpressionContinuation,
                                );
                                self.listener.handle_no_type_arguments(next4);
                                self.listener.handle_no_arguments(next4);
                                self.listener.handle_send(next3, next3);
                                self.listener.handle_dot_access(
                                    next2, next3, /* isNullAware = */ false,
                                );
                                token = next3;
                                expression_handled = true;
                            }
                        }
                    }
                } else if self.is_a(next1, TokenType::STRING) {
                    let next2 = self.next(next1);
                    if self.is_a(next2, TokenType::COMMA)
                        || self.is_a(next2, TokenType::CLOSE_PAREN)
                    {
                        // Shortcut common cases:
                        // "STRING COMMA" and "STRING CLOSE_PAREN"
                        self.listener.begin_literal_string(next1);
                        self.listener.end_literal_string(0, next1);
                        token = next1;
                        expression_handled = true;
                    }
                }
            }
            if !expression_handled {
                token = self.parse_expression(token);
            }
            next = self.next(token);
            if let Some(colon) = colon {
                self.listener.handle_named_argument(colon);
            } else {
                let arg = self.next(begin_expression_token);
                self.listener.handle_positional_argument(arg);
            }
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
        debug_assert!(self.is_a(token, TokenType::CLOSE_PAREN));
        self.may_parse_function_expressions = old;
        self.listener.end_arguments(argument_count, begin, token);
        token
    }

    /// Dart (line 9610): `Token parseIsOperatorRest(Token token)`
    ///
    /// ```
    /// typeTest::
    ///   'is' '!'? type
    /// ;
    /// ```
    pub fn parse_is_operator_rest(&mut self, token: TokenId) -> TokenId {
        let mut token = self.next(token);
        let operator = token;
        debug_assert!(self.is_a(operator, Keyword::IS));
        let mut not: Option<TokenId> = None;
        if self.is_a(self.next(token), TokenType::BANG) {
            token = self.next(token);
            not = Some(token);
        }
        self.listener.begin_is_operator_type(operator);
        let type_info = self.compute_type_after_is_or_as(token);
        token = type_info.ensure_type_not_void(token, self);
        self.listener.end_is_operator_type(operator);
        self.listener.handle_is_operator(operator, not);
        self.skip_chained_as_is_operators(token)
    }

    /// Dart (line 9625): `TypeInfo computeTypeAfterIsOrAs(Token token)`
    pub fn compute_type_after_is_or_as(&mut self, token: TokenId) -> TypeInfo {
        let mut type_info = compute_type_mut(self.tokens_mut(),
            token,
            /* required = */ true,
            false,
            false,
        );
        if type_info.is_nullable() {
            let skip_token = type_info.skip_type_mut(self.tokens_mut(), token);
            let next = self.next(skip_token);
            if self.is_a(next, TokenType::CLOSE_PAREN)
                || self.is_a(next, TokenType::CLOSE_CURLY_BRACKET)
                || self.is_a(next, TokenType::CLOSE_SQUARE_BRACKET)
                || self.is_a(next, TokenType::QUESTION)
                || self.is_a(next, TokenType::QUESTION_QUESTION)
                || self.is_a(next, TokenType::COMMA)
                || self.is_a(next, TokenType::SEMICOLON)
                || self.is_a(next, TokenType::COLON)
                || self.is_a(next, Keyword::IS)
                || self.is_a(next, Keyword::AS)
                || self.is_a(next, TokenType::PERIOD_PERIOD)
                || self.is_a(next, TokenType::BAR_BAR)
                || self.is_a(next, TokenType::AMPERSAND_AMPERSAND)
                || self.is_a(next, TokenType::EOF)
            {
                // TODO(danrubel): investigate other situations
                // where `?` should be considered part of the type info
                // rather than the start of a conditional expression.
                return type_info;
            }
            if self.is_a(next, TokenType::OPEN_CURLY_BRACKET) || self.is_a(next, Keyword::WHEN) {
                // <expression> is/as <type> ? {
                //   This could be either a nullable type (e.g. last initializer in a
                //   constructor with a body), or a non-nullable type and a conditional.
                // <expression> is/as <type> ? when
                //   This could be either a nullable type (e.g. a cast pattern followed
                //   by a guard), or a non-nullable type and a conditional (where the
                //   first token of the "then" expression is the identifier `when`).
                // If it can be successfully parsed as a conditional, we do so.
                let is_conditional = self.can_parse_as_conditional(skip_token);
                if !is_conditional {
                    return type_info;
                }
            }
            type_info = type_info.as_non_nullable();
        }
        type_info
    }

    /// Dart (line 9673): `Token parseAsOperatorRest(Token token)`
    ///
    /// ```
    /// typeCast:
    ///   'as' type
    /// ;
    /// ```
    pub fn parse_as_operator_rest(&mut self, token: TokenId) -> TokenId {
        let mut token = self.next(token);
        let operator = token;
        debug_assert!(self.is_a(operator, Keyword::AS));
        self.listener.begin_as_operator_type(operator);
        let type_info = self.compute_type_after_is_or_as(token);
        token = type_info.ensure_type_not_void(token, self);
        self.listener.end_as_operator_type(operator);
        self.listener.handle_as_operator(operator);
        self.skip_chained_as_is_operators(token)
    }

    /// Dart (line 9684): `Token skipChainedAsIsOperators(Token token)`
    pub fn skip_chained_as_is_operators(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        loop {
            let mut next = self.next(token);
            let mut value = self.string_value(next);
            if value != Some("is") && value != Some("as") {
                return token;
            }
            // The is- and as-operators cannot be chained.
            // TODO(danrubel): Consider a better error message.
            self.report_recoverable_error_with_token(next, diag::unexpected_token);
            if self.is_a(self.next(next), TokenType::BANG) {
                next = self.next(next);
            }
            let type_info = self.compute_type_after_is_or_as(next);
            token = type_info.skip_type_mut(self.tokens_mut(), next);
            next = self.next(token);
            value = self.string_value(next);
        }
    }

    /// Dart (line 9706): `bool looksLikeLocalFunction(Token token)`
    ///
    /// Returns true if [token] could be the start of a function declaration
    /// without a return type.
    pub fn looks_like_local_function(&mut self, token: TokenId) -> bool {
        let mut token = token;
        if self.is_identifier(token) {
            if self.is_a(self.next(token), TokenType::LT) {
                let type_param = compute_type_param_or_arg_mut(self.tokens_mut(), token, false, false);
                if type_param == NO_TYPE_PARAM_OR_ARG {
                    return false;
                }
                token = type_param.skip_mut(self.tokens_mut(), token);
            }
            token = self.next(token);
            if self.is_a(token, TokenType::OPEN_PAREN) {
                token = self.next(self.end_group(token).unwrap());
                return self.is_a(token, TokenType::OPEN_CURLY_BRACKET)
                    || self.is_a(token, TokenType::FUNCTION)
                    || self.is_a(token, Keyword::ASYNC)
                    || self.is_a(token, Keyword::SYNC);
            } else if self.is_a(token, TokenType::FUNCTION) {
                // Recovery: Looks like a local function that is missing parenthesis.
                return true;
            }
        }
        false
    }

    /// Dart (line 9731): `bool looksLikeFunctionBody(Token token)`
    ///
    /// Returns true if [token] could be the start of a function body.
    pub fn looks_like_function_body(&mut self, token: TokenId) -> bool {
        self.is_a(token, TokenType::OPEN_CURLY_BRACKET)
            || self.is_a(token, TokenType::FUNCTION)
            || self.is_a(token, Keyword::ASYNC)
            || self.is_a(token, Keyword::SYNC)
    }

    /// Dart (line 9738): `Token parseExpressionStatementOrConstDeclaration(Token start)`
    pub fn parse_expression_statement_or_const_declaration(&mut self, start: TokenId) -> TokenId {
        let const_token = self.next(start);
        debug_assert!(self.is_a(const_token, Keyword::CONST));
        if !is_modifier(self.tokens(), self.next(const_token)) {
            let type_info = compute_type_mut(self.tokens_mut(),
                const_token,
                /* required = */ false,
                false,
                false,
            );
            if type_info == NO_TYPE {
                let mut next = self.next(const_token);
                if !self.is_identifier(next) {
                    return self.parse_expression_statement(start);
                }
                next = self.next(next);
                if !(self.is_a(next, TokenType::EQ) ||
                    // Recovery
                    self.is_keyword_or_identifier(next)
                    || self.is_a(next, TokenType::SEMICOLON)
                    || self.is_a(next, TokenType::COMMA)
                    || self.is_a(next, TokenType::OPEN_CURLY_BRACKET))
                {
                    return self.parse_expression_statement(start);
                }
            }
            return self.parse_expression_statement_or_declaration_after_modifiers(
                const_token,
                start,
                /* lateToken = */ None,
                Some(const_token),
                Some(&type_info),
                None,
            );
        }
        self.parse_expression_statement_or_declaration(start, None)
    }

    /// Dart (line 9786): `Token parseExpressionStatementOrDeclaration( Token start, [ ForPartsContext? forPartsContext, ])`
    ///
    /// This method has two modes based upon [forPartsContext].
    ///
    /// If [forPartsContext] is `null` (the default), then the parser is currently
    /// processing a statement or declaration.  This method will parse a local
    /// variable declaration, a local function, or an expression statement, and
    /// then return the last consumed token.
    ///
    /// If [forPartsContext] is non-null, then this method will only parse the
    /// metadata, modifiers, and type of a local variable declaration if it
    /// exists; it is the responsibility of the caller to call
    /// [parseVariablesDeclarationRest] to finish parsing the local variable
    /// declaration.  Or it will parse the metadata, `var` or `final` keyword, and
    /// pattern of a pattern variable declaration, and store the `var` or `final`
    /// keyword in [forPartsContext]; it is the responsibility of the caller to
    /// consume the rest of the pattern variable declaration.  Or, if neither a
    /// local variable declaration nor a pattern variable declaration is found,
    /// then this method will return [start].
    pub fn parse_expression_statement_or_declaration(
        &mut self,
        start: TokenId,
        for_parts_context: Option<&mut ForPartsContext>,
    ) -> TokenId {
        let mut token = start;
        let mut next = self.next(token);
        if self.is_a(next, TokenType::AT) {
            token = self.parse_metadata_star(token);
            next = self.next(token);
        }

        let mut late_token: Option<TokenId> = None;
        let mut var_final_or_const: Option<TokenId> = None;

        if is_modifier(self.tokens(), next) {
            if self.is_a(next, Keyword::AUGMENT) && self.is_a(self.next(next), Keyword::SUPER) {
                return self.parse_expression_statement(start);
            } else if self.is_a(next, Keyword::VAR)
                || self.is_a(next, Keyword::FINAL)
                || self.is_a(next, Keyword::CONST)
            {
                token = self.next(token);
                var_final_or_const = Some(token);
                next = self.next(token);
            } else if self.is_a(next, Keyword::LATE) {
                token = next;
                late_token = Some(next);
                next = self.next(token);
                if is_modifier(self.tokens(), next)
                    && (self.is_a(next, Keyword::VAR) || self.is_a(next, Keyword::FINAL))
                {
                    token = next;
                    var_final_or_const = Some(next);
                    next = self.next(token);
                }
            }

            if is_modifier(self.tokens(), next) {
                // Recovery
                let mut context = ModifierContext::new();
                context.late_token = late_token;
                context.set_var_final_or_const(self.tokens(), var_final_or_const);

                token = context.parse_variable_declaration_modifiers(self, token);
                next = self.next(token);

                late_token = context.late_token;
                var_final_or_const = context.var_final_or_const();
            }
        }

        self.parse_expression_statement_or_declaration_after_modifiers(
            token,
            start,
            late_token,
            var_final_or_const,
            /* typeInfo = */ None,
            for_parts_context,
        )
    }

    /// Dart (line 9847): `Token parseExpressionStatementOrDeclarationAfterModifiers( Token beforeType, Token start, Token? lateToken, Token? varFinalOrConst, TypeInfo? typeInfo, [ ForPartsContext? forPartsContext, ])`
    ///
    /// See [parseExpressionStatementOrDeclaration].
    ///
    /// If `start.next` is an `@` token (i.e. this is a declaration with metadata)
    /// then the caller should parse it before calling this method; otherwise,
    /// this method will handle the lack of metadata appropriately.
    pub fn parse_expression_statement_or_declaration_after_modifiers(
        &mut self,
        before_type: TokenId,
        start: TokenId,
        late_token: Option<TokenId>,
        var_final_or_const: Option<TokenId>,
        type_info: Option<&TypeInfo>,
        for_parts_context: Option<&mut ForPartsContext>,
    ) -> TokenId {
        let mut for_parts_context = for_parts_context;
        if self.is_patterns_feature_enabled
            && let Some(var_final_or_const) = var_final_or_const
            && (self.is_a(var_final_or_const, Keyword::VAR)
                || self.is_a(var_final_or_const, Keyword::FINAL))
        {
            let after_outer_pattern = self.skip_outer_pattern(before_type);
            if let Some(after_outer_pattern) = after_outer_pattern
                && (self.is_a(self.next(after_outer_pattern), TokenType::EQ)
                    || (for_parts_context.is_some()
                        && self.is_a(self.next(after_outer_pattern), Keyword::IN)))
            {
                if let Some(late_token) = late_token {
                    self.report_recoverable_error(
                        late_token,
                        diag::late_pattern_variable_declaration(),
                    );
                }
                // If there was any metadata, then the caller was responsible for
                // parsing it; if not, then we need to let the listener know there
                // wasn't any.
                if !self.is_a(self.next(start), TokenType::AT) {
                    let start_next = self.next(start);
                    self.listener.begin_metadata_star(start_next);
                    self.listener.end_metadata_star(/* count = */ 0);
                }
                if let Some(for_parts_context) = for_parts_context {
                    for_parts_context.pattern_keyword = Some(var_final_or_const);
                    return self.parse_pattern(before_type, PatternContext::Declaration, 1);
                } else {
                    return self.parse_pattern_variable_declaration_statement(
                        before_type,
                        start,
                        var_final_or_const,
                    );
                }
            }
        }

        let mut type_info: TypeInfo = match type_info {
            Some(type_info) => type_info.clone(),
            None => compute_type_mut(self.tokens_mut(),
                before_type,
                /* required = */ false,
                false,
                false,
            ),
        };

        let mut token = type_info.skip_type_mut(self.tokens_mut(), before_type);
        let mut next = self.next(token);

        if for_parts_context.is_some() {
            if let Some(late_token) = late_token {
                self.report_recoverable_error_with_token(late_token, diag::extraneous_modifier);
            }
        } else {
            if self.looks_like_local_function(next) {
                // Parse a local function declaration.
                if let Some(var_final_or_const) = var_final_or_const {
                    self.report_recoverable_error_with_token(
                        var_final_or_const,
                        diag::extraneous_modifier,
                    );
                } else if let Some(late_token) = late_token {
                    self.report_recoverable_error_with_token(late_token, diag::extraneous_modifier);
                }
                // If there was any metadata, then the caller was responsible for
                // parsing it; if not, then we need to let the listener know there
                // wasn't any.
                if !self.is_a(self.next(start), TokenType::AT) {
                    let start_next = self.next(start);
                    self.listener.begin_metadata_star(start_next);
                    self.listener.end_metadata_star(/* count = */ 0);
                }
                let before_formals = compute_type_param_or_arg_mut(self.tokens_mut(), next, false, false)
                    .parse_variables(next, self);
                let start_next = self.next(start);
                self.listener.begin_local_function_declaration(start_next);
                token = type_info.parse_type(before_type, self);
                let start_next = self.next(start);
                return self.parse_named_function_rest(
                    token,
                    start_next,
                    before_formals,
                    /* isFunctionExpression = */ false,
                );
            }
        }

        if before_type == start && type_info.is_nullable() && type_info.could_be_expression() {
            debug_assert!(self.is_a(token, TokenType::QUESTION));
            if !looks_like_name(self.tokens(), next) {
                self.report_recoverable_error(next, diag::expected_identifier(self.lexeme(next)));
                next = self.rewriter().insert_synthetic_identifier(next, "");
            }
            let after_identifier = self.next(next);
            //
            // found <typeref> `?` <identifier>
            // with no annotations or modifiers preceding it
            //
            if self.is_a(after_identifier, TokenType::EQ) {
                //
                // look past the next expression
                // to determine if this is part of a conditional expression
                //
                self.push_null_listener();
                let original_rewriter = self.begin_undoable_rewriter();
                let after = self.parse_expression_without_cascade(after_identifier);
                let after_expression = self.next(after);
                // Undo all changes and reset.
                self.end_undoable_rewriter(original_rewriter);
                self.pop_null_listener();

                if self.is_a(after_expression, TokenType::COLON) {
                    // Looks like part of a conditional expression.
                    // Drop the type information and reset the last consumed token.
                    type_info = NO_TYPE;
                    token = start;
                    next = self.next(token);
                }
            } else if !self.is_keyword(after_identifier)
                && !(self.is_a(after_identifier, TokenType::SEMICOLON)
                    || self.is_a(after_identifier, TokenType::COMMA)
                    || self.is_a(after_identifier, TokenType::CLOSE_PAREN)
                    || self.is_a(after_identifier, TokenType::EOF))
            {
                // Looks like part of a conditional expression.
                // Drop the type information and reset the last consumed token.
                type_info = NO_TYPE;
                token = start;
                next = self.next(token);
            }
        }

        if token == start {
            // If no annotation, modifier, or type, and this is not a local function
            // then this must be an expression statement.
            if for_parts_context.is_some() {
                return start;
            } else {
                return self.parse_expression_statement(start);
            }
        }

        if self.is_built_in(next) && before_type == start && type_info.could_be_expression() {
            // Detect expressions such as identifier `as` identifier
            // and treat those as expressions.
            if self.is_a(next, Keyword::AS) || self.is_a(next, Keyword::IS) {
                let kind = self.kind(self.next(next));
                if EQ_TOKEN != kind && SEMICOLON_TOKEN != kind && COMMA_TOKEN != kind {
                    if for_parts_context.is_some() {
                        if !self.is_a(self.next(next), Keyword::IN) {
                            return start;
                        }
                    } else {
                        return self.parse_expression_statement(start);
                    }
                }
            }
        }

        if self.is_identifier(next) {
            // Only report these errors if there is an identifier. If there is not an
            // identifier, then allow ensureIdentifier to report an error
            // and don't report errors here.
            match var_final_or_const {
                None => {
                    if type_info == NO_TYPE {
                        self.report_recoverable_error(
                            next,
                            diag::missing_const_final_var_or_type(),
                        );
                    }
                }
                Some(var_final_or_const) => {
                    if self.is_a(var_final_or_const, Keyword::VAR) {
                        if type_info != NO_TYPE {
                            self.report_recoverable_error(
                                var_final_or_const,
                                diag::type_after_var(),
                            );
                        }
                    }
                }
            }
        }

        // If there was any metadata, then the caller was responsible for parsing
        // it; if not, then we need to let the listener know there wasn't any.
        if !self.is_a(self.next(start), TokenType::AT) {
            let start_next = self.next(start);
            self.listener.begin_metadata_star(start_next);
            self.listener.end_metadata_star(/* count = */ 0);
        }
        // Having settled on a variable declaration possibly do some error recovery.
        if self.is_a(self.next(before_type), TokenType::LT) {
            // E.g. `final <int> foo = [42];` where we're missing `List` before
            // `<int>`.
            let message = diag::expected_identifier(self.lexeme(self.next(before_type)));
            self.insert_synthetic_identifier(
                before_type,
                IdentifierContext::LocalVariableDeclaration,
                Some(message),
                None,
            );
            type_info = compute_type_mut(self.tokens_mut(),
                before_type,
                /* required = */ true,
                false,
                false,
            );
        }
        token = type_info.parse_type(before_type, self);
        next = self.next(token);
        self.listener
            .begin_variables_declaration(next, late_token, var_final_or_const);
        if for_parts_context.is_none() {
            token =
                self.parse_variables_declaration_rest(token, /* endWithSemicolon = */ true);
        }
        token
    }

    /// Dart (line 10063): `Token parseVariablesDeclarationRest(Token token, bool endWithSemicolon)`
    pub fn parse_variables_declaration_rest(
        &mut self,
        token: TokenId,
        end_with_semicolon: bool,
    ) -> TokenId {
        let mut token = token;
        let mut count: i32 = 1;
        token = self.parse_optionally_initialized_identifier(token);
        while self.is_a(self.next(token), TokenType::COMMA) {
            let next = self.next(token);
            token = self.parse_optionally_initialized_identifier(next);
            count += 1;
        }
        if end_with_semicolon {
            let semicolon = self.ensure_semicolon(token);
            self.listener
                .end_variables_declaration(count, Some(semicolon));
            semicolon
        } else {
            self.listener
                .end_variables_declaration(count, /* endToken = */ None);
            token
        }
    }

    /// Dart (line 10080): `Token parseOptionallyInitializedIdentifier(Token token)`
    pub fn parse_optionally_initialized_identifier(&mut self, token: TokenId) -> TokenId {
        let name_token = self.ensure_identifier(token, IdentifierContext::LocalVariableDeclaration);
        self.listener.begin_initialized_identifier(name_token);
        let token = self.parse_variable_initializer_opt(name_token);
        self.listener.end_initialized_identifier(name_token);
        token
    }

    /// Dart (line 10096): `Token parseIfStatement(Token token)`
    ///
    /// ```
    /// ifStatement:
    ///   'if' '(' expression ')' statement ('else' statement)?
    /// ;
    /// ```
    pub fn parse_if_statement(&mut self, token: TokenId) -> TokenId {
        let if_token = self.next(token);
        debug_assert!(self.is_a(if_token, Keyword::IF));
        self.listener.begin_if_statement(if_token);
        let allow_case = self.is_patterns_feature_enabled;
        let mut token = self.ensure_parenthesized_condition(if_token, allow_case);
        let then_begin_token = self.next(token);
        self.listener.begin_then_statement(then_begin_token);
        token = self.parse_statement(token);
        self.listener.end_then_statement(then_begin_token, token);
        let mut else_token: Option<TokenId> = None;
        if self.is_a(self.next(token), Keyword::ELSE) {
            let else_tok = self.next(token);
            else_token = Some(else_tok);
            self.listener.begin_else_statement(else_tok);
            token = self.parse_statement(else_tok);
            self.listener.end_else_statement(else_tok, token);
        }
        self.listener.end_if_statement(if_token, else_token, token);
        token
    }

    /// Dart (line 10137): `Token parseForStatement(Token token, Token? awaitToken)`
    ///
    /// ```
    /// forStatement:
    ///   'await'? 'for' '(' forLoopParts ')' statement
    /// ;
    ///
    ///  forLoopParts:
    ///      localVariableDeclaration ';' expression? ';' expressionList?
    ///    | expression? ';' expression? ';' expressionList?
    ///    | localVariableDeclaration 'in' expression
    ///    | identifier 'in' expression
    ///    | metadata ( 'final' | 'var' ) outerPattern 'in' expression
    /// ;
    ///
    /// forInitializerStatement:
    ///   localVariableDeclaration |
    ///   expression? ';'
    /// ;
    /// ```
    pub fn parse_for_statement(&mut self, token: TokenId, await_token: Option<TokenId>) -> TokenId {
        let mut token = self.next(token);
        let for_token = token;
        debug_assert!(await_token.is_none_or(|t| self.is_a(t, Keyword::AWAIT)));
        debug_assert!(self.is_a(token, Keyword::FOR));
        self.listener.begin_for_statement(for_token);

        let mut for_parts_context = ForPartsContext::default();
        token = self.parse_for_loop_parts_start(await_token, for_token, &mut for_parts_context);
        let pattern_keyword = for_parts_context.pattern_keyword;
        if let Some(pattern_keyword) = pattern_keyword {
            if self.is_a(self.next(token), TokenType::EQ) {
                // Process `for ( pattern = expression ; ... ; ... )`
                let equals = self.next(token);
                token = self.parse_expression(equals);
                self.listener
                    .handle_for_initializer_pattern_variable_assignment(pattern_keyword, equals);
                return self.parse_for_rest(await_token, token, for_token);
            } else {
                // Process `for ( pattern in expression )`
                debug_assert!(self.is_a(self.next(token), Keyword::IN));
                return self.parse_for_in_rest(
                    token,
                    await_token,
                    for_token,
                    Some(pattern_keyword),
                    /* identifier = */ None,
                );
            }
        }
        let identifier = self.next(token);
        token = self.parse_for_loop_parts_mid(token, await_token, for_token);
        if self.is_a(self.next(token), Keyword::IN) || self.is_a(self.next(token), TokenType::COLON)
        {
            // Process `for ( ... in ... )`
            self.parse_for_in_rest(
                token,
                await_token,
                for_token,
                /* patternKeyword = */ None,
                Some(identifier),
            )
        } else {
            // Process `for ( ... ; ... ; ... )`
            self.parse_for_rest(await_token, token, for_token)
        }
    }

    /// Dart (line 10187): `Token parseForLoopPartsStart( Token? awaitToken, Token forToken, ForPartsContext forPartsContext, )`
    ///
    /// Parse the start of a for loop control structure
    /// from the open parenthesis up to but not including the identifier.
    pub fn parse_for_loop_parts_start(
        &mut self,
        await_token: Option<TokenId>,
        for_token: TokenId,
        for_parts_context: &mut ForPartsContext,
    ) -> TokenId {
        let mut left_parenthesis = self.next(for_token);
        if !self.is_a(left_parenthesis, TokenType::OPEN_PAREN) {
            // Recovery
            self.report_recoverable_error(left_parenthesis, diag::expected_but_got("("));

            let offset = self.offset(left_parenthesis);
            let byte = self.tokens().byte_offset(left_parenthesis);
            let synthetic_open =
                self.tokens_mut()
                    .push_synthetic(TokenType::OPEN_PAREN, offset, byte);
            let open_paren = self.rewriter().insert_token(for_token, synthetic_open);

            let mut token;
            if await_token.is_some() {
                token = self.rewriter().insert_synthetic_identifier(open_paren, "");
                token = self.rewriter().insert_synthetic_keyword(token, Keyword::IN);
                token = self.rewriter().insert_synthetic_identifier(token, "");
            } else {
                token = self
                    .rewriter()
                    .insert_synthetic_token(open_paren, TokenType::SEMICOLON);
                token = self
                    .rewriter()
                    .insert_synthetic_token(token, TokenType::SEMICOLON);
            }

            let synthetic_close =
                self.tokens_mut()
                    .push_synthetic(TokenType::CLOSE_PAREN, offset, byte);
            token = self.rewriter().insert_token(token, synthetic_close);
            self.tokens_mut().get_mut(open_paren).end_group = token;

            token = self.rewriter().insert_synthetic_identifier(token, "");
            self.rewriter()
                .insert_synthetic_token(token, TokenType::SEMICOLON);

            left_parenthesis = open_paren;
        }

        // Pass `true` so that the [parseExpressionStatementOrDeclaration] only
        // parses the metadata, modifiers, and type of a local variable
        // declaration if it exists. This enables capturing [beforeIdentifier]
        // for later error reporting.
        self.parse_expression_statement_or_declaration(left_parenthesis, Some(for_parts_context))
    }

    /// Dart (line 10243): `Token parseForLoopPartsMid(Token token, Token? awaitToken, Token forToken)`
    ///
    /// Parse the remainder of the local variable declaration
    /// or an expression if no local variable declaration was found.
    pub fn parse_for_loop_parts_mid(
        &mut self,
        token: TokenId,
        await_token: Option<TokenId>,
        for_token: TokenId,
    ) -> TokenId {
        let mut token = token;
        if token != self.next(for_token) {
            token =
                self.parse_variables_declaration_rest(token, /* endWithSemicolon = */ false);
            let for_in = self.is_a(self.next(token), Keyword::IN)
                || self.is_a(self.next(token), TokenType::COLON);
            self.listener
                .handle_for_initializer_local_variable_declaration(token, for_in);
        } else if self.is_a(self.next(token), TokenType::SEMICOLON) {
            let next = self.next(token);
            self.listener.handle_for_initializer_empty_statement(next);
        } else {
            token = self.parse_expression(token);
            let for_in = self.is_a(self.next(token), Keyword::IN)
                || self.is_a(self.next(token), TokenType::COLON)
                // If this is an empty `await for`, we rewrite it into an
                // `await for (_ in _)`.
                || (await_token.is_some() && self.is_a(self.next(token), TokenType::CLOSE_PAREN));
            self.listener
                .handle_for_initializer_expression_statement(token, for_in);
        }
        let next = self.next(token);
        if self.is_a(next, TokenType::SEMICOLON) {
            if let Some(await_token) = await_token {
                self.report_recoverable_error(await_token, diag::invalid_await_for());
            }
        } else if !self.is_a(next, Keyword::IN) {
            // Recovery
            if self.is_a(next, TokenType::COLON) {
                self.report_recoverable_error(next, diag::colon_in_place_of_in());
            } else if await_token.is_some() {
                self.report_recoverable_error(next, diag::expected_but_got("in"));
                let offset = self.offset(next);
                let byte = self.tokens().byte_offset(next);
                let in_keyword = self.tokens_mut().push_synthetic(Keyword::IN, offset, byte);
                self.tokens_mut().set_next(in_keyword, next);
                self.tokens_mut().set_next(token, in_keyword);
            }
        }
        token
    }

    /// Dart (line 10299): `Token parseForRest(Token? awaitToken, Token token, Token forToken)`
    ///
    /// This method parses the portion of the forLoopParts that starts with the
    /// first semicolon (the one that terminates the forInitializerStatement).
    ///
    /// ```
    ///  forLoopParts:
    ///      localVariableDeclaration ';' expression? ';' expressionList?
    ///    | expression? ';' expression? ';' expressionList?
    ///    | localVariableDeclaration 'in' expression
    ///    | identifier 'in' expression
    /// ;
    /// ```
    pub fn parse_for_rest(
        &mut self,
        await_token: Option<TokenId>,
        token: TokenId,
        for_token: TokenId,
    ) -> TokenId {
        let mut token = self.parse_for_loop_parts_rest(token, for_token, await_token);
        let next = self.next(token);
        self.listener.begin_for_statement_body(next);
        let saved_loop_state = self.loop_state;
        self.loop_state = LoopState::InsideLoop;
        token = self.parse_statement(token);
        self.loop_state = saved_loop_state;
        self.listener.end_for_statement_body(token);
        self.listener.end_for_statement(token);
        token
    }

    /// Dart (line 10311): `Token parseForLoopPartsRest(Token token, Token forToken, Token? awaitToken)`
    pub fn parse_for_loop_parts_rest(
        &mut self,
        token: TokenId,
        for_token: TokenId,
        await_token: Option<TokenId>,
    ) -> TokenId {
        let mut token = token;
        let left_parenthesis = self.next(for_token);
        debug_assert!(self.is_a(for_token, Keyword::FOR));
        debug_assert!(self.is_a(left_parenthesis, TokenType::OPEN_PAREN));

        let left_separator = self.ensure_semicolon(token);
        if self.is_a(self.next(left_separator), TokenType::SEMICOLON) {
            token = self.parse_empty_statement(left_separator);
        } else {
            token = self.parse_expression_statement(left_separator);
        }
        let right_separator = token;
        let mut expression_count: i32 = 0;
        loop {
            let next = self.next(token);
            if self.is_a(next, TokenType::CLOSE_PAREN) {
                token = next;
                break;
            }
            let after = self.parse_expression(token);
            token = self.next(after);
            expression_count += 1;
            if !self.is_a(token, TokenType::COMMA) {
                break;
            }
        }
        if Some(token) != self.end_group(left_parenthesis) {
            self.report_recoverable_error_with_token(token, diag::unexpected_token);
            token = self.end_group(left_parenthesis).unwrap();
        }
        self.listener.handle_for_loop_parts(
            for_token,
            left_parenthesis,
            left_separator,
            right_separator,
            expression_count,
        );
        token
    }

    /// Dart (line 10363): `Token parseForInRest( Token token, Token? awaitToken, Token forToken, Token? patternKeyword, Token? identifier, )`
    ///
    /// This method parses the portion of the forLoopParts that starts with the
    /// keyword 'in'. For the sake of recovery, we accept a colon in place of the
    /// keyword.
    ///
    /// ```
    ///  forLoopParts:
    ///      localVariableDeclaration ';' expression? ';' expressionList?
    ///    | expression? ';' expression? ';' expressionList?
    ///    | localVariableDeclaration 'in' expression
    ///    | identifier 'in' expression
    ///    | metadata ( 'final' | 'var' ) outerPattern 'in' expression
    /// ;
    /// ```
    pub fn parse_for_in_rest(
        &mut self,
        token: TokenId,
        await_token: Option<TokenId>,
        for_token: TokenId,
        pattern_keyword: Option<TokenId>,
        identifier: Option<TokenId>,
    ) -> TokenId {
        let mut token = self.parse_for_in_loop_parts_rest(
            token,
            await_token,
            for_token,
            pattern_keyword,
            identifier,
        );
        let next = self.next(token);
        self.listener.begin_for_in_body(next);
        let saved_loop_state = self.loop_state;
        self.loop_state = LoopState::InsideLoop;
        token = self.parse_statement(token);
        self.loop_state = saved_loop_state;
        self.listener.end_for_in_body(token);
        self.listener.end_for_in(token);
        token
    }

    /// Dart (line 10387): `Token parseForInLoopPartsRest( Token token, Token? awaitToken, Token forToken, Token? patternKeyword, Token? identifier, )`
    pub fn parse_for_in_loop_parts_rest(
        &mut self,
        token: TokenId,
        await_token: Option<TokenId>,
        for_token: TokenId,
        pattern_keyword: Option<TokenId>,
        identifier: Option<TokenId>,
    ) -> TokenId {
        let mut token = token;
        let in_keyword = self.next(token);
        debug_assert!(self.is_a(for_token, Keyword::FOR));
        debug_assert!(self.is_a(self.next(for_token), TokenType::OPEN_PAREN));
        debug_assert!(
            self.is_a(in_keyword, Keyword::IN) || self.is_a(in_keyword, TokenType::COLON)
        );

        if let Some(await_token) = await_token
            && !self.in_async()
        {
            self.report_recoverable_error(await_token, diag::await_for_not_async());
        }

        if let Some(identifier) = identifier {
            if !self.is_identifier(identifier) {
                // TODO(jensj): This should probably (sometimes) be
                // codeExpectedIdentifierButGotKeyword instead.
                self.report_recoverable_error_with_token(identifier, diag::expected_identifier);
            } else if identifier != token {
                let identifier_next = self.next(identifier);
                if self.is_a(identifier_next, TokenType::EQ) {
                    self.report_recoverable_error(
                        identifier_next,
                        diag::initialized_variable_in_for_each(),
                    );
                } else {
                    self.report_recoverable_error_with_token(
                        identifier_next,
                        diag::unexpected_token,
                    );
                }
            }
        }
        let in_next = self.next(in_keyword);
        self.listener.begin_for_in_expression(in_next);
        token = self.parse_expression(in_keyword);
        let for_next = self.next(for_token);
        token = self.ensure_close_paren(token, for_next);
        self.listener.end_for_in_expression(token);
        let for_next = self.next(for_token);
        self.listener.handle_for_in_loop_parts(
            await_token,
            for_token,
            for_next,
            pattern_keyword,
            in_keyword,
        );
        token
    }

    /// Dart (line 10441): `Token parseWhileStatement(Token token)`
    ///
    /// ```
    /// whileStatement:
    ///   'while' '(' expression ')' statement
    /// ;
    /// ```
    pub fn parse_while_statement(&mut self, token: TokenId) -> TokenId {
        let while_token = self.next(token);
        debug_assert!(self.is_a(while_token, Keyword::WHILE));
        self.listener.begin_while_statement(while_token);
        let mut token =
            self.ensure_parenthesized_condition(while_token, /* allowCase = */ false);
        let next = self.next(token);
        self.listener.begin_while_statement_body(next);
        let saved_loop_state = self.loop_state;
        self.loop_state = LoopState::InsideLoop;
        token = self.parse_statement(token);
        self.loop_state = saved_loop_state;
        self.listener.end_while_statement_body(token);
        self.listener.end_while_statement(while_token, token);
        token
    }

    /// Dart (line 10461): `Token parseDoWhileStatement(Token token)`
    ///
    /// ```
    /// doStatement:
    ///   'do' statement 'while' '(' expression ')' ';'
    /// ;
    /// ```
    pub fn parse_do_while_statement(&mut self, token: TokenId) -> TokenId {
        let do_token = self.next(token);
        debug_assert!(self.is_a(do_token, Keyword::DO));
        self.listener.begin_do_while_statement(do_token);
        let do_next = self.next(do_token);
        self.listener.begin_do_while_statement_body(do_next);
        let saved_loop_state = self.loop_state;
        self.loop_state = LoopState::InsideLoop;
        let mut token = self.parse_statement(do_token);
        self.loop_state = saved_loop_state;
        self.listener.end_do_while_statement_body(token);
        let mut while_token = self.next(token);
        if !self.is_a(while_token, Keyword::WHILE) {
            self.report_recoverable_error(while_token, diag::expected_but_got("while"));
            while_token = self
                .rewriter()
                .insert_synthetic_keyword(token, Keyword::WHILE);
        }
        token = self.ensure_parenthesized_condition(while_token, /* allowCase = */ false);
        token = self.ensure_semicolon(token);
        self.listener
            .end_do_while_statement(do_token, while_token, token);
        token
    }
}
