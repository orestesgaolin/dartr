// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/parser_impl.dart (lines 2525-4392)

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
use crate::identifier_context_impl::looks_like_start_of_next_top_level_declaration;
use crate::listener::Listener;
use crate::listener_stack::{DeclarationHeaderRecovery, Layer, MixinHeaderRecovery};
use crate::literal_entry_info::LiteralEntryInfo;
use crate::loop_state::LoopState;
use crate::member_kind::MemberKind;
use crate::modifier_context::{ModifierContext, is_modifier};
use crate::type_info::{
    NO_TYPE, NO_TYPE_PARAM_OR_ARG, TypeInfo, TypeParamOrArgInfo, compute_type,
    compute_type_param_or_arg,
};
use crate::util::is_any_of;

impl<L: Listener> Parser<L> {
    /// Dart (line 2525): `Token parseQualified( Token token, IdentifierContext context, IdentifierContext continuationContext, )`
    pub fn parse_qualified(
        &mut self,
        token: TokenId,
        context: IdentifierContext,
        continuation_context: IdentifierContext,
    ) -> TokenId {
        let mut token = self.ensure_identifier(token, context);
        while self.is_a(self.next(token), TokenType::PERIOD) {
            token = self.parse_qualified_rest(token, continuation_context);
        }
        token
    }

    /// ```text
    /// qualifiedRestOpt:
    ///   qualifiedRest?
    /// ;
    /// ```
    ///
    /// Dart (line 2542): `Token parseQualifiedRestOpt( Token token, IdentifierContext continuationContext, )`
    pub fn parse_qualified_rest_opt(
        &mut self,
        token: TokenId,
        continuation_context: IdentifierContext,
    ) -> TokenId {
        if self.is_a(self.next(token), TokenType::PERIOD) {
            self.parse_qualified_rest(token, continuation_context)
        } else {
            token
        }
    }

    /// ```text
    /// qualifiedRest:
    ///   '.' identifier
    /// ;
    /// ```
    ///
    /// Dart (line 2558): `Token parseQualifiedRest(Token token, IdentifierContext context)`
    pub fn parse_qualified_rest(&mut self, token: TokenId, context: IdentifierContext) -> TokenId {
        let mut token = self.next(token);
        debug_assert!(self.is_a(token, TokenType::PERIOD));
        self.try_rewrite_new_to_identifier(token, context);
        let period = token;
        token = self.ensure_identifier(token, context);
        self.listener.handle_qualified(period);
        token
    }

    /// Dart (line 2568): `Token skipBlock(Token token)`
    pub fn skip_block(&mut self, token: TokenId) -> TokenId {
        // The scanner ensures that `{` always has a closing `}`.
        let block = self.ensure_block(token, /* missingBlockKind = */ None);
        self.end_group(block).unwrap()
    }

    /// Parse the portion of a enum declaration after 'enum'.
    ///
    /// ```text
    /// enumType:
    ///     :    'augment'? 'enum' classNamePart mixins? interfaces? '{'
    ///          enumEntry (',' enumEntry)* ','?
    ///          (';' (metadata memberDeclaration)*)?
    ///          '}'
    /// primaryConstructorNoConst
    ///     :    typeIdentifier typeParameters?
    ///          ('.' identifierOrNew)? declaringParameterList
    ///     ;
    /// classNamePart
    ///     :    'const'? primaryConstructorNoConst
    ///     |    typeWithParameters
    ///     ;
    /// typeWithParameters
    ///     :    typeIdentifier typeParameters?
    ///     ;
    /// enumEntry
    ///     :    metadata id argumentPart?
    ///     |    metadata id typeArguments? '.' id arguments
    /// ```
    ///
    /// Dart (line 2596): `Token parseEnum(Token beginToken, Token? augmentToken, Token enumKeyword)`
    pub fn parse_enum(
        &mut self,
        begin_token: TokenId,
        augment_token: Option<TokenId>,
        enum_keyword: TokenId,
    ) -> TokenId {
        debug_assert!(self.is_a(enum_keyword, Keyword::ENUM));
        self.listener.begin_enum_declaration_prelude(enum_keyword);
        let mut token = enum_keyword;
        let mut const_token: Option<TokenId> = None;
        if self.is_a(self.next(token), Keyword::CONST) {
            token = self.next(token);
            const_token = Some(token);
        }
        token = self.ensure_identifier(token, IdentifierContext::EnumDeclaration);
        let name_token = token;
        let name: String = self.lexeme(name_token).to_string();
        token = compute_type_param_or_arg_mut(self.tokens_mut(),
            token,
            /* inDeclaration = */ true,
            /* allowsVariance = */ true,
        )
        .parse_variables(token, self);
        self.listener
            .begin_enum_declaration(begin_token, augment_token, enum_keyword, name_token);
        token = self.parse_primary_constructor_opt(DeclarationKind::Enum, token, const_token, true);
        token = self.parse_enum_header_opt(token, enum_keyword);
        let mut left_brace = self.next(token);
        let mut element_count: i32 = 0;
        let mut member_count: i32 = 0;
        if self.is_a(left_brace, TokenType::SEMICOLON) {
            if !self.is_primary_constructors_feature_enabled {
                self.report_experiment_not_enabled(
                    ExperimentalFlag::PrimaryConstructors,
                    left_brace,
                    left_brace,
                );
            }
            self.listener
                .handle_enum_header(augment_token, enum_keyword, left_brace);
            self.listener.handle_no_enum_body(left_brace);
            token = left_brace;
        } else if self.is_a(left_brace, TokenType::OPEN_CURLY_BRACKET) {
            self.listener
                .handle_enum_header(augment_token, enum_keyword, left_brace);
            self.listener.begin_enum_body(left_brace);
            token = left_brace;
            loop {
                let mut next = self.next(token);
                if self.is_a(next, TokenType::CLOSE_CURLY_BRACKET)
                    || self.is_a(next, TokenType::SEMICOLON)
                {
                    token = next;
                    break;
                }
                token = self.parse_enum_element(token);
                next = self.next(token);
                element_count += 1;
                if self.is_a(next, TokenType::COMMA) {
                    token = next;
                } else if self.is_a(next, TokenType::CLOSE_CURLY_BRACKET)
                    || self.is_a(next, TokenType::SEMICOLON)
                {
                    token = next;
                    break;
                } else {
                    // Recovery
                    let end_group = self.end_group(left_brace).unwrap();
                    if self.is_synthetic(end_group) {
                        // The scanner did not place the synthetic '}' correctly.
                        token = self.rewriter().move_synthetic(token, end_group);
                        break;
                    } else if self.is_identifier(next) {
                        // If the next token is an identifier, assume a missing comma.
                        // TODO(danrubel): Consider improved recovery for missing `}`
                        // both here and when the scanner inserts a synthetic `}`
                        // for situations such as `enum Letter {a, b   Letter e;`.
                        self.report_recoverable_error(next, diag::expected_but_got(","));
                    } else {
                        // Otherwise assume a missing `}` and exit the loop
                        self.report_recoverable_error(next, diag::expected_but_got("}"));
                        token = self.end_group(left_brace).unwrap();
                        break;
                    }
                }
            }
            self.listener.handle_enum_elements(token, element_count);
            if self.is_a(token, TokenType::SEMICOLON) {
                while self.not_eof_or_type(TokenType::CLOSE_CURLY_BRACKET, self.next(token)) {
                    token = self.parse_class_or_mixin_or_extension_or_enum_member_impl(
                        token,
                        DeclarationKind::Enum,
                        Some(&name),
                    );
                    member_count += 1;
                }
                token = self.next(token);
                debug_assert!(
                    self.is_eof(token) || self.is_a(token, TokenType::CLOSE_CURLY_BRACKET)
                );
            }
            self.listener.end_enum_body(left_brace, token);
        } else {
            left_brace = self.ensure_block(token, Some(BlockKind::EnumDeclaration));
            self.listener
                .handle_enum_header(augment_token, enum_keyword, left_brace);
            self.listener.begin_enum_body(left_brace);
            self.listener.handle_enum_elements(token, element_count);
            token = self.end_group(left_brace).unwrap();
            self.listener.end_enum_body(left_brace, token);
        }
        // assert(
        //   token.isA(TokenType.CLOSE_CURLY_BRACKET) ||
        //       token.isA(TokenType.SEMICOLON),
        // );
        self.listener.end_enum_declaration(
            begin_token,
            enum_keyword,
            left_brace,
            member_count,
            token,
        );
        token
    }

