// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/modifier_context.dart

//! STUB: the API that the parser uses. The port replaces the `todo!()`s and
//! keeps these names and signatures. Dart `ModifierContext(parser)` keeps a
//! reference to the parser; here the methods take `parser` as their first
//! argument: Dart `context.parseClassModifiers(token, keyword)` is
//! `context.parse_class_modifiers(self, token, keyword)` in the parser.

#![allow(unused_variables)]

use dartr_syntax::{TokenId, Tokens};

use crate::formal_parameter_kind::FormalParameterKind;
use crate::listener::Listener;
use crate::member_kind::MemberKind;
use crate::parser_impl::Parser;

/// Dart `isModifier`.
pub fn is_modifier(tokens: &Tokens, token: TokenId) -> bool {
    todo!()
}

/// Dart `ModifierContext`: the modifiers seen so far.
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
    /// Dart `new ModifierContext(parser)`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Dart setter `staticOrCovariant`.
    pub fn set_static_or_covariant(&mut self, tokens: &Tokens, static_or_covariant: Option<TokenId>) {
        todo!()
    }

    /// Dart getter `varFinalOrConst`.
    pub fn var_final_or_const(&self) -> Option<TokenId> {
        self.var_token.or(self.final_token).or(self.const_token)
    }

    /// Dart setter `varFinalOrConst`.
    pub fn set_var_final_or_const(&mut self, tokens: &Tokens, var_final_or_const: Option<TokenId>) {
        todo!()
    }

    pub fn parse_class_modifiers<L: Listener>(&mut self, parser: &mut Parser<L>, token: TokenId, keyword: TokenId) -> TokenId {
        todo!()
    }

    pub fn parse_enum_modifiers<L: Listener>(&mut self, parser: &mut Parser<L>, token: TokenId, keyword: TokenId) -> TokenId {
        todo!()
    }

    pub fn parse_extension_modifiers<L: Listener>(&mut self, parser: &mut Parser<L>, token: TokenId, keyword: TokenId) -> TokenId {
        todo!()
    }

    pub fn parse_mixin_modifiers<L: Listener>(&mut self, parser: &mut Parser<L>, token: TokenId, keyword: TokenId) -> TokenId {
        todo!()
    }

    pub fn parse_top_level_keyword_modifiers<L: Listener>(&mut self, parser: &mut Parser<L>, token: TokenId, keyword: TokenId) -> TokenId {
        todo!()
    }

    pub fn parse_class_member_modifiers<L: Listener>(&mut self, parser: &mut Parser<L>, token: TokenId) -> TokenId {
        todo!()
    }

    pub fn parse_formal_parameter_modifiers<L: Listener>(
        &mut self,
        parser: &mut Parser<L>,
        token: TokenId,
        parameter_kind: FormalParameterKind,
        member_kind: MemberKind,
    ) -> TokenId {
        todo!()
    }

    pub fn parse_library_directive_modifiers<L: Listener>(&mut self, parser: &mut Parser<L>, token: TokenId, keyword: TokenId) -> TokenId {
        todo!()
    }

    pub fn parse_modifiers_after_factory<L: Listener>(&mut self, parser: &mut Parser<L>, token: TokenId) -> TokenId {
        todo!()
    }

    pub fn parse_top_level_member_modifiers<L: Listener>(&mut self, parser: &mut Parser<L>, token: TokenId) -> TokenId {
        todo!()
    }

    pub fn parse_typedef_modifiers<L: Listener>(&mut self, parser: &mut Parser<L>, token: TokenId, keyword: TokenId) -> TokenId {
        todo!()
    }

    pub fn parse_variable_declaration_modifiers<L: Listener>(&mut self, parser: &mut Parser<L>, token: TokenId) -> TokenId {
        todo!()
    }
}
