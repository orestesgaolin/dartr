// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/parser_impl.dart (lines 4393-6686)

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
use crate::identifier_context::IdentifierContext;
use crate::listener::Listener;
use crate::listener_stack::Layer;
use crate::literal_entry_info::LiteralEntryInfo;
use crate::loop_state::LoopState;
use crate::member_kind::MemberKind;
use crate::modifier_context::{ModifierContext, is_modifier};
use crate::type_info::{
    NO_TYPE, NO_TYPE_PARAM_OR_ARG, TypeInfo, TypeParamOrArgInfo, compute_type,
    compute_type_param_or_arg, is_valid_non_record_type_reference,
};
use crate::util::find_previous_non_zero_length_token;

impl<L: Listener> Parser<L> {
    /// Dart (line 4393): `Token parseFields( Token beforeStart, Token? abstractToken, Token? augmentToken, Token? externalToken, Token? staticToken, Token? covariantToken, Token? lateToken, Token? varFinalOrConst, Token beforeType, TypeInfo typeInfo, Token name, DeclarationKind kind, String? enclosingDeclarationName, bool nameIsRecovered, )`
    pub fn parse_fields(
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
        type_info: &TypeInfo,
        name: TokenId,
        kind: DeclarationKind,
        enclosing_declaration_name: Option<&str>,
        name_is_recovered: bool,
    ) -> TokenId {
        let mut covariant_token = covariant_token;
        let mut name = name;
        self.listener.begin_fields(
            kind,
            augment_token,
            abstract_token,
            external_token,
            static_token,
            covariant_token,
            late_token,
            var_final_or_const,
            before_start,
        );

        // Covariant affects only the setter and final fields do not have a setter,
        // unless it's a late field (dartbug.com/40805).
        // Field that are covariant late final with initializers are checked further
        // down.
        if let Some(covariant) = covariant_token
            && late_token.is_none()
        {
            if let Some(vfc) = var_final_or_const
                && self.is_a(vfc, Keyword::FINAL)
            {
                self.report_recoverable_error(covariant, diag::final_and_covariant());
                covariant_token = None;
            }
        }
        if *type_info == NO_TYPE {
            if var_final_or_const.is_none() {
                self.report_recoverable_error(name, diag::missing_const_final_var_or_type());
            }
        } else {
            if let Some(vfc) = var_final_or_const
                && self.is_a(vfc, Keyword::VAR)
            {
                self.report_recoverable_error(vfc, diag::type_after_var());
            }
        }
        if let Some(abstract_token) = abstract_token
            && external_token.is_some()
        {
            self.report_recoverable_error(abstract_token, diag::abstract_external_field());
        }

        let mut token = type_info.parse_type(before_type, self);
        // assert(token.next == name || token.next!.isEof);

        let context = if kind == DeclarationKind::TopLevel {
            IdentifierContext::TopLevelVariableDeclaration
        } else {
            IdentifierContext::FieldDeclaration
        };
        name = self.ensure_identifier_potentially_recovered(
            token,
            context,
            /* isRecovered = */ name_is_recovered,
        );
        let first_name = name;

        // Check for covariant late final with initializer.
        if let Some(covariant) = covariant_token
            && late_token.is_some()
        {
            if let Some(vfc) = var_final_or_const
                && self.is_a(vfc, Keyword::FINAL)
            {
                let next = self.next(name);
                if self.is_a(next, TokenType::EQ) {
                    self.report_recoverable_error(
                        covariant,
                        diag::final_and_covariant_late_with_initializer(),
                    );
                    covariant_token = None;
                }
            }
        }

        let mut field_count: i32 = 1;
        token = self.parse_field_initializer_opt(
            name,
            name,
            late_token,
            abstract_token,
            augment_token,
            external_token,
            var_final_or_const,
            kind,
            enclosing_declaration_name,
        );
        while self.is_a(self.next(token), TokenType::COMMA) {
            let comma = self.next(token);
            name = self.ensure_identifier(comma, context);
            token = self.parse_field_initializer_opt(
                name,
                name,
                late_token,
                abstract_token,
                augment_token,
                external_token,
                var_final_or_const,
                kind,
                enclosing_declaration_name,
            );
            field_count += 1;
        }
        let semicolon = self.next(token);
        if self.is_a(semicolon, TokenType::SEMICOLON) {
            token = semicolon;
        } else {
            token = self.ensure_semicolon(token);
        }
        match kind {
            DeclarationKind::TopLevel => {}
            DeclarationKind::Class | DeclarationKind::Mixin | DeclarationKind::Enum => {}
            DeclarationKind::Extension => {
                if abstract_token.is_some() {
                    self.report_recoverable_error(first_name, diag::abstract_extension_field());
                }
                if static_token.is_none() && external_token.is_none() {
                    self.report_recoverable_error(
                        first_name,
                        diag::extension_declares_instance_field(),
                    );
                }
            }
            DeclarationKind::ExtensionType => {
                if static_token.is_none() && external_token.is_none() {
                    self.report_recoverable_error(
                        first_name,
                        diag::extension_type_declares_instance_field(),
                    );
                }
            }
        }
        let begin = self.next(before_start);
        if kind == DeclarationKind::TopLevel {
            self.listener.end_top_level_fields(
                augment_token,
                abstract_token,
                external_token,
                static_token,
                covariant_token,
                late_token,
                var_final_or_const,
                field_count,
                begin,
                token,
            );
        } else {
            self.listener.end_fields(
                kind,
                abstract_token,
                augment_token,
                external_token,
                static_token,
                covariant_token,
                late_token,
                var_final_or_const,
                field_count,
                begin,
                token,
            );
        }
        token
    }

    /// Dart (line 4561): `Token parseTopLevelMethod( Token beforeStart, Token? augmentToken, Token? externalToken, Token beforeType, TypeInfo typeInfo, Token? getOrSet, Token name, bool nameIsRecovered, )`
    pub fn parse_top_level_method(
        &mut self,
        before_start: TokenId,
        augment_token: Option<TokenId>,
        external_token: Option<TokenId>,
        before_type: TokenId,
        type_info: &TypeInfo,
        get_or_set: Option<TokenId>,
        name: TokenId,
        name_is_recovered: bool,
    ) -> TokenId {
        let mut name = name;
        self.listener
            .begin_top_level_method(before_start, augment_token, external_token);

        let mut token = type_info.parse_type(before_type, self);
        // assert(token.next == (getOrSet ?? name) || token.next!.isEof);
        name = self.ensure_identifier_potentially_recovered(
            get_or_set.unwrap_or(token),
            IdentifierContext::TopLevelFunctionDeclaration,
            /* isRecovered = */ name_is_recovered,
        );

        let mut is_getter = false;
        match get_or_set {
            None => {
                token = self.parse_method_type_var(name);
            }
            Some(get_or_set) => {
                is_getter = self.is_a(get_or_set, Keyword::GET);
                token = name;
                let next = self.next(token);
                self.listener.handle_no_type_variables(next);
            }
        }
        token = self.parse_getter_or_formal_parameters(
            token,
            name,
            is_getter,
            MemberKind::TopLevelMethod,
        );
        let saved_async_modifier = self.async_state;
        let async_token = self.next(token);
        token = self.parse_async_modifier_opt(token);
        if let Some(get_or_set) = get_or_set
            && !self.in_plain_sync()
            && self.is_a(get_or_set, Keyword::SET)
        {
            self.report_recoverable_error(async_token, diag::setter_not_sync());
        }
        let is_external = external_token.is_some();
        if is_external && !self.is_a(self.next(token), TokenType::SEMICOLON) {
            self.report_recoverable_error(
                external_token.unwrap(),
                diag::external_method_with_body(),
            );
        }
        token = self.parse_function_body(
            token,
            /* ofFunctionExpression = */ false,
            is_external || self.is_augmentations_feature_enabled,
        );
        self.async_state = saved_async_modifier;
        let begin = self.next(before_start);
        self.listener.end_top_level_method(begin, get_or_set, token);
        token
    }

    /// Dart (line 4615): `Token parseMethodTypeVar(Token name)`
    pub fn parse_method_type_var(&mut self, name: TokenId) -> TokenId {
        let mut name = name;
        if self.is_a(self.next(name), TokenType::BANG) {
            // Recovery
            name = self.next(name);
            self.report_recoverable_error_with_token(name, diag::unexpected_token);
        }
        if !self.is_a(self.next(name), TokenType::LT) {
            return NO_TYPE_PARAM_OR_ARG.parse_variables(name, self);
        }
        let type_var =
            compute_type_param_or_arg_mut(self.tokens_mut(), name, /* inDeclaration = */ true, false);
        let mut token = type_var.parse_variables(name, self);
        if self.is_a(self.next(token), TokenType::EQ) {
            // Recovery
            token = self.next(token);
            self.report_recoverable_error_with_token(token, diag::unexpected_token);
        }
        token
    }

    /// Dart (line 4637): `Token parseFieldInitializerOpt( Token token, Token name, Token? lateToken, Token? abstractToken, Token? augmentToken, Token? externalToken, Token? varFinalOrConst, DeclarationKind kind, String? enclosingDeclarationName, )`
    pub fn parse_field_initializer_opt(
        &mut self,
        token: TokenId,
        name: TokenId,
        late_token: Option<TokenId>,
        abstract_token: Option<TokenId>,
        augment_token: Option<TokenId>,
        external_token: Option<TokenId>,
        var_final_or_const: Option<TokenId>,
        kind: DeclarationKind,
        enclosing_declaration_name: Option<&str>,
    ) -> TokenId {
        let mut token = token;
        if Some(self.lexeme(name)) == enclosing_declaration_name {
            self.report_recoverable_error(name, diag::member_with_same_name_as_class());
        }
        let next = self.next(token);
        if self.is_a(next, TokenType::EQ) {
            let assignment = next;
            self.listener.begin_field_initializer(next);
            token = self.parse_expression(next);
            self.listener.end_field_initializer(assignment, token);
        } else {
            if let Some(vfc) = var_final_or_const
                && !self.is_synthetic(name)
            {
                if self.is_a(vfc, Keyword::CONST) {
                    let message = diag::const_field_without_initializer(self.lexeme(name));
                    self.report_recoverable_error(name, message);
                } else if kind == DeclarationKind::TopLevel
                    && self.is_a(vfc, Keyword::FINAL)
                    && late_token.is_none()
                    && abstract_token.is_none()
                    && external_token.is_none()
                {
                    let message = diag::final_field_without_initializer(self.lexeme(name));
                    self.report_recoverable_error(name, message);
                }
            }
            let next = self.next(token);
            self.listener.handle_no_field_initializer(next);
        }
        token
    }

