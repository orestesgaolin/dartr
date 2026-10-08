// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/parser_impl.dart (lines 419-2524)

#[allow(unused_imports)]
use crate::type_info::{
    compute_method_type_arguments_mut, compute_type_mut, compute_type_param_or_arg_mut,
    compute_variable_pattern_type_mut,
};
use dartr_diagnostics::cfe::CfeMessage;
use dartr_diagnostics::cfe_codes as diag;
use dartr_syntax::token_constants::*;
use dartr_syntax::{Keyword, TokenId, TokenType};

use super::Parser;
use crate::directive_context::DirectiveContext;
use crate::experimental_features::ExperimentalFlag;
use crate::formal_parameter_kind::FormalParameterKind;
use crate::identifier_context::{IdentifierContext, is_ok_next_value_in_formal_parameter};
use crate::listener::Listener;
use crate::listener_stack::{ImportRecovery, Layer};
use crate::member_kind::MemberKind;
use crate::modifier_context::{ModifierContext, is_modifier};
use crate::type_info::{
    NO_TYPE, NO_TYPE_PARAM_OR_ARG, TypeParamOrArgInfo,
    is_valid_non_record_type_reference,
};

impl<L: Listener> Parser<L> {
    /// Dart `token.next` (nullable): `None` when the token has no next
    /// token.
    #[inline]
    fn next_opt(&self, token: TokenId) -> Option<TokenId> {
        self.token(token).next.get()
    }

    /// Dart `token?.endGroup?.next?.isA(type) ?? false`.
    #[inline]
    fn end_group_next_is_a(&self, token: TokenId, ty: TokenType) -> bool {
        match self.end_group(token) {
            Some(end_group) => match self.next_opt(end_group) {
                Some(next) => self.is_a(next, ty),
                None => false,
            },
            None => false,
        }
    }

    /// Dart (line 419): `Token parseUnit(Token token)`
    pub fn parse_unit(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        // Skip over error tokens and report them at the end
        // so that the parser has the chance to adjust the error location.
        let error_token = token;
        token = self.skip_error_tokens(error_token);

        self.listener.begin_compilation_unit(token);
        let mut count: i32 = 0;
        let mut directive_state = DirectiveContext::new(self.is_enhanced_parts_feature_enabled);
        token = self.synthetic_previous_token(token);
        if self.ty(self.next(token)) == TokenType::SCRIPT_TAG {
            let next = self.next(token);
            directive_state.check_script_tag(self, next);
            token = self.parse_script(token);
        }
        while !self.is_eof(self.next(token)) {
            let start = self.next(token);
            token = self.parse_top_level_declaration_impl(token, Some(&mut directive_state));
            self.listener.end_top_level_declaration(token);
            count += 1;
            if start == self.next(token) {
                // Recovery:
                // If progress has not been made reaching the end of the token stream,
                // then report an error and skip the current token.
                token = self.next(token);
                self.listener.begin_metadata_star(token);
                self.listener.end_metadata_star(/* count = */ 0);
                self.report_recoverable_error_with_token(token, diag::expected_declaration);
                self.listener.handle_invalid_top_level_declaration(token);
                self.listener.end_top_level_declaration(token);
                count += 1;
            }
        }
        token = self.next(token);
        // assert(token.isEof);
        self.report_all_error_tokens(error_token);
        self.listener.end_compilation_unit(count, token);
        // Clear fields that could lead to memory leak.
        self.undo_log = None;
        token
    }

    /// This method exists for analyzer compatibility only
    /// and will be removed once analyzer/cfe integration is complete.
    ///
    /// Similar to [parseUnit], this method parses a compilation unit,
    /// but stops when it reaches the first declaration or EOF.
    ///
    /// This method is only invoked from outside the parser. As a result, this
    /// method takes the next token to be consumed rather than the last consumed
    /// token and returns the token after the last consumed token rather than the
    /// last consumed token.
    ///
    /// Dart (line 472): `Token parseDirectives(Token token)`
    pub fn parse_directives(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        self.listener.begin_compilation_unit(token);
        let count: i32 = 0;
        let mut directive_state = DirectiveContext::new(self.is_enhanced_parts_feature_enabled);
        token = self.synthetic_previous_token(token);
        while !self.is_eof(self.next(token)) {
            let start = self.next(token);
            let next_value = self.string_value(self.next(start));

            // If a built-in keyword is being used as function name, then stop.
            if next_value == Some(".") || next_value == Some("<") || next_value == Some("(") {
                break;
            }

            if self.ty(self.next(token)) == TokenType::SCRIPT_TAG {
                let next = self.next(token);
                directive_state.check_script_tag(self, next);
                token = self.parse_script(token);
            } else {
                token = self.parse_metadata_star(token);
                let keyword = self.next(token);
                let value = self.string_value(keyword);
                if value == Some("import") {
                    directive_state.check_import(self, keyword);
                    token = self.parse_import(keyword);
                } else if value == Some("export") {
                    directive_state.check_export(self, keyword);
                    token = self.parse_export(keyword);
                } else if value == Some("library") {
                    directive_state.check_library(self, keyword);
                    token = self.parse_library_name(keyword);
                } else if value == Some("part") {
                    token = self.parse_part_or_part_of(keyword, Some(&mut directive_state));
                } else if value == Some(";") {
                    token = start;
                    self.listener.handle_directives_only();
                } else {
                    self.listener.handle_directives_only();
                    break;
                }
            }
            self.listener.end_top_level_declaration(token);
        }
        token = self.next(token);
        self.listener.end_compilation_unit(count, token);
        // Clear fields that could lead to memory leak.
        self.undo_log = None;
        token
    }

    /// Parse a top-level declaration.
    ///
    /// This method is only invoked from outside the parser. As a result, this
    /// method takes the next token to be consumed rather than the last consumed
    /// token and returns the token after the last consumed token rather than the
    /// last consumed token.
    ///
    /// Dart (line 531): `Token parseTopLevelDeclaration(Token token)`
    pub fn parse_top_level_declaration(&mut self, token: TokenId) -> TokenId {
        let previous = self.synthetic_previous_token(token);
        let token =
            self.parse_top_level_declaration_impl(previous, /* directiveState = */ None);
        self.listener.end_top_level_declaration(token);
        self.next(token)
    }

    /// ```text
    /// topLevelDefinition:
    ///   classDefinition |
    ///   enumType |
    ///   typeAlias |
    ///   'external'? functionSignature ';' |
    ///   'external'? getterSignature ';' |
    ///   'external''? setterSignature ';' |
    ///   functionSignature functionBody |
    ///   returnType? 'get' identifier functionBody |
    ///   returnType? 'set' identifier formalParameterList functionBody |
    ///   ('final' | 'const') type? staticFinalDeclarationList ';' |
    ///   variableDeclaration ';'
    /// ;
    /// ```
    ///
    /// Dart (line 555): `Token parseTopLevelDeclarationImpl( Token token, DirectiveContext? directiveState, )`
    pub fn parse_top_level_declaration_impl(
        &mut self,
        token: TokenId,
        mut directive_state: Option<&mut DirectiveContext>,
    ) -> TokenId {
        let mut token = self.parse_metadata_star(token);
        let mut next = self.next(token);
        if self.is_top_level_keyword(next) {
            return self.parse_top_level_keyword_declaration(
                /* beginToken = */ self.next(token),
                /* modifierStart = */ token,
                /* keyword = */ next,
                /* sealedToken = */ None,
                /* baseToken = */ None,
                /* interfaceToken = */ None,
                directive_state,
            );
        }
        let begin_token = self.next(token);
        let mut modifier_start = token;
        // Skip modifiers to find a top level keyword or identifier
        if self.is_modifier(next) {
            if self.is_a(next, Keyword::VAR)
                || self.is_a(next, Keyword::LATE)
                || (self.is_a(next, Keyword::FINAL)
                    && (!self.is_a(self.next(next), Keyword::CLASS)
                        && !self.is_a(self.next(next), Keyword::MIXIN)
                        && !self.is_a(self.next(next), Keyword::ENUM)))
                // Ignore using 'final' as a modifier for a class, a mixin, or an
                // enum, but allow in other contexts.
                || (self.is_a(next, Keyword::CONST) && !self.is_a(self.next(next), Keyword::CLASS))
            {
                // Ignore `const class` so that it is reported below as an invalid
                // modifier on a class.
                if let Some(d) = directive_state.as_deref_mut() {
                    d.check_declaration();
                }
                return self.parse_top_level_member_impl(token);
            }
            while self.is_modifier(self.next(token)) {
                token = self.next(token);
            }
        }
        next = self.next(token);
        let mut sealed_token: Option<TokenId> = None;
        let mut base_token: Option<TokenId> = None;
        let mut interface_token: Option<TokenId> = None;
        if self.is_identifier(next) && self.is_a(next, Keyword::SEALED) {
            sealed_token = Some(next);
            if self.is_a(self.next(next), Keyword::CLASS)
                || self.is_a(self.next(next), Keyword::MIXIN)
                || self.is_a(self.next(next), Keyword::ENUM)
            {
                next = self.next(next);
            } else if self.is_a(self.next(next), Keyword::ABSTRACT)
                && self.is_a(self.next(self.next(next)), Keyword::CLASS)
            {
                // Defer error handling of sealed abstract to
                // [parseClassOrNamedMixinApplication] after the abstract is parsed.
                modifier_start = next;
                next = self.next(self.next(next));
            }
        } else if self.is_identifier(next) && self.is_a(next, Keyword::BASE) {
            base_token = Some(next);
            if self.is_a(self.next(next), Keyword::CLASS)
                || self.is_a(self.next(next), Keyword::MIXIN)
                || self.is_a(self.next(next), Keyword::ENUM)
            {
                next = self.next(next);
            }
        } else if self.is_identifier(next) && self.is_a(next, Keyword::INTERFACE) {
            interface_token = Some(next);
            if self.is_a(self.next(next), Keyword::CLASS)
                || self.is_a(self.next(next), Keyword::MIXIN)
                || self.is_a(self.next(next), Keyword::ENUM)
            {
                next = self.next(next);
            }
            // TODO(kallentu): Handle incorrect ordering of modifiers.
        }
        if self.is_top_level_keyword(next) {
            return self.parse_top_level_keyword_declaration(
                /* beginToken = */ begin_token,
                /* modifierStart = */ modifier_start,
                /* keyword = */ next,
                /* sealedToken = */ sealed_token,
                /* baseToken = */ base_token,
                /* interfaceToken = */ interface_token,
                directive_state,
            );
        } else if self.is_keyword_or_identifier(next) {
            // TODO(danrubel): improve parseTopLevelMember
            // so that we don't parse modifiers twice.
            if let Some(d) = directive_state.as_deref_mut() {
                d.check_declaration();
            }
            return self.parse_top_level_member_impl(modifier_start);
        } else if self.next_opt(modifier_start) != Some(next) {
            if let Some(d) = directive_state.as_deref_mut() {
                d.check_declaration();
            }
            // Handle the edge case where a modifier is being used as an identifier
            return self.parse_top_level_member_impl(modifier_start);
        } else if
        /* record type */
        self.is_a(next, TokenType::OPEN_PAREN) {
            if let Some(d) = directive_state.as_deref_mut() {
                d.check_declaration();
            }
            return self.parse_top_level_member_impl(modifier_start);
        }

        // Recovery
        if self.is_operator(next) && self.is_a(self.next(next), TokenType::OPEN_PAREN) {
            // This appears to be a top level operator declaration, which is invalid.
            self.report_recoverable_error(next, diag::top_level_operator());
            // Insert a synthetic identifier
            // and continue parsing as a top level function.
            let value = format!("#synthetic_function_{}", self.char_offset(next));
            self.rewriter().insert_synthetic_identifier(next, &value);
            return self.parse_top_level_member_impl(next);
        }
        // Ignore any preceding modifiers and just report the unexpected token
        self.listener.begin_top_level_member(next);
        self.parse_invalid_top_level_declaration(token)
    }