    /// Dart (line 2717): `Token parseEnumHeaderOpt(Token token, Token enumKeyword)`
    pub fn parse_enum_header_opt(&mut self, token: TokenId, enum_keyword: TokenId) -> TokenId {
        let mut token = token;
        const LOOK_FOR_NEXT: &[TokenType] = &[
            TokenType::OPEN_CURLY_BRACKET,
            Keyword::WITH,
            Keyword::IMPLEMENTS,
        ];
        if !is_any_of(self.tokens(), self.next(token), LOOK_FOR_NEXT) {
            // Recovery: Possible unexpected tokens before any clauses.
            let skip_token = self.recovery_small_look_ahead_skip_tokens(token, LOOK_FOR_NEXT);
            if let Some(skip_token) = skip_token {
                token = skip_token;
            }
        }

        let before_with = token;
        token = self.parse_enum_with_clause_opt(token);

        // bool _isOneOfFollowingValues(Token token) =>
        //     token.isA(TokenType.OPEN_CURLY_BRACKET) ||
        //     token.isA(Keyword.IMPLEMENTS);
        let is_one_of_following_values = |parser: &Self, token: TokenId| -> bool {
            parser.is_a(token, TokenType::OPEN_CURLY_BRACKET)
                || parser.is_a(token, Keyword::IMPLEMENTS)
        };

        while !is_one_of_following_values(self, self.next(token)) {
            // Recovery: Skip unexpected tokens and more with clauses.
            // Note that if we find a "with" we've seen one already (otherwise the
            // parseEnumWithClauseOpt call above would have found this 'with').
            let mut skip_token =
                self.recovery_enum_with(token, diag::multiple_clauses("enum", "with"));
            if skip_token.is_none() {
                skip_token = self.recovery_small_look_ahead_skip_tokens(token, LOOK_FOR_NEXT);
            }

            if let Some(skip_token) = skip_token {
                // Skipped tokens.
                token = skip_token;
            } else {
                break;
            }
        }

        token = self.parse_class_or_mixin_or_enum_implements_opt(token);

        let mut has_with_clauses: Option<bool> = None;
        while !self.is_a(self.next(token), TokenType::OPEN_CURLY_BRACKET) {
            if has_with_clauses.is_none() {
                has_with_clauses = Some(self.is_a(self.next(before_with), Keyword::WITH));
            }

            // Recovery: Skip unexpected tokens and more with/implements clauses.
            let message = if has_with_clauses.unwrap() {
                diag::multiple_clauses("enum", "with")
            } else {
                diag::out_of_order_clauses("with", "implements")
            };
            let mut skip_token = self.recovery_enum_with(token, message);
            if skip_token.is_some() {
                has_with_clauses = Some(true);
            }
            if skip_token.is_none() {
                // Note that if we find a "implements" we've seen one already (otherwise
                // the parseClassOrMixinOrEnumImplementsOpt call above would have found
                // this 'implements').
                skip_token = self
                    .recovery_enum_implements(token, diag::multiple_clauses("enum", "implements"));
            }
            if skip_token.is_none() {
                skip_token = self.recovery_small_look_ahead_skip_tokens(token, LOOK_FOR_NEXT);
            }

            if let Some(skip_token) = skip_token {
                // Skipped tokens.
                token = skip_token;
            } else {
                break;
            }
        }

        token
    }

    /// Dart (line 2811): `Token? recoveryEnumWith(Token token, codes.Message message)`
    pub fn recovery_enum_with(&mut self, token: TokenId, message: CfeMessage) -> Option<TokenId> {
        if self.is_a(self.next(token), Keyword::WITH) {
            let next = self.next(token);
            self.report_recoverable_error(next, message);
            self.push_null_listener();
            let token = self.parse_enum_with_clause_opt(token);
            self.pop_null_listener();
            return Some(token);
        }
        None
    }

    /// Dart (line 2823): `Token? recoveryEnumImplements(Token token, codes.Message message)`
    pub fn recovery_enum_implements(
        &mut self,
        token: TokenId,
        message: CfeMessage,
    ) -> Option<TokenId> {
        if self.is_a(self.next(token), Keyword::IMPLEMENTS) {
            let next = self.next(token);
            self.report_recoverable_error(next, message);
            self.push_null_listener();
            let token = self.parse_class_or_mixin_or_enum_implements_opt(token);
            self.pop_null_listener();
            return Some(token);
        }
        None
    }

    /// Allow a small lookahead (currently up to 3 tokens) trying to find any in
    /// [lookFor].
    ///
    /// If any wanted token is found an error is issued about unexpected tokens,
    /// and the last skipped token is returned.
    /// Otherwise null is returned.
    ///
    /// Dart (line 2841): `Token? recoverySmallLookAheadSkipTokens( Token token, List<TokenType> lookFor, )`
    pub fn recovery_small_look_ahead_skip_tokens(
        &mut self,
        token: TokenId,
        look_for: &[TokenType],
    ) -> Option<TokenId> {
        // Recovery: Allow a small lookahead for '{'. E.g. the user might be in
        // the middle of writing 'with' or 'implements'.
        let mut skip_token = self.next(token);
        let mut found_wanted = false;

        if looks_like_start_of_next_top_level_declaration(self.tokens(), skip_token) {
            return None;
        }

        let mut skipped: i32 = 0;
        while skipped < 3 {
            skipped += 1;
            if is_any_of(self.tokens(), self.next(skip_token), look_for) {
                found_wanted = true;
                break;
            }

            skip_token = self.next(skip_token);
            if looks_like_start_of_next_top_level_declaration(self.tokens(), skip_token) {
                return None;
            }
        }

        if found_wanted {
            // Give error and skip the tokens.
            if skipped == 1 {
                let message = diag::unexpected_token(self.lexeme(skip_token));
                self.report_recoverable_error(skip_token, message);
            } else {
                let next = self.next(token);
                self.report_recoverable_error_with_end(next, skip_token, diag::unexpected_tokens());
            }
            return Some(skip_token);
        }

        None
    }

    /// Dart (line 2884): `Token parseEnumElement(Token token)`
    pub fn parse_enum_element(&mut self, token: TokenId) -> TokenId {
        let begin_token = token;
        let mut token = self.parse_metadata_star(token);

        let mut augment_token: Option<TokenId> = None;
        if self.is_a(self.next(token), Keyword::AUGMENT) {
            augment_token = Some(self.next(token));
            token = self.next(token);
        }

        token = self.ensure_identifier(token, IdentifierContext::EnumValueDeclaration);
        let mut has_type_arguments_or_dot = false;
        {
            // This is almost a verbatim copy of [parseConstructorReference] inserted
            // to provide better recovery.
            let start = token;
            self.listener
                .handle_no_type_name_in_constructor_reference(self.next(token));
            self.listener.begin_constructor_reference(start);
            let type_arg: TypeParamOrArgInfo =
                compute_type_param_or_arg_mut(self.tokens_mut(), token, false, false);
            if type_arg != NO_TYPE_PARAM_OR_ARG {
                has_type_arguments_or_dot = true;
            }
            token = type_arg.parse_arguments(token, self);
            let mut period: Option<TokenId> = None;
            if self.is_a(self.next(token), TokenType::PERIOD) {
                has_type_arguments_or_dot = true;
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
                ConstructorReferenceContext::Const,
            );
        }
        let next = self.next(token);
        if self.is_a(next, TokenType::OPEN_PAREN) || has_type_arguments_or_dot {
            token = self.parse_constructor_invocation_arguments(token);
        } else {
            self.listener.handle_no_arguments(token);
        }
        self.listener
            .handle_enum_element(begin_token, augment_token);
        token
    }