    /// Dart (line 4680): `Token parseVariableInitializerOpt(Token token)`
    pub fn parse_variable_initializer_opt(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        if self.is_a(self.next(token), TokenType::EQ) {
            let assignment = self.next(token);
            self.listener.begin_variable_initializer(assignment);
            token = self.parse_expression(assignment);
            self.listener.end_variable_initializer(assignment);
        } else {
            self.listener.handle_no_variable_initializer(token);
        }
        token
    }

    /// Dart (line 4692): `Token parseInitializersOpt(Token token)`
    pub fn parse_initializers_opt(&mut self, token: TokenId) -> TokenId {
        if self.is_a(self.next(token), TokenType::COLON) {
            let next = self.next(token);
            self.parse_initializers(next)
        } else {
            self.listener.handle_no_initializers();
            token
        }
    }

    /// ```
    /// initializers:
    ///   ':' initializerListEntry (',' initializerListEntry)*
    /// ;
    /// ```
    /// Dart (line 4706): `Token parseInitializers(Token token)`
    pub fn parse_initializers(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        let begin = token;
        // assert(begin.isA(TokenType.COLON));
        self.listener.begin_initializers(begin);
        let mut count: i32 = 0;
        let old = self.may_parse_function_expressions;
        self.may_parse_function_expressions = false;
        let mut next = begin;
        loop {
            token = self.parse_initializer(next);
            count += 1;
            next = self.next(token);
            if !self.is_a(next, TokenType::COMMA) {
                // Recovery: Found an identifier which could be
                // 1) missing preceding `,` thus it's another initializer, or
                // 2) missing preceding `;` thus it's a class member, or
                // 3) missing preceding '{' thus it's a statement
                if self.is_a(next, Keyword::ASSERT) {
                    next = self.next(next);
                    if !self.is_a(next, TokenType::OPEN_PAREN) {
                        break;
                    }
                    // Looks like assert expression ... fall through to insert comma.
                } else if self.is_a(next, Keyword::THIS) || self.is_a(next, Keyword::SUPER) {
                    next = self.next(next);
                    if !self.is_a(next, TokenType::OPEN_PAREN)
                        && !self.is_a(next, TokenType::PERIOD)
                    {
                        break;
                    }
                    // `this` or `super` followed by either `.` or `(`.
                    // Fall through to insert comma.
                } else if self.is_identifier(next) {
                    next = self.next(next);
                    if !self.is_a(next, TokenType::EQ) {
                        break;
                    }
                    // Looks like field assignment... fall through to insert comma.
                } else {
                    break;
                }
                // TODO(danrubel): Consider enhancing this to indicate that we are
                // expecting one of `,` or `;` or `{`
                self.report_recoverable_error(token, diag::expected_after_but_got(","));
                next = self
                    .rewriter()
                    .insert_synthetic_token(token, TokenType::COMMA);
            }
        }
        self.may_parse_function_expressions = old;
        self.listener.end_initializers(count, begin, token);
        token
    }

    /// ```
    /// initializerListEntry:
    ///   'super' ('.' identifier)? arguments |
    ///   fieldInitializer |
    ///   assertion
    /// ;
    ///
    /// fieldInitializer:
    ///   ('this' '.')? identifier '=' conditionalExpression cascadeSection*
    /// ;
    /// ```
    /// Dart (line 4770): `Token parseInitializer(Token token)`
    pub fn parse_initializer(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        let mut next = self.next(token);
        self.listener.begin_initializer(next);
        let before_expression = token;
        if self.is_a(next, Keyword::ASSERT) {
            token = self.parse_assert(token, Assert::Initializer);
            self.listener.end_initializer(token);
            return token;
        } else if self.is_a(next, Keyword::SUPER) {
            return self.parse_super_initializer_expression(token);
        } else if self.is_a(next, Keyword::THIS) {
            token = next;
            next = self.next(token);
            if self.is_a(next, TokenType::PERIOD) {
                token = next;
                let after_identifier = self.tokens().get(self.next(token)).next.get();
                if let Some(after_identifier) = after_identifier
                    && self.is_a(after_identifier, TokenType::OPEN_PAREN)
                {
                    self.try_rewrite_new_to_identifier(token, IdentifierContext::FieldInitializer);
                }
                next = self.next(token);
                if self.is_identifier(next) {
                    token = next;
                } else {
                    // Recovery
                    token = self.insert_synthetic_identifier(
                        token,
                        IdentifierContext::FieldInitializer,
                        None,
                        None,
                    );
                }
                next = self.next(token);
                if self.is_a(next, TokenType::EQ) {
                    return self.parse_initializer_expression_rest(before_expression);
                }
            }
            if self.is_a(next, TokenType::OPEN_PAREN) {
                token = self.parse_initializer_expression_rest(before_expression);
                next = self.next(token);
                if self.is_a(next, TokenType::OPEN_CURLY_BRACKET)
                    || self.is_a(next, TokenType::FUNCTION)
                {
                    self.report_recoverable_error(next, diag::redirecting_constructor_with_body());
                }
                return token;
            }
            // Recovery
            if self.is_a(token, Keyword::THIS) {
                // TODO(danrubel): Consider a better error message indicating that
                // `this.<fieldname>=` is expected.
                self.report_recoverable_error(next, diag::expected_but_got("."));
                self.rewriter()
                    .insert_synthetic_token(token, TokenType::PERIOD);
                let period = self.next(token);
                token = self.rewriter().insert_synthetic_identifier(period, "");
                next = self.next(token);
            }
            // Fall through to recovery
        } else if self.is_identifier(next) {
            let next2 = self.next(next);
            if self.is_a(next2, TokenType::EQ) {
                return self.parse_initializer_expression_rest(token);
            }
            // Recovery: If this looks like an expression,
            // then fall through to insert the LHS and `=` of the assignment,
            // otherwise insert an `=` and synthetic identifier.
            if !self.is_operator(next2) && !self.is_a(next2, TokenType::PERIOD) {
                token = self.rewriter().insert_synthetic_token(next, TokenType::EQ);
                token = self.insert_synthetic_identifier(
                    token,
                    IdentifierContext::Expression,
                    Some(diag::missing_assignment_in_initializer()),
                    Some(next),
                );
                return self.parse_initializer_expression_rest(before_expression);
            }
        } else {
            // Recovery: Insert a synthetic assignment.
            token = self.insert_synthetic_identifier(
                token,
                IdentifierContext::FieldInitializer,
                Some(diag::expected_an_initializer()),
                Some(token),
            );
            token = self.rewriter().insert_synthetic_token(token, TokenType::EQ);
            token = self.rewriter().insert_synthetic_identifier(token, "");
            return self.parse_initializer_expression_rest(before_expression);
        }
        // Recovery:
        // Insert a synthetic identifier and assignment operator
        // to ensure that the expression is indeed an assignment.
        // Failing to do so causes this test to fail:
        // pkg/front_end/testcases/regress/issue_31192.dart
        // TODO(danrubel): Investigate better recovery.
        token = self.insert_synthetic_identifier(
            before_expression,
            IdentifierContext::FieldInitializer,
            Some(diag::missing_assignment_in_initializer()),
            None,
        );
        self.rewriter().insert_synthetic_token(token, TokenType::EQ);
        self.parse_initializer_expression_rest(before_expression)
    }

    /// Parse the `super` initializer:
    /// ```
    ///   'super' ('.' identifier)? arguments ;
    /// ```
    /// Dart (line 4876): `Token parseSuperInitializerExpression(Token start)`
    pub fn parse_super_initializer_expression(&mut self, start: TokenId) -> TokenId {
        let mut token = self.next(start);
        // assert(token.isA(Keyword.SUPER));
        let mut next = self.next(token);
        if self.is_a(next, TokenType::PERIOD) {
            token = next;
            self.try_rewrite_new_to_identifier(
                token,
                IdentifierContext::ConstructorReferenceContinuation,
            );
            next = self.next(token);
            if self.kind(next) != IDENTIFIER_TOKEN {
                next = IdentifierContext::ExpressionContinuation.ensure_identifier(token, self);
            }
            token = next;
            next = self.next(token);
        }
        if !self.is_a(next, TokenType::OPEN_PAREN) {
            // Recovery
            if self.is_a(next, TokenType::QUESTION_PERIOD) {
                // An error for `super?.` is reported in parseSuperExpression.
                token = next;
                next = self.next(token);
                if !self.is_identifier(next) {
                    // Insert a synthetic identifier but don't report another error.
                    next = self.rewriter().insert_synthetic_identifier(token, "");
                }
                token = next;
                next = self.next(token);
            }
            if self.is_a(next, TokenType::EQ) {
                if self.is_a(token, Keyword::SUPER) {
                    // parseExpression will report error on assignment to super
                } else {
                    self.report_recoverable_error(
                        token,
                        diag::field_initialized_outside_declaring_class(),
                    );
                }
            } else if !self.is_a(next, TokenType::OPEN_PAREN) {
                self.report_recoverable_error(next, diag::expected_after_but_got("("));
                self.rewriter()
                    .insert_parens(token, /* includeIdentifier = */ false);
            }
        }
        self.parse_initializer_expression_rest(start)
    }

    /// Dart (line 4929): `Token parseInitializerExpressionRest(Token token)`
    pub fn parse_initializer_expression_rest(&mut self, token: TokenId) -> TokenId {
        let token = self.parse_expression(token);
        self.listener.end_initializer(token);
        token
    }

    /// If the next token is an opening curly brace, return it. Otherwise, use
    /// [missingBlockKind] to report an error, insert an opening and a closing
    /// curly brace, and return the newly inserted opening curly brace. If
    /// [missingBlockKind] is `null`, then use a default error message instead.
    /// Dart (line 4939): `Token ensureBlock(Token token, BlockKind? missingBlockKind)`
    pub fn ensure_block(
        &mut self,
        token: TokenId,
        missing_block_kind: Option<BlockKind>,
    ) -> TokenId {
        let next = self.next(token);
        if self.is_a(next, TokenType::OPEN_CURLY_BRACKET) {
            return next;
        }
        let template = missing_block_kind.and_then(|k| k.template());
        match template {
            None => {
                let message = missing_block_kind.and_then(|k| k.message());
                match message {
                    None => {
                        // TODO(danrubel): rename ExpectedButGot to ExpectedBefore
                        self.report_recoverable_error(next, diag::expected_but_got("{"));
                    }
                    Some(message) => {
                        self.report_recoverable_error(token, message);
                    }
                }
            }
            Some(template) => {
                let message = template(self.lexeme(next));
                self.report_recoverable_error(next, message);
            }
        }
        self.insert_block(token)
    }

