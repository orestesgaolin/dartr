// Dart source: pkg/analyzer/lib/src/fasta/ast_builder.dart
//
// GENERATED FILE. DO NOT EDIT. Run `python3 tools/codegen/gen_builder_listener.py`.

//! The `Listener` implementation of [`AstBuilder`]: every event that
//! the builder implements moves the token arena of the parser into
//! `self.ast.tokens` for the duration of the event and calls the inherent
//! method of the same name. The other events are the no-op defaults of
//! the trait.

#![allow(unused_imports)]

use dartr_diagnostics::cfe::{CfeCode, CfeMessage};
use dartr_parser::assert::Assert;
use dartr_parser::block_kind::BlockKind;
use dartr_parser::constructor_reference_context::ConstructorReferenceContext;
use dartr_parser::declaration_kind::{DeclarationHeaderKind, DeclarationKind};
use dartr_parser::experimental_features::ExperimentalFlag;
use dartr_parser::formal_parameter_kind::FormalParameterKind;
use dartr_parser::identifier_context::IdentifierContext;
use dartr_parser::listener::Listener;
use dartr_parser::member_kind::MemberKind;
use dartr_syntax::{TokenId, Tokens};

use super::AstBuilder;

impl Listener for AstBuilder {
    #[inline]
    fn handle_directives_only(&mut self, tokens: &mut Tokens) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_directives_only(self);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_extraneous_expression(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        message: CfeMessage,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_extraneous_expression(self, token, message);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_formal_parameters(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        kind: MemberKind,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_formal_parameters(self, token, kind);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_function_body(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_function_body(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_initializer(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_initializer(self, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_native_function_body_ignored(
        &mut self,
        tokens: &mut Tokens,
        native_token: TokenId,
        semicolon: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_native_function_body_ignored(self, native_token, semicolon);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_native_function_body_skipped(
        &mut self,
        tokens: &mut Tokens,
        native_token: TokenId,
        semicolon: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_native_function_body_skipped(self, native_token, semicolon);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_catch_clause(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_catch_clause(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_name(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_name(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_type_arguments(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_type_arguments(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn report_variance_modifier_not_enabled(
        &mut self,
        tokens: &mut Tokens,
        variance: Option<TokenId>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::report_variance_modifier_not_enabled(self, variance);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_implicit_formal_parameters(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_implicit_formal_parameters(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_arguments(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_arguments(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_type(&mut self, tokens: &mut Tokens, last_consumed: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_type(self, last_consumed);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_type_variables(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_type_variables(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_recoverable_error(
        &mut self,
        tokens: &mut Tokens,
        message: CfeMessage,
        start_token: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_recoverable_error(self, message, start_token, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_error_token(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_error_token(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_unescape_error(
        &mut self,
        tokens: &mut Tokens,
        message: CfeMessage,
        location: TokenId,
        string_offset: i32,
        length: i32,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_unescape_error(self, message, location, string_offset, length);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }
}