    /// Parse any top-level declaration that begins with a keyword.
    /// [beginToken] is the first token after any metadata that is parsed as
    /// part of the declaration. [modifierStart] is the token before any modifiers
    /// preceding [keyword]. [beginToken] may point to some out-of-order modifiers
    /// before [modifierStart].
    ///
    /// Dart (line 673): `Token parseTopLevelKeywordDeclaration( Token beginToken, Token modifierStart, Token keyword, Token? sealedToken, Token? baseToken, Token? interfaceToken, DirectiveContext? directiveState, )`
    pub fn parse_top_level_keyword_declaration(
        &mut self,
        begin_token: TokenId,
        modifier_start: TokenId,
        keyword: TokenId,
        sealed_token: Option<TokenId>,
        base_token: Option<TokenId>,
        interface_token: Option<TokenId>,
        mut directive_state: Option<&mut DirectiveContext>,
    ) -> TokenId {
        // assert(keyword.isTopLevelKeyword);
        let value = self.string_value(keyword);
        if value == Some("class") {
            return self.handle_modifiers_for_class_declaration(
                begin_token,
                modifier_start,
                keyword,
                sealed_token,
                base_token,
                interface_token,
                /* mixinToken = */ None,
                directive_state,
            );
        } else if value == Some("enum") {
            if let Some(d) = directive_state.as_deref_mut() {
                d.check_declaration();
            }
            let mut context = ModifierContext::new();
            context.parse_enum_modifiers(self, modifier_start, keyword);
            // Enums can't declare any explicit modifier.
            if let Some(base_token) = base_token {
                self.report_recoverable_error(base_token, diag::base_enum());
            }
            if let Some(final_token) = context.final_token {
                self.report_recoverable_error(final_token, diag::final_enum());
            }
            if let Some(interface_token) = interface_token {
                self.report_recoverable_error(interface_token, diag::interface_enum());
            }
            if let Some(sealed_token) = sealed_token {
                self.report_recoverable_error(sealed_token, diag::sealed_enum());
            }
            return self.parse_enum(begin_token, context.augment_token, keyword);
        } else {
            // The remaining top level keywords are built-in keywords
            // and can be used in a top level declaration
            // as an identifier such as "abstract<T>() => 0;"
            // or as a prefix such as "abstract.A b() => 0;".
            // This also means that `typedef ({int? j}) => 0;` is a method, but with
            // records something like `typedef ({int? j}) X();` is a typedef.
            let next_value = self.string_value(self.next(keyword));
            let mut typedef_with_record = false;
            if value == Some("typedef") && next_value == Some("(") {
                let end_paren = self.end_group(self.next(keyword));
                if let Some(end_paren) = end_paren {
                    if self.is_identifier_or_question_identifier(self.next(end_paren)) {
                        // Looks like a typedef with a record.
                        let type_info = compute_type_mut(self.tokens_mut(),
                            keyword,
                            /* required = */ false,
                            false,
                            false,
                        );
                        if type_info.is_record_type() {
                            typedef_with_record = true;
                        }
                    }
                }
            }

            if (next_value == Some("(") || next_value == Some(".")) && !typedef_with_record {
                if let Some(d) = directive_state.as_deref_mut() {
                    d.check_declaration();
                }
                return self.parse_top_level_member_impl(modifier_start);
            } else if next_value == Some("<") {
                if value == Some("extension") {
                    // The name in an extension declaration is optional:
                    // `extension<T> on ...`
                    let end_group = self.end_group(self.next(keyword));
                    if let Some(end_group) = end_group {
                        if self.is_a(self.next(end_group), Keyword::ON) {
                            if let Some(d) = directive_state.as_deref_mut() {
                                d.check_declaration();
                            }
                            let mut context = ModifierContext::new();
                            context.parse_extension_modifiers(self, modifier_start, keyword);
                            return self.parse_extension(
                                begin_token,
                                context.augment_token,
                                keyword,
                            );
                        }
                    }
                } else if value == Some("typedef") {
                    // Having a method called typedef is ok, but we might also want to
                    // recover.
                    if self.end_group_next_is_a(self.next(keyword), TokenType::EQ) {
                        // Recovery:
                        // `typedef` `<` [...] `>` `=`
                        // This isn't a legal method name. Assume we're missing the name of
                        // the typedef.
                        let mut context = ModifierContext::new();
                        context.parse_typedef_modifiers(self, modifier_start, keyword);
                        if let Some(d) = directive_state.as_deref_mut() {
                            d.check_declaration();
                        }
                        return self.parse_typedef(context.augment_token, keyword);
                    }
                }
                if let Some(d) = directive_state.as_deref_mut() {
                    d.check_declaration();
                }
                return self.parse_top_level_member_impl(modifier_start);
            } else {
                let mut context = ModifierContext::new();
                if value == Some("import") {
                    context.parse_top_level_keyword_modifiers(self, modifier_start, keyword);
                    if let Some(d) = directive_state.as_deref_mut() {
                        d.check_import(self, keyword);
                    }
                    return self.parse_import(keyword);
                } else if value == Some("export") {
                    context.parse_top_level_keyword_modifiers(self, modifier_start, keyword);
                    if let Some(d) = directive_state.as_deref_mut() {
                        d.check_export(self, keyword);
                    }
                    return self.parse_export(keyword);
                } else if value == Some("typedef") {
                    context.parse_typedef_modifiers(self, modifier_start, keyword);
                    if let Some(d) = directive_state.as_deref_mut() {
                        d.check_declaration();
                    }
                    return self.parse_typedef(context.augment_token, keyword);
                } else if value == Some("mixin") {
                    if next_value == Some("class") {
                        let class_keyword = self.next(keyword);
                        return self.handle_modifiers_for_class_declaration(
                            begin_token,
                            modifier_start,
                            class_keyword,
                            sealed_token,
                            base_token,
                            interface_token,
                            Some(keyword),
                            directive_state,
                        );
                    }
                    context.parse_mixin_modifiers(self, modifier_start, keyword);
                    // Mixins can't have any modifier other than a base modifier.
                    if let Some(final_token) = context.final_token {
                        self.report_recoverable_error(final_token, diag::final_mixin());
                    }
                    if let Some(interface_token) = interface_token {
                        self.report_recoverable_error(interface_token, diag::interface_mixin());
                    }
                    if let Some(sealed_token) = sealed_token {
                        self.report_recoverable_error(sealed_token, diag::sealed_mixin());
                    }
                    if let Some(d) = directive_state.as_deref_mut() {
                        d.check_declaration();
                    }
                    return self.parse_mixin(
                        begin_token,
                        context.augment_token,
                        base_token,
                        keyword,
                    );
                } else if value == Some("extension") {
                    context.parse_extension_modifiers(self, modifier_start, keyword);
                    if let Some(d) = directive_state.as_deref_mut() {
                        d.check_declaration();
                    }
                    let begin = self.next(modifier_start);
                    return self.parse_extension(begin, context.augment_token, keyword);
                } else if value == Some("part") {
                    context.parse_top_level_keyword_modifiers(self, modifier_start, keyword);
                    return self.parse_part_or_part_of(keyword, directive_state);
                } else if value == Some("library") {
                    if let Some(d) = directive_state.as_deref_mut() {
                        d.check_library(self, keyword);
                    }
                    context.parse_library_directive_modifiers(self, modifier_start, keyword);
                    if let Some(augment_keyword) = context.augment_token {
                        return self.parse_library_augmentation(augment_keyword, keyword);
                    } else {
                        return self.parse_library_name(keyword);
                    }
                }
            }
        }

        panic!(
            "Internal error: Unhandled top level keyword '{}'.",
            value.unwrap_or("null")
        );
    }

    /// Dart (line 836): `Token _handleModifiersForClassDeclaration( Token beginToken, Token modifierStart, Token classKeyword, Token? sealedToken, Token? baseToken, Token? interfaceToken, Token? mixinToken, DirectiveContext? directiveState, )`
    pub fn handle_modifiers_for_class_declaration(
        &mut self,
        begin_token: TokenId,
        modifier_start: TokenId,
        class_keyword: TokenId,
        sealed_token: Option<TokenId>,
        base_token: Option<TokenId>,
        interface_token: Option<TokenId>,
        mixin_token: Option<TokenId>,
        mut directive_state: Option<&mut DirectiveContext>,
    ) -> TokenId {
        if let Some(d) = directive_state.as_deref_mut() {
            d.check_declaration();
        }
        let mut context = ModifierContext::new();
        if let Some(mixin_token) = mixin_token {
            context.parse_class_modifiers(self, modifier_start, mixin_token);

            // Mixin classes can't have any modifier other than a base modifier.
            if let Some(final_token) = context.final_token {
                self.report_recoverable_error(final_token, diag::final_mixin_class());
            }
            if let Some(interface_token) = interface_token {
                self.report_recoverable_error(interface_token, diag::interface_mixin_class());
            }
            if let Some(sealed_token) = sealed_token {
                self.report_recoverable_error(sealed_token, diag::sealed_mixin_class());
            }
        } else {
            context.parse_class_modifiers(self, modifier_start, class_keyword);
        }
        self.parse_class_or_named_mixin_application(
            begin_token,
            context.abstract_token,
            sealed_token,
            base_token,
            interface_token,
            context.final_token,
            context.augment_token,
            mixin_token,
            class_keyword,
        )
    }

    /// Dart (line 877): `bool _isIdentifierOrQuestionIdentifier(Token token)`
    pub fn is_identifier_or_question_identifier(&mut self, token: TokenId) -> bool {
        if self.is_identifier(token) {
            return true;
        }
        if self.is_a(token, TokenType::QUESTION) {
            return self.is_identifier(self.next(token));
        }
        false
    }