    /// Dart (line 4961): `Token insertBlock(Token token)`
    pub fn insert_block(&mut self, token: TokenId) -> TokenId {
        let next = self.next(token);
        let offset = self.offset(next);
        let byte_offset = self.tokens().byte_offset(next);
        let begin =
            self.tokens_mut()
                .push_synthetic(TokenType::OPEN_CURLY_BRACKET, offset, byte_offset);
        let begin_group = self.rewriter().insert_token(token, begin);
        let end =
            self.tokens_mut()
                .push_synthetic(TokenType::CLOSE_CURLY_BRACKET, offset, byte_offset);
        let end_group = self.rewriter().insert_token(begin_group, end);
        self.tokens_mut().get_mut(begin_group).end_group = end_group;
        begin_group
    }

    /// If the next token is a closing parenthesis, return it.
    /// Otherwise, report an error and return the closing parenthesis
    /// associated with the specified open parenthesis.
    /// Dart (line 4983): `Token ensureCloseParen(Token token, Token openParen)`
    pub fn ensure_close_paren(&mut self, token: TokenId, open_paren: TokenId) -> TokenId {
        let next = self.next(token);
        if self.is_a(next, TokenType::CLOSE_PAREN) {
            return next;
        }
        let end_group = self.end_group(open_paren).unwrap();
        if self.is_synthetic(end_group) {
            // Scanner has already reported a missing `)` error,
            // but placed the `)` in the wrong location, so move it.
            return self.rewriter().move_synthetic(token, end_group);
        }

        // TODO(danrubel): Pass in context for better error message.
        self.report_recoverable_error(next, diag::expected_but_got(")"));

        // Scanner guarantees a closing parenthesis
        // TODO(danrubel): Improve recovery by having callers parse tokens
        // between `token` and `openParen.endGroup`.
        self.end_group(open_paren).unwrap()
    }

    /// If the next token is a colon, return it. Otherwise, report an
    /// error, insert a synthetic colon, and return the inserted colon.
    /// Dart (line 5008): `Token ensureColon(Token token)`
    pub fn ensure_colon(&mut self, token: TokenId) -> TokenId {
        let next = self.next(token);
        if self.is_a(next, TokenType::COLON) {
            return next;
        }
        let message = diag::expected_but_got(":");
        let offset = self.char_offset(next);
        let byte_offset = self.tokens().byte_offset(next);
        let new_token = self
            .tokens_mut()
            .push_synthetic(TokenType::COLON, offset, byte_offset);
        self.rewrite_and_recover(token, message, new_token)
    }

    /// If the next token is a function arrow (`=>`), return it.  Otherwise report
    /// an error, insert a synthetic function arrow, and return the inserted
    /// function arrow.
    /// Dart (line 5019): `Token ensureFunctionArrow(Token token)`
    pub fn ensure_function_arrow(&mut self, token: TokenId) -> TokenId {
        let next = self.next(token);
        if self.is_a(next, TokenType::FUNCTION) {
            return next;
        }
        let message = diag::expected_but_got("=>");
        let offset = self.char_offset(next);
        let byte_offset = self.tokens().byte_offset(next);
        let new_token = self
            .tokens_mut()
            .push_synthetic(TokenType::FUNCTION, offset, byte_offset);
        self.rewrite_and_recover(token, message, new_token)
    }

    /// If the token after [token] is a not literal string,
    /// then insert a synthetic literal string.
    /// Call `parseLiteralString` and return the result.
    /// Dart (line 5030): `Token ensureLiteralString(Token token)`
    pub fn ensure_literal_string(&mut self, token: TokenId) -> TokenId {
        let next = self.next(token);
        if self.kind(next) != STRING_TOKEN {
            let message = diag::expected_string(self.lexeme(next));
            let offset = self.char_offset(next);
            let byte_offset = self.tokens().byte_offset(next);
            let new_token = self.tokens_mut().push_synthetic_string(
                TokenType::STRING,
                "\"\"",
                offset,
                byte_offset,
                /* _length = */ Some(0),
            );
            self.rewrite_and_recover(token, message, new_token);
        }
        self.parse_literal_string(token)
    }

    /// If the token after [token] is a semi-colon, return it.
    /// Otherwise, report an error, insert a synthetic semi-colon,
    /// and return the inserted semi-colon.
    /// Dart (line 5048): `Token ensureSemicolon(Token token)`
    pub fn ensure_semicolon(&mut self, token: TokenId) -> TokenId {
        // TODO(danrubel): Once all expect(';'...) call sites have been converted
        // to use this method, remove similar semicolon recovery code
        // from the handleError method in element_listener.dart.
        let next = self.next(token);
        if self.is_a(next, TokenType::SEMICOLON) {
            return next;
        }

        // Find a token on the same line as where the ';' should be inserted.
        // Reporting the error on this token makes it easier
        // for users to understand and fix the error.
        let error_token = find_previous_non_zero_length_token(self.tokens(), token);
        self.report_recoverable_error(error_token, diag::expected_after_but_got(";"));
        self.rewriter()
            .insert_synthetic_token(token, TokenType::SEMICOLON)
    }

    /// Report an error at the token after [token] that has the given [message].
    /// Insert the [newToken] after [token] and return [newToken].
    /// Dart (line 5067): `Token rewriteAndRecover(Token token, codes.Message message, Token newToken)`
    pub fn rewrite_and_recover(
        &mut self,
        token: TokenId,
        message: CfeMessage,
        new_token: TokenId,
    ) -> TokenId {
        let next = self.next(token);
        self.report_recoverable_error(next, message);
        self.rewriter().insert_token(token, new_token)
    }

    /// Replace the token after [token] with `[` followed by `]`
    /// and return [token].
    /// Dart (line 5074): `Token rewriteSquareBrackets(Token token)`
    pub fn rewrite_square_brackets(&mut self, token: TokenId) -> TokenId {
        let next = self.next(token);
        // assert(next.isA(TokenType.INDEX));
        let offset = self.offset(next);
        let byte_offset = self.tokens().byte_offset(next);
        let preceding_comments = self.token(next).preceding_comments;
        let replacement;
        if self.is_synthetic(next) {
            let begin = self.tokens_mut().push_synthetic(
                TokenType::OPEN_SQUARE_BRACKET,
                offset,
                byte_offset,
            );
            self.tokens_mut().get_mut(begin).preceding_comments = preceding_comments;
            let end = self.tokens_mut().push_synthetic(
                TokenType::CLOSE_SQUARE_BRACKET,
                offset,
                byte_offset,
            );
            replacement = self.link(begin, end);
        } else {
            let begin = self.tokens_mut().push_simple(
                TokenType::OPEN_SQUARE_BRACKET,
                offset,
                byte_offset,
                preceding_comments,
            );
            let end = self.tokens_mut().push_simple(
                TokenType::CLOSE_SQUARE_BRACKET,
                offset + 1,
                byte_offset + 1,
                TokenId::NONE,
            );
            replacement = self.link(begin, end);
        }
        self.rewriter().replace_token_following(token, replacement);
        token
    }

    /// Report the given token as unexpected and return the next token if the next
    /// token is one of the [expectedNext], otherwise just return the given token.
    /// Dart (line 5103): `Token skipUnexpectedTokenOpt(Token token, List<String> expectedNext)`
    pub fn skip_unexpected_token_opt(&mut self, token: TokenId, expected_next: &[&str]) -> TokenId {
        let next = self.next(token);
        // Dart `next.keyword == null`.
        if !self.is_keyword(next) {
            let next_value = self.string_value(self.next(next));
            for &expected_value in expected_next {
                if next_value == Some(expected_value) {
                    self.report_recoverable_error_with_token(next, diag::unexpected_token);
                    return next;
                }
            }
        }
        token
    }

    /// Dart (line 5117): `Token parseNativeClause(Token token)`
    pub fn parse_native_clause(&mut self, token: TokenId) -> TokenId {
        let mut token = self.next(token);
        let native_token = token;
        // assert(nativeToken.isA(Keyword.NATIVE));
        let mut has_name = false;
        if self.kind(self.next(token)) == STRING_TOKEN {
            has_name = true;
            token = self.parse_literal_string(token);
        }
        self.listener.handle_native_clause(native_token, has_name);
        self.report_recoverable_error(native_token, diag::native_clause_should_be_annotation());
        token
    }

    /// Dart (line 5130): `Token skipClassOrMixinOrExtensionBody(Token token)`
    pub fn skip_class_or_mixin_or_extension_body(&mut self, token: TokenId) -> TokenId {
        // The scanner ensures that `{` always has a closing `}`.
        self.ensure_block(token, /* missingBlockKind = */ None)
    }

    /// ```
    /// classBody:
    ///   '{' classMember* '}'
    /// ;
    /// ```
    /// Dart (line 5140): `Token parseClassOrMixinOrExtensionBody( Token token, DeclarationKind kind, String? enclosingDeclarationName, )`
    pub fn parse_class_or_mixin_or_extension_body(
        &mut self,
        token: TokenId,
        kind: DeclarationKind,
        enclosing_declaration_name: Option<&str>,
    ) -> TokenId {
        let mut token = self.next(token);
        let begin = token;
        // assert(token.isA(TokenType.OPEN_CURLY_BRACKET));
        self.listener
            .begin_class_or_mixin_or_extension_body(kind, token);
        let mut count: i32 = 0;
        while self.not_eof_or_type(TokenType::CLOSE_CURLY_BRACKET, self.next(token)) {
            token = self.parse_class_or_mixin_or_extension_or_enum_member_impl(
                token,
                kind,
                enclosing_declaration_name,
            );
            count += 1;
        }
        token = self.next(token);
        // assert(token.isEof || token.isA(TokenType.CLOSE_CURLY_BRACKET));
        self.listener
            .end_class_or_mixin_or_extension_body(kind, count, begin, token);
        token
    }

    /// Dart (line 5163): `bool isUnaryMinus(Token token) =>`
    pub fn is_unary_minus(&mut self, token: TokenId) -> bool {
        self.kind(token) == IDENTIFIER_TOKEN
            && self.lexeme(token) == "unary"
            && self.is_a(self.next(token), TokenType::MINUS)
    }

    /// Parse a class member.
    ///
    /// This method is only invoked from outside the parser. As a result, this
    /// method takes the next token to be consumed rather than the last consumed
    /// token and returns the token after the last consumed token rather than the
    /// last consumed token.
    /// Dart (line 5174): `Token parseClassMember(Token token, String? className)`
    pub fn parse_class_member(&mut self, token: TokenId, class_name: Option<&str>) -> TokenId {
        let before = self.synthetic_previous_token(token);
        let token = self.parse_class_or_mixin_or_extension_or_enum_member_impl(
            before,
            DeclarationKind::Class,
            class_name,
        );
        self.next(token)
    }

