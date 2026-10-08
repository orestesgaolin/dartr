// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/modifier_context.dart

//! Dart `ModifierContext(parser)` keeps a reference to the parser; here the
//! methods take `parser` as their first argument: Dart
//! `context.parseClassModifiers(token, keyword)` is
//! `context.parse_class_modifiers(self, token, keyword)` in the parser.

use dartr_diagnostics::cfe_codes as diag;
use dartr_syntax::{Keyword, KeywordStyle, TokenId, TokenType, Tokens};

use crate::formal_parameter_kind::FormalParameterKind;
use crate::listener::Listener;
use crate::member_kind::MemberKind;
use crate::parser_impl::Parser;

/// Dart (line 13): `bool isModifier(Token token)`.
pub fn is_modifier(tokens: &Tokens, token: TokenId) -> bool {
    let ty = tokens.ty(token);
    if !ty.is_modifier() {
        return false;
    } else if ty.keyword_style() == Some(KeywordStyle::BuiltIn) {
        // A built-in keyword can only be a modifier as long as it is
        // followed by another keyword or an identifier. Otherwise, it is the
        // identifier.
        //
        // For example, `external` is a modifier in this declaration:
        //   external Foo foo();
        // but is the identifier in this declaration
        //   external() => true;
        // and in
        //   for (final external in list) { }
        let next = tokens.next(token);
        let next_ty = tokens.ty(next);
        // Dart `next.keyword`: `null` when `next` is not a keyword token.
        let keyword_is_null = !next_ty.is_keyword();
        if keyword_is_null && !tokens.get(next).is_identifier() || next_ty == Keyword::IN {
            // Record type is a possibility.
            if next_ty == TokenType::OPEN_PAREN {
                let after_group = tokens.next(tokens.get(next).end_group.get().unwrap());
                if tokens.get(after_group).is_identifier()
                    || this_or_super_with_dot(tokens, after_group)
                {
                    // We've seen either
                    // [modifier] [record type] [identifier], or
                    // [modifier] [record type] `this` `.`, or
                    // [modifier] [record type] `super` `.`
                    return true;
                } else if tokens.ty(after_group) == TokenType::QUESTION
                    && (tokens.get(tokens.next(after_group)).is_identifier()
                        || this_or_super_with_dot(tokens, tokens.next(after_group)))
                {
                    // We've seen either
                    // [modifier] [record type] `?` [identifier], or
                    // [modifier] [record type] `?` `this` `.`, or
                    // [modifier] [record type] `?` `super` `.`
                    return true;
                }
            }
            return false;
        }
    }
    true
}

/// Dart (line 55): `bool _thisOrSuperWithDot(Token token)`.
fn this_or_super_with_dot(tokens: &Tokens, token: TokenId) -> bool {
    let ty = tokens.ty(token);
    if ty == Keyword::THIS || ty == Keyword::SUPER {
        return tokens.ty(tokens.next(token)) == TokenType::PERIOD;
    }
    false
}

/// Dart `ModifierContext`: the modifiers seen so far.
///
/// This class is used to parse modifiers in most locations where modifiers
/// can occur, but does not call handleModifier or handleModifiers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModifierContext {
    pub abstract_token: Option<TokenId>,
    pub augment_token: Option<TokenId>,
    pub const_token: Option<TokenId>,
    pub covariant_token: Option<TokenId>,
    pub external_token: Option<TokenId>,
    pub final_token: Option<TokenId>,
    pub late_token: Option<TokenId>,
    pub required_token: Option<TokenId>,
    pub static_token: Option<TokenId>,
    pub var_token: Option<TokenId>,
    /// Dart `_afterFactory`: set when parsing modifiers after the `factory`
    /// token.
    pub(crate) after_factory: bool,
}

impl ModifierContext {
    /// Dart (line 80): `new ModifierContext(parser)`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Dart (line 82): setter `staticOrCovariant`.
    pub fn set_static_or_covariant(
        &mut self,
        tokens: &Tokens,
        static_or_covariant: Option<TokenId>,
    ) {
        match static_or_covariant {
            None => {
                self.covariant_token = None;
                self.static_token = None;
            }
            Some(t) if tokens.ty(t) == Keyword::COVARIANT => {
                self.covariant_token = Some(t);
                self.static_token = None;
            }
            Some(t) if tokens.ty(t) == Keyword::STATIC => {
                self.covariant_token = None;
                self.static_token = Some(t);
            }
            Some(t) => {
                panic!(
                    "Internal error: Unexpected staticOrCovariant '{}'.",
                    tokens.lexeme(t)
                );
            }
        }
    }