    /// ```text
    /// libraryAugmentationDirective:
    ///   'augment' 'library' uri ';'
    /// ;
    /// ```
    ///
    /// Dart (line 890): `Token parseLibraryAugmentation(Token augmentKeyword, Token libraryKeyword)`
    pub fn parse_library_augmentation(
        &mut self,
        augment_keyword: TokenId,
        library_keyword: TokenId,
    ) -> TokenId {
        // assert(augmentKeyword.isA(Keyword.AUGMENT));
        // assert(libraryKeyword.isA(Keyword.LIBRARY));
        self.listener
            .begin_uncategorized_top_level_declaration(library_keyword);
        self.listener
            .begin_library_augmentation(augment_keyword, library_keyword);
        let start = library_keyword;
        let token = self.ensure_literal_string(start);
        let semicolon = self.ensure_semicolon(token);
        self.listener
            .end_library_augmentation(augment_keyword, library_keyword, semicolon);
        semicolon
    }

    /// ```text
    /// libraryDirective:
    ///   'library' qualified? ';'
    /// ;
    /// ```
    ///
    /// Dart (line 907): `Token parseLibraryName(Token libraryKeyword)`
    pub fn parse_library_name(&mut self, library_keyword: TokenId) -> TokenId {
        // assert(libraryKeyword.isA(Keyword.LIBRARY));
        self.listener
            .begin_uncategorized_top_level_declaration(library_keyword);
        self.listener.begin_library_name(library_keyword);
        let mut token = self.next(library_keyword);
        let has_name = !self.is_a(token, TokenType::SEMICOLON);
        if has_name {
            token = self.parse_qualified(
                library_keyword,
                IdentifierContext::LibraryName,
                IdentifierContext::LibraryNameContinuation,
            );
            token = self.ensure_semicolon(token);
        } else {
            token = self.ensure_semicolon(library_keyword);
        }
        self.listener
            .end_library_name(library_keyword, token, has_name);
        token
    }

    /// ```text
    /// importPrefix:
    ///   'deferred'? 'as' identifier
    /// ;
    /// ```
    ///
    /// Dart (line 932): `Token parseImportPrefixOpt(Token token)`
    pub fn parse_import_prefix_opt(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        let next = self.next(token);
        if self.is_a(next, Keyword::DEFERRED) && self.is_a(self.next(next), Keyword::AS) {
            let deferred_token = next;
            let as_keyword = self.next(next);
            token = self.ensure_identifier(as_keyword, IdentifierContext::ImportPrefixDeclaration);
            self.listener
                .handle_import_prefix(Some(deferred_token), Some(as_keyword));
        } else if self.is_a(next, Keyword::AS) {
            let as_keyword = next;
            token = self.ensure_identifier(next, IdentifierContext::ImportPrefixDeclaration);
            self.listener
                .handle_import_prefix(/* deferredKeyword = */ None, Some(as_keyword));
        } else {
            self.listener.handle_import_prefix(
                /* deferredKeyword = */ None, /* asKeyword = */ None,
            );
        }
        token
    }

    /// ```text
    /// importDirective:
    ///   'import' uri ('if' '(' test ')' uri)* importPrefix? combinator* ';'
    /// ;
    /// ```
    ///
    /// Dart (line 960): `Token parseImport(Token importKeyword)`
    pub fn parse_import(&mut self, import_keyword: TokenId) -> TokenId {
        // assert(importKeyword.isA(Keyword.IMPORT));
        self.listener
            .begin_uncategorized_top_level_declaration(import_keyword);
        self.listener.begin_import(import_keyword);
        let start = import_keyword;
        let mut token = self.ensure_literal_string(start);
        let uri = token;
        token = self.parse_conditional_uri_star(token);
        token = self.parse_import_prefix_opt(token);
        token = self.parse_combinator_star(token);
        token = self.next(token);
        if self.is_a(token, TokenType::SEMICOLON) {
            self.listener.end_import(import_keyword, Some(token));
            token
        } else {
            // Recovery
            self.listener
                .end_import(import_keyword, /* semicolon = */ None);
            self.parse_import_recovery(uri)
        }
    }

    /// Recover given out-of-order clauses in an import directive where [token] is
    /// the import keyword.
    ///
    /// Dart (line 982): `Token parseImportRecovery(Token token)`
    pub fn parse_import_recovery(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        // Dart: `final Listener primaryListener = listener;
        // final ImportRecoveryListener recoveryListener =
        //     new ImportRecoveryListener();`

        // Reparse to determine which clauses have already been parsed
        // but intercept the events so they are not sent to the primary listener
        self.listener.push_layer(Layer::ImportRecovery {
            forwarding: false,
            state: ImportRecovery::default(),
        });
        token = self.parse_conditional_uri_star(token);
        token = self.parse_import_prefix_opt(token);
        token = self.parse_combinator_star(token);

        let mut first_deferred_keyword: Option<TokenId>;
        let mut has_prefix: bool;
        let mut has_combinator: bool;
        {
            let (forwarding, state) = self.listener.import_recovery();
            first_deferred_keyword = state.deferred_keyword;
            has_prefix = state.as_keyword.is_some();
            has_combinator = state.has_combinator;

            // Update the recovery listener to forward subsequent events
            // to the primary listener
            *forwarding = true;
        }

        // Parse additional out-of-order clauses.
        let mut semicolon: Option<TokenId> = None;
        loop {
            let start = self.next(token);

            // Check for extraneous token in the middle of an import statement.
            token = self
                .skip_unexpected_token_opt(token, &["if", "deferred", "as", "hide", "show", ";"]);

            // During recovery, clauses are parsed in the same order
            // and generate the same events as in the parseImport method above.
            {
                let (_, state) = self.listener.import_recovery();
                *state = ImportRecovery::default();
            }
            token = self.parse_conditional_uri_star(token);
            if self.listener.import_recovery().1.if_keyword.is_some() {
                if first_deferred_keyword.is_some() {
                    // TODO(danrubel): report error indicating conditional should
                    // be moved before deferred keyword
                } else if has_prefix {
                    // TODO(danrubel): report error indicating conditional should
                    // be moved before prefix clause
                } else if has_combinator {
                    // TODO(danrubel): report error indicating conditional should
                    // be moved before combinators
                }
            }

            if self.is_a(self.next(token), Keyword::DEFERRED)
                && !self.is_a(self.next(self.next(token)), Keyword::AS)
            {
                let deferred_keyword = self.next(token);
                self.listener
                    .handle_import_prefix(Some(deferred_keyword), /* asKeyword = */ None);
                token = self.next(token);
            } else {
                token = self.parse_import_prefix_opt(token);
            }
            let recovery_deferred_keyword = self.listener.import_recovery().1.deferred_keyword;
            if let Some(recovery_deferred_keyword) = recovery_deferred_keyword {
                if first_deferred_keyword.is_some() {
                    self.report_recoverable_error(
                        recovery_deferred_keyword,
                        diag::duplicate_deferred(),
                    );
                } else {
                    if has_prefix {
                        self.report_recoverable_error(
                            recovery_deferred_keyword,
                            diag::deferred_after_prefix(),
                        );
                    }
                    first_deferred_keyword = Some(recovery_deferred_keyword);
                }
            }
            let recovery_as_keyword = self.listener.import_recovery().1.as_keyword;
            if let Some(recovery_as_keyword) = recovery_as_keyword {
                if has_prefix {
                    self.report_recoverable_error(recovery_as_keyword, diag::duplicate_prefix());
                } else {
                    if has_combinator {
                        self.report_recoverable_error(
                            recovery_as_keyword,
                            diag::prefix_after_combinator(),
                        );
                    }
                    has_prefix = true;
                }
            }

            token = self.parse_combinator_star(token);
            has_combinator = has_combinator || self.listener.import_recovery().1.has_combinator;

            if self.is_a(self.next(token), TokenType::SEMICOLON) {
                semicolon = Some(self.next(token));
            } else if start == self.next(token) {
                // If no forward progress was made, insert ';' so that we exit loop.
                semicolon = Some(self.ensure_semicolon(token));
            }
            self.listener.handle_recover_import(semicolon);
            if semicolon.is_some() {
                break;
            }
        }

        if let Some(first_deferred_keyword) = first_deferred_keyword {
            if !has_prefix {
                self.report_recoverable_error(
                    first_deferred_keyword,
                    diag::missing_prefix_in_deferred_import(),
                );
            }
        }

        // Dart keeps the recovery listener (now forwarding every event and
        // error to the primary listener) as `listener`; removing the layer
        // gives the same events and keeps the layer stack balanced.
        self.pop_layer();

        semicolon.unwrap()
    }

    /// ```text
    /// conditionalUris:
    ///   conditionalUri*
    /// ;
    /// ```
    ///
    /// Dart (line 1101): `Token parseConditionalUriStar(Token token)`
    pub fn parse_conditional_uri_star(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        let next = self.next(token);
        self.listener.begin_conditional_uris(next);
        let mut count: i32 = 0;
        while self.is_a(self.next(token), Keyword::IF) {
            count += 1;
            token = self.parse_conditional_uri(token);
        }
        self.listener.end_conditional_uris(count);
        token
    }

    /// ```text
    /// conditionalUri:
    ///   'if' '(' dottedName ('==' literalString)? ')' uri
    /// ;
    /// ```
    ///
    /// Dart (line 1117): `Token parseConditionalUri(Token token)`
    pub fn parse_conditional_uri(&mut self, token: TokenId) -> TokenId {
        let mut token = self.next(token);
        let if_keyword = token;
        // assert(token.isA(Keyword.IF));
        self.listener.begin_conditional_uri(token);
        let mut left_paren = self.next(token);
        if !self.is_a(left_paren, TokenType::OPEN_PAREN) {
            self.report_recoverable_error(left_paren, diag::expected_but_got("("));
            left_paren = self
                .rewriter()
                .insert_parens(token, /* includeIdentifier = */ true);
        }
        token = self.parse_dotted_name(left_paren);
        let mut next = self.next(token);
        let mut equality_sign: Option<TokenId> = None;
        if self.is_a(next, TokenType::EQ_EQ) {
            equality_sign = Some(next);
            token = self.ensure_literal_string(next);
            next = self.next(token);
        }
        if Some(next) != self.end_group(left_paren) {
            let end_group = self.end_group(left_paren).unwrap();
            if self.is_synthetic(end_group) {
                // The scanner did not place the synthetic ')' correctly, so move it.
                next = self.rewriter().move_synthetic(token, end_group);
            } else {
                self.report_recoverable_error_with_token(next, diag::unexpected_token);
                next = end_group;
            }
        }
        token = next;
        // assert(token.isA(TokenType.CLOSE_PAREN));

        token = self.ensure_literal_string(token);
        self.listener
            .end_conditional_uri(if_keyword, left_paren, equality_sign);
        token
    }