    /// Dart (line 2937): `Token parseClassOrNamedMixinApplication( Token beginToken, Token? abstractToken, Token? sealedToken, Token? baseToken, Token? interfaceToken, Token? finalToken, Token? augmentToken, Token? mixinToken, Token classKeyword, )`
    pub fn parse_class_or_named_mixin_application(
        &mut self,
        begin_token: TokenId,
        abstract_token: Option<TokenId>,
        sealed_token: Option<TokenId>,
        base_token: Option<TokenId>,
        interface_token: Option<TokenId>,
        final_token: Option<TokenId>,
        augment_token: Option<TokenId>,
        mixin_token: Option<TokenId>,
        class_keyword: TokenId,
    ) -> TokenId {
        debug_assert!(self.is_a(class_keyword, Keyword::CLASS));
        self.listener
            .begin_class_or_mixin_or_named_mixin_application_prelude(begin_token);
        let mut token = class_keyword;
        let mut const_token: Option<TokenId> = None;
        if self.is_a(self.next(token), Keyword::CONST) {
            token = self.next(token);
            const_token = Some(token);
        }
        let name =
            self.ensure_identifier(token, IdentifierContext::ClassOrMixinOrExtensionDeclaration);
        token = compute_type_param_or_arg_mut(self.tokens_mut(),
            name,
            /* inDeclaration = */ true,
            /* allowsVariance = */ true,
        )
        .parse_variables(name, self);
        if abstract_token.is_some() {
            if let Some(sealed_token) = sealed_token {
                self.report_recoverable_error(sealed_token, diag::abstract_sealed_class());
            } else if let Some(final_token) = final_token {
                if let Some(base_token) = base_token {
                    self.report_recoverable_error_with_end(
                        final_token,
                        base_token,
                        diag::abstract_final_base_class(),
                    );
                } else if let Some(interface_token) = interface_token {
                    self.report_recoverable_error_with_end(
                        final_token,
                        interface_token,
                        diag::abstract_final_interface_class(),
                    );
                }
            }
        }
        if self.is_a(self.next(token), TokenType::EQ) {
            if let Some(const_token) = const_token {
                self.report_recoverable_error(
                    const_token,
                    diag::const_without_primary_constructor(),
                );
            }
            if let Some(augment_token) = augment_token {
                self.report_recoverable_error(
                    augment_token,
                    diag::mixin_application_class_augmentation(),
                );
            }
            self.listener.begin_named_mixin_application(
                begin_token,
                abstract_token,
                sealed_token,
                base_token,
                interface_token,
                final_token,
                augment_token,
                mixin_token,
                name,
            );
            self.parse_named_mixin_application(token, begin_token, class_keyword)
        } else {
            self.listener.begin_class_declaration(
                begin_token,
                abstract_token,
                sealed_token,
                base_token,
                interface_token,
                final_token,
                augment_token,
                mixin_token,
                name,
            );
            let class_name = self.lexeme(name).to_string();
            self.parse_class(token, begin_token, class_keyword, const_token, &class_name)
        }
    }

    /// Dart (line 3027): `Token parseNamedMixinApplication( Token token, Token begin, Token classKeyword, )`
    pub fn parse_named_mixin_application(
        &mut self,
        token: TokenId,
        begin: TokenId,
        class_keyword: TokenId,
    ) -> TokenId {
        let mut token = self.next(token);
        let equals = token;
        debug_assert!(self.is_a(equals, TokenType::EQ));
        token = compute_type_mut(self.tokens_mut(),
            token,
            /* required = */ true,
            false,
            false,
        )
        .ensure_type_not_void(token, self);
        token = self.parse_mixin_application_rest(token);
        let mut implements_keyword: Option<TokenId> = None;
        if self.is_a(self.next(token), Keyword::IMPLEMENTS) {
            let keyword = self.next(token);
            implements_keyword = Some(keyword);
            token = self.parse_type_list(keyword);
        }
        token = self.ensure_semicolon(token);
        self.listener.end_named_mixin_application(
            begin,
            class_keyword,
            equals,
            implements_keyword,
            token,
        );
        token
    }

    /// Parse the portion of a class declaration (not a mixin application) that
    /// follows the end of the type parameters.
    ///
    /// ```text
    /// classDeclaration
    ///     :    'augment'? (classModifiers | mixinClassModifiers)
    ///          'class' classNamePart superclass? interfaces? classBody
    ///     ;
    /// primaryConstructorNoConst
    ///     :    typeIdentifier typeParameters?
    ///          ('.' identifierOrNew)? declaringParameterList
    ///     ;
    /// classNamePart
    ///     :    'const'? primaryConstructorNoConst
    ///     |    typeWithParameters
    ///     ;
    /// typeWithParameters
    ///     :    typeIdentifier typeParameters?
    ///     ;
    /// classBody
    ///     :    '{' (metadata memberDeclaration)* '}'
    ///     |    ';'
    ///     ;
    /// ```
    ///
    /// Dart (line 3079): `Token parseClass( Token token, Token beginToken, Token classKeyword, Token? constToken, String className, )`
    pub fn parse_class(
        &mut self,
        token: TokenId,
        begin_token: TokenId,
        class_keyword: TokenId,
        const_token: Option<TokenId>,
        class_name: &str,
    ) -> TokenId {
        let start = token;
        let mut token =
            self.parse_primary_constructor_opt(DeclarationKind::Class, token, const_token, true);
        token = self.parse_class_header_opt(token, begin_token, class_keyword);
        if self.is_a(self.next(token), TokenType::SEMICOLON) {
            token = self.next(token);
            let semicolon_token = token;
            if !self.is_primary_constructors_feature_enabled {
                self.report_experiment_not_enabled(
                    ExperimentalFlag::PrimaryConstructors,
                    semicolon_token,
                    semicolon_token,
                );
            }
            self.listener.handle_no_class_body(semicolon_token);
        } else {
            if !self.is_a(self.next(token), TokenType::OPEN_CURLY_BRACKET) {
                // Recovery
                token = self.parse_class_header_recovery(start, begin_token, class_keyword);
                self.ensure_block(token, Some(BlockKind::ClassDeclaration));
            }
            token = self.parse_class_or_mixin_or_extension_body(
                token,
                DeclarationKind::Class,
                Some(class_name),
            );
        }
        self.listener.end_class_declaration(begin_token, token);
        token
    }

    /// Dart (line 3119): `Token parseClassHeaderOpt(Token token, Token begin, Token classKeyword)`
    pub fn parse_class_header_opt(
        &mut self,
        token: TokenId,
        begin: TokenId,
        class_keyword: TokenId,
    ) -> TokenId {
        let mut token = self.parse_class_extends_opt(token, DeclarationHeaderKind::Class);
        token = self.parse_class_with_clause_opt(token);
        token = self.parse_class_or_mixin_or_enum_implements_opt(token);
        let mut native_token: Option<TokenId> = None;
        if self.is_a(self.next(token), Keyword::NATIVE) {
            native_token = Some(self.next(token));
            token = self.parse_native_clause(token);
        }
        self.listener
            .handle_class_header(begin, class_keyword, native_token);
        token
    }

    /// Recover given out-of-order clauses in a class header.
    ///
    /// Dart (line 3133): `Token parseClassHeaderRecovery(Token token, Token begin, Token classKeyword)`
    pub fn parse_class_header_recovery(
        &mut self,
        token: TokenId,
        begin: TokenId,
        class_keyword: TokenId,
    ) -> TokenId {
        self.parse_declaration_header_recovery_internal(
            token,
            begin,
            class_keyword,
            DeclarationHeaderKind::Class,
        )
    }

    /// Recover given out-of-order clauses in an extension type header.
    ///
    /// Dart (line 3143): `Token parseExtensionTypeHeaderRecovery(Token token, Token extensionKeyword)`
    pub fn parse_extension_type_header_recovery(
        &mut self,
        token: TokenId,
        extension_keyword: TokenId,
    ) -> TokenId {
        self.parse_declaration_header_recovery_internal(
            token,
            extension_keyword,
            extension_keyword,
            DeclarationHeaderKind::ExtensionType,
        )
    }

    /// The fields of the current Dart `DeclarationHeaderRecoveryListener`.
    fn declaration_header_recovery_state(&mut self) -> DeclarationHeaderRecovery {
        *self.listener.declaration_header_recovery().1
    }