    /// Dart (line 98): getter `varFinalOrConst`.
    pub fn var_final_or_const(&self) -> Option<TokenId> {
        self.var_token.or(self.final_token).or(self.const_token)
    }

    /// Dart (line 100): setter `varFinalOrConst`.
    pub fn set_var_final_or_const(&mut self, tokens: &Tokens, var_final_or_const: Option<TokenId>) {
        match var_final_or_const {
            None => {
                self.var_token = None;
                self.final_token = None;
                self.const_token = None;
            }
            Some(t) if tokens.ty(t) == Keyword::VAR => {
                self.var_token = Some(t);
                self.final_token = None;
                self.const_token = None;
            }
            Some(t) if tokens.ty(t) == Keyword::FINAL => {
                self.var_token = None;
                self.final_token = Some(t);
                self.const_token = None;
            }
            Some(t) if tokens.ty(t) == Keyword::CONST => {
                self.var_token = None;
                self.final_token = None;
                self.const_token = Some(t);
            }
            Some(t) => {
                panic!(
                    "Internal error: Unexpected varFinalOrConst '{}'.",
                    tokens.lexeme(t)
                );
            }
        }
    }

    /// Dart (line 123): Parse modifiers for class declarations.
    pub fn parse_class_modifiers<L: Listener>(
        &mut self,
        parser: &mut Parser<L>,
        token: TokenId,
        keyword: TokenId,
    ) -> TokenId {
        let token = self.parse_modifiers(parser, token);
        if self.const_token.is_some() {
            self.report_top_level_modifier_error(parser, self.const_token, keyword);
        }
        if self.external_token.is_some() {
            self.report_top_level_modifier_error(parser, self.external_token, keyword);
        }
        self.report_extraneous_modifier(parser, self.covariant_token);
        self.report_extraneous_modifier(parser, self.late_token);
        self.report_extraneous_modifier(parser, self.required_token);
        self.report_extraneous_modifier(parser, self.static_token);
        self.report_extraneous_modifier(parser, self.var_token);
        token
    }

    /// Dart (line 140): Parse modifiers for enum declarations.
    pub fn parse_enum_modifiers<L: Listener>(
        &mut self,
        parser: &mut Parser<L>,
        token: TokenId,
        keyword: TokenId,
    ) -> TokenId {
        let token = self.parse_modifiers(parser, token);
        self.report_top_level_modifier_error(parser, self.const_token, keyword);
        self.report_top_level_modifier_error(parser, self.external_token, keyword);
        self.report_extraneous_modifier(parser, self.abstract_token);
        self.report_extraneous_modifier(parser, self.covariant_token);
        self.report_extraneous_modifier(parser, self.late_token);
        self.report_extraneous_modifier(parser, self.required_token);
        self.report_extraneous_modifier(parser, self.static_token);
        self.report_extraneous_modifier(parser, self.var_token);
        token
    }

    /// Dart (line 154): Parse modifiers for extension declarations.
    pub fn parse_extension_modifiers<L: Listener>(
        &mut self,
        parser: &mut Parser<L>,
        token: TokenId,
        keyword: TokenId,
    ) -> TokenId {
        let token = self.parse_modifiers(parser, token);
        self.report_top_level_modifier_error(parser, self.const_token, keyword);
        self.report_top_level_modifier_error(parser, self.external_token, keyword);
        self.report_extraneous_modifier(parser, self.abstract_token);
        self.report_extraneous_modifier(parser, self.covariant_token);
        self.report_extraneous_modifier(parser, self.late_token);
        self.report_extraneous_modifier(parser, self.required_token);
        self.report_extraneous_modifier(parser, self.static_token);
        self.report_extraneous_modifier(parser, self.var_token);
        token
    }