    /// ```text
    /// dottedName:
    ///   identifier ('.' identifier)*
    /// ;
    /// ```
    ///
    /// Dart (line 1160): `Token parseDottedName(Token token)`
    pub fn parse_dotted_name(&mut self, token: TokenId) -> TokenId {
        let mut token = self.ensure_identifier(token, IdentifierContext::DottedName);
        let first_identifier = token;
        let mut count: i32 = 1;
        while self.is_a(self.next(token), TokenType::PERIOD) {
            let next = self.next(token);
            token = self.ensure_identifier(next, IdentifierContext::DottedNameContinuation);
            count += 1;
        }
        self.listener.handle_dotted_name(count, first_identifier);
        token
    }

    /// ```text
    /// exportDirective:
    ///   'export' uri conditional-uris* combinator* ';'
    /// ;
    /// ```
    ///
    /// Dart (line 1180): `Token parseExport(Token exportKeyword)`
    pub fn parse_export(&mut self, export_keyword: TokenId) -> TokenId {
        // assert(exportKeyword.isA(Keyword.EXPORT));
        self.listener
            .begin_uncategorized_top_level_declaration(export_keyword);
        self.listener.begin_export(export_keyword);
        let mut token = self.ensure_literal_string(export_keyword);
        token = self.parse_conditional_uri_star(token);
        token = self.parse_combinator_star(token);
        token = self.ensure_semicolon(token);
        self.listener.end_export(export_keyword, token);
        token
    }

    /// ```text
    /// combinators:
    ///   (hideCombinator | showCombinator)*
    /// ;
    /// ```
    ///
    /// Dart (line 1197): `Token parseCombinatorStar(Token token)`
    pub fn parse_combinator_star(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        let mut next = self.next(token);
        self.listener.begin_combinators(next);
        let mut count: i32 = 0;
        loop {
            let value = self.string_value(next);
            if value == Some("hide") {
                token = self.parse_hide(token);
            } else if value == Some("show") {
                token = self.parse_show(token);
            } else {
                self.listener.end_combinators(count);
                break;
            }
            next = self.next(token);
            count += 1;
        }
        token
    }

    /// ```text
    /// hideCombinator:
    ///   'hide' identifierList
    /// ;
    /// ```
    ///
    /// Dart (line 1222): `Token parseHide(Token token)`
    pub fn parse_hide(&mut self, token: TokenId) -> TokenId {
        let hide_keyword = self.next(token);
        // assert(hideKeyword.isA(Keyword.HIDE));
        self.listener.begin_hide(hide_keyword);
        let token = self.parse_identifier_list(hide_keyword);
        self.listener.end_hide(hide_keyword);
        token
    }

    /// ```text
    /// showCombinator:
    ///   'show' identifierList
    /// ;
    /// ```
    ///
    /// Dart (line 1236): `Token parseShow(Token token)`
    pub fn parse_show(&mut self, token: TokenId) -> TokenId {
        let show_keyword = self.next(token);
        // assert(showKeyword.isA(Keyword.SHOW));
        self.listener.begin_show(show_keyword);
        let token = self.parse_identifier_list(show_keyword);
        self.listener.end_show(show_keyword);
        token
    }

    /// ```text
    /// identifierList:
    ///   identifier (',' identifier)*
    /// ;
    /// ```
    ///
    /// Dart (line 1250): `Token parseIdentifierList(Token token)`
    pub fn parse_identifier_list(&mut self, token: TokenId) -> TokenId {
        let mut token = self.ensure_identifier(token, IdentifierContext::Combinator);
        let mut count: i32 = 1;
        while self.is_a(self.next(token), TokenType::COMMA) {
            let next = self.next(token);
            token = self.ensure_identifier(next, IdentifierContext::Combinator);
            count += 1;
        }
        self.listener.handle_identifier_list(count);
        token
    }

    /// ```text
    /// typeList:
    ///   type (',' type)*
    /// ;
    /// ```
    ///
    /// Dart (line 1266): `Token parseTypeList(Token token)`
    pub fn parse_type_list(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        let next = self.next(token);
        self.listener.begin_type_list(next);
        token = compute_type_mut(self.tokens_mut(),
            token,
            /* required = */ true,
            false,
            false,
        )
        .ensure_type_or_void(token, self);
        let mut count: i32 = 1;
        while self.is_a(self.next(token), TokenType::COMMA) {
            let next = self.next(token);
            token = compute_type_mut(self.tokens_mut(),
                next,
                /* required = */ true,
                false,
                false,
            )
            .ensure_type_or_void(next, self);
            count += 1;
        }
        self.listener.end_type_list(count);
        token
    }

    /// Dart (line 1284): `Token parsePartOrPartOf(Token partKeyword, DirectiveContext? directiveState)`
    pub fn parse_part_or_part_of(
        &mut self,
        part_keyword: TokenId,
        mut directive_state: Option<&mut DirectiveContext>,
    ) -> TokenId {
        // assert(partKeyword.isA(Keyword.PART));
        self.listener
            .begin_uncategorized_top_level_declaration(part_keyword);
        if self.is_a(self.next(part_keyword), Keyword::OF) {
            if let Some(d) = directive_state.as_deref_mut() {
                d.check_part_of(self, part_keyword);
            }
            self.parse_part_of(part_keyword)
        } else {
            if let Some(d) = directive_state.as_deref_mut() {
                d.check_part(self, part_keyword);
            }
            self.parse_part(part_keyword)
        }
    }

    /// ```text
    /// partDirective:
    ///   'part' uri ';'
    /// ;
    /// ```
    ///
    /// Dart (line 1301): `Token parsePart(Token partKeyword)`
    pub fn parse_part(&mut self, part_keyword: TokenId) -> TokenId {
        // assert(partKeyword.isA(Keyword.PART));
        self.listener.begin_part(part_keyword);
        let mut token = self.ensure_literal_string(part_keyword);
        token = self.ensure_semicolon(token);
        self.listener.end_part(part_keyword, token);
        token
    }

    /// ```text
    /// partOfDirective:
    ///   'part' 'of' (qualified | uri) ';'
    /// ;
    /// ```
    ///
    /// Dart (line 1315): `Token parsePartOf(Token partKeyword)`
    pub fn parse_part_of(&mut self, part_keyword: TokenId) -> TokenId {
        let of_keyword = self.next(part_keyword);
        // assert(partKeyword.isA(Keyword.PART));
        // assert(ofKeyword.isA(Keyword.OF));
        self.listener.begin_part_of(part_keyword);
        let has_name = self.is_identifier(self.next(of_keyword));
        let mut token;
        if has_name {
            token = self.parse_qualified(
                of_keyword,
                IdentifierContext::PartName,
                IdentifierContext::PartNameContinuation,
            );
        } else {
            token = self.ensure_literal_string(of_keyword);
        }
        token = self.ensure_semicolon(token);
        self.listener
            .end_part_of(part_keyword, of_keyword, token, has_name);
        token
    }

    /// ```text
    /// metadata:
    ///   annotation*
    /// ;
    /// ```
    ///
    /// Dart (line 1341): `Token parseMetadataStar(Token token)`
    pub fn parse_metadata_star(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        let next = self.next(token);
        self.listener.begin_metadata_star(next);
        let mut count: i32 = 0;
        while self.is_a(self.next(token), TokenType::AT) {
            token = self.parse_metadata(token);
            count += 1;
        }
        self.listener.end_metadata_star(count);
        token
    }

    /// ```text
    /// <metadata> ::= (‘@’ <metadatum>)*
    /// <metadatum> ::= <identifier>
    ///   | <qualifiedName>
    ///   | <constructorDesignation> <arguments>
    /// <qualifiedName> ::= <typeIdentifier> ‘.’ <identifier>
    ///   | <typeIdentifier> ‘.’ <typeIdentifier> ‘.’ <identifier>
    /// <constructorDesignation> ::= <typeIdentifier>
    ///   | <qualifiedName>
    ///   | <typeName> <typeArguments> (‘.’ <identifier>)?
    /// <typeName> ::= <typeIdentifier> (‘.’ <typeIdentifier>)?
    /// ```
    /// (where typeIdentifier is an identifier that's not on the list of
    /// built in identifiers)
    /// So these are legal:
    /// * identifier
    /// qualifiedName:
    /// * typeIdentifier.identifier
    /// * typeIdentifier.typeIdentifier.identifier
    /// via constructorDesignation part 1
    /// * typeIdentifier(arguments)
    /// via constructorDesignation part 2
    /// * typeIdentifier.identifier(arguments)
    /// * typeIdentifier.typeIdentifier.identifier(arguments)
    /// via constructorDesignation part 3
    /// * typeIdentifier<typeArguments>(arguments)
    /// * typeIdentifier<typeArguments>.identifier(arguments)
    /// * typeIdentifier.typeIdentifier<typeArguments>(arguments)
    /// * typeIdentifier.typeIdentifier<typeArguments>.identifier(arguments)
    ///
    /// So in another way (ignoring the difference between typeIdentifier and
    /// identifier):
    /// * 1, 2 or 3 identifiers with or without arguments.
    /// * 1 or 2 identifiers, then type arguments, then possibly followed by a
    ///   single identifier, and then (required!) arguments.
    ///
    /// Note that if this is updated [skipMetadata] (in util.dart) should be
    /// updated as well.
    ///
    /// Dart (line 1390): `Token parseMetadata(Token token)`
    pub fn parse_metadata(&mut self, token: TokenId) -> TokenId {
        let at_token = self.next(token);
        // assert(atToken.isA(TokenType.AT));
        self.listener.begin_metadata(at_token);
        let mut token = self.ensure_identifier(at_token, IdentifierContext::MetadataReference);
        token = self.parse_qualified_rest_opt(token, IdentifierContext::MetadataContinuation);
        let has_type_arguments = self.is_a(self.next(token), TokenType::LT);
        token = compute_type_param_or_arg_mut(self.tokens_mut(), token, false, false)
            .parse_arguments(token, self);
        let mut period: Option<TokenId> = None;
        if self.is_a(self.next(token), TokenType::PERIOD) {
            let p = self.next(token);
            period = Some(p);
            token = self
                .ensure_identifier(p, IdentifierContext::MetadataContinuationAfterTypeArguments);
        }
        if has_type_arguments && !self.is_a(self.next(token), TokenType::OPEN_PAREN) {
            self.report_recoverable_error(token, diag::metadata_type_arguments_uninstantiated());
        }
        token = self.parse_arguments_opt_metadata(token, has_type_arguments);
        self.listener.end_metadata(at_token, period, token);
        token
    }