    /// Recover given out-of-order clauses in a class, enum, mixin, extension, or
    /// extension type header.
    ///
    /// Dart (line 3154): `Token parseDeclarationHeaderRecoveryInternal( Token token, Token begin, Token declarationKeyword, DeclarationHeaderKind kind, )`
    pub fn parse_declaration_header_recovery_internal(
        &mut self,
        token: TokenId,
        begin: TokenId,
        declaration_keyword: TokenId,
        kind: DeclarationHeaderKind,
    ) -> TokenId {
        let mut token = token;
        // final Listener primaryListener = listener;
        // final DeclarationHeaderRecoveryListener recoveryListener =
        //     new DeclarationHeaderRecoveryListener();

        // Reparse to determine which clauses have already been parsed
        // but intercept the events so they are not sent to the primary listener.
        self.listener.push_layer(Layer::DeclarationHeaderRecovery {
            forwarding: false,
            state: Default::default(),
        });
        match kind {
            DeclarationHeaderKind::Class => {
                token = self.parse_class_header_opt(token, begin, declaration_keyword);
            }
            DeclarationHeaderKind::ExtensionType => {
                token = self.parse_class_or_mixin_or_enum_implements_opt(token);
            }
        }
        let state = self.declaration_header_recovery_state();
        let mut has_extends = state.extends_keyword.is_some();
        let mut has_implements = state.implements_keyword.is_some();
        let mut has_with = state.with_keyword.is_some();

        // Update the recovery listener to forward subsequent events
        // to the primary listener.
        *self.listener.declaration_header_recovery().0 = true;

        // Parse additional out-of-order clauses
        let mut start: TokenId;
        loop {
            start = token;

            // Check for extraneous token in the middle of a declaration header.
            token = self.skip_unexpected_token_opt(token, &["extends", "with", "implements", "{"]);

            // During recovery, clauses are parsed in the same order
            // and generate the same events as in the parseClassHeader method above.
            *self.listener.declaration_header_recovery().1 = Default::default();

            let next = self.next(token);
            if self.is_keyword_or_identifier(next) && ["extend", "on"].contains(&self.lexeme(next))
            {
                self.report_recoverable_error(next, diag::expected_instead("extends"));
                token = self.parse_class_extends_seen_extends_clause(next, token, kind);
            } else {
                token = self.parse_class_extends_opt(token, kind);
            }

            if let Some(extends_keyword) = self.declaration_header_recovery_state().extends_keyword
            {
                match kind {
                    DeclarationHeaderKind::Class => {
                        if has_extends {
                            self.report_recoverable_error(
                                extends_keyword,
                                diag::multiple_extends(),
                            );
                        } else {
                            if has_with {
                                self.report_recoverable_error(
                                    extends_keyword,
                                    diag::with_before_extends(),
                                );
                            } else if has_implements {
                                self.report_recoverable_error(
                                    extends_keyword,
                                    diag::implements_before_extends(),
                                );
                            }
                            has_extends = true;
                        }
                    }
                    DeclarationHeaderKind::ExtensionType => {
                        self.report_recoverable_error(
                            extends_keyword,
                            diag::extension_type_extends(),
                        );
                    }
                }
            }

            token = self.parse_class_with_clause_opt(token);

            if let Some(with_keyword) = self.declaration_header_recovery_state().with_keyword {
                match kind {
                    DeclarationHeaderKind::Class => {
                        if has_with {
                            self.report_recoverable_error(with_keyword, diag::multiple_with());
                        } else {
                            if has_implements {
                                self.report_recoverable_error(
                                    with_keyword,
                                    diag::implements_before_with(),
                                );
                            }
                            has_with = true;
                        }
                    }
                    DeclarationHeaderKind::ExtensionType => {
                        self.report_recoverable_error(with_keyword, diag::extension_type_with());
                    }
                }
            }

            token = self.parse_class_or_mixin_or_enum_implements_opt(token);

            if let Some(implements_keyword) =
                self.declaration_header_recovery_state().implements_keyword
            {
                if has_implements {
                    self.report_recoverable_error(implements_keyword, diag::multiple_implements());
                } else {
                    has_implements = true;
                }
            }

            self.listener.handle_recover_declaration_header(kind);

            // Exit if a declaration body is detected, or if no progress has been made
            if !(!self.is_a(self.next(token), TokenType::OPEN_CURLY_BRACKET) && start != token) {
                break;
            }
        }

        // listener = primaryListener;
        self.pop_layer();
        token
    }

    /// Dart (line 3288): `Token parseClassExtendsOpt(Token token, DeclarationHeaderKind kind)`
    pub fn parse_class_extends_opt(
        &mut self,
        token: TokenId,
        kind: DeclarationHeaderKind,
    ) -> TokenId {
        let mut token = token;
        // extends <typeNotVoid>
        let next = self.next(token);
        if self.is_a(next, Keyword::EXTENDS) {
            token = self.parse_class_extends_seen_extends_clause(next, token, kind);
        } else {
            self.listener.handle_no_type(token);
            self.listener
                .handle_class_extends(/* extendsKeyword = */ None, /* typeCount = */ 1);
        }
        token
    }

    /// Dart (line 3303): `Token parseClassExtendsSeenExtendsClause( Token extendsKeyword, Token token, DeclarationHeaderKind kind, )`
    pub fn parse_class_extends_seen_extends_clause(
        &mut self,
        extends_keyword: TokenId,
        token: TokenId,
        kind: DeclarationHeaderKind,
    ) -> TokenId {
        let mut next = extends_keyword;
        let mut token = compute_type_mut(self.tokens_mut(),
            next,
            /* required = */ true,
            false,
            false,
        )
        .ensure_type_not_void(next, self);
        let mut count: i32 = 1;

        // Error recovery: extends <typeNotVoid>, <typeNotVoid> [...]
        if self.is_a(self.next(token), TokenType::COMMA) {
            match kind {
                DeclarationHeaderKind::Class => {
                    let comma = self.next(token);
                    self.report_recoverable_error(comma, diag::multiple_extends());
                }
                DeclarationHeaderKind::ExtensionType => {
                    // This is an error case. The error is reported elsewhere.
                }
            }

            while self.is_a(self.next(token), TokenType::COMMA) {
                next = self.next(token);
                token = compute_type_mut(self.tokens_mut(),
                    next,
                    /* required = */ true,
                    false,
                    false,
                )
                .ensure_type_not_void(next, self);
                count += 1;
            }
        }

        self.listener
            .handle_class_extends(Some(extends_keyword), count);
        token
    }

    /// ```text
    /// implementsClause:
    ///   'implements' typeName (',' typeName)*
    /// ;
    /// ```
    ///
    /// Dart (line 3345): `Token parseClassOrMixinOrEnumImplementsOpt(Token token)`
    pub fn parse_class_or_mixin_or_enum_implements_opt(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        let mut implements_keyword: Option<TokenId> = None;
        let mut interfaces_count: i32 = 0;
        if self.is_a(self.next(token), Keyword::IMPLEMENTS) {
            implements_keyword = Some(self.next(token));
            loop {
                let next = self.next(token);
                token = compute_type_mut(self.tokens_mut(),
                    next,
                    /* required = */ true,
                    false,
                    false,
                )
                .ensure_type_not_void(next, self);
                interfaces_count += 1;
                if !self.is_a(self.next(token), TokenType::COMMA) {
                    break;
                }
            }
        }
        self.listener
            .handle_implements(implements_keyword, interfaces_count);
        token
    }

    /// Parse a mixin declaration.
    ///
    /// ```text
    /// mixinDeclaration:
    ///   metadata? 'augment'? 'base'? 'mixin' [SimpleIdentifier]
    ///        [TypeParameterList]? [OnClause]? [ImplementsClause]?
    ///        '{' [ClassMember]* '}'
    /// ;
    /// ```
    ///
    /// Dart (line 3371): `Token parseMixin( Token beginToken, Token? augmentToken, Token? baseToken, Token mixinKeyword, )`
    pub fn parse_mixin(
        &mut self,
        begin_token: TokenId,
        augment_token: Option<TokenId>,
        base_token: Option<TokenId>,
        mixin_keyword: TokenId,
    ) -> TokenId {
        debug_assert!(self.is_a(mixin_keyword, Keyword::MIXIN));
        self.listener
            .begin_class_or_mixin_or_named_mixin_application_prelude(mixin_keyword);
        let mut token = mixin_keyword;
        let mut const_keyword: Option<TokenId> = None;
        if self.is_a(self.next(token), Keyword::CONST) {
            // Error recovery. Error reported in [parsePrimaryConstructorOpt] called
            // through [parseMixinHeaderOpt].
            token = self.next(token);
            const_keyword = Some(token);
        }
        let name =
            self.ensure_identifier(token, IdentifierContext::ClassOrMixinOrExtensionDeclaration);
        let header_start = compute_type_param_or_arg_mut(self.tokens_mut(),
            name,
            /* inDeclaration = */ true,
            /* allowsVariance = */ true,
        )
        .parse_variables(name, self);
        self.listener.begin_mixin_declaration(
            begin_token,
            augment_token,
            base_token,
            mixin_keyword,
            name,
        );
        token = self.parse_mixin_header_opt(header_start, const_keyword, mixin_keyword);
        if self.is_a(self.next(token), TokenType::SEMICOLON) {
            token = self.next(token);
            let semicolon_token = token;
            if !self.is_primary_constructors_feature_enabled {
                self.report_experiment_not_enabled(
                    ExperimentalFlag::PrimaryConstructors,
                    semicolon_token,
                    semicolon_token,
                );
            }
            self.listener.handle_no_mixin_body(semicolon_token);
        } else {
            if !self.is_a(self.next(token), TokenType::OPEN_CURLY_BRACKET) {
                // Recovery
                token = self.parse_mixin_header_recovery(token, mixin_keyword, header_start);
                self.ensure_block(token, Some(BlockKind::MixinDeclaration));
            }
            let mixin_name = self.lexeme(name).to_string();
            token = self.parse_class_or_mixin_or_extension_body(
                token,
                DeclarationKind::Mixin,
                Some(&mixin_name),
            );
        }
        self.listener.end_mixin_declaration(begin_token, token);
        token
    }