    /// Dart (line 168): Parse modifiers for mixin declarations.
    pub fn parse_mixin_modifiers<L: Listener>(
        &mut self,
        parser: &mut Parser<L>,
        token: TokenId,
        keyword: TokenId,
    ) -> TokenId {
        let token = self.parse_modifiers(parser, token);
        self.report_top_level_modifier_error(parser, self.const_token, keyword);
        self.report_top_level_modifier_error(parser, self.external_token, keyword);
        self.report_extraneous_modifier(parser, self.abstract_token);
        self.report_extraneous_modifier(parser, self.covariant_token);
        self.report_extraneous_modifier(parser, self.late_token);
        self.report_extraneous_modifier(parser, self.required_token);
        self.report_extraneous_modifier(parser, self.static_token);
        self.report_extraneous_modifier(parser, self.var_token);
        token
    }

    /// Dart (line 183): Parse modifiers for library, import, export, part
    /// (of) directives and typedef and extension declarations.
    pub fn parse_top_level_keyword_modifiers<L: Listener>(
        &mut self,
        parser: &mut Parser<L>,
        token: TokenId,
        keyword: TokenId,
    ) -> TokenId {
        let token = self.parse_modifiers(parser, token);
        self.report_top_level_modifier_error(parser, self.const_token, keyword);
        self.report_top_level_modifier_error(parser, self.external_token, keyword);
        self.report_extraneous_modifier(parser, self.abstract_token);
        self.report_extraneous_modifier(parser, self.augment_token);
        self.report_extraneous_modifier(parser, self.covariant_token);
        self.report_extraneous_modifier(parser, self.final_token);
        self.report_extraneous_modifier(parser, self.late_token);
        self.report_extraneous_modifier(parser, self.required_token);
        self.report_extraneous_modifier(parser, self.static_token);
        self.report_extraneous_modifier(parser, self.var_token);
        token
    }

    /// Dart (line 199): Parse modifiers for class methods and fields.
    pub fn parse_class_member_modifiers<L: Listener>(
        &mut self,
        parser: &mut Parser<L>,
        token: TokenId,
    ) -> TokenId {
        let token = self.parse_modifiers(parser, token);
        self.report_extraneous_modifier(parser, self.required_token);
        token
    }

    /// Dart (line 206): Parse modifiers for formal parameters.
    pub fn parse_formal_parameter_modifiers<L: Listener>(
        &mut self,
        parser: &mut Parser<L>,
        token: TokenId,
        parameter_kind: FormalParameterKind,
        member_kind: MemberKind,
    ) -> TokenId {
        let token = self.parse_modifiers(parser, token);

        if member_kind == MemberKind::PrimaryConstructor {
            if !parameter_kind.is_named() {
                self.report_extraneous_modifier(parser, self.required_token);
            }
        } else {
            if parameter_kind != FormalParameterKind::OptionalNamed {
                self.report_extraneous_modifier(parser, self.required_token);
            }
        }

        match member_kind {
            MemberKind::StaticMethod | MemberKind::TopLevelMethod | MemberKind::AnonymousMethod => {
                self.report_extraneous_modifier(parser, self.covariant_token);
            }
            MemberKind::ExtensionNonStaticMethod | MemberKind::ExtensionStaticMethod => {
                self.report_extraneous_modifier_in_extension(parser, self.covariant_token);
            }
            MemberKind::ExtensionTypeNonStaticMethod | MemberKind::ExtensionTypeStaticMethod => {
                self.report_extraneous_modifier_in_extension_type(parser, self.covariant_token);
            }
            MemberKind::PrimaryConstructor => {
                if let Some(covariant_token) = self.covariant_token {
                    if parser.is_primary_constructors_feature_enabled {
                        if self.var_final_or_const().is_none()
                            || !parser.is_a(self.var_final_or_const().unwrap(), Keyword::VAR)
                        {
                            parser.report_recoverable_error(
                                covariant_token,
                                diag::invalid_covariant_modifier_in_primary_constructor(),
                            );
                        }
                    } else {
                        parser.report_recoverable_error_with_token(
                            covariant_token,
                            diag::extraneous_modifier_in_primary_constructor,
                        );
                    }
                }
            }
            MemberKind::Catch
            | MemberKind::Factory
            | MemberKind::FunctionTypeAlias
            | MemberKind::FunctionTypedParameter
            | MemberKind::GeneralizedFunctionType
            | MemberKind::Local
            | MemberKind::NonStaticMethod
            | MemberKind::NonStaticField
            | MemberKind::StaticField
            | MemberKind::TopLevelField => {}
        }
        if self.const_token.is_some() {
            self.report_extraneous_modifier(parser, self.const_token);
        } else if member_kind == MemberKind::GeneralizedFunctionType {
            if let Some(var_final_or_const) = self.var_final_or_const() {
                parser.report_recoverable_error(
                    var_final_or_const,
                    diag::function_typed_parameter_var(),
                );
            }
        }
        self.report_extraneous_modifier(parser, self.abstract_token);
        self.report_extraneous_modifier(parser, self.external_token);
        self.report_extraneous_modifier(parser, self.late_token);
        self.report_extraneous_modifier(parser, self.static_token);
        token
    }