    /// ```text
    /// scriptTag:
    ///   '#!' (˜NEWLINE)* NEWLINE
    /// ;
    /// ```
    ///
    /// Dart (line 1422): `Token parseScript(Token token)`
    pub fn parse_script(&mut self, token: TokenId) -> TokenId {
        let token = self.next(token);
        // assert(identical(token.type, TokenType.SCRIPT_TAG));
        self.listener.handle_script(token);
        token
    }

    /// ```text
    /// typeAlias:
    ///   metadata 'typedef' typeAliasBody |
    ///   metadata 'typedef' identifier typeParameters? '=' functionType ';'
    /// ;
    ///
    /// functionType:
    ///   returnType? 'Function' typeParameters? parameterTypeList
    ///
    /// typeAliasBody:
    ///   functionTypeAlias
    /// ;
    ///
    /// functionTypeAlias:
    ///   functionPrefix typeParameters? formalParameterList ‘;’
    /// ;
    ///
    /// functionPrefix:
    ///   returnType? identifier
    /// ;
    /// ```
    ///
    /// Dart (line 1450): `Token parseTypedef(Token? augmentToken, Token typedefKeyword)`
    pub fn parse_typedef(
        &mut self,
        augment_token: Option<TokenId>,
        typedef_keyword: TokenId,
    ) -> TokenId {
        // assert(typedefKeyword.isA(Keyword.TYPEDEF));
        if let Some(augment_token) = augment_token {
            self.report_recoverable_error(augment_token, diag::typedef_augmentation());
        }
        self.listener
            .begin_uncategorized_top_level_declaration(typedef_keyword);
        self.listener.begin_typedef(typedef_keyword);
        let type_info = compute_type_mut(self.tokens_mut(),
            typedef_keyword,
            /* required = */ false,
            false,
            false,
        );
        let mut token = type_info.skip_type_mut(self.tokens_mut(), typedef_keyword);
        let mut next = self.next(token);
        let mut equals: Option<TokenId> = None;
        let mut type_param =
            compute_type_param_or_arg_mut(self.tokens_mut(), next, /* inDeclaration = */ true, false);
        let mut new_style = false;
        let mut new_style_parse_as_recovered = false;
        if type_info == NO_TYPE {
            let mut skip = type_param.skip_mut(self.tokens_mut(), next);
            if self.is_a(self.next(skip), TokenType::EQ) {
                new_style = true;

                // Parse as recovered here to 'force' using it as an identifier as we've
                // already established that the next token is the equal sign we're
                // looking for.
                new_style_parse_as_recovered = true;
            } else if self.is_a(skip, TokenType::EQ) {
                // Recovery: `typedef =` insert missing identifier and parse as new
                // style.
                new_style = true;
                new_style_parse_as_recovered = false;
            } else if self.is_a(skip, TokenType::LT) {
                if self.end_group_next_is_a(skip, TokenType::EQ) {
                    let new_type_param = compute_type_param_or_arg_mut(self.tokens_mut(),
                        token,
                        /* inDeclaration = */ true,
                        false,
                    );
                    skip = new_type_param.skip_mut(self.tokens_mut(), token);
                    // This if shouldn't be necessary, but let's do it anyway.
                    if self.is_a(self.next(skip), TokenType::EQ) {
                        // Recovery: `typedef <whatever> =` insert missing identifier and
                        // parse as new style.
                        type_param = new_type_param;
                        new_style = true;
                        new_style_parse_as_recovered = false;
                    }
                }
            }
        }
        if new_style {
            // New style typedef, e.g. typedef foo = void Function();".
            token = self.ensure_identifier_potentially_recovered(
                token,
                IdentifierContext::TypedefDeclaration,
                /* isRecovered = */ new_style_parse_as_recovered,
            );

            token = type_param.parse_variables(token, self);
            next = self.next(token);
            // parseVariables rewrites so even though we checked in the if,
            // we might not have an equal here now.
            if !self.is_a(next, TokenType::EQ) && self.is_a(self.next(next), TokenType::EQ) {
                // Recovery after recovery: A token was inserted, but we'll skip it now
                // to get more in line with what we thought in the if before.
                next = self.next(next);
            }
            if self.is_a(next, TokenType::EQ) {
                let equals_token = next;
                equals = Some(equals_token);
                let mut ty = compute_type_mut(self.tokens_mut(),
                    equals_token,
                    /* required = */ true,
                    false,
                    false,
                );
                if !ty.is_function_type() {
                    // Recovery: In certain cases insert missing 'Function' and missing
                    // parens.
                    let skipped_type = ty.skip_type_mut(self.tokens_mut(), equals_token);
                    let skipped_type_next = self.next(skipped_type);
                    if self.is_a(skipped_type_next, TokenType::OPEN_PAREN)
                        && self.end_group(skipped_type_next).is_some()
                        && self.is_a(
                            self.next(self.end_group(skipped_type_next).unwrap()),
                            TokenType::SEMICOLON,
                        )
                    {
                        // Turn "<return type>? '(' <whatever> ')';"
                        // into "<return type>? Function '(' <whatever> ')';".
                        // Assume the type is meant as the return type.
                        let function_token = self
                            .rewriter()
                            .insert_synthetic_keyword(skipped_type, Keyword::FUNCTION);
                        self.report_recoverable_error(
                            function_token,
                            diag::expected_but_got("Function"),
                        );
                        ty = compute_type_mut(self.tokens_mut(),
                            equals_token,
                            /* required = */ true,
                            false,
                            false,
                        );
                    } else if ty == NO_TYPE
                        && self.is_a(skipped_type_next, TokenType::LT)
                        && self.end_group(skipped_type_next).is_some()
                    {
                        // Recover these two:
                        // "<whatever>;" => "Function<whatever>();"
                        // "<whatever>(<whatever>);" => "Function<whatever>(<whatever>);"
                        let end_group = self.end_group(skipped_type_next).unwrap();
                        let mut recover = false;
                        let end_group_next = self.next(end_group);
                        if self.is_a(end_group_next, TokenType::SEMICOLON) {
                            // Missing parenthesis. Insert them.
                            // Turn "<whatever>;" into "<whatever>();"
                            // Insert missing 'Function' below.
                            let message =
                                self.missing_parameter_message(MemberKind::FunctionTypeAlias);
                            self.report_recoverable_error(end_group, message);
                            self.rewriter()
                                .insert_parens(end_group, /* includeIdentifier = */ false);
                            recover = true;
                        } else if self.is_a(end_group_next, TokenType::OPEN_PAREN)
                            && self.end_group(end_group_next).is_some()
                            && self.is_a(
                                self.next(self.end_group(end_group_next).unwrap()),
                                TokenType::SEMICOLON,
                            )
                        {
                            // "<whatever>(<whatever>);". Insert missing 'Function' below.
                            recover = true;
                        }

                        if recover {
                            // Assume the '<' indicates type arguments to the function.
                            // Insert 'Function' before them.
                            let function_token = self
                                .rewriter()
                                .insert_synthetic_keyword(equals_token, Keyword::FUNCTION);
                            self.report_recoverable_error(
                                function_token,
                                diag::expected_but_got("Function"),
                            );
                            ty = compute_type_mut(self.tokens_mut(),
                                equals_token,
                                /* required = */ true,
                                false,
                                false,
                            );
                        }
                    } else {
                        // E.g. "typedef j = foo;" -- don't attempt any recovery.
                    }
                }
                token = ty.ensure_type_or_void(equals_token, self);
            } else {
                // A rewrite caused the = to disappear
                token =
                    self.parse_formal_parameters_required_opt(next, MemberKind::FunctionTypeAlias);
            }
        } else {
            // Old style typedef, e.g. "typedef void foo();".
            token = type_info.parse_type(typedef_keyword, self);
            next = self.next(token);
            let mut is_identifier_recovered = false;
            if self.kind(next) != IDENTIFIER_TOKEN && {
                let skipped = type_param.skip_mut(self.tokens_mut(), next);
                self.is_a(self.next(skipped), TokenType::OPEN_PAREN)
            } {
                // Recovery: Not a valid identifier, but is used as such.
                is_identifier_recovered = true;
            }
            token = self.ensure_identifier_potentially_recovered(
                token,
                IdentifierContext::TypedefDeclaration,
                is_identifier_recovered,
            );
            token = type_param.parse_variables(token, self);
            token = self.parse_formal_parameters_required_opt(token, MemberKind::FunctionTypeAlias);
        }
        token = self.ensure_semicolon(token);
        self.listener
            .end_typedef(augment_token, typedef_keyword, equals, token);
        token
    }

    /// Parse a mixin application starting from `with`. Assumes that the first
    /// type has already been parsed.
    ///
    /// Dart (line 1616): `Token parseMixinApplicationRest(Token token)`
    pub fn parse_mixin_application_rest(&mut self, token: TokenId) -> TokenId {
        let mut with_keyword = self.next(token);
        if !self.is_a(with_keyword, Keyword::WITH) {
            // Recovery: Report an error and insert synthetic `with` clause.
            self.report_recoverable_error(with_keyword, diag::expected_but_got("with"));
            with_keyword = self
                .rewriter()
                .insert_synthetic_keyword(token, Keyword::WITH);
            if !is_valid_non_record_type_reference(self.tokens(), self.next(with_keyword)) {
                self.rewriter()
                    .insert_synthetic_identifier(with_keyword, "");
            }
        }
        let token = self.parse_type_list(with_keyword);
        self.listener
            .handle_named_mixin_application_with_clause(with_keyword);
        token
    }

    /// Dart (line 1634): `Token parseClassWithClauseOpt(Token token)`
    pub fn parse_class_with_clause_opt(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        // <mixins> ::= with <typeNotVoidList>
        let with_keyword = self.next(token);
        if self.is_a(with_keyword, Keyword::WITH) {
            token = self.parse_type_list(with_keyword);
            self.listener.handle_class_with_clause(with_keyword);
        } else {
            self.listener.handle_class_no_with_clause();
        }
        token
    }

    /// Dart (line 1646): `Token parseEnumWithClauseOpt(Token token)`
    pub fn parse_enum_with_clause_opt(&mut self, token: TokenId) -> TokenId {
        let mut token = token;
        // <mixins> ::= with <typeNotVoidList>
        let with_keyword = self.next(token);
        if self.is_a(with_keyword, Keyword::WITH) {
            token = self.parse_type_list(with_keyword);
            self.listener.handle_enum_with_clause(with_keyword);
        } else {
            self.listener.handle_enum_no_with_clause();
        }
        token
    }