    /// Dart (line 3429): `Token parseMixinHeaderOpt( Token token, Token? constKeyword, Token mixinKeyword, )`
    pub fn parse_mixin_header_opt(
        &mut self,
        token: TokenId,
        const_keyword: Option<TokenId>,
        mixin_keyword: TokenId,
    ) -> TokenId {
        let mut token =
            self.parse_primary_constructor_opt(DeclarationKind::Mixin, token, const_keyword, true);
        token = self.parse_mixin_on_opt(token);
        token = self.parse_class_or_mixin_or_enum_implements_opt(token);
        self.listener.handle_mixin_header(mixin_keyword);
        token
    }

    /// The fields of the current Dart `MixinHeaderRecoveryListener`.
    fn mixin_header_recovery_state(&mut self) -> MixinHeaderRecovery {
        *self.listener.mixin_header_recovery().1
    }

    /// Dart (line 3445): `Token parseMixinHeaderRecovery( Token token, Token mixinKeyword, Token headerStart, )`
    pub fn parse_mixin_header_recovery(
        &mut self,
        token: TokenId,
        mixin_keyword: TokenId,
        header_start: TokenId,
    ) -> TokenId {
        // final Listener primaryListener = listener;
        // final MixinHeaderRecoveryListener recoveryListener =
        //     new MixinHeaderRecoveryListener();

        // Reparse to determine which clauses have already been parsed
        // but intercept the events so they are not sent to the primary listener.
        self.listener.push_layer(Layer::MixinHeaderRecovery {
            forwarding: false,
            state: Default::default(),
        });
        let mut token =
            self.parse_mixin_header_opt(header_start, /* constKeyword */ None, mixin_keyword);
        let state = self.mixin_header_recovery_state();
        let mut has_on = state.on_keyword.is_some();
        let mut has_implements = state.implements_keyword.is_some();

        // Update the recovery listener to forward subsequent events
        // to the primary listener.
        *self.listener.mixin_header_recovery().0 = true;

        // Parse additional out-of-order clauses
        let mut start: TokenId;
        loop {
            start = token;

            // Check for extraneous token in the middle of a class header.
            token = self.skip_unexpected_token_opt(token, &["on", "implements", "{"]);

            // During recovery, clauses are parsed in the same order and
            // generate the same events as in the parseMixinHeaderOpt method above.
            *self.listener.mixin_header_recovery().1 = Default::default();

            let next = self.next(token);
            if self.is_keyword_or_identifier(next)
                && ["extend", "extends"].contains(&self.lexeme(next))
            {
                self.report_recoverable_error(next, diag::expected_instead("on"));
                token = self.parse_mixin_on(token);
            } else {
                token = self.parse_mixin_on_opt(token);
            }

            if let Some(on_keyword) = self.mixin_header_recovery_state().on_keyword {
                if has_on {
                    self.report_recoverable_error(on_keyword, diag::multiple_on_clauses());
                } else {
                    if has_implements {
                        self.report_recoverable_error(on_keyword, diag::implements_before_on());
                    }
                    has_on = true;
                }
            }

            token = self.parse_class_or_mixin_or_enum_implements_opt(token);

            if let Some(implements_keyword) = self.mixin_header_recovery_state().implements_keyword
            {
                if has_implements {
                    self.report_recoverable_error(implements_keyword, diag::multiple_implements());
                } else {
                    has_implements = true;
                }
            }

            if self.is_a(self.next(token), Keyword::WITH) {
                let with_keyword = self.next(token);
                self.report_recoverable_error(with_keyword, diag::mixin_with_clause());
                token = self.parse_type_list(with_keyword);
                self.listener.handle_mixin_with_clause(with_keyword);
            }

            self.listener.handle_recover_mixin_header();

            // Exit if a mixin body is detected, or if no progress has been made
            if !(!self.is_a(self.next(token), TokenType::OPEN_CURLY_BRACKET) && start != token) {
                break;
            }
        }

        // listener = primaryListener;
        self.pop_layer();
        token
    }

    /// ```text
    /// onClause:
    ///   'on' typeName (',' typeName)*
    /// ;
    /// ```
    ///
    /// Dart (line 3547): `Token parseMixinOnOpt(Token token)`
    pub fn parse_mixin_on_opt(&mut self, token: TokenId) -> TokenId {
        if !self.is_a(self.next(token), Keyword::ON) {
            self.listener
                .handle_mixin_on(/* onKeyword = */ None, /* typeCount = */ 0);
            return token;
        }
        self.parse_mixin_on(token)
    }

    /// Dart (line 3555): `Token parseMixinOn(Token token)`
    pub fn parse_mixin_on(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        let on_keyword = self.next(token);
        // During recovery, the [onKeyword] can be "extend" or "extends"
        debug_assert!(
            self.is_a(on_keyword, Keyword::ON)
                || self.is_a(on_keyword, Keyword::EXTENDS)
                || self.lexeme(on_keyword) == "extend"
        );
        let mut type_count: i32 = 0;
        loop {
            let next = self.next(token);
            token = compute_type_mut(self.tokens_mut(),
                next,
                /* required = */ true,
                false,
                false,
            )
            .ensure_type_not_void(next, self);
            type_count += 1;
            if !self.is_a(self.next(token), TokenType::COMMA) {
                break;
            }
        }
        self.listener.handle_mixin_on(Some(on_keyword), type_count);
        token
    }

    /// Parses an extension or extension type declaration.
    ///
    /// Dart (line 3576): `Token parseExtension( Token beginToken, Token? augmentToken, Token extensionKeyword, )`
    pub fn parse_extension(
        &mut self,
        begin_token: TokenId,
        augment_token: Option<TokenId>,
        extension_keyword: TokenId,
    ) -> TokenId {
        debug_assert!(self.is_a(extension_keyword, Keyword::EXTENSION));
        let token = extension_keyword;
        self.listener
            .begin_extension_declaration_prelude(extension_keyword);
        let next = self.next(token);
        if self.is_identifier(next) && self.lexeme(next) == "type" {
            // 'extension' 'type'
            let type_keyword = next;
            self.parse_extension_type_declaration(
                begin_token,
                next,
                augment_token,
                extension_keyword,
                type_keyword,
            )
        } else {
            self.parse_extension_declaration(begin_token, token, augment_token, extension_keyword)
        }
    }