    /// Parse a mixin member.
    ///
    /// This method is only invoked from outside the parser. As a result, this
    /// method takes the next token to be consumed rather than the last consumed
    /// token and returns the token after the last consumed token rather than the
    /// last consumed token.
    /// Dart (line 5188): `Token parseMixinMember(Token token, String mixinName)`
    pub fn parse_mixin_member(&mut self, token: TokenId, mixin_name: &str) -> TokenId {
        let before = self.synthetic_previous_token(token);
        let token = self.parse_class_or_mixin_or_extension_or_enum_member_impl(
            before,
            DeclarationKind::Mixin,
            Some(mixin_name),
        );
        self.next(token)
    }

    /// Parse an extension member.
    ///
    /// This method is only invoked from outside the parser. As a result, this
    /// method takes the next token to be consumed rather than the last consumed
    /// token and returns the token after the last consumed token rather than the
    /// last consumed token.
    /// Dart (line 5202): `Token parseExtensionMember(Token token, String extensionName)`
    pub fn parse_extension_member(&mut self, token: TokenId, extension_name: &str) -> TokenId {
        let before = self.synthetic_previous_token(token);
        let token = self.parse_class_or_mixin_or_extension_or_enum_member_impl(
            before,
            DeclarationKind::Extension,
            Some(extension_name),
        );
        self.next(token)
    }

    /// Dart (line 5210): `bool isReservedKeyword(Token token)`
    pub fn is_reserved_keyword(&mut self, token: TokenId) -> bool {
        if !self.is_keyword(token) {
            return false;
        }
        self.is_reserved_word(token)
    }

    /// Dart (line 5215): `bool indicatesMethodOrField(Token token)`
    pub fn indicates_method_or_field(&mut self, token: TokenId) -> bool {
        let value = self.string_value(token);
        if value == Some(";")
            || value == Some("=")
            || value == Some("(")
            || value == Some("{")
            || value == Some("=>")
            || value == Some("<")
        {
            return true;
        }
        false
    }

    /// ```
    /// classMember:
    ///   fieldDeclaration |
    ///   constructorDeclaration |
    ///   methodDeclaration
    /// ;
    ///
    /// mixinMember:
    ///   fieldDeclaration |
    ///   methodDeclaration
    /// ;
    ///
    /// extensionMember:
    ///   staticFieldDeclaration |
    ///   methodDeclaration
    /// ;
    /// ```
    /// Dart (line 5245): `Token parseClassOrMixinOrExtensionOrEnumMemberImpl( Token token, DeclarationKind kind, String? enclosingDeclarationName, )`
    pub fn parse_class_or_mixin_or_extension_or_enum_member_impl(
        &mut self,
        token: TokenId,
        kind: DeclarationKind,
        enclosing_declaration_name: Option<&str>,
    ) -> TokenId {
        let mut token = self.parse_metadata_star(token);
        let before_start = token;

        let skipped_non_late_late: Option<TokenId> = None;

        let mut covariant_token: Option<TokenId> = None;
        let mut abstract_token: Option<TokenId> = None;
        let mut augment_token: Option<TokenId> = None;
        let mut external_token: Option<TokenId> = None;
        let mut late_token: Option<TokenId> = None;
        let mut static_token: Option<TokenId> = None;
        let mut var_final_or_const: Option<TokenId> = None;

        let mut next = self.next(token);
        if is_modifier(self.tokens(), next) {
            if self.is_a(next, Keyword::EXTERNAL) {
                token = next;
                external_token = Some(token);
                next = self.next(token);
            } else if self.is_a(next, Keyword::AUGMENT) {
                token = next;
                augment_token = Some(token);
                next = self.next(token);
            } else if self.is_a(next, Keyword::ABSTRACT) {
                token = next;
                abstract_token = Some(token);
                next = self.next(token);
            }
            if is_modifier(self.tokens(), next) {
                if self.is_a(next, Keyword::STATIC) && abstract_token.is_none() {
                    token = next;
                    static_token = Some(token);
                    next = self.next(token);
                } else if self.is_a(next, Keyword::COVARIANT) {
                    token = next;
                    covariant_token = Some(token);
                    next = self.next(token);
                }
                if is_modifier(self.tokens(), next) {
                    if self.is_a(next, Keyword::FINAL) {
                        token = next;
                        var_final_or_const = Some(token);
                        next = self.next(token);
                    } else if self.is_a(next, Keyword::VAR) {
                        token = next;
                        var_final_or_const = Some(token);
                        next = self.next(token);
                    } else if self.is_a(next, Keyword::CONST) && covariant_token.is_none() {
                        token = next;
                        var_final_or_const = Some(token);
                        next = self.next(token);
                    } else if self.is_a(next, Keyword::LATE) {
                        token = next;
                        late_token = Some(token);
                        next = self.next(token);
                        if is_modifier(self.tokens(), next) && self.is_a(next, Keyword::FINAL) {
                            token = next;
                            var_final_or_const = Some(token);
                            next = self.next(token);
                        }
                    }
                    if is_modifier(self.tokens(), next) {
                        let mut context = ModifierContext::new();
                        context.covariant_token = covariant_token;
                        context.augment_token = augment_token;
                        context.external_token = external_token;
                        context.late_token = late_token;
                        context.static_token = static_token;
                        context.set_var_final_or_const(self.tokens(), var_final_or_const);
                        context.abstract_token = abstract_token;

                        token = context.parse_class_member_modifiers(self, token);
                        next = self.next(token);

                        covariant_token = context.covariant_token;
                        external_token = context.external_token;
                        late_token = context.late_token;
                        static_token = context.static_token;
                        var_final_or_const = context.var_final_or_const();
                        abstract_token = context.abstract_token;
                    }
                }
            }
        }

        if late_token.is_none() {
            // `late` was used as a modifier in non-nnbd mode. An error has been
            // emitted. Still use it as a late token for the remainder in an attempt
            // to avoid cascading errors (and for passing to the listener).
            late_token = skipped_non_late_late;
        }

        self.listener.begin_member();

        let before_type = token;
        if let Some(vfc) = var_final_or_const
            && !self.is_a(vfc, Keyword::CONST)
        {
            let after_outer_pattern = self.skip_outer_pattern(before_type);
            if let Some(after_outer_pattern) = after_outer_pattern
                && self.is_a(self.next(after_outer_pattern), TokenType::EQ)
            {
                let start = self.next(before_type);
                self.report_recoverable_error_with_end(
                    start,
                    after_outer_pattern,
                    diag::pattern_variable_declaration_outside_function_or_method(),
                );
                let synthetic_name = self.rewriter().insert_synthetic_identifier(before_type, "");

                let after = self.next(after_outer_pattern);
                self.rewriter().drop_range(synthetic_name, after);
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
                    &NO_TYPE,
                    synthetic_name,
                    kind,
                    enclosing_declaration_name,
                    /* nameIsRecovered = */ true,
                );
                self.listener.end_member();
                return token;
            }
        }
        let mut type_info = compute_type_mut(self.tokens_mut(),
            token,
            /* required = */ false,
            /* inDeclaration = */ true,
            false,
        );
        token = type_info.skip_type_mut(self.tokens_mut(), token);
        next = self.next(token);