    /// Parse the formal parameters of a getter (which shouldn't have parameters)
    /// or function or method.
    ///
    /// Dart (line 1660): `Token parseGetterOrFormalParameters( Token token, Token name, bool isGetter, MemberKind kind, )`
    pub fn parse_getter_or_formal_parameters(
        &mut self,
        token: TokenId,
        name: TokenId,
        is_getter: bool,
        kind: MemberKind,
    ) -> TokenId {
        let mut token = token;
        let mut name = name;
        let next = self.next(token);
        if self.is_a(next, TokenType::OPEN_PAREN) {
            if is_getter {
                self.report_recoverable_error(next, diag::getter_with_formals());
            }
            token = self.parse_formal_parameters(token, kind);
        } else if is_getter {
            self.listener.handle_no_formal_parameters(next, kind);
        } else {
            // Recovery
            if self.is_a(name, Keyword::OPERATOR) {
                let next = self.next(name);
                if self.is_operator(next) {
                    name = next;
                } else if self.is_unary_minus(next) {
                    name = self.next(next);
                }
            }
            let message = self.missing_parameter_message(kind);
            self.report_recoverable_error(name, message);
            token = self
                .rewriter()
                .insert_parens(token, /* includeIdentifier = */ false);
            token = self.parse_formal_parameters_rest(token, kind);
        }
        token
    }

    /// Dart (line 1691): `Token parseFormalParametersOpt(Token token, MemberKind kind)`
    pub fn parse_formal_parameters_opt(&mut self, token: TokenId, kind: MemberKind) -> TokenId {
        let mut token = token;
        let next = self.next(token);
        if self.is_a(next, TokenType::OPEN_PAREN) {
            token = self.parse_formal_parameters(token, kind);
        } else {
            self.listener.handle_no_formal_parameters(next, kind);
        }
        token
    }

    /// Dart (line 1701): `Token skipFormalParameters(Token token, MemberKind kind)`
    pub fn skip_formal_parameters(&mut self, token: TokenId, kind: MemberKind) -> TokenId {
        let next = self.next(token);
        self.skip_formal_parameters_rest(next, kind)
    }

    /// Dart (line 1705): `Token skipFormalParametersRest(Token token, MemberKind kind)`
    pub fn skip_formal_parameters_rest(&mut self, token: TokenId, kind: MemberKind) -> TokenId {
        // assert(token.isA(TokenType.OPEN_PAREN));
        // TODO(ahe): Shouldn't this be `beginFormalParameters`?
        self.listener.begin_optional_formal_parameters(token);
        let close_brace = self.end_group(token).unwrap();
        // assert(closeBrace.isA(TokenType.CLOSE_PAREN));
        self.listener
            .end_formal_parameters(/* count = */ 0, token, close_brace, kind);
        close_brace
    }

    /// Parse a record type similarly as a formal parameter list of a function.
    ///
    /// recordType          ::= '(' recordTypeFields ',' recordTypeNamedFields ')'
    /// ```text
    ///                       | '(' recordTypeFields ','? ')'
    ///                       | '(' recordTypeNamedFields? ')'
    /// ```
    ///
    /// recordTypeFields      ::= recordTypeField ( ',' recordTypeField )*
    /// recordTypeField       ::= metadata type identifier?
    ///
    /// recordTypeNamedFields ::= '{' recordTypeNamedField
    /// ```text
    ///                           ( ',' recordTypeNamedField )* ','? '}'
    /// ```
    /// recordTypeNamedField  ::= metadata type identifier
    ///
    /// Dart (line 1727): `Token parseRecordType( Token start, Token token, bool isQuestionMarkPartOfType, )`
    pub fn parse_record_type(
        &mut self,
        start: TokenId,
        token: TokenId,
        is_question_mark_part_of_type: bool,
    ) -> TokenId {
        let mut token = self.next(token);
        // assert(token.isA(TokenType.OPEN_PAREN));

        self.listener.begin_record_type(start);

        let begin = token;

        // parameterCount counting the presence of named fields as 1.
        let mut parameter_count: i32 = 0;
        let mut has_named_fields = false;
        let mut saw_comma = false;
        let mut illegal_trailing_comma: Option<TokenId> = None;
        loop {
            let mut next = self.next(token);
            if self.is_a(next, TokenType::CLOSE_PAREN) {
                token = next;
                break;
            } else if parameter_count == 0
                && self.is_a(next, TokenType::COMMA)
                && self.is_a(self.next(next), TokenType::CLOSE_PAREN)
            {
                illegal_trailing_comma = Some(next);
                token = self.next(next);
                break;
            }
            parameter_count += 1;
            let value = self.string_value(next);
            if value == Some("{") {
                has_named_fields = true;
                token = self.parse_record_type_named_fields(token);
                token = self.ensure_close_paren(token, begin);
                break;
            }
            token = self.parse_record_type_field(token, /* identifierIsOptional = */ true);
            next = self.next(token);
            if !self.is_a(next, TokenType::COMMA) {
                let next = self.next(token);
                if self.is_a(next, TokenType::CLOSE_PAREN) {
                    token = next;
                } else {
                    // Recovery.
                    // TODO: This is copied from parseFormalParametersRest.
                    // We could possibly either have more specific recovery here
                    // or have the recovery in a shared method.
                    let begin_end_group = self.end_group(begin).unwrap();
                    if self.is_synthetic(begin_end_group) {
                        // Scanner has already reported a missing `)` error,
                        // but placed the `)` in the wrong location, so move it.
                        token = self.rewriter().move_synthetic(token, begin_end_group);
                    } else if self.kind(next) == IDENTIFIER_TOKEN
                        && self.kind(self.next(next)) == IDENTIFIER_TOKEN
                    {
                        // Looks like a missing comma
                        let comma = self.new_synthetic_token(TokenType::COMMA, next);
                        token = self.rewrite_and_recover(token, diag::expected_but_got(","), comma);
                        continue;
                    } else {
                        token = self.ensure_close_paren(token, begin);
                    }
                }
                break;
            } else {
                saw_comma = true;
            }
            token = next;
        }
        // assert(token.isA(TokenType.CLOSE_PAREN));

        if parameter_count == 0 && illegal_trailing_comma.is_some() {
            // Empty record type with a comma `(,)`.
            self.report_recoverable_error(
                illegal_trailing_comma.unwrap(),
                diag::record_type_zero_fields_but_trailing_comma(),
            );
        } else if parameter_count == 1 && !has_named_fields && !saw_comma {
            // Single non-named element without trailing comma.
            self.report_recoverable_error(
                token,
                diag::record_type_one_positional_field_no_trailing_comma(),
            );
        }

        // Only consume the `?` if it is part of the type.
        let mut question_mark: Option<TokenId> = Some(self.next(token));
        if self.is_a(question_mark.unwrap(), TokenType::QUESTION) && is_question_mark_part_of_type {
            token = question_mark.unwrap();
        } else {
            question_mark = None;
        }
        self.listener.end_record_type(
            start,
            question_mark,
            parameter_count,
            /* hasNamedFields = */ has_named_fields,
        );

        token
    }

    /// Dart (line 1831): `Token parseRecordTypeField( Token token,`
    pub fn parse_record_type_field(
        &mut self,
        token: TokenId,
        identifier_is_optional: bool,
    ) -> TokenId {
        self.listener.begin_record_type_entry();
        let mut token = self.parse_metadata_star(token);
        token = compute_type_mut(self.tokens_mut(),
            token,
            /* required = */ true,
            false,
            false,
        )
        .ensure_type_or_void(token, self);
        if self.is_identifier(self.next(token)) || !identifier_is_optional {
            token = self.ensure_identifier(token, IdentifierContext::RecordFieldDeclaration);
        } else {
            let next = self.next(token);
            self.listener.handle_no_name(next);
        }
        self.listener.end_record_type_entry();
        token
    }

    /// Dart (line 1850): `Token parseRecordTypeNamedFields(Token token)`
    pub fn parse_record_type_named_fields(&mut self, token: TokenId) -> TokenId {
        let mut token = self.next(token);
        let begin = token;
        // assert(token.isA(TokenType.OPEN_CURLY_BRACKET));
        self.listener.begin_record_type_named_fields(begin);
        let mut parameter_count: i32 = 0;
        let mut next;
        loop {
            next = self.next(token);
            if self.is_a(next, TokenType::CLOSE_CURLY_BRACKET) {
                // breaking with next pointing to '}'.
                break;
            }
            token = self.parse_record_type_field(token, /* identifierIsOptional = */ false);
            next = self.next(token);
            parameter_count += 1;
            if !self.is_a(next, TokenType::COMMA) {
                if !self.is_a(next, TokenType::CLOSE_CURLY_BRACKET) {
                    // Recovery
                    self.report_recoverable_error(next, diag::expected_but_got("}"));
                    // Scanner guarantees a closing bracket.
                    next = self.end_group(begin).unwrap();
                }
                // breaking with next pointing to '}'.
                break;
            }
            token = next;
        }
        token = next;
        // assert(token.isA(TokenType.CLOSE_CURLY_BRACKET));
        if parameter_count == 0 {
            self.report_recoverable_error(token, diag::empty_record_type_named_fields_list());
        }
        self.listener
            .end_record_type_named_fields(parameter_count, begin);
        token
    }

    /// Parses the formal parameter list of a function.
    ///
    /// If `kind == MemberKind.GeneralizedFunctionType`, then names may be
    /// omitted (except for named arguments). Otherwise, types may be omitted.
    ///
    /// Dart (line 1893): `Token parseFormalParametersRequiredOpt(Token token, MemberKind kind)`
    pub fn parse_formal_parameters_required_opt(
        &mut self,
        token: TokenId,
        kind: MemberKind,
    ) -> TokenId {
        let mut next = self.next(token);
        if !self.is_a(next, TokenType::OPEN_PAREN) {
            let message = self.missing_parameter_message(kind);
            self.report_recoverable_error(next, message);
            next = self
                .rewriter()
                .insert_parens(token, /* includeIdentifier = */ false);
        }
        self.parse_formal_parameters_rest(next, kind)
    }

    /// Parses the formal parameter list of a function given that the left
    /// parenthesis is known to exist.
    ///
    /// If `kind == MemberKind.GeneralizedFunctionType`, then names may be
    /// omitted (except for named arguments). Otherwise, types may be omitted.
    ///
    /// Dart (line 1907): `Token parseFormalParameters(Token token, MemberKind kind)`
    pub fn parse_formal_parameters(&mut self, token: TokenId, kind: MemberKind) -> TokenId {
        let next = self.next(token);
        self.parse_formal_parameters_rest(next, kind)
    }