    /// Parses an extension declaration after
    ///
    ///    'extension'
    ///
    /// This parses
    ///
    /// ```text
    ///    <identifier>? <typeParameters>?
    ///       (('.' <identifier>)? <implementsClause>) | ('on' <type> '?'?)
    ///   `{'
    ///     <memberDeclaration>*
    ///   `}'
    /// ```
    ///
    /// Dart (line 3618): `Token parseExtensionDeclaration( Token beginToken, Token token, Token? augmentToken, Token extensionKeyword, )`
    pub fn parse_extension_declaration(
        &mut self,
        begin_token: TokenId,
        token: TokenId,
        augment_token: Option<TokenId>,
        extension_keyword: TokenId,
    ) -> TokenId {
        let mut token = token;
        debug_assert!(self.is_a(extension_keyword, Keyword::EXTENSION));
        let mut const_keyword: Option<TokenId> = None;
        if self.is_a(self.next(token), Keyword::CONST) {
            // Error recovery. Error reported in [parsePrimaryConstructorOpt].
            token = self.next(token);
            const_keyword = Some(token);
        }
        let mut name: Option<TokenId> = Some(self.next(token));
        let n = self.next(token);
        if self.is_identifier(n) && !self.is_a(n, Keyword::ON) {
            token = n;
            if self.is_built_in(n) {
                self.report_recoverable_error_with_token(
                    token,
                    diag::built_in_identifier_in_declaration,
                );
            }
        } else {
            name = None;
        }
        token =
            compute_type_param_or_arg_mut(self.tokens_mut(), token, /* inDeclaration = */ true, false)
                .parse_variables(token, self);
        self.listener
            .begin_extension_declaration(augment_token, extension_keyword, name);
        token = self.parse_primary_constructor_opt(
            DeclarationKind::Extension,
            token,
            const_keyword,
            true,
        );

        let mut on_keyword: Option<TokenId> = Some(self.next(token));
        if augment_token.is_some() {
            let on = on_keyword.unwrap();
            if !self.is_a(on, Keyword::ON) {
                // Extension augmentations should not provide `on` clauses.
                on_keyword = None;
            } else {
                // If `on` clause is provided, report, but parse it.
                self.report_recoverable_error(on, diag::extension_augmentation_has_on_clause());
                let type_info: TypeInfo =
                    compute_type_mut(self.tokens_mut(), on, /* required = */ true, false, false);
                token = type_info.ensure_type_or_void(on, self);
            }
        } else {
            let mut on = on_keyword.unwrap();
            if !self.is_a(on, Keyword::ON) {
                // Recovery
                if self.is_a(on, Keyword::EXTENDS)
                    || self.is_a(on, Keyword::IMPLEMENTS)
                    || self.is_a(on, Keyword::WITH)
                {
                    self.report_recoverable_error(on, diag::expected_instead("on"));
                } else {
                    self.report_recoverable_error(token, diag::expected_after_but_got("on"));
                    on = self.rewriter().insert_synthetic_keyword(token, Keyword::ON);
                    on_keyword = Some(on);
                }
            }
            let type_info: TypeInfo =
                compute_type_mut(self.tokens_mut(), on, /* required = */ true, false, false);
            token = type_info.ensure_type_or_void(on, self);
        }

        if self.is_a(self.next(token), TokenType::SEMICOLON) {
            token = self.next(token);
            let semicolon_token = token;
            if !self.is_primary_constructors_feature_enabled {
                self.report_experiment_not_enabled(
                    ExperimentalFlag::PrimaryConstructors,
                    semicolon_token,
                    semicolon_token,
                );
            }
            self.listener.handle_no_extension_body(semicolon_token);
        } else {
            if !self.is_a(self.next(token), TokenType::OPEN_CURLY_BRACKET) {
                // Recovery
                let mut next = self.next(token);
                while !self.is_eof(next) {
                    if self.is_a(next, TokenType::COMMA)
                        || self.is_a(next, Keyword::EXTENDS)
                        || self.is_a(next, Keyword::IMPLEMENTS)
                        || self.is_a(next, Keyword::ON)
                        || self.is_a(next, Keyword::WITH)
                    {
                        // Report an error and skip `,` or specific keyword
                        // optionally followed by an identifier
                        self.report_recoverable_error_with_token(next, diag::unexpected_token);
                        token = next;
                        next = self.next(token);
                        if self.is_identifier(next) {
                            token = next;
                            next = self.next(token);
                        }
                    } else {
                        break;
                    }
                }
                self.ensure_block(token, Some(BlockKind::ExtensionDeclaration));
            }
            let name_lexeme: Option<String> = name.map(|n| self.lexeme(n).to_string());
            token = self.parse_class_or_mixin_or_extension_body(
                token,
                DeclarationKind::Extension,
                name_lexeme.as_deref(),
            );
        }
        self.listener
            .end_extension_declaration(begin_token, extension_keyword, on_keyword, token);
        token
    }

    /// Dart (line 3739): `Token parsePrimaryConstructorOpt( DeclarationKind kind, Token token, Token? constKeyword,`
    /// `{ bool allowExtensionTypeRepresentation = true, })`
    pub fn parse_primary_constructor_opt(
        &mut self,
        kind: DeclarationKind,
        token: TokenId,
        const_keyword: Option<TokenId>,
        allow_extension_type_representation: bool,
    ) -> TokenId {
        let mut token = token;
        debug_assert!(
            allow_extension_type_representation || kind == DeclarationKind::ExtensionType
        );
        if self.is_a(self.next(token), TokenType::OPEN_PAREN)
            || self.is_a(self.next(token), TokenType::PERIOD)
        {
            let begin_primary_constructor = self.next(token);

            match kind {
                DeclarationKind::TopLevel => {
                    // assert(false, "Unexpected primary constructor kind: $kind");
                }
                DeclarationKind::Mixin => {
                    let error_token = const_keyword.unwrap_or(begin_primary_constructor);
                    if self.is_primary_constructors_feature_enabled {
                        self.report_recoverable_error(
                            error_token,
                            diag::mixin_primary_constructor(),
                        );
                    } else {
                        let message = diag::unexpected_token(self.lexeme(error_token));
                        self.report_recoverable_error(error_token, message);
                    }
                }
                DeclarationKind::Extension => {
                    let error_token = const_keyword.unwrap_or(begin_primary_constructor);
                    if self.is_primary_constructors_feature_enabled {
                        self.report_recoverable_error(
                            error_token,
                            diag::extension_primary_constructor(),
                        );
                    } else {
                        let message = diag::unexpected_token(self.lexeme(error_token));
                        self.report_recoverable_error(error_token, message);
                    }
                }
                DeclarationKind::Class => {
                    // Valid case.
                }
                DeclarationKind::ExtensionType => {
                    if !allow_extension_type_representation {
                        self.report_recoverable_error(
                            const_keyword.unwrap_or(begin_primary_constructor),
                            diag::extension_type_augmentation_specifies_representation_field(),
                        );
                    }
                }
                DeclarationKind::Enum => {
                    // Valid case.
                }
            }

            self.listener
                .begin_primary_constructor(begin_primary_constructor);
            let has_constructor_name = self.is_a(begin_primary_constructor, TokenType::PERIOD);
            if has_constructor_name {
                token = self.ensure_identifier(
                    begin_primary_constructor,
                    IdentifierContext::PrimaryConstructorDeclaration,
                );
            }
            if self.is_a(self.next(token), TokenType::OPEN_PAREN) {
                token = self.parse_formal_parameters(token, MemberKind::PrimaryConstructor);
            } else {
                let report_missing_parameters = match kind {
                    DeclarationKind::Class | DeclarationKind::Enum => true,
                    DeclarationKind::ExtensionType => allow_extension_type_representation,
                    _ => false,
                };
                if report_missing_parameters {
                    self.report_recoverable_error(
                        token,
                        diag::missing_primary_constructor_parameters(),
                    );
                }
                self.listener
                    .handle_no_formal_parameters(token, MemberKind::PrimaryConstructor);
            }
            self.listener.end_primary_constructor(
                kind,
                begin_primary_constructor,
                token,
                const_keyword,
                has_constructor_name,
            );
        } else {
            if kind == DeclarationKind::ExtensionType && allow_extension_type_representation {
                self.report_recoverable_error(token, diag::missing_primary_constructor());
            } else if let Some(const_keyword) = const_keyword {
                if self.is_primary_constructors_feature_enabled {
                    match kind {
                        DeclarationKind::TopLevel => {
                            // assert(false, "Unexpected primary constructor kind: $kind");
                        }
                        DeclarationKind::Mixin => {
                            self.report_recoverable_error(
                                const_keyword,
                                diag::mixin_primary_constructor(),
                            );
                        }
                        DeclarationKind::Extension => {
                            self.report_recoverable_error(
                                const_keyword,
                                diag::extension_primary_constructor(),
                            );
                        }
                        DeclarationKind::Class
                        | DeclarationKind::ExtensionType
                        | DeclarationKind::Enum => {
                            self.report_recoverable_error(
                                const_keyword,
                                diag::const_without_primary_constructor(),
                            );
                        }
                    }
                } else {
                    let message = diag::unexpected_token(self.lexeme(const_keyword));
                    self.report_recoverable_error(const_keyword, message);
                }
            }
            self.listener
                .handle_no_primary_constructor(kind, token, const_keyword);
        }
        token
    }

    /// Dart (line 3869): `Token parsePrimaryConstructorBody(Token token)`
    pub fn parse_primary_constructor_body(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        let begin_token = token;
        self.listener.begin_primary_constructor_body(token);

        let mut before_initializers: Option<TokenId> = Some(token);
        token = self.parse_initializers_opt(token);
        if Some(token) == before_initializers {
            before_initializers = None;
        }

        let next = self.next(token);
        if self.is_a(next, Keyword::ASYNC) || self.is_a(next, Keyword::SYNC) {
            let mut modifier: String = self.lexeme(next).to_string();
            if self.is_a(self.next(next), TokenType::STAR) {
                modifier.push('*');
            }
            self.report_recoverable_error(
                next,
                diag::primary_constructor_body_with_modifier(&modifier),
            );
        }

        token = self.parse_async_modifier_opt(token);
        let allow_abstract = self.in_plain_sync();
        token = self.parse_function_body(
            token,
            /* ofFunctionExpression = */ false,
            /* allowAbstract = */ allow_abstract,
        );

        let begin_initializers = before_initializers.map(|t| self.next(t));
        self.listener
            .end_primary_constructor_body(begin_token, begin_initializers, token);
        token
    }