    /// Dart (line 279): Parse modifiers for library directives.
    pub fn parse_library_directive_modifiers<L: Listener>(
        &mut self,
        parser: &mut Parser<L>,
        token: TokenId,
        keyword: TokenId,
    ) -> TokenId {
        let token = self.parse_modifiers(parser, token);
        self.report_top_level_modifier_error(parser, self.const_token, keyword);
        self.report_top_level_modifier_error(parser, self.external_token, keyword);
        self.report_extraneous_modifier(parser, self.abstract_token);
        self.report_extraneous_modifier(parser, self.covariant_token);
        self.report_extraneous_modifier(parser, self.final_token);
        self.report_extraneous_modifier(parser, self.late_token);
        self.report_extraneous_modifier(parser, self.required_token);
        self.report_extraneous_modifier(parser, self.static_token);
        self.report_extraneous_modifier(parser, self.var_token);
        token
    }

    /// Dart (line 294): Parse modifiers after the `factory` token.
    pub fn parse_modifiers_after_factory<L: Listener>(
        &mut self,
        parser: &mut Parser<L>,
        token: TokenId,
    ) -> TokenId {
        self.after_factory = true;
        let token = self.parse_modifiers(parser, token);
        if let Some(abstract_token) = self.abstract_token {
            parser.report_recoverable_error(abstract_token, diag::abstract_class_member());
        }
        self.report_extraneous_modifier(parser, self.late_token);
        self.report_extraneous_modifier(parser, self.required_token);
        token
    }

    /// Dart (line 306): Parse modifiers for top level functions and fields.
    pub fn parse_top_level_member_modifiers<L: Listener>(
        &mut self,
        parser: &mut Parser<L>,
        token: TokenId,
    ) -> TokenId {
        let token = self.parse_modifiers(parser, token);
        self.report_extraneous_modifier(parser, self.covariant_token);
        self.report_extraneous_modifier(parser, self.required_token);
        self.report_extraneous_modifier(parser, self.static_token);
        token
    }

    /// Dart (line 314): `parseTypedefModifiers`.
    pub fn parse_typedef_modifiers<L: Listener>(
        &mut self,
        parser: &mut Parser<L>,
        token: TokenId,
        keyword: TokenId,
    ) -> TokenId {
        let token = self.parse_modifiers(parser, token);
        self.report_top_level_modifier_error(parser, self.const_token, keyword);
        self.report_top_level_modifier_error(parser, self.external_token, keyword);
        self.report_extraneous_modifier(parser, self.abstract_token);
        self.report_extraneous_modifier(parser, self.covariant_token);
        self.report_extraneous_modifier(parser, self.final_token);
        self.report_extraneous_modifier(parser, self.late_token);
        self.report_extraneous_modifier(parser, self.required_token);
        self.report_extraneous_modifier(parser, self.static_token);
        self.report_extraneous_modifier(parser, self.var_token);
        token
    }

    /// Dart (line 329): Parse modifiers for variable declarations.
    pub fn parse_variable_declaration_modifiers<L: Listener>(
        &mut self,
        parser: &mut Parser<L>,
        token: TokenId,
    ) -> TokenId {
        let token = self.parse_modifiers(parser, token);
        self.report_extraneous_modifier(parser, self.abstract_token);
        self.report_extraneous_modifier(parser, self.covariant_token);
        self.report_extraneous_modifier(parser, self.external_token);
        self.report_extraneous_modifier(parser, self.required_token);
        self.report_extraneous_modifier(parser, self.static_token);
        self.report_extraneous_modifier(parser, self.augment_token);
        token
    }