    /// Parses the formal parameter list of a function given that the left
    /// parenthesis passed in as [token].
    ///
    /// If `kind == MemberKind.GeneralizedFunctionType`, then names may be
    /// omitted (except for named arguments). Otherwise, types may be omitted.
    ///
    /// Dart (line 1916): `Token parseFormalParametersRest(Token token, MemberKind kind)`
    pub fn parse_formal_parameters_rest(&mut self, token: TokenId, kind: MemberKind) -> TokenId {
        let mut token = token;
        let begin = token;
        // assert(token.isA(TokenType.OPEN_PAREN));
        self.listener.begin_formal_parameters(begin, kind);
        let mut parameter_count: i32 = 0;
        loop {
            let mut next = self.next(token);
            if self.is_a(next, TokenType::CLOSE_PAREN) {
                token = next;
                break;
            }
            parameter_count += 1;
            let value = self.string_value(next);
            if value == Some("[") {
                token = self.parse_optional_positional_parameters(token, kind);
                token = self.ensure_close_paren(token, begin);
                break;
            } else if value == Some("{") {
                token = self.parse_optional_named_parameters(token, kind);
                token = self.ensure_close_paren(token, begin);
                break;
            } else if value == Some("[]") {
                // Recovery
                token = self.rewrite_square_brackets(token);
                token = self.parse_optional_positional_parameters(token, kind);
                token = self.ensure_close_paren(token, begin);
                break;
            }
            token =
                self.parse_formal_parameter(token, FormalParameterKind::RequiredPositional, kind);
            next = self.next(token);
            if !self.is_a(next, TokenType::COMMA) {
                let next = self.next(token);
                if self.is_a(next, TokenType::CLOSE_PAREN) {
                    token = next;
                } else {
                    // Recovery
                    let begin_end_group = self.end_group(begin).unwrap();
                    if self.is_synthetic(begin_end_group) {
                        // Scanner has already reported a missing `)` error,
                        // but placed the `)` in the wrong location, so move it.
                        token = self.rewriter().move_synthetic(token, begin_end_group);
                    } else if self.kind(next) == IDENTIFIER_TOKEN
                        && self.kind(self.next(next)) == IDENTIFIER_TOKEN
                    {
                        // Looks like a missing comma
                        let comma = self.new_synthetic_token(TokenType::COMMA, next);
                        token = self.rewrite_and_recover(token, diag::expected_but_got(","), comma);
                        continue;
                    } else {
                        token = self.ensure_close_paren(token, begin);
                    }
                }
                break;
            }
            token = next;
        }
        // assert(token.isA(TokenType.CLOSE_PAREN));
        self.listener
            .end_formal_parameters(parameter_count, begin, token, kind);
        token
    }

    /// Dart `new SyntheticToken(type, at.charOffset)`: a new synthetic token
    /// at the position of [at].
    fn new_synthetic_token(&mut self, ty: TokenType, at: TokenId) -> TokenId {
        let offset = self.char_offset(at);
        let byte_offset = self.tokens().byte_offset(at);
        self.tokens_mut().push_synthetic(ty, offset, byte_offset)
    }

    /// Dart `new SyntheticStringToken(TokenType.IDENTIFIER, '',
    /// at.charOffset, /* _length = */ 0)`.
    fn new_empty_synthetic_identifier(&mut self, at: TokenId) -> TokenId {
        let offset = self.char_offset(at);
        let byte_offset = self.tokens().byte_offset(at);
        self.tokens_mut().push_synthetic_string(
            TokenType::IDENTIFIER,
            "",
            offset,
            byte_offset,
            Some(0),
        )
    }

    /// Return the message that should be produced when the formal parameters are
    /// missing.
    ///
    /// Dart (line 1984): `codes.Message missingParameterMessage(MemberKind kind)`
    pub fn missing_parameter_message(&mut self, kind: MemberKind) -> CfeMessage {
        match kind {
            MemberKind::FunctionTypeAlias => diag::missing_typedef_parameters(),
            MemberKind::StaticMethod | MemberKind::NonStaticMethod => {
                diag::missing_method_parameters()
            }
            MemberKind::TopLevelMethod
            | MemberKind::ExtensionNonStaticMethod
            | MemberKind::ExtensionStaticMethod
            | MemberKind::ExtensionTypeNonStaticMethod
            | MemberKind::ExtensionTypeStaticMethod
            | MemberKind::Catch
            | MemberKind::Factory
            | MemberKind::FunctionTypedParameter
            | MemberKind::GeneralizedFunctionType
            | MemberKind::Local
            | MemberKind::AnonymousMethod
            | MemberKind::NonStaticField
            | MemberKind::StaticField
            | MemberKind::TopLevelField
            | MemberKind::PrimaryConstructor => diag::missing_function_parameters(),
        }
    }