    /// Parses an extension type declaration after
    ///
    ///    'extension' 'type'
    ///
    /// This parses
    ///
    ///    'const'? <identifier> <typeParameters>?
    /// ```text
    ///        ('.' <identifier>)? <formals> '{' <memberDeclaration>* '}'
    /// ```
    ///
    /// Dart (line 3915): `Token parseExtensionTypeDeclaration( Token beginToken, Token token, Token? augmentToken, Token extensionKeyword, Token typeKeyword, )`
    pub fn parse_extension_type_declaration(
        &mut self,
        begin_token: TokenId,
        token: TokenId,
        augment_token: Option<TokenId>,
        extension_keyword: TokenId,
        type_keyword: TokenId,
    ) -> TokenId {
        let mut token = token;
        debug_assert!(self.is_identifier(token) && self.lexeme(token) == "type");
        let mut const_keyword: Option<TokenId> = None;
        if self.is_a(self.next(token), Keyword::CONST) {
            // 'extension' 'type' 'const' <identifier>
            token = self.next(token);
            const_keyword = Some(token);
        }
        let name: TokenId;
        if self.is_identifier(self.next(token)) {
            name = self.next(token);
            if self.is_built_in(name) {
                self.report_recoverable_error_with_token(
                    token,
                    diag::built_in_identifier_in_declaration,
                );
            }
        } else {
            name = IdentifierContext::ClassOrMixinOrExtensionDeclaration
                .ensure_identifier(token, self);
        }
        token = name;
        token =
            compute_type_param_or_arg_mut(self.tokens_mut(), token, /* inDeclaration = */ true, false)
                .parse_variables(token, self);
        self.listener
            .begin_extension_type_declaration(augment_token, extension_keyword, name);
        token = self.parse_primary_constructor_opt(
            DeclarationKind::ExtensionType,
            token,
            const_keyword,
            /* allowExtensionTypeRepresentation: */ augment_token.is_none(),
        );

        let start = token;
        token = self.parse_class_or_mixin_or_enum_implements_opt(token);
        if self.is_a(self.next(token), TokenType::SEMICOLON) {
            token = self.next(token);
            let semicolon_token = token;
            if !self.is_primary_constructors_feature_enabled {
                self.report_experiment_not_enabled(
                    ExperimentalFlag::PrimaryConstructors,
                    semicolon_token,
                    semicolon_token,
                );
            }
            self.listener.handle_no_extension_type_body(semicolon_token);
        } else {
            if !self.is_a(self.next(token), TokenType::OPEN_CURLY_BRACKET) {
                // TODO(johnniwinther): Reuse logic from [parseClassHeaderRecovery] to
                // handle `extends`, `with` and out-of-order/duplicate clauses.
                token = self.parse_extension_type_header_recovery(start, extension_keyword);

                // Recovery
                self.ensure_block(token, Some(BlockKind::ExtensionTypeDeclaration));
            }
            let name_lexeme = self.lexeme(name).to_string();
            token = self.parse_class_or_mixin_or_extension_body(
                token,
                DeclarationKind::ExtensionType,
                Some(&name_lexeme),
            );
        }
        self.listener.end_extension_type_declaration(
            begin_token,
            augment_token,
            extension_keyword,
            type_keyword,
            token,
        );
        token
    }

    /// Dart (line 3995): `Token parseStringPart(Token token)`
    pub fn parse_string_part(&mut self, token: TokenId) -> TokenId {
        let mut next = self.next(token);
        if self.kind(next) != STRING_TOKEN {
            self.report_recoverable_error_with_token(next, diag::expected_string);
            let offset = self.char_offset(next);
            let byte_offset = self.tokens().byte_offset(next);
            let new_token = self.tokens_mut().push_synthetic_string(
                TokenType::STRING,
                "",
                offset,
                byte_offset,
                None,
            );
            next = self.rewriter().insert_token(token, new_token);
        }
        self.listener.handle_string_part(next);
        next
    }

    /// Insert a synthetic identifier after the given [token] and create an error
    /// message based on the given [context]. Return the synthetic identifier that
    /// was inserted.
    ///
    /// Dart (line 4011): `Token insertSyntheticIdentifier( Token token, IdentifierContext context,`
    /// `{ codes.Message? message, Token? messageOnToken, })`
    pub fn insert_synthetic_identifier(
        &mut self,
        token: TokenId,
        context: IdentifierContext,
        message: Option<CfeMessage>,
        message_on_token: Option<TokenId>,
    ) -> TokenId {
        let next = self.next(token);
        let error_token = message_on_token.unwrap_or(next);
        let message = match message {
            Some(message) => message,
            None => (context.recovery_template())(self.lexeme(next)),
        };
        self.report_recoverable_error(error_token, message);
        self.rewriter().insert_synthetic_identifier(token, "")
    }

    /// Parse a simple identifier at the given [token], and return the identifier
    /// that was parsed.
    ///
    /// If the token is not an identifier, or is not appropriate for use as an
    /// identifier in the given [context], create a synthetic identifier, report
    /// an error, and return the synthetic identifier.
    ///
    /// Dart (line 4031): `Token ensureIdentifier(Token token, IdentifierContext context)`
    pub fn ensure_identifier(&mut self, token: TokenId, context: IdentifierContext) -> TokenId {
        self.try_rewrite_new_to_identifier(token, context);
        let mut identifier = self.next(token);
        if self.kind(identifier) != IDENTIFIER_TOKEN {
            identifier = context.ensure_identifier(token, self);
            debug_assert!(self.is_keyword_or_identifier(identifier));
        }
        self.listener.handle_identifier(identifier, context);
        identifier
    }

    /// Returns `true` if [token] is either an identifier or a `new` token.  This
    /// can be used to match identifiers in contexts where a constructor name can
    /// appear, since `new` can be used to refer to the unnamed constructor.
    ///
    /// Dart (line 4045): `bool _isNewOrIdentifier(Token token)`
    pub fn is_new_or_identifier(&mut self, token: TokenId) -> bool {
        if self.is_identifier(token) {
            return true;
        }
        if self.kind(token) == KEYWORD_TOKEN {
            let value: Option<&str> = self.string_value(token);
            if value == Some("new") {
                // Treat `new` as an identifier so that it can represent an unnamed
                // constructor.
                return true;
            }
        }
        false
    }

    /// If the token following [token] is a `new` keyword, and [context] is a
    /// context that permits `new` to be treated as an identifier, rewrites the
    /// `new` token to an identifier token, and reports the rewritten token to the
    /// listener.  Otherwise does nothing.
    ///
    /// Dart (line 4063): `void _tryRewriteNewToIdentifier(Token token, IdentifierContext context)`
    #[inline]
    pub fn try_rewrite_new_to_identifier(&mut self, token: TokenId, context: IdentifierContext) {
        if !context.allows_new_as_identifier() {
            return;
        }
        self.try_rewrite_new_to_identifier_impl(token);
    }

    /// Dart (line 4068): `void _tryRewriteNewToIdentifierImpl(Token token)`
    pub fn try_rewrite_new_to_identifier_impl(&mut self, token: TokenId) {
        let identifier = self.next(token);
        if self.kind(identifier) != KEYWORD_TOKEN {
            return;
        }

        let value: Option<&str> = self.string_value(identifier);
        if value != Some("new") {
            return;
        }

        // `new` after `.` is treated as an identifier so that it can represent
        // an unnamed constructor.
        let new_token = self
            .tokens_mut()
            .push_string_like(TokenType::IDENTIFIER, identifier);
        let replacement_token = self.rewriter().replace_token_following(token, new_token);
        self.listener.handle_new_as_identifier(replacement_token);
    }

    /// Parse a simple identifier at the given [token], and return the identifier
    /// that was parsed.
    ///
    /// If the token is not an identifier, or is not appropriate for use as an
    /// identifier in the given [context], create a synthetic identifier, report
    /// an error, and return the synthetic identifier.
    /// [isRecovered] is passed to [context] which - if true - allows implementers
    /// to use the token as an identifier, even if it isn't a valid identifier.
    ///
    /// Dart (line 4096): `Token ensureIdentifierPotentiallyRecovered( Token token, IdentifierContext context, bool isRecovered, )`
    pub fn ensure_identifier_potentially_recovered(
        &mut self,
        token: TokenId,
        context: IdentifierContext,
        is_recovered: bool,
    ) -> TokenId {
        let mut identifier = self.next(token);
        if self.kind(identifier) != IDENTIFIER_TOKEN {
            identifier = context.ensure_identifier_potentially_recovered(token, self, is_recovered);
            debug_assert!(self.is_keyword_or_identifier(identifier));
        }
        self.listener.handle_identifier(identifier, context);
        identifier
    }