    /// Dart (line 351): `_parseModifiers`.
    ///
    /// Parse modifiers during recovery when modifiers are out of order
    /// or invalid. Typically clients call methods like
    /// [parseClassMemberModifiers] which in turn calls this method,
    /// rather than calling this method directly.
    ///
    /// The various modifier token parameters represent tokens of modifiers
    /// that have already been parsed prior to recovery. The [staticOrCovariant]
    /// parameter is for convenience if caller has a token that may be either
    /// `static` or `covariant`. The first non-null parameter of
    /// [staticOrCovariant], [staticToken], or [covariantToken] will be used,
    /// in that order, and the others ignored.
    fn parse_modifiers<L: Listener>(
        &mut self,
        parser: &mut Parser<L>,
        mut token: TokenId,
    ) -> TokenId {
        // Process invalid and out-of-order modifiers
        let mut next = parser.next(token);
        loop {
            let value = parser.string_value(next);
            if is_modifier(parser.tokens(), next) {
                match value {
                    Some("abstract") => token = self.parse_abstract(parser, token),
                    Some("augment") => token = self.parse_augment(parser, token),
                    Some("const") => token = self.parse_const(parser, token),
                    Some("covariant") => token = self.parse_covariant(parser, token),
                    Some("external") => token = self.parse_external(parser, token),
                    Some("final") => token = self.parse_final(parser, token),
                    Some("late") => token = self.parse_late(parser, token),
                    Some("required") => token = self.parse_required(parser, token),
                    Some("static") => token = self.parse_static(parser, token),
                    Some("var") => token = self.parse_var(parser, token),
                    _ => panic!("Internal Error: Unhandled modifier: {:?}", value),
                }
            } else if self.after_factory && value == Some("factory") {
                parser.report_recoverable_error_with_token(next, diag::duplicated_modifier);
                token = next;
            } else {
                break;
            }
            next = parser.next(token);
        }
        token
    }

    /// Dart (line 391): `_parseAbstract`.
    fn parse_abstract<L: Listener>(&mut self, parser: &mut Parser<L>, token: TokenId) -> TokenId {
        let next = parser.next(token);
        debug_assert!(parser.is_a(next, Keyword::ABSTRACT));
        if self.abstract_token.is_none() {
            self.abstract_token = Some(next);

            if let Some(var_final_or_const) = self.var_final_or_const() {
                self.report_modifier_out_of_order_token(parser, next, var_final_or_const);
            } else if let Some(covariant_token) = self.covariant_token {
                self.report_modifier_out_of_order_token(parser, next, covariant_token);
            }
            return next;
        }

        // Recovery
        parser.report_recoverable_error_with_token(next, diag::duplicated_modifier);
        next
    }

    /// Dart (line 410): `_parseAugment`.
    fn parse_augment<L: Listener>(&mut self, parser: &mut Parser<L>, token: TokenId) -> TokenId {
        let next = parser.next(token);
        debug_assert!(parser.is_a(next, Keyword::AUGMENT));
        if self.augment_token.is_none() {
            self.augment_token = Some(next);

            if let Some(t) = self.var_final_or_const() {
                self.report_modifier_out_of_order_token(parser, next, t);
            } else if let Some(t) = self.abstract_token {
                self.report_modifier_out_of_order_token(parser, next, t);
            } else if let Some(t) = self.const_token {
                self.report_modifier_out_of_order_token(parser, next, t);
            } else if let Some(t) = self.covariant_token {
                self.report_modifier_out_of_order_token(parser, next, t);
            } else if let Some(t) = self.final_token {
                self.report_modifier_out_of_order_token(parser, next, t);
            } else if let Some(t) = self.late_token {
                self.report_modifier_out_of_order_token(parser, next, t);
            } else if let Some(t) = self.static_token {
                self.report_modifier_out_of_order_token(parser, next, t);
            } else if let Some(t) = self.external_token {
                self.report_conflicting_modifiers(parser, next, t);
            }
            return next;
        }

        // Recovery
        parser.report_recoverable_error_with_token(next, diag::duplicated_modifier);
        next
    }