        let mut new_token: Option<TokenId> = None;
        let mut get_or_set: Option<TokenId> = None;
        let mut name_is_recovered = false;
        if self.is_a(next, Keyword::NEW) {
            new_token = Some(next);
            if !self.is_primary_constructors_feature_enabled {
                self.report_experiment_not_enabled(
                    ExperimentalFlag::PrimaryConstructors,
                    next,
                    next,
                );
            }
        } else if self.ty(next) != TokenType::IDENTIFIER {
            let value = self.string_value(next);
            if value == Some("get") || value == Some("set") {
                let next_next = self.next(next);
                if self.is_identifier(next_next) {
                    token = next;
                    get_or_set = Some(token);
                    next = self.next(token);
                } else if self.is_reserved_keyword(next_next) && {
                    let after = self.next(next_next);
                    self.indicates_method_or_field(after)
                } {
                    // Recovery: Getter or setter followed by a reserved word (name).
                    token = next;
                    get_or_set = Some(token);
                    next = self.next(token);
                    name_is_recovered = true;
                }
                // Fall through to continue parsing `get` or `set` as an identifier.
            } else if value == Some("factory") {
                let next2 = self.next(next);
                if self.is_identifier(next2)
                    || self.is_modifier(next2)
                    || self.is_a(next2, Keyword::NEW)
                {
                    if before_type != token {
                        self.report_recoverable_error(token, diag::type_before_factory());
                    }
                    if let Some(abstract_token) = abstract_token {
                        self.report_recoverable_error(
                            abstract_token,
                            diag::abstract_class_member(),
                        );
                    }
                    token = self.parse_factory_method(
                        token,
                        kind,
                        before_start,
                        augment_token,
                        external_token,
                        static_token.or(covariant_token),
                        var_final_or_const,
                        /* hasName = */ true,
                    );
                    self.listener.end_member();
                    return token;
                } else if self.is_primary_constructors_feature_enabled
                    && self.is_a(next2, TokenType::OPEN_PAREN)
                {
                    if type_info == NO_TYPE
                        && covariant_token.is_none()
                        //externalToken = null &&
                        && late_token.is_none()
                        && static_token.is_none()
                        && (var_final_or_const.is_none()
                            || self.is_a(var_final_or_const.unwrap(), Keyword::CONST))
                        && abstract_token.is_none()
                    {
                        token = self.parse_factory_method(
                            token,
                            kind,
                            before_start,
                            augment_token,
                            external_token,
                            static_token.or(covariant_token),
                            var_final_or_const,
                            /* hasName = */ false,
                        );
                        self.listener.end_member();
                        return token;
                    }
                }
                // Fall through to continue parsing `factory` as an identifier.
            } else if value == Some("operator") {
                let next2 = self.next(next);
                let type_param = compute_type_param_or_arg_mut(self.tokens_mut(), next, false, false);
                // `operator` can be used as an identifier as in
                // `int operator<T>()` or `int operator = 2`
                if self.is_user_definable_operator(next2) && type_param == NO_TYPE_PARAM_OR_ARG {
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
                        &type_info,
                        get_or_set,
                        new_token,
                        name,
                        kind,
                        enclosing_declaration_name,
                        name_is_recovered,
                    );
                    self.listener.end_member();
                    return token;
                } else if self.is_a(next2, TokenType::EQ_EQ_EQ)
                    || self.is_a(next2, TokenType::BANG_EQ_EQ)
                    || (self.is_operator(next2)
                        && !self.is_a(next2, TokenType::EQ)
                        && !self.is_a(next2, TokenType::LT))
                {
                    // Recovery: Invalid operator
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
                } else if self.is_unary_minus(next2) {
                    // Recovery
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
                        &type_info,
                        get_or_set,
                        new_token,
                        name,
                        kind,
                        enclosing_declaration_name,
                        name_is_recovered,
                    );
                    self.listener.end_member();
                    return token;
                }
                // Fall through to continue parsing `operator` as an identifier.
            } else if value == Some("this") {
                let next2 = self.next(next);
                if self.is_a(next2, TokenType::COLON)
                    || self.is_a(next2, TokenType::SEMICOLON)
                    || self.is_a(next2, TokenType::OPEN_CURLY_BRACKET)
                    || self.is_a(next2, TokenType::FUNCTION) // =>
                    || self.is_a(next2, Keyword::ASYNC)
                    || self.is_a(next2, Keyword::SYNC)
                {
                    if !self.is_primary_constructors_feature_enabled {
                        self.report_experiment_not_enabled(
                            ExperimentalFlag::PrimaryConstructors,
                            next,
                            next,
                        );
                    }

                    if let Some(var_final_or_const) = var_final_or_const {
                        self.report_recoverable_error_with_token(
                            var_final_or_const,
                            diag::extraneous_modifier,
                        );
                    }
                    if let Some(external_token) = external_token {
                        self.report_recoverable_error_with_token(
                            external_token,
                            diag::extraneous_modifier,
                        );
                    }
                    if let Some(static_token) = static_token {
                        self.report_recoverable_error_with_token(
                            static_token,
                            diag::extraneous_modifier,
                        );
                    }
                    if let Some(covariant_token) = covariant_token {
                        self.report_recoverable_error_with_token(
                            covariant_token,
                            diag::extraneous_modifier,
                        );
                    }
                    if let Some(late_token) = late_token {
                        self.report_recoverable_error_with_token(
                            late_token,
                            diag::extraneous_modifier,
                        );
                    }
                    token = self.parse_primary_constructor_body(next);
                    self.listener.end_member();
                    return token;
                }
            } else if !self.is_identifier(next)
                || (value == Some("typedef")
                    && token == before_start
                    && self.is_identifier(self.next(next)))
            {
                if let Some(abstract_token) = abstract_token {
                    self.report_recoverable_error(abstract_token, diag::abstract_class_member());
                }
                // Recovery
                return self.recover_from_invalid_member(
                    token,
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
                    get_or_set,
                    new_token,
                    kind,
                    enclosing_declaration_name,
                );
            }
        } else if type_info == NO_TYPE && var_final_or_const.is_none() {
            let next2 = self.next(next);
            if self.is_user_definable_operator(next2) && self.end_group(next2).is_none() {
                let value = self.string_value(self.next(next2));
                if value == Some("(") || value == Some("{") || value == Some("=>") {
                    // Recovery: Missing `operator` keyword
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
            } else if self.is_reserved_keyword(next2) && {
                let after = self.next(next2);
                self.indicates_method_or_field(after)
            } {
                // Recovery: Use the reserved keyword despite that not being legal.
                type_info = compute_type_mut(self.tokens_mut(),
                    token,
                    /* required = */ true,
                    /* inDeclaration = */ true,
                    false,
                );
                token = type_info.skip_type_mut(self.tokens_mut(), token);
                next = self.next(token);
                name_is_recovered = true;
            }
        }

        // At this point, token is before the name, and next is the name
        next = self.next(next);
        let value = self.string_value(next);
        if get_or_set.is_some()
            || new_token.is_some()
            || value == Some("(")
            || value == Some("{")
            || value == Some("<")
            || value == Some(".")
            || value == Some("=>")
        {
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
                &type_info,
                get_or_set,
                new_token,
                name,
                kind,
                enclosing_declaration_name,
                name_is_recovered,
            );
        } else {
            if let Some(get_or_set) = get_or_set {
                self.report_recoverable_error_with_token(get_or_set, diag::extraneous_modifier);
            }
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
                &type_info,
                name,
                kind,
                enclosing_declaration_name,
                name_is_recovered,
            );
        }
        self.listener.end_member();
        token
    }

    /// Returns `true` if a method-like declaration is determined to be a
    /// constructor.
    ///
    /// [name] is the token for the (first) name of the declaration or the
    /// `operator` token if [isOperator] is `true`.
    ///
    /// [getOrSet] is the token for `get` or `set` if this occurred prior to the
    /// [name].
    ///
    /// [newToken] is the token for `new` if this occurred prior to the name.
    ///
    /// [enclosingDeclarationName] is the name of the enclosing class, mixin,
    /// enum, extension or extension type.
    /// Dart (line 5689): `bool _isConstructor( Token name, Token? getOrSet, Token? newToken, String? enclosingDeclarationName, bool isOperator, )`
    pub fn is_constructor(
        &mut self,
        name: TokenId,
        get_or_set: Option<TokenId>,
        new_token: Option<TokenId>,
        enclosing_declaration_name: Option<&str>,
        is_operator: bool,
    ) -> bool {
        // TODO(johnniwinther): Update this to match what we want and not what we
        //  happened to do during error recovery.
        if new_token.is_some() {
            return true;
        }
        let mut after_name;
        if self.is_keyword_or_identifier(name) {
            after_name = self.next(name);
        } else {
            // Recovery (identifier is synthesized)
            after_name = name;
        }
        if self.is_a(after_name, TokenType::PERIOD) {
            // This is only legal for constructors.
            return true;
        }

        if is_operator {
            if self.is_operator(after_name) {
                after_name = self.next(after_name);
            } else if self.is_unary_minus(after_name) {
                after_name = self.next(self.next(after_name));
            }
        }
        if self.is_a(after_name, TokenType::BANG) {
            // Recovery
            after_name = self.next(after_name);
        }
        if self.is_a(after_name, TokenType::LT) {
            if let Some(end_group) = self.end_group(after_name) {
                after_name = self.next(end_group);
                if self.is_a(after_name, TokenType::EQ) {
                    // Recovery
                    after_name = self.next(after_name);
                }
            }
        }

        if self.is_a(after_name, TokenType::OPEN_PAREN) {
            let after_paren = self.next(self.end_group(after_name).unwrap());
            if self.is_a(after_paren, TokenType::COLON) {
                return true;
            }
        } else if self.is_a(after_name, TokenType::COLON) {
            return true;
        }

        if get_or_set.is_some() {
            return false;
        }

        if Some(self.lexeme(name)) == enclosing_declaration_name {
            return true;
        }

        false
    }

    /// Dart (line 5754): `Token parseMethod( Token beforeStart, Token? abstractToken, Token? augmentToken, Token? externalToken, Token? staticToken, Token? covariantToken, Token? lateToken, Token? varFinalOrConst, Token beforeType, TypeInfo typeInfo, Token? getOrSet, Token? newToken, Token name, DeclarationKind kind, String? enclosingDeclarationName, bool nameIsRecovered, )`
    pub fn parse_method(
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
        type_info: &TypeInfo,
        get_or_set: Option<TokenId>,
        new_token: Option<TokenId>,
        name: TokenId,
        kind: DeclarationKind,
        enclosing_declaration_name: Option<&str>,
        name_is_recovered: bool,
    ) -> TokenId {
        let mut static_token = static_token;
        let mut covariant_token = covariant_token;
        let mut var_final_or_const = var_final_or_const;
        if let Some(abstract_token) = abstract_token {
            self.report_recoverable_error(abstract_token, diag::abstract_class_member());
        }
        if let Some(late_token) = late_token {
            self.report_recoverable_error_with_token(late_token, diag::extraneous_modifier);
        }
        let mut is_operator = false;
        if get_or_set.is_none() && self.is_a(name, Keyword::OPERATOR) {
            let mut operator = self.next(name);
            if self.is_operator(operator)
                || self.kind(operator) == EQ_EQ_EQ_TOKEN
                || self.kind(operator) == BANG_EQ_EQ_TOKEN
                || self.is_unary_minus(operator)
            {
                is_operator = true;
                if self.is_a(operator, TokenType::GT_GT)
                    && self.is_a(self.next(operator), TokenType::GT)
                    && self.end(operator) == self.char_offset(self.next(operator))
                {
                    // Special case use of triple-shift in cases where it isn't enabled.
                    let operator_next = self.next(operator);
                    self.report_experiment_not_enabled(
                        ExperimentalFlag::TripleShift,
                        operator,
                        operator_next,
                    );
                    operator = self.rewriter().replace_next_tokens_with_synthetic_token(
                        name,
                        /* count = */ 2,
                        TokenType::GT_GT_GT,
                    );
                }
            }
        }

        let is_constructor = self.is_constructor(
            name,
            get_or_set,
            new_token,
            enclosing_declaration_name,
            is_operator,
        );

        if let Some(static_tok) = static_token {
            if is_operator {
                self.report_recoverable_error(static_tok, diag::static_operator());
                static_token = None;
            }
        } else if let Some(covariant) = covariant_token {
            if get_or_set.is_none() || self.is_a(get_or_set.unwrap(), Keyword::GET) {
                self.report_recoverable_error(covariant, diag::covariant_member());
                covariant_token = None;
            }
        }
        if let Some(vfc) = var_final_or_const {
            if self.is_a(vfc, Keyword::CONST) {
                if get_or_set.is_some() {
                    self.report_recoverable_error_with_token(vfc, diag::extraneous_modifier);
                    var_final_or_const = None;
                }
            } else if self.is_a(vfc, Keyword::VAR) {
                self.report_recoverable_error(vfc, diag::var_return_type());
                var_final_or_const = None;
            } else {
                // assert(varFinalOrConst.isA(Keyword.FINAL));
                self.report_recoverable_error_with_token(vfc, diag::extraneous_modifier);
                var_final_or_const = None;
            }
        }

        if is_constructor {
            self.listener.begin_constructor(
                kind,
                augment_token,
                external_token,
                static_token,
                covariant_token,
                var_final_or_const,
                get_or_set,
                new_token,
                name,
                enclosing_declaration_name,
            );
        } else {
            // TODO(danrubel): Consider parsing the name before calling beginMethod
            // rather than passing the name token into beginMethod.
            self.listener.begin_method(
                kind,
                augment_token,
                external_token,
                static_token,
                covariant_token,
                var_final_or_const,
                get_or_set,
                name,
                enclosing_declaration_name,
            );
        }

        let mut token = type_info.parse_type(before_type, self);
        // assert(
        //   token.next == (getOrSet ?? name) ||
        //       // [skipType] and [parseType] for something ending in `>>` is
        //       // different because [`>>`] is split to [`>`, `>`] in both cases. For
        //       // skip it's cached as the end but for parse a new pair is created
        //       // (which is also woven into the token stream). At least for now we
        //       // allow this and let the assert not fail because of it.
        //       (token.next!.isA(name.type) && token.next!.offset == name.offset),
        // );
        token = get_or_set.unwrap_or(token);

        let mut has_qualified_name = false;

        if is_operator {
            token = self.parse_operator_name(token);
        } else if new_token.is_some() {
            token = self.next(token);
            if self.is_identifier(self.next(token)) {
                token = self.next(token);
                let identifier = token;
                self.listener
                    .handle_identifier(identifier, IdentifierContext::MethodDeclaration);
                // Recovery: This call only does something if the next token is
                // a '.' --- that's not legal for constructors using 'new' so we'll
                // report an error and recover better by allowing it.
                let qualified = self.parse_qualified_rest_opt(
                    token,
                    IdentifierContext::MethodDeclarationContinuation,
                );
                if token != qualified {
                    has_qualified_name = true;
                    self.report_recoverable_error(token, diag::new_constructor_qualified_name());
                }
                token = qualified;
            } else if self.is_a(self.next(token), Keyword::NEW) {
                // This a constructor declaration like `new new();` which isn't allowed.
                token = self.next(token);
                let identifier = token;
                self.report_recoverable_error(identifier, diag::new_constructor_new_name());
                self.listener
                    .handle_identifier(identifier, IdentifierContext::MethodDeclaration);
            } else if self.is_a(self.next(token), TokenType::PERIOD)
                && self.is_identifier(self.next(self.next(token)))
            {
                // This a constructor declaration like `new.name();` which isn't
                // allowed.
                let dot = self.next(token);
                self.report_recoverable_error(dot, diag::new_constructor_dot_name());
                token = self.next(dot);
                let identifier = token;
                self.listener
                    .handle_identifier(identifier, IdentifierContext::MethodDeclaration);
                // Recovery: This call only does something if the next token is
                // a '.' --- that's not legal for constructors using 'new' so we'll
                // report an error and recover better by allowing it.
                let qualified = self.parse_qualified_rest_opt(
                    token,
                    IdentifierContext::MethodDeclarationContinuation,
                );
                if token != qualified {
                    has_qualified_name = true;
                    self.report_recoverable_error(token, diag::new_constructor_qualified_name());
                }
                token = qualified;
            } else {
                self.listener
                    .handle_no_identifier(token, IdentifierContext::MethodDeclaration);
            }
        } else {
            token = self.ensure_identifier_potentially_recovered(
                token,
                IdentifierContext::MethodDeclaration,
                /* isRecovered = */ name_is_recovered,
            );
            // Possible recovery: This call only does something if the next token is
            // a '.' --- that's not legal for get or set, but an error is reported
            // later, and it will recover better if we allow it.
            let qualified = self
                .parse_qualified_rest_opt(token, IdentifierContext::MethodDeclarationContinuation);
            if token != qualified {
                has_qualified_name = true;
            }
            token = qualified;
        }

        let mut is_considered_getter = false;
        match get_or_set {
            None => {
                token = self.parse_method_type_var(token);
            }
            Some(get_or_set) => {
                is_considered_getter = self.is_a(get_or_set, Keyword::GET);
                let next = self.next(token);
                self.listener.handle_no_type_variables(next);

                // If it becomes considered a constructor below, don't consider it a
                // getter now (this also enforces parenthesis (and thus parameters)).
                if has_qualified_name {
                    is_considered_getter = false;
                } else if is_considered_getter && self.is_a(self.next(token), TokenType::COLON) {
                    is_considered_getter = false;
                } else if is_considered_getter
                    && Some(self.lexeme(name)) == enclosing_declaration_name
                {
                    // This is a simple case of an badly named getter so we don't consider
                    // that a constructor. We issue an error about the name below.
                }
            }
        }
        let member_kind = match kind {
            DeclarationKind::TopLevel
            | DeclarationKind::Class
            | DeclarationKind::Mixin
            | DeclarationKind::Enum => {
                if static_token.is_some() {
                    MemberKind::StaticMethod
                } else {
                    MemberKind::NonStaticMethod
                }
            }
            DeclarationKind::Extension => {
                if static_token.is_some() {
                    MemberKind::ExtensionStaticMethod
                } else {
                    MemberKind::ExtensionNonStaticMethod
                }
            }
            DeclarationKind::ExtensionType => {
                if static_token.is_some() {
                    MemberKind::ExtensionTypeStaticMethod
                } else {
                    MemberKind::ExtensionTypeNonStaticMethod
                }
            }
        };
        let before_param = token;
        let before_initializers_token =
            self.parse_getter_or_formal_parameters(token, name, is_considered_getter, member_kind);
        let mut before_initializers = Some(before_initializers_token);
        token = self.parse_initializers_opt(before_initializers_token);
        if token == before_initializers_token {
            before_initializers = None;
        }

        let saved_async_modifier = self.async_state;
        let async_token = self.next(token);
        token = self.parse_async_modifier_opt(token);
        if let Some(get_or_set) = get_or_set
            && !self.in_plain_sync()
            && self.is_a(get_or_set, Keyword::SET)
        {
            self.report_recoverable_error(async_token, diag::setter_not_sync());
        }
        let body_start = self.next(token);
        if external_token.is_some() {
            if !self.is_a(body_start, TokenType::SEMICOLON) {
                self.report_recoverable_error(body_start, diag::external_method_with_body());
            }
        }
        if self.is_a(body_start, TokenType::EQ) {
            self.report_recoverable_error(body_start, diag::redirection_in_non_factory());
            token = self.parse_redirecting_factory_body(token);
        } else {
            let allow_abstract = (static_token.is_none()
                || external_token.is_some()
                || self.is_augmentations_feature_enabled)
                && self.in_plain_sync();
            token = self.parse_function_body(
                token,
                /* ofFunctionExpression = */ false,
                /* allowAbstract = */ allow_abstract,
            );
        }
        self.async_state = saved_async_modifier;

        if is_constructor {
            //
            // constructor
            //
            if new_token.is_none() && Some(self.lexeme(name)) != enclosing_declaration_name {
                self.report_recoverable_error(name, diag::constructor_with_wrong_name());
            }
            if let Some(static_token) = static_token {
                self.report_recoverable_error(static_token, diag::static_constructor());
            }
            if let Some(get_or_set) = get_or_set {
                if self.is_a(get_or_set, Keyword::GET) {
                    self.report_recoverable_error(get_or_set, diag::getter_constructor());
                } else {
                    self.report_recoverable_error(get_or_set, diag::setter_constructor());
                }
            }
            if *type_info != NO_TYPE {
                let type_start = self.next(before_type);
                self.report_recoverable_error(type_start, diag::constructor_with_return_type());
            }
            if let Some(before_initializers) = before_initializers
                && external_token.is_some()
            {
                let initializers = self.next(before_initializers);
                self.report_recoverable_error(
                    initializers,
                    diag::external_constructor_with_initializer(),
                );
            }

            match kind {
                DeclarationKind::Mixin => {
                    self.report_recoverable_error(name, diag::mixin_declares_constructor());
                }
                DeclarationKind::Extension => {
                    self.report_recoverable_error(name, diag::extension_declares_constructor());
                }
                DeclarationKind::Class | DeclarationKind::ExtensionType | DeclarationKind::Enum => {
                }
                DeclarationKind::TopLevel => {
                    panic!("Internal error: TopLevel constructor.");
                }
            }
            let begin = self.next(before_start);
            let begin_param = self.next(before_param);
            let begin_initializers = before_initializers.map(|t| self.next(t));
            self.listener.end_constructor(
                kind,
                begin,
                new_token,
                begin_param,
                begin_initializers,
                token,
            );
        } else {
            //
            // method
            //
            if Some(self.lexeme(name)) == enclosing_declaration_name && get_or_set.is_some() {
                // Recovery: The (simple) get/set member name is invalid.
                // Report an error and continue with invalid name
                // (keeping it as a getter/setter).
                self.report_recoverable_error(name, diag::member_with_same_name_as_class());
            }
            if let Some(vfc) = var_final_or_const {
                // assert(varFinalOrConst.isA(Keyword.CONST));
                self.report_recoverable_error(vfc, diag::const_method());
            }
            match kind {
                DeclarationKind::Class | DeclarationKind::Mixin | DeclarationKind::Enum => {}
                DeclarationKind::Extension => {
                    if self.is_a(body_start, TokenType::SEMICOLON) && external_token.is_none() {
                        let error_token = if is_operator { self.next(name) } else { name };
                        self.report_recoverable_error(
                            error_token,
                            diag::extension_declares_abstract_member(),
                        );
                    }
                }
                DeclarationKind::ExtensionType => {
                    if self.is_a(body_start, TokenType::SEMICOLON) && external_token.is_none() {
                        let error_token = if is_operator { self.next(name) } else { name };
                        self.report_recoverable_error(
                            error_token,
                            diag::extension_type_declares_abstract_member(),
                        );
                    }
                }
                DeclarationKind::TopLevel => {
                    panic!("Internal error: TopLevel method.");
                }
            }
            // TODO(danrubel): Remove beginInitializers token from method events
            let begin = self.next(before_start);
            let begin_param = self.next(before_param);
            let begin_initializers = before_initializers.map(|t| self.next(t));
            self.listener.end_method(
                kind,
                get_or_set,
                begin,
                begin_param,
                begin_initializers,
                token,
            );
        }
        token
    }

    /// Dart (line 6141): `Token parseFactoryMethod( Token token, DeclarationKind kind, Token beforeStart, Token? augmentToken, Token? externalToken, Token? staticOrCovariant, Token? varFinalOrConst, bool hasName, )`
    pub fn parse_factory_method(
        &mut self,
        token: TokenId,
        kind: DeclarationKind,
        before_start: TokenId,
        augment_token: Option<TokenId>,
        external_token: Option<TokenId>,
        static_or_covariant: Option<TokenId>,
        var_final_or_const: Option<TokenId>,
        has_name: bool,
    ) -> TokenId {
        let mut external_token = external_token;
        let mut static_or_covariant = static_or_covariant;
        let mut var_final_or_const = var_final_or_const;
        let mut token = self.next(token);
        let factory_keyword = token;
        // assert(factoryKeyword.isA(Keyword.FACTORY));

        if !is_valid_non_record_type_reference(self.tokens(), self.next(token)) {
            // Recovery
            let mut context = ModifierContext::new();
            context.external_token = external_token;
            context.set_static_or_covariant(self.tokens(), static_or_covariant);
            context.set_var_final_or_const(self.tokens(), var_final_or_const);

            token = context.parse_modifiers_after_factory(self, token);

            external_token = context.external_token;
            static_or_covariant = context.static_token.or(context.covariant_token);
            var_final_or_const = context.var_final_or_const();
        }

        if let Some(static_or_covariant) = static_or_covariant {
            self.report_recoverable_error_with_token(
                static_or_covariant,
                diag::extraneous_modifier,
            );
        }
        if let Some(vfc) = var_final_or_const
            && !self.is_a(vfc, Keyword::CONST)
        {
            self.report_recoverable_error_with_token(vfc, diag::extraneous_modifier);
            var_final_or_const = None;
        }

        self.listener.begin_factory(
            kind,
            before_start,
            augment_token,
            external_token,
            var_final_or_const,
        );
        if !has_name {
            self.listener
                .handle_no_identifier(token, IdentifierContext::MethodDeclaration);
        } else if self.is_a(self.next(token), Keyword::NEW)
            && !self.is_a(self.next(self.next(token)), TokenType::PERIOD)
        {
            token = self.next(token);
            let identifier = token;
            self.report_recoverable_error(identifier, diag::factory_constructor_new_name());
            self.listener
                .handle_identifier(identifier, IdentifierContext::MethodDeclaration);
        } else {
            token = self.ensure_identifier(token, IdentifierContext::MethodDeclaration);
            token = self
                .parse_qualified_rest_opt(token, IdentifierContext::MethodDeclarationContinuation);
        }
        token = self.parse_method_type_var(token);
        token = self.parse_formal_parameters_required_opt(token, MemberKind::Factory);
        let async_token = self.next(token);
        token = self.parse_async_modifier_opt(token);
        let next = self.next(token);
        if !self.in_plain_sync() {
            self.report_recoverable_error(async_token, diag::factory_not_sync());
        }
        if self.is_a(next, TokenType::EQ) {
            if external_token.is_some() {
                self.report_recoverable_error(next, diag::external_factory_redirection());
            }
            token = self.parse_redirecting_factory_body(token);
        } else if external_token.is_some() {
            if !self.is_a(next, TokenType::SEMICOLON) {
                self.report_recoverable_error(next, diag::external_factory_with_body());
            }
            token = self.parse_function_body(
                token, /* ofFunctionExpression = */ false, /* allowAbstract = */ true,
            );
        } else {
            if let Some(vfc) = var_final_or_const
                && !self.is_a(next, Keyword::NATIVE)
            {
                if self.is_a(vfc, Keyword::CONST) {
                    self.listener.handle_const_factory(vfc);
                }
            }
            let allow_abstract = self.is_augmentations_feature_enabled;
            token = self.parse_function_body(
                token,
                /* ofFunctionExpression = */ false,
                /* allowAbstract = */ allow_abstract,
            );
        }
        match kind {
            DeclarationKind::Class | DeclarationKind::Enum | DeclarationKind::ExtensionType => {}
            DeclarationKind::Mixin => {
                self.report_recoverable_error(factory_keyword, diag::mixin_declares_constructor());
            }
            DeclarationKind::Extension => {
                self.report_recoverable_error(
                    factory_keyword,
                    diag::extension_declares_constructor(),
                );
            }
            DeclarationKind::TopLevel => {
                panic!("Internal error: TopLevel factory.");
            }
        }
        let begin = self.next(before_start);
        self.listener
            .end_factory(kind, begin, factory_keyword, token);
        token
    }

    /// Dart (line 6258): `Token parseOperatorName(Token token)`
    pub fn parse_operator_name(&mut self, token: TokenId) -> TokenId {
        let before_token = token;
        let token = self.next(token);
        // assert(token.isA(Keyword.OPERATOR));
        let mut next = self.next(token);
        if self.is_user_definable_operator(next) {
            if compute_type_param_or_arg_mut(self.tokens_mut(), token, false, false) != NO_TYPE_PARAM_OR_ARG
            {
                // `operator` is being used as an identifier.
                // For example: `int operator<T>(foo) => 0;`
                self.listener
                    .handle_identifier(token, IdentifierContext::MethodDeclaration);
                token
            } else {
                self.listener.handle_operator_name(token, next);
                next
            }
        } else if self.is_a(next, TokenType::OPEN_PAREN) {
            self.ensure_identifier(before_token, IdentifierContext::OPERATOR_NAME)
        } else if self.is_unary_minus(next) {
            // Recovery
            self.report_recoverable_error_with_token(next, diag::unexpected_token);
            next = self.next(next);
            self.listener.handle_operator_name(token, next);
            next
        } else {
            // Recovery
            // Scanner reports an error for `===` and `!==`.
            if self.ty(next) != TokenType::EQ_EQ_EQ && self.ty(next) != TokenType::BANG_EQ_EQ {
                // The user has specified an invalid operator name.
                // Report the error, accept the invalid operator name, and move on.
                self.report_recoverable_error_with_token(next, diag::invalid_operator);
            }
            self.listener.handle_invalid_operator_name(token, next);
            next
        }
    }

    /// Dart (line 6295): `Token parseFunctionExpression(Token token)`
    pub fn parse_function_expression(&mut self, token: TokenId) -> TokenId {
        let begin_token = self.next(token);
        self.listener.begin_function_expression(begin_token);
        let mut token = self.parse_formal_parameters_required_opt(token, MemberKind::Local);
        token = self.parse_async_opt_body(
            token, /* ofFunctionExpression = */ true, /* allowAbstract = */ false,
        );
        self.listener.end_function_expression(begin_token, token);
        token
    }

    /// Dart (line 6308): `Token parseFunctionLiteral( Token start, Token beforeName, Token name, TypeInfo typeInfo, TypeParamOrArgInfo typeParam, IdentifierContext context, )`
    pub fn parse_function_literal(
        &mut self,
        start: TokenId,
        before_name: TokenId,
        name: TokenId,
        type_info: &TypeInfo,
        type_param: TypeParamOrArgInfo,
        context: IdentifierContext,
    ) -> TokenId {
        let formals = type_param.parse_variables(name, self);
        let begin = self.next(start);
        self.listener.begin_named_function_expression(begin);
        type_info.parse_type(start, self);
        let begin = self.next(start);
        self.parse_named_function_rest(
            before_name,
            begin,
            formals,
            /* isFunctionExpression = */ true,
        )
    }

    /// Parses the rest of a named function declaration starting from its name
    /// (the token following [beforeName]) but then skips any type parameters and
    /// continue parsing from [formals] (the formal parameters).
    ///
    /// If [isFunctionExpression] is true, this method parses the rest of named
    /// function expression which isn't legal syntax in Dart.  Useful for
    /// recovering from JavaScript code being pasted into a Dart program, as it
    /// will interpret `function foo() {}` as a named function expression with
    /// return type `function` and name `foo`.
    ///
    /// Precondition: the parser has previously generated these events:
    ///
    /// - Type variables.
    /// - `beginLocalFunctionDeclaration` if [isFunctionExpression] is false,
    ///   otherwise `beginNamedFunctionExpression`.
    /// - Return type.
    /// Dart (line 6343): `Token parseNamedFunctionRest( Token beforeName, Token begin, Token formals, bool isFunctionExpression, )`
    pub fn parse_named_function_rest(
        &mut self,
        before_name: TokenId,
        begin: TokenId,
        formals: TokenId,
        is_function_expression: bool,
    ) -> TokenId {
        let mut token = self.next(before_name);
        self.listener.begin_function_name(token);
        let identifier =
            self.ensure_identifier(before_name, IdentifierContext::LocalFunctionDeclaration);
        token = self.next(identifier);
        if is_function_expression {
            let name = self.next(before_name);
            self.report_recoverable_error(name, diag::named_function_expression());
        }
        self.listener
            .end_function_name(begin, token, is_function_expression);
        token = self.parse_formal_parameters_required_opt(formals, MemberKind::Local);
        token = self.parse_initializers_opt(token);
        token = self.parse_async_opt_body(
            token,
            is_function_expression,
            /* allowAbstract = */ false,
        );
        if is_function_expression {
            self.listener.end_named_function_expression(token);
        } else {
            self.listener.end_local_function_declaration(token);
        }
        token
    }

    /// Parses a function body optionally preceded by an async modifier (see
    /// [parseAsyncModifierOpt]).  This method is used in both expression context
    /// (when [ofFunctionExpression] is true) and statement context. In statement
    /// context (when [ofFunctionExpression] is false), and if the function body
    /// is on the form `=> expression`, a trailing semicolon is required.
    ///
    /// It's an error if there's no function body unless [allowAbstract] is true.
    /// Dart (line 6381): `Token parseAsyncOptBody( Token token, bool ofFunctionExpression, bool allowAbstract, )`
    pub fn parse_async_opt_body(
        &mut self,
        token: TokenId,
        of_function_expression: bool,
        allow_abstract: bool,
    ) -> TokenId {
        let saved_async_modifier = self.async_state;
        let mut token = self.parse_async_modifier_opt(token);
        token = self.parse_function_body(token, of_function_expression, allow_abstract);
        self.async_state = saved_async_modifier;
        token
    }

    /// Dart (line 6393): `Token parseConstructorReference( Token token, ConstructorReferenceContext constructorReferenceContext, [ TypeParamOrArgInfo? typeArg, ])`
    pub fn parse_constructor_reference(
        &mut self,
        token: TokenId,
        constructor_reference_context: ConstructorReferenceContext,
        type_arg: Option<TypeParamOrArgInfo>,
    ) -> TokenId {
        // Note that there's an almost verbatim copy in [parseEnumElement] so
        // any change here should be added there too.
        let start = self.ensure_identifier(token, IdentifierContext::ConstructorReference);
        self.listener.begin_constructor_reference(start);
        let mut token = self
            .parse_qualified_rest_opt(start, IdentifierContext::ConstructorReferenceContinuation);
        let type_arg = match type_arg {
            Some(type_arg) => type_arg,
            None => compute_type_param_or_arg_mut(self.tokens_mut(), token, false, false),
        };
        token = type_arg.parse_arguments(token, self);
        let mut period: Option<TokenId> = None;
        if self.is_a(self.next(token), TokenType::PERIOD) {
            let p = self.next(token);
            period = Some(p);
            token = self.ensure_identifier(
                p,
                IdentifierContext::ConstructorReferenceContinuationAfterTypeArguments,
            );
        } else {
            self.listener
                .handle_no_constructor_reference_continuation_after_type_arguments(token);
        }
        self.listener.end_constructor_reference(
            start,
            period,
            token,
            constructor_reference_context,
        );
        token
    }

    /// Dart (line 6430): `Token parseRedirectingFactoryBody(Token token)`
    pub fn parse_redirecting_factory_body(&mut self, token: TokenId) -> TokenId {
        let mut token = self.next(token);
        // assert(token.isA(TokenType.EQ));
        self.listener.begin_redirecting_factory_body(token);
        let equals = token;
        token = self.parse_constructor_reference(
            token,
            ConstructorReferenceContext::RedirectingFactory,
            None,
        );
        token = self.ensure_semicolon(token);
        self.listener.end_redirecting_factory_body(equals, token);
        token
    }

    /// Dart (line 6444): `Token skipFunctionBody(Token token, bool isExpression, bool allowAbstract)`
    pub fn skip_function_body(
        &mut self,
        token: TokenId,
        is_expression: bool,
        allow_abstract: bool,
    ) -> TokenId {
        // assert(!isExpression);
        let mut token = self.skip_async_modifier(token);
        let mut next = self.next(token);
        if self.is_a(next, Keyword::NATIVE) {
            let native_token = next;
            // TODO(danrubel): skip the native clause rather than parsing it
            // or remove this code completely when we remove support
            // for the `native` clause.
            token = self.parse_native_clause(token);
            next = self.next(token);
            if self.is_a(next, TokenType::SEMICOLON) {
                self.listener
                    .handle_native_function_body_skipped(native_token, next);
                return self.next(token);
            }
            self.listener
                .handle_native_function_body_ignored(native_token, next);
            // Fall through to recover and skip function body
        }
        let value = self.string_value(next);
        if value == Some(";") {
            token = next;
            if !allow_abstract {
                self.report_recoverable_error(token, diag::expected_body());
            }
            self.listener.handle_no_function_body(token);
        } else if value == Some("=>") {
            let begin_token = next;
            token = self.parse_expression(next);
            // There ought to be a semicolon following the expression, but we check
            // before advancing in order to be consistent with the way the method
            // [parseFunctionBody] recovers when the semicolon is missing.
            if self.is_a(self.next(token), TokenType::SEMICOLON) {
                token = self.next(token);
            }
            self.listener.handle_function_body_skipped(
                begin_token,
                token,
                /* isExpressionBody = */ true,
            );
        } else if value == Some("=") {
            token = next;
            let begin_token = token;
            self.report_recoverable_error(token, diag::expected_body());
            token = self.parse_expression(token);
            // There ought to be a semicolon following the expression, but we check
            // before advancing in order to be consistent with the way the method
            // [parseFunctionBody] recovers when the semicolon is missing.
            if self.is_a(self.next(token), TokenType::SEMICOLON) {
                token = self.next(token);
            }
            self.listener.handle_function_body_skipped(
                begin_token,
                token,
                /* isExpressionBody = */ true,
            );
        } else {
            let begin_token = self.next(token);
            token = self.skip_block(token);
            self.listener.handle_function_body_skipped(
                begin_token,
                token,
                /* isExpressionBody = */ false,
            );
        }
        token
    }

    /// Parses a function body.  This method is used in both expression context
    /// (when [ofFunctionExpression] is true) and statement context. In statement
    /// context (when [ofFunctionExpression] is false), and if the function body
    /// is on the form `=> expression`, a trailing semicolon is required.
    ///
    /// It's an error if there's no function body unless [allowAbstract] is true.
    /// Dart (line 6516): `Token parseFunctionBody( Token token, bool ofFunctionExpression, bool allowAbstract, )`
    pub fn parse_function_body(
        &mut self,
        token: TokenId,
        of_function_expression: bool,
        allow_abstract: bool,
    ) -> TokenId {
        let mut token = token;
        let mut next = self.next(token);
        if self.is_a(next, Keyword::NATIVE) {
            let native_token = next;
            token = self.parse_native_clause(token);
            next = self.next(token);
            if self.is_a(next, TokenType::SEMICOLON) {
                self.listener
                    .handle_native_function_body(native_token, next);
                return next;
            }
            self.report_recoverable_error(next, diag::external_method_with_body());
            self.listener
                .handle_native_function_body_ignored(native_token, next);
            // Ignore the native keyword and fall through to parse the body
        }
        if self.is_a(next, TokenType::SEMICOLON) {
            if !allow_abstract {
                self.report_recoverable_error(next, diag::expected_body());
            }
            self.listener.handle_empty_function_body(next);
            return next;
        } else if self.is_a(next, TokenType::FUNCTION) {
            return self.parse_expression_function_body(next, of_function_expression);
        } else if self.is_a(next, TokenType::EQ) {
            // Recover from a bad factory method.
            self.report_recoverable_error(next, diag::expected_body());
            let after = self.next(next);
            let offset = self.char_offset(after);
            let byte_offset = self.tokens().byte_offset(after);
            let arrow = self
                .tokens_mut()
                .push_synthetic(TokenType::FUNCTION, offset, byte_offset);
            next = self.rewriter().insert_token(next, arrow);
            let begin = next;
            token = self.parse_expression(next);
            if !of_function_expression {
                token = self.ensure_semicolon(token);
                self.listener
                    .handle_expression_function_body(begin, Some(token));
            } else {
                self.listener
                    .handle_expression_function_body(begin, /* endToken = */ None);
            }
            return token;
        }
        let mut begin = next;
        let mut statement_count: i32 = 0;
        if !self.is_a(next, TokenType::OPEN_CURLY_BRACKET) {
            // Recovery
            // If `return` used instead of `=>`, then report an error and continue
            if self.is_a(next, Keyword::RETURN) {
                self.report_recoverable_error(next, diag::expected_body());
                let after = self.next(next);
                let offset = self.char_offset(after);
                let byte_offset = self.tokens().byte_offset(after);
                let arrow =
                    self.tokens_mut()
                        .push_synthetic(TokenType::FUNCTION, offset, byte_offset);
                next = self.rewriter().insert_token(next, arrow);
                return self.parse_expression_function_body(next, of_function_expression);
            }
            // If there is a stray simple identifier in the function expression
            // because the user is typing (e.g. `() asy => null;`)
            // then report an error, skip the token, and continue parsing.
            if self.is_keyword_or_identifier(next)
                && self.is_a(self.next(next), TokenType::FUNCTION)
            {
                self.report_recoverable_error_with_token(next, diag::unexpected_token);
                let arrow = self.next(next);
                return self.parse_expression_function_body(arrow, of_function_expression);
            }
            if self.is_keyword_or_identifier(next)
                && self.is_a(self.next(next), TokenType::OPEN_CURLY_BRACKET)
            {
                self.report_recoverable_error_with_token(next, diag::unexpected_token);
                token = next;
                next = self.next(token);
                begin = next;
                // Fall through to parse the block.
            } else {
                token = self.ensure_block(token, Some(BlockKind::FunctionBody));
                self.listener.handle_invalid_function_body(token);
                return self.end_group(token).unwrap();
            }
        }

        let saved_loop_state = self.loop_state;
        self.loop_state = LoopState::OutsideLoop;
        self.listener.begin_block_function_body(begin);
        token = next;
        while self.not_eof_or_type(TokenType::CLOSE_CURLY_BRACKET, self.next(token)) {
            let start_token = self.next(token);
            token = self.parse_statement(token);
            if self.next(token) == start_token {
                // No progress was made, so we report the current token as being invalid
                // and move forward.
                let message = diag::unexpected_token(self.lexeme(token));
                self.report_recoverable_error(token, message);
                token = self.next(token);
            }
            statement_count += 1;
        }
        token = self.next(token);
        // assert(token.isEof || token.isA(TokenType.CLOSE_CURLY_BRACKET));
        self.listener
            .end_block_function_body(statement_count, begin, token);
        self.loop_state = saved_loop_state;
        token
    }

    /// Dart (line 6617): `Token parseExpressionFunctionBody(Token token, bool ofFunctionExpression)`
    pub fn parse_expression_function_body(
        &mut self,
        token: TokenId,
        of_function_expression: bool,
    ) -> TokenId {
        // assert(token.isA(TokenType.FUNCTION));
        let begin = token;
        let mut token = self.parse_expression(token);
        if !of_function_expression {
            token = self.ensure_semicolon(token);
            self.listener
                .handle_expression_function_body(begin, Some(token));
        } else {
            self.listener
                .handle_expression_function_body(begin, /* endToken = */ None);
        }
        if self.in_generator() {
            self.listener
                .handle_invalid_statement(begin, diag::generator_returns_value());
        }
        token
    }

    /// Dart (line 6633): `Token skipAsyncModifier(Token token)`
    pub fn skip_async_modifier(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        let mut value = self.string_value(self.next(token));
        if value == Some("async") {
            token = self.next(token);
            value = self.string_value(self.next(token));

            if value == Some("*") {
                token = self.next(token);
            }
        } else if value == Some("sync") {
            token = self.next(token);
            value = self.string_value(self.next(token));

            if value == Some("*") {
                token = self.next(token);
            }
        }
        token
    }

    /// Dart (line 6653): `Token parseAsyncModifierOpt(Token token)`
    pub fn parse_async_modifier_opt(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        let mut async_token: Option<TokenId> = None;
        let mut star: Option<TokenId> = None;
        self.async_state = AsyncModifier::Sync;
        let mut next = self.next(token);
        if self.is_a(next, Keyword::ASYNC) {
            token = next;
            async_token = Some(token);
            next = self.next(token);
            if self.is_a(next, TokenType::STAR) {
                self.async_state = AsyncModifier::AsyncStar;
                star = Some(next);
                token = next;
            } else {
                self.async_state = AsyncModifier::Async;
            }
        } else if self.is_a(next, Keyword::SYNC) {
            token = next;
            async_token = Some(token);
            next = self.next(token);
            if self.is_a(next, TokenType::STAR) {
                self.async_state = AsyncModifier::SyncStar;
                star = Some(next);
                token = next;
            } else {
                self.report_recoverable_error(token, diag::invalid_sync_modifier());
            }
        }
        self.listener.handle_async_modifier(async_token, star);
        if !self.in_plain_sync() && self.is_a(self.next(token), TokenType::SEMICOLON) {
            let next = self.next(token);
            self.report_recoverable_error(next, diag::abstract_not_sync());
        }
        token
    }
}