    // Dart `notEofOrType` (line 4114) is in `mod.rs`.

    /// Dart (line 4118): `Token parseTypeVariablesOpt(Token token)`
    pub fn parse_type_variables_opt(&mut self, token: TokenId) -> TokenId {
        compute_type_param_or_arg_mut(self.tokens_mut(), token, /* inDeclaration = */ true, false)
            .parse_variables(token, self)
    }

    /// Parse a top level field or function.
    ///
    /// This method is only invoked from outside the parser. As a result, this
    /// method takes the next token to be consumed rather than the last consumed
    /// token and returns the token after the last consumed token rather than the
    /// last consumed token.
    ///
    /// Dart (line 4131): `Token parseTopLevelMember(Token token)`
    pub fn parse_top_level_member(&mut self, token: TokenId) -> TokenId {
        let previous = self.synthetic_previous_token(token);
        let token = self.parse_metadata_star(previous);
        let end = self.parse_top_level_member_impl(token);
        self.next(end)
    }

    /// Dart (line 4136): `Token parseTopLevelMemberImpl(Token token)`
    // The Dart code has a dead store to `next` (kept).
    #[allow(unused_assignments)]
    pub fn parse_top_level_member_impl(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        let before_start = token;
        let mut next = self.next(token);
        self.listener.begin_top_level_member(next);

        let skipped_non_late_late: Option<TokenId> = None;

        let mut abstract_token: Option<TokenId> = None;
        let mut external_token: Option<TokenId> = None;
        let mut augment_token: Option<TokenId> = None;
        let mut late_token: Option<TokenId> = None;
        let mut var_final_or_const: Option<TokenId> = None;

        if is_modifier(self.tokens(), next) {
            if self.is_a(next, Keyword::EXTERNAL) {
                token = next;
                external_token = Some(token);
                next = self.next(token);
            } else if self.is_a(next, Keyword::AUGMENT) {
                token = next;
                augment_token = Some(token);
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
                } else if self.is_a(next, Keyword::CONST) {
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
                    // Recovery
                    if var_final_or_const.is_some()
                        && (self.is_a(next, Keyword::FINAL)
                            || self.is_a(next, Keyword::VAR)
                            || self.is_a(next, Keyword::CONST))
                    {
                        // If another `var`, `final`, or `const` then fall through
                        // to parse that as part of the next top level declaration.
                    } else {
                        let mut context = ModifierContext::new();
                        context.external_token = external_token;
                        context.augment_token = augment_token;
                        context.late_token = late_token;
                        context.set_var_final_or_const(self.tokens(), var_final_or_const);

                        token = context.parse_top_level_member_modifiers(self, token);
                        next = self.next(token);

                        abstract_token = context.abstract_token;
                        augment_token = context.augment_token;
                        external_token = context.external_token;
                        late_token = context.late_token;
                        var_final_or_const = context.var_final_or_const();
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
                return self.parse_fields(
                    before_start,
                    abstract_token,
                    augment_token,
                    external_token,
                    /* staticToken = */ None,
                    /* covariantToken = */ None,
                    late_token,
                    var_final_or_const,
                    before_type,
                    &NO_TYPE,
                    synthetic_name,
                    DeclarationKind::TopLevel,
                    /* enclosingDeclarationName = */ None,
                    /* nameIsRecovered = */ true,
                );
            }
        }
        let mut type_info: TypeInfo = compute_type_mut(self.tokens_mut(),
            token,
            /* required = */ false,
            /* inDeclaration = */ true,
            false,
        );
        token = type_info.skip_type_mut(self.tokens_mut(), token);
        next = self.next(token);

        let mut get_or_set: Option<TokenId> = None;
        let mut value: Option<&'static str> = self.string_value(next);
        if value == Some("get") || value == Some("set") {
            if self.is_identifier(self.next(next)) {
                token = next;
                get_or_set = Some(token);
                next = self.next(token);
            }
        }

        let mut name_is_recovered = false;

        // Recovery: If the code is
        // <return type>? <reserved word> <token indicating method or field>
        // take the reserved keyword as the name.
        if type_info == NO_TYPE && var_final_or_const.is_none() && {
            let next_next = self.next(next);
            self.is_reserved_keyword(next_next) && {
                let next_next_next = self.next(next_next);
                self.indicates_method_or_field(next_next_next)
            }
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

        if !self.is_a(next, TokenType::IDENTIFIER) {
            value = self.string_value(next);
            if value == Some("factory") || value == Some("operator") {
                // `factory` and `operator` can be used as an identifier.
                value = self.string_value(self.next(next));
                if get_or_set.is_none()
                    && value != Some("(")
                    && value != Some("{")
                    && value != Some("<")
                    && value != Some("=>")
                    && value != Some("=")
                    && value != Some(";")
                    && value != Some(",")
                {
                    // Recovery
                    value = self.string_value(next);
                    if value == Some("factory") {
                        self.report_recoverable_error(next, diag::factory_top_level_declaration());
                    } else {
                        self.report_recoverable_error(next, diag::top_level_operator());
                        if self.is_operator(self.next(next)) {
                            token = next;
                            next = self.next(token);
                            if self.is_a(self.next(next), TokenType::OPEN_PAREN) {
                                let name =
                                    format!("#synthetic_identifier_{}", self.char_offset(next));
                                self.rewriter().insert_synthetic_identifier(next, &name);
                            }
                        }
                    }
                    self.listener.handle_invalid_top_level_declaration(next);
                    return next;
                }
                // Fall through and continue parsing
            } else if !self.is_identifier(next) {
                // Recovery
                if self.is_keyword(next) {
                    // Fall through to parse the keyword as the identifier.
                    // ensureIdentifier will report the error.
                } else if token == before_start {
                    // Ensure we make progress.
                    return self.parse_invalid_top_level_declaration(token);
                } else {
                    // Looks like a declaration missing an identifier.
                    // Insert synthetic identifier and fall through.
                    self.insert_synthetic_identifier(
                        token,
                        IdentifierContext::MethodDeclaration,
                        None,
                        None,
                    );
                    next = self.next(token);
                }
            }
        }
        // At this point, `token` is beforeName.

        // Recovery: Inserted ! after method name.
        if self.is_a(self.next(next), TokenType::BANG) {
            next = self.next(next);
        }

        next = self.next(next);
        value = self.string_value(next);
        if get_or_set.is_some()
            || value == Some("(")
            || value == Some("{")
            || value == Some("<")
            || value == Some(".")
            || value == Some("=>")
        {
            if let Some(var_final_or_const) = var_final_or_const {
                if self.is_a(var_final_or_const, Keyword::VAR) {
                    self.report_recoverable_error(var_final_or_const, diag::var_return_type());
                } else {
                    self.report_recoverable_error_with_token(
                        var_final_or_const,
                        diag::extraneous_modifier,
                    );
                }
            } else if let Some(late_token) = late_token {
                self.report_recoverable_error_with_token(late_token, diag::extraneous_modifier);
            }
            if let Some(abstract_token) = abstract_token {
                self.report_recoverable_error_with_token(abstract_token, diag::extraneous_modifier);
            }
            let name = self.next(token);
            return self.parse_top_level_method(
                before_start,
                augment_token,
                external_token,
                before_type,
                &type_info,
                get_or_set,
                name,
                name_is_recovered,
            );
        }

        if let Some(get_or_set) = get_or_set {
            self.report_recoverable_error_with_token(get_or_set, diag::extraneous_modifier);
        }
        if !self.is_augmentations_feature_enabled {
            if let Some(abstract_token) = abstract_token {
                self.report_recoverable_error_with_token(abstract_token, diag::extraneous_modifier);
            }
        }
        let name = self.next(token);
        self.parse_fields(
            before_start,
            abstract_token,
            augment_token,
            external_token,
            /* staticToken = */ None,
            /* covariantToken = */ None,
            late_token,
            var_final_or_const,
            before_type,
            &type_info,
            name,
            DeclarationKind::TopLevel,
            /* enclosingDeclarationName = */ None,
            name_is_recovered,
        )
    }
}