    /// Dart (line 441): `_parseConst`.
    fn parse_const<L: Listener>(&mut self, parser: &mut Parser<L>, token: TokenId) -> TokenId {
        let next = parser.next(token);
        debug_assert!(parser.is_a(next, Keyword::CONST));
        if self.var_final_or_const().is_none() && self.covariant_token.is_none() {
            self.const_token = Some(next);

            if self.after_factory {
                self.report_modifier_out_of_order(parser, next, "factory");
            } else if let Some(late_token) = self.late_token {
                self.report_conflicting_modifiers(parser, next, late_token);
            }
            return next;
        }

        // Recovery
        if self.const_token.is_some() {
            parser.report_recoverable_error_with_token(next, diag::duplicated_modifier);
        } else if let Some(covariant_token) = self.covariant_token {
            self.report_conflicting_modifiers(parser, next, covariant_token);
        } else if self.final_token.is_some() {
            parser.report_recoverable_error(next, diag::const_and_final());
        } else if let Some(var_token) = self.var_token {
            self.report_conflicting_modifiers(parser, next, var_token);
        } else {
            panic!(
                "Internal Error: Unexpected varFinalOrConst: {:?}",
                self.var_final_or_const()
            );
        }
        next
    }

    /// Dart (line 470): `_parseCovariant`.
    fn parse_covariant<L: Listener>(&mut self, parser: &mut Parser<L>, token: TokenId) -> TokenId {
        let next = parser.next(token);
        debug_assert!(parser.is_a(next, Keyword::COVARIANT));
        if self.const_token.is_none()
            && self.covariant_token.is_none()
            && self.static_token.is_none()
            && !self.after_factory
        {
            self.covariant_token = Some(next);

            if let Some(t) = self.var_token {
                self.report_modifier_out_of_order_token(parser, next, t);
            } else if let Some(t) = self.final_token {
                self.report_modifier_out_of_order_token(parser, next, t);
            } else if let Some(t) = self.late_token {
                self.report_modifier_out_of_order_token(parser, next, t);
            }
            return next;
        }

        // Recovery
        if self.covariant_token.is_some() {
            parser.report_recoverable_error_with_token(next, diag::duplicated_modifier);
        } else if self.after_factory {
            self.report_extraneous_modifier(parser, Some(next));
        } else if let Some(const_token) = self.const_token {
            self.report_conflicting_modifiers(parser, next, const_token);
        } else if self.static_token.is_some() {
            parser.report_recoverable_error(next, diag::covariant_and_static());
        } else {
            panic!(
                "Internal Error: Unhandled recovery: {}",
                parser.lexeme(next)
            );
        }
        next
    }

    /// Dart (line 504): `_parseExternal`.
    fn parse_external<L: Listener>(&mut self, parser: &mut Parser<L>, token: TokenId) -> TokenId {
        let next = parser.next(token);
        debug_assert!(parser.is_a(next, Keyword::EXTERNAL));
        if self.external_token.is_none() {
            self.external_token = Some(next);

            if self.after_factory {
                self.report_modifier_out_of_order(parser, next, "factory");
            } else if let Some(t) = self.const_token {
                self.report_modifier_out_of_order_token(parser, next, t);
            } else if let Some(t) = self.static_token {
                self.report_modifier_out_of_order_token(parser, next, t);
            } else if let Some(t) = self.late_token {
                self.report_modifier_out_of_order_token(parser, next, t);
            } else if let Some(t) = self.var_final_or_const() {
                self.report_modifier_out_of_order_token(parser, next, t);
            } else if let Some(t) = self.covariant_token {
                self.report_modifier_out_of_order_token(parser, next, t);
            } else if let Some(t) = self.augment_token {
                self.report_conflicting_modifiers(parser, next, t);
            }
            return next;
        }

        // Recovery
        parser.report_recoverable_error_with_token(next, diag::duplicated_modifier);
        next
    }

    /// Dart (line 533): `_parseFinal`.
    fn parse_final<L: Listener>(&mut self, parser: &mut Parser<L>, token: TokenId) -> TokenId {
        let next = parser.next(token);
        debug_assert!(parser.is_a(next, Keyword::FINAL));
        if self.var_final_or_const().is_none() && !self.after_factory {
            self.final_token = Some(next);
            return next;
        }

        // Recovery
        if self.final_token.is_some() {
            parser.report_recoverable_error_with_token(next, diag::duplicated_modifier);
        } else if self.after_factory {
            self.report_extraneous_modifier(parser, Some(next));
        } else if self.const_token.is_some() {
            parser.report_recoverable_error(next, diag::const_and_final());
        } else if self.var_token.is_some() {
            parser.report_recoverable_error(next, diag::final_and_var());
        } else if let Some(late_token) = self.late_token {
            self.report_modifier_out_of_order_token(parser, next, late_token);
        } else {
            panic!(
                "Internal Error: Unexpected varFinalOrConst: {:?}",
                self.var_final_or_const()
            );
        }
        next
    }