    /// ```text
    /// normalFormalParameter:
    ///   functionFormalParameter |
    ///   fieldFormalParameter |
    ///   simpleFormalParameter
    /// ;
    ///
    /// functionFormalParameter:
    ///   metadata 'covariant'? returnType? identifier formalParameterList
    /// ;
    ///
    /// simpleFormalParameter:
    ///   metadata 'covariant'? finalConstVarOrType? identifier |
    /// ;
    ///
    /// fieldFormalParameter:
    ///   metadata finalConstVarOrType? 'this' '.' identifier formalParameterList?
    /// ;
    /// ```
    ///
    /// Dart (line 2029): `Token parseFormalParameter( Token token, FormalParameterKind parameterKind, MemberKind memberKind, )`
    // The dead stores to `next` mirror the Dart code.
    #[allow(unused_assignments)]
    pub fn parse_formal_parameter(
        &mut self,
        token: TokenId,
        parameter_kind: FormalParameterKind,
        member_kind: MemberKind,
    ) -> TokenId {
        let mut parameter_kind = parameter_kind;
        let mut token = self.parse_metadata_star(token);

        let skipped_non_required_required: Option<TokenId> = None;
        let mut next = self.next(token);
        let start = next;

        let in_function_type = member_kind == MemberKind::GeneralizedFunctionType;

        let mut required_token: Option<TokenId> = None;
        let mut covariant_token: Option<TokenId> = None;
        let mut var_final_or_const: Option<TokenId> = None;
        if is_modifier(self.tokens(), next) {
            if self.is_a(next, Keyword::REQUIRED) {
                if parameter_kind == FormalParameterKind::OptionalNamed {
                    parameter_kind = FormalParameterKind::RequiredNamed;
                    token = next;
                    required_token = Some(token);
                    next = self.next(token);
                }
            }

            if is_modifier(self.tokens(), next) {
                if self.is_a(next, Keyword::COVARIANT) {
                    match member_kind {
                        MemberKind::StaticMethod
                        | MemberKind::TopLevelMethod
                        | MemberKind::ExtensionNonStaticMethod
                        | MemberKind::ExtensionStaticMethod
                        | MemberKind::ExtensionTypeNonStaticMethod
                        | MemberKind::ExtensionTypeStaticMethod
                        | MemberKind::PrimaryConstructor => {
                            // Error cases reported in
                            // [ModifierContext.parseFormalParameterModifiers].
                        }
                        MemberKind::Catch
                        | MemberKind::Factory
                        | MemberKind::FunctionTypeAlias
                        | MemberKind::FunctionTypedParameter
                        | MemberKind::GeneralizedFunctionType
                        | MemberKind::Local
                        | MemberKind::AnonymousMethod
                        | MemberKind::NonStaticMethod
                        | MemberKind::NonStaticField
                        | MemberKind::StaticField
                        | MemberKind::TopLevelField => {
                            token = next;
                            covariant_token = Some(token);
                            next = self.next(token);
                        }
                    }
                }

                if is_modifier(self.tokens(), next) {
                    if !in_function_type {
                        if self.is_a(next, Keyword::VAR) {
                            token = next;
                            var_final_or_const = Some(token);
                            next = self.next(token);
                        } else if self.is_a(next, Keyword::FINAL) {
                            token = next;
                            var_final_or_const = Some(token);
                            next = self.next(token);
                        }
                    }

                    if is_modifier(self.tokens(), next) {
                        // Recovery
                        let mut context = ModifierContext::new();
                        context.covariant_token = covariant_token;
                        context.required_token = required_token;
                        context.set_var_final_or_const(self.tokens(), var_final_or_const);

                        token = context.parse_formal_parameter_modifiers(
                            self,
                            token,
                            parameter_kind,
                            member_kind,
                        );
                        next = self.next(token);

                        covariant_token = context.covariant_token;
                        required_token = context.required_token;
                        var_final_or_const = context.var_final_or_const();
                    }
                }
            }
        }

        if required_token.is_none() {
            // `required` was used as a modifier in non-nnbd mode. An error has been
            // emitted. Still use it as a required token for the remainder in an
            // attempt to avoid cascading errors (and for passing to the listener).
            required_token = skipped_non_required_required;
        }

        self.listener.begin_formal_parameter(
            start,
            member_kind,
            required_token,
            covariant_token,
            var_final_or_const,
        );

        // Type is required in a generalized function type, but optional otherwise.
        let before_type = token;
        let mut type_info = compute_type_mut(self.tokens_mut(),
            token,
            in_function_type,
            /* inDeclaration = */ false,
            /* acceptKeywordForSimpleType = */ true,
        );
        token = type_info.skip_type_mut(self.tokens_mut(), token);
        next = self.next(token);
        if type_info == NO_TYPE
            && (self.is_a(next, TokenType::PERIOD)
                || (self.is_identifier(next) && self.is_a(self.next(next), TokenType::PERIOD)))
        {
            // Recovery: Malformed type reference.
            type_info = compute_type_mut(self.tokens_mut(),
                before_type,
                /* required = */ true,
                false,
                false,
            );
            token = type_info.skip_type_mut(self.tokens_mut(), before_type);
            next = self.next(token);
        }

        let mut this_keyword: Option<TokenId> = None;
        let mut super_keyword: Option<TokenId> = None;
        let mut period_after_this_or_super: Option<TokenId> = None;
        let mut name_context = IdentifierContext::FormalParameterDeclaration;

        if !in_function_type && (self.is_a(next, Keyword::THIS) || self.is_a(next, Keyword::SUPER))
        {
            let original_token = token;
            if self.is_a(next, Keyword::THIS) {
                token = next;
                this_keyword = Some(token);
            } else {
                token = next;
                super_keyword = Some(token);
            }
            next = self.next(token);
            if !self.is_a(next, TokenType::PERIOD) {
                if is_ok_next_value_in_formal_parameter(self.tokens(), next) {
                    // Recover by not parsing as 'this' --- an error will be given
                    // later that it's not an allowed identifier.
                    token = original_token;
                    next = self.next(token);
                    this_keyword = None;
                    super_keyword = None;
                } else {
                    // Recover from a missing period by inserting one.
                    let period = self.new_synthetic_token(TokenType::PERIOD, next);
                    next = self.rewrite_and_recover(token, diag::expected_but_got("."), period);
                    // These 3 lines are duplicated here and below.
                    token = next;
                    period_after_this_or_super = Some(token);
                    next = self.next(token);
                    name_context = IdentifierContext::FieldInitializer;
                }
            } else {
                // These 3 lines are duplicated here and above.
                token = next;
                period_after_this_or_super = Some(token);
                next = self.next(token);
                name_context = IdentifierContext::FieldInitializer;
            }
        }
        if member_kind == MemberKind::PrimaryConstructor {
            if let Some(var_final_or_const) = var_final_or_const {
                if !self.is_a(var_final_or_const, Keyword::CONST) {
                    if let Some(this_keyword) = this_keyword {
                        self.report_recoverable_error(
                            this_keyword,
                            diag::initializing_declaring_parameter(),
                        );
                    } else if let Some(super_keyword) = super_keyword {
                        self.report_recoverable_error(
                            super_keyword,
                            diag::super_initializing_declaring_parameter(),
                        );
                    }
                }
            }
        }

        if self.is_identifier(next) {
            token = next;
            next = self.next(token);
        }
        let mut before_inline_function_type: Option<TokenId> = None;
        let mut type_param: TypeParamOrArgInfo = NO_TYPE_PARAM_OR_ARG;
        if self.is_a(next, TokenType::LT) {
            type_param = compute_type_param_or_arg_mut(self.tokens_mut(), token, false, false);
            if type_param != NO_TYPE_PARAM_OR_ARG {
                let closer = type_param.skip_mut(self.tokens_mut(), token);
                if self.is_a(self.next(closer), TokenType::OPEN_PAREN) {
                    if let Some(var_final_or_const) = var_final_or_const {
                        if member_kind != MemberKind::PrimaryConstructor {
                            self.report_recoverable_error(
                                var_final_or_const,
                                diag::function_typed_parameter_var(),
                            );
                        } else {
                            if !self.is_primary_constructors_feature_enabled {
                                self.report_experiment_not_enabled(
                                    ExperimentalFlag::PrimaryConstructors,
                                    var_final_or_const,
                                    var_final_or_const,
                                );
                            }
                        }
                    }
                    before_inline_function_type = Some(token);
                    token = self.end_group(self.next(closer)).unwrap();
                    next = self.next(token);
                }
            }
        } else if self.is_a(next, TokenType::OPEN_PAREN) {
            if let Some(var_final_or_const) = var_final_or_const {
                if member_kind != MemberKind::PrimaryConstructor {
                    self.report_recoverable_error(
                        var_final_or_const,
                        diag::function_typed_parameter_var(),
                    );
                } else {
                    if !self.is_primary_constructors_feature_enabled {
                        self.report_experiment_not_enabled(
                            ExperimentalFlag::PrimaryConstructors,
                            var_final_or_const,
                            var_final_or_const,
                        );
                    }
                }
            }
            before_inline_function_type = Some(token);
            token = self.end_group(next).unwrap();
            next = self.next(token);
        }
        let mut var_or_final: Option<TokenId> = None;
        if let Some(var_final_or_const) = var_final_or_const {
            if self.is_a(var_final_or_const, Keyword::VAR) {
                if self.is_primary_constructors_feature_enabled
                    && member_kind != MemberKind::PrimaryConstructor
                {
                    self.report_recoverable_error_with_token(
                        var_final_or_const,
                        diag::extraneous_modifier,
                    );
                }
                var_or_final = Some(var_final_or_const);
                if type_info != NO_TYPE {
                    if member_kind != MemberKind::PrimaryConstructor {
                        self.report_recoverable_error(var_final_or_const, diag::type_after_var());
                    } else {
                        if !self.is_primary_constructors_feature_enabled {
                            self.report_experiment_not_enabled(
                                ExperimentalFlag::PrimaryConstructors,
                                var_final_or_const,
                                var_final_or_const,
                            );
                        }
                    }
                }
            } else if self.is_a(var_final_or_const, Keyword::FINAL) {
                if self.is_primary_constructors_feature_enabled
                    && member_kind != MemberKind::PrimaryConstructor
                {
                    self.report_recoverable_error_with_token(
                        var_final_or_const,
                        diag::extraneous_modifier,
                    );
                }
                var_or_final = Some(var_final_or_const);
            }
        }

        let mut end_inline_function_type: Option<TokenId> = None;
        if let Some(before_inline_function_type) = before_inline_function_type {
            let mut end = type_param.parse_variables(before_inline_function_type, self);
            let begin = self.next(before_inline_function_type);
            self.listener.begin_function_typed_formal_parameter(begin);
            token = type_info.parse_type(before_type, self);
            end =
                self.parse_formal_parameters_required_opt(end, MemberKind::FunctionTypedParameter);
            let mut question: Option<TokenId> = None;
            if self.is_a(self.next(end), TokenType::QUESTION) {
                end = self.next(end);
                question = Some(end);
            }
            end_inline_function_type = Some(end);
            self.listener
                .end_function_typed_formal_parameter(before_inline_function_type, question);

            // Generalized function types don't allow inline function types.
            // The following isn't allowed:
            //    int Function(int bar(String x)).
            if in_function_type {
                let next = self.next(before_inline_function_type);
                self.report_recoverable_error(next, diag::invalid_inline_function_type());
            }
        } else if in_function_type {
            token = type_info.ensure_type_or_void(before_type, self);
        } else {
            token = type_info.parse_type(before_type, self);
        }

        let name_token;
        if let Some(period_after_this_or_super) = period_after_this_or_super {
            token = period_after_this_or_super;
        }
        let is_named_parameter = parameter_kind.is_named();
        next = self.next(token);
        if in_function_type
            && !is_named_parameter
            && !self.is_keyword_or_identifier(next)
            && before_inline_function_type.is_none()
        {
            name_token = self.next(token);
            self.listener.handle_no_name(name_token);
        } else {
            token = self.ensure_identifier(token, name_context);
            name_token = token;
        }
        if let Some(end_inline_function_type) = end_inline_function_type {
            token = end_inline_function_type;
        }
        next = self.next(token);

        let value = self.string_value(next);
        let mut initializer_start: Option<TokenId> = None;
        let mut initializer_end: Option<TokenId> = None;
        if value == Some("=") || value == Some(":") {
            let equal = next;
            initializer_start = Some(self.next(equal));
            self.listener
                .begin_formal_parameter_default_value_expression();
            token = self.parse_expression(equal);
            initializer_end = Some(token);
            next = self.next(token);
            self.listener
                .end_formal_parameter_default_value_expression();
            // TODO(danrubel): Consider removing the last parameter from the
            // handleValuedFormalParameter event... it appears to be unused.
            self.listener
                .handle_valued_formal_parameter(equal, next, parameter_kind);
            if parameter_kind.is_required_positional() {
                self.report_recoverable_error(equal, diag::required_parameter_with_default());
            } else if parameter_kind.is_optional_positional() && value == Some(":") {
                self.report_recoverable_error(equal, diag::positional_parameter_with_equals());
            } else if in_function_type
                || member_kind == MemberKind::FunctionTypeAlias
                || member_kind == MemberKind::FunctionTypedParameter
            {
                self.report_recoverable_error(equal, diag::function_type_default_value());
            }
        } else {
            self.listener.handle_formal_parameter_without_value(next);
        }
        self.listener.end_formal_parameter(
            var_or_final,
            this_keyword,
            super_keyword,
            period_after_this_or_super,
            name_token,
            initializer_start,
            initializer_end,
            parameter_kind,
            member_kind,
        );
        token
    }

    /// ```text
    /// defaultFormalParameter:
    ///   normalFormalParameter ('=' expression)?
    /// ;
    /// ```
    ///
    /// Dart (line 2398): `Token parseOptionalPositionalParameters(Token token, MemberKind kind)`
    pub fn parse_optional_positional_parameters(
        &mut self,
        token: TokenId,
        kind: MemberKind,
    ) -> TokenId {
        let mut token = self.next(token);
        let begin = token;
        // assert(token.isA(TokenType.OPEN_SQUARE_BRACKET));
        self.listener.begin_optional_formal_parameters(begin);
        let mut parameter_count: i32 = 0;
        loop {
            let mut next = self.next(token);
            if self.is_a(next, TokenType::CLOSE_SQUARE_BRACKET) {
                break;
            }
            token =
                self.parse_formal_parameter(token, FormalParameterKind::OptionalPositional, kind);
            next = self.next(token);
            parameter_count += 1;
            if !self.is_a(next, TokenType::COMMA) {
                if !self.is_a(next, TokenType::CLOSE_SQUARE_BRACKET) {
                    // Recovery
                    self.report_recoverable_error(next, diag::expected_but_got("]"));
                    // Scanner guarantees a closing bracket.
                    next = self.end_group(begin).unwrap();
                    while self.next_opt(token) != Some(next) {
                        token = self.next(token);
                    }
                }
                break;
            }
            token = next;
        }
        if parameter_count == 0 {
            let next = self.next(token);
            let identifier = self.new_empty_synthetic_identifier(next);
            self.rewrite_and_recover(token, diag::empty_optional_parameter_list(), identifier);
            token =
                self.parse_formal_parameter(token, FormalParameterKind::OptionalPositional, kind);
            parameter_count += 1;
        }
        token = self.next(token);
        // assert(token.isA(TokenType.CLOSE_SQUARE_BRACKET));
        self.listener
            .end_optional_formal_parameters(parameter_count, begin, token, kind);
        token
    }

    /// ```text
    /// defaultNamedParameter:
    ///   normalFormalParameter ('=' expression)? |
    ///   normalFormalParameter (':' expression)?
    /// ;
    /// ```
    ///
    /// Dart (line 2462): `Token parseOptionalNamedParameters(Token token, MemberKind kind)`
    pub fn parse_optional_named_parameters(&mut self, token: TokenId, kind: MemberKind) -> TokenId {
        let mut token = self.next(token);
        let begin = token;
        // assert(token.isA(TokenType.OPEN_CURLY_BRACKET));
        self.listener.begin_optional_formal_parameters(begin);
        let mut parameter_count: i32 = 0;
        loop {
            let mut next = self.next(token);
            if self.is_a(next, TokenType::CLOSE_CURLY_BRACKET) {
                break;
            }
            token = self.parse_formal_parameter(token, FormalParameterKind::OptionalNamed, kind);
            next = self.next(token);
            parameter_count += 1;
            if !self.is_a(next, TokenType::COMMA) {
                if !self.is_a(next, TokenType::CLOSE_CURLY_BRACKET) {
                    // Recovery
                    self.report_recoverable_error(next, diag::expected_but_got("}"));
                    // Scanner guarantees a closing bracket.
                    next = self.end_group(begin).unwrap();
                    while self.next_opt(token) != Some(next) {
                        token = self.next(token);
                    }
                }
                break;
            }
            token = next;
        }
        if parameter_count == 0 {
            let next = self.next(token);
            let identifier = self.new_empty_synthetic_identifier(next);
            self.rewrite_and_recover(token, diag::empty_named_parameter_list(), identifier);
            token = self.parse_formal_parameter(token, FormalParameterKind::OptionalNamed, kind);
            parameter_count += 1;
        }
        token = self.next(token);
        // assert(token.isA(TokenType.CLOSE_CURLY_BRACKET));
        self.listener
            .end_optional_formal_parameters(parameter_count, begin, token, kind);
        token
    }
}