    /// Dart (line 558): `_parseLate`.
    fn parse_late<L: Listener>(&mut self, parser: &mut Parser<L>, token: TokenId) -> TokenId {
        let next = parser.next(token);
        debug_assert!(parser.is_a(next, Keyword::LATE));
        if self.late_token.is_none() {
            self.late_token = Some(next);

            if let Some(t) = self.const_token {
                self.report_conflicting_modifiers(parser, next, t);
            } else if let Some(t) = self.var_token {
                self.report_modifier_out_of_order_token(parser, next, t);
            } else if let Some(t) = self.final_token {
                self.report_modifier_out_of_order_token(parser, next, t);
            }
            return next;
        }

        // Recovery
        parser.report_recoverable_error_with_token(next, diag::duplicated_modifier);
        next
    }

    /// Dart (line 579): `_parseRequired`.
    fn parse_required<L: Listener>(&mut self, parser: &mut Parser<L>, token: TokenId) -> TokenId {
        let next = parser.next(token);
        debug_assert!(parser.is_a(next, Keyword::REQUIRED));
        if self.required_token.is_none() {
            self.required_token = Some(next);

            if let Some(t) = self.const_token {
                self.report_modifier_out_of_order_token(parser, next, t);
            } else if let Some(t) = self.covariant_token {
                self.report_modifier_out_of_order_token(parser, next, t);
            } else if let Some(t) = self.final_token {
                self.report_modifier_out_of_order_token(parser, next, t);
            } else if let Some(t) = self.var_token {
                self.report_modifier_out_of_order_token(parser, next, t);
            }
            return next;
        }

        // Recovery
        parser.report_recoverable_error_with_token(next, diag::duplicated_modifier);
        next
    }

    /// Dart (line 602): `_parseStatic`.
    fn parse_static<L: Listener>(&mut self, parser: &mut Parser<L>, token: TokenId) -> TokenId {
        let next = parser.next(token);
        debug_assert!(parser.is_a(next, Keyword::STATIC));
        if self.covariant_token.is_none() && self.static_token.is_none() && !self.after_factory {
            self.static_token = Some(next);

            if let (Some(t), true) = (self.abstract_token, parser.is_augmentations_feature_enabled)
            {
                self.report_modifier_out_of_order_token(parser, next, t);
            } else if let Some(t) = self.const_token {
                self.report_modifier_out_of_order_token(parser, next, t);
            } else if let Some(t) = self.final_token {
                self.report_modifier_out_of_order_token(parser, next, t);
            } else if let Some(t) = self.late_token {
                self.report_modifier_out_of_order_token(parser, next, t);
            } else if let Some(t) = self.var_token {
                self.report_modifier_out_of_order_token(parser, next, t);
            }
            return next;
        }

        // Recovery
        if self.covariant_token.is_some() {
            parser.report_recoverable_error(next, diag::covariant_and_static());
        } else if self.static_token.is_some() {
            parser.report_recoverable_error_with_token(next, diag::duplicated_modifier);
        } else if self.after_factory {
            self.report_extraneous_modifier(parser, Some(next));
        } else {
            panic!(
                "Internal Error: Unhandled recovery: {}",
                parser.lexeme(next)
            );
        }
        next
    }

    /// Dart (line 635): `_parseVar`.
    fn parse_var<L: Listener>(&mut self, parser: &mut Parser<L>, token: TokenId) -> TokenId {
        let next = parser.next(token);
        debug_assert!(parser.is_a(next, Keyword::VAR));
        if self.var_final_or_const().is_none() && !self.after_factory {
            self.var_token = Some(next);
            return next;
        }

        // Recovery
        if self.var_token.is_some() {
            parser.report_recoverable_error_with_token(next, diag::duplicated_modifier);
        } else if self.after_factory {
            self.report_extraneous_modifier(parser, Some(next));
        } else if let Some(const_token) = self.const_token {
            self.report_conflicting_modifiers(parser, next, const_token);
        } else if self.final_token.is_some() {
            parser.report_recoverable_error(next, diag::final_and_var());
        } else {
            panic!(
                "Internal Error: Unexpected varFinalOrConst: {:?}",
                self.var_final_or_const()
            );
        }
        next
    }

    /// Dart (line 658): `reportConflictingModifiers`.
    pub fn report_conflicting_modifiers<L: Listener>(
        &self,
        parser: &mut Parser<L>,
        modifier: TokenId,
        earlier_modifier: TokenId,
    ) {
        let message =
            diag::conflicting_modifiers(parser.lexeme(modifier), parser.lexeme(earlier_modifier));
        parser.report_recoverable_error(modifier, message);
    }

    /// Dart (line 668): `reportExtraneousModifier`.
    pub fn report_extraneous_modifier<L: Listener>(
        &self,
        parser: &mut Parser<L>,
        modifier: Option<TokenId>,
    ) {
        if let Some(modifier) = modifier {
            parser.report_recoverable_error_with_token(modifier, diag::extraneous_modifier);
        }
    }

    /// Dart (line 676): `reportTopLevelModifierError`.
    ///
    /// Report an error for the given modifier preceding a top level keyword
    /// such as `import` or `class`.
    pub fn report_top_level_modifier_error<L: Listener>(
        &self,
        parser: &mut Parser<L>,
        modifier: Option<TokenId>,
        after_modifiers: TokenId,
    ) {
        if let Some(modifier) = modifier {
            if parser.is_a(modifier, Keyword::CONST) && parser.is_a(after_modifiers, Keyword::CLASS)
            {
                parser.report_recoverable_error(modifier, diag::const_class());
            } else if parser.is_a(modifier, Keyword::EXTERNAL) {
                if parser.is_a(after_modifiers, Keyword::CLASS) {
                    parser.report_recoverable_error(modifier, diag::external_class());
                } else if parser.is_a(after_modifiers, Keyword::ENUM) {
                    parser.report_recoverable_error(modifier, diag::external_enum());
                } else if parser.is_a(after_modifiers, Keyword::TYPEDEF) {
                    parser.report_recoverable_error(modifier, diag::external_typedef());
                } else {
                    parser.report_recoverable_error_with_token(modifier, diag::extraneous_modifier);
                }
            } else {
                parser.report_recoverable_error_with_token(modifier, diag::extraneous_modifier);
            }
        }
    }

    /// Dart (line 702): `reportExtraneousModifierInExtension`.
    pub fn report_extraneous_modifier_in_extension<L: Listener>(
        &self,
        parser: &mut Parser<L>,
        modifier: Option<TokenId>,
    ) {
        if let Some(modifier) = modifier {
            parser.report_recoverable_error_with_token(
                modifier,
                diag::extraneous_modifier_in_extension,
            );
        }
    }

    /// Dart (line 711): `reportExtraneousModifierInExtensionType`.
    pub fn report_extraneous_modifier_in_extension_type<L: Listener>(
        &self,
        parser: &mut Parser<L>,
        modifier: Option<TokenId>,
    ) {
        if let Some(modifier) = modifier {
            parser.report_recoverable_error_with_token(
                modifier,
                diag::extraneous_modifier_in_extension_type,
            );
        }
    }

    /// Dart (line 720): `reportExtraneousModifierInPrimaryConstructor`.
    pub fn report_extraneous_modifier_in_primary_constructor<L: Listener>(
        &self,
        parser: &mut Parser<L>,
        modifier: Option<TokenId>,
    ) {
        if let Some(modifier) = modifier {
            parser.report_recoverable_error_with_token(
                modifier,
                diag::extraneous_modifier_in_primary_constructor,
            );
        }
    }

    /// Dart (line 729): `reportModifierOutOfOrder`.
    pub fn report_modifier_out_of_order<L: Listener>(
        &self,
        parser: &mut Parser<L>,
        modifier: TokenId,
        before_modifier: &str,
    ) {
        let message = diag::modifier_out_of_order(parser.lexeme(modifier), before_modifier);
        parser.report_recoverable_error(modifier, message);
    }

    /// Dart `reportModifierOutOfOrder(modifier, beforeModifier.lexeme)`: the
    /// same as [`Self::report_modifier_out_of_order`] with the lexeme of a
    /// token (avoids borrowing `parser` twice).
    fn report_modifier_out_of_order_token<L: Listener>(
        &self,
        parser: &mut Parser<L>,
        modifier: TokenId,
        before_modifier: TokenId,
    ) {
        let message =
            diag::modifier_out_of_order(parser.lexeme(modifier), parser.lexeme(before_modifier));
        parser.report_recoverable_error(modifier, message);
    }
}
