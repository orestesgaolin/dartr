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
    fn handle_object_pattern_fields(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_object_pattern_fields(self, count, begin_token, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_invalid_top_level_block(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_invalid_top_level_block(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_implements(
        &mut self,
        tokens: &mut Tokens,
        implements_keyword: Option<TokenId>,
        interfaces_count: i32,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_implements(self, implements_keyword, interfaces_count);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_recover_declaration_header(
        &mut self,
        tokens: &mut Tokens,
        kind: DeclarationHeaderKind,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_recover_declaration_header(self, kind);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_class_body(&mut self, tokens: &mut Tokens, semicolon_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_class_body(self, semicolon_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_extension_type_body(&mut self, tokens: &mut Tokens, semicolon_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_extension_type_body(self, semicolon_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_mixin_on(
        &mut self,
        tokens: &mut Tokens,
        on_keyword: Option<TokenId>,
        type_count: i32,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_mixin_on(self, on_keyword, type_count);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_mixin_header(&mut self, tokens: &mut Tokens, mixin_keyword: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_mixin_header(self, mixin_keyword);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_recover_mixin_header(&mut self, tokens: &mut Tokens) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_recover_mixin_header(self);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_mixin_body(&mut self, tokens: &mut Tokens, semicolon_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_mixin_body(self, semicolon_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_extension_body(&mut self, tokens: &mut Tokens, semicolon_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_extension_body(self, semicolon_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_primary_constructor(
        &mut self,
        tokens: &mut Tokens,
        kind: DeclarationKind,
        token: TokenId,
        const_keyword: Option<TokenId>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_primary_constructor(self, kind, token, const_keyword);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_directives_only(&mut self, tokens: &mut Tokens) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_directives_only(self);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_enum_body(&mut self, tokens: &mut Tokens, semicolon_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_enum_body(self, semicolon_token);
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
    fn handle_mixin_with_clause(&mut self, tokens: &mut Tokens, with_keyword: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_mixin_with_clause(self, with_keyword);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_named_mixin_application_with_clause(
        &mut self,
        tokens: &mut Tokens,
        with_keyword: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_named_mixin_application_with_clause(self, with_keyword);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_identifier_list(&mut self, tokens: &mut Tokens, count: i32) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_identifier_list(self, count);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_import_prefix(
        &mut self,
        tokens: &mut Tokens,
        deferred_keyword: Option<TokenId>,
        as_keyword: Option<TokenId>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_import_prefix(self, deferred_keyword, as_keyword);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_recover_import(&mut self, tokens: &mut Tokens, semicolon: Option<TokenId>) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_recover_import(self, semicolon);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_field_initializer(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_field_initializer(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_variable_initializer(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_variable_initializer(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_initializer(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_initializer(self, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_initializers(&mut self, tokens: &mut Tokens) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_initializers(self);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_invalid_expression(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_invalid_expression(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_invalid_function_body(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_invalid_function_body(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_label(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_label(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_literal_map_entry(
        &mut self,
        tokens: &mut Tokens,
        colon: TokenId,
        end_token: TokenId,
        null_aware_key_token: Option<TokenId>,
        null_aware_value_token: Option<TokenId>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_literal_map_entry(
            self,
            colon,
            end_token,
            null_aware_key_token,
            null_aware_value_token,
        );
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_map_pattern_entry(
        &mut self,
        tokens: &mut Tokens,
        colon: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_map_pattern_entry(self, colon, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_interpolation_expression(
        &mut self,
        tokens: &mut Tokens,
        left_bracket: TokenId,
        right_bracket: Option<TokenId>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_interpolation_expression(self, left_bracket, right_bracket);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_invalid_member(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_invalid_member(self, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_native_function_body(
        &mut self,
        tokens: &mut Tokens,
        native_token: TokenId,
        semicolon: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_native_function_body(self, native_token, semicolon);
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
    fn handle_send(&mut self, tokens: &mut Tokens, begin_token: TokenId, end_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_send(self, begin_token, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_throw_expression(
        &mut self,
        tokens: &mut Tokens,
        throw_token: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_throw_expression(self, throw_token, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_invalid_top_level_declaration(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_invalid_top_level_declaration(self, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_catch_clause(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_catch_clause(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_type(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        question_mark: Option<TokenId>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_type(self, begin_token, question_mark);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_non_null_assert_expression(&mut self, tokens: &mut Tokens, bang: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_non_null_assert_expression(self, bang);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_null_assert_pattern(&mut self, tokens: &mut Tokens, bang: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_null_assert_pattern(self, bang);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_null_check_pattern(&mut self, tokens: &mut Tokens, question: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_null_check_pattern(self, question);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_wildcard_pattern(
        &mut self,
        tokens: &mut Tokens,
        keyword: Option<TokenId>,
        wildcard: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_wildcard_pattern(self, keyword, wildcard);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_name(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_name(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_invalid_type_arguments(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_invalid_type_arguments(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_type_arguments(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_type_arguments(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_type_variables_defined(&mut self, tokens: &mut Tokens, token: TokenId, count: i32) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_type_variables_defined(self, token, count);
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
    fn handle_spread_expression(&mut self, tokens: &mut Tokens, spread_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_spread_expression(self, spread_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_null_aware_element(&mut self, tokens: &mut Tokens, null_aware_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_null_aware_element(self, null_aware_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_rest_pattern(&mut self, tokens: &mut Tokens, dots: TokenId, has_sub_pattern: bool) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_rest_pattern(self, dots, has_sub_pattern);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_identifier(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        context: IdentifierContext,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_identifier(self, token, context);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_indexed_expression(
        &mut self,
        tokens: &mut Tokens,
        question: Option<TokenId>,
        open_square_bracket: TokenId,
        close_square_bracket: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_indexed_expression(
            self,
            question,
            open_square_bracket,
            close_square_bracket,
        );
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_is_operator(
        &mut self,
        tokens: &mut Tokens,
        is_operator: TokenId,
        not: Option<TokenId>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_is_operator(self, is_operator, not);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_literal_bool(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_literal_bool(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_literal_double(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_literal_double(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_literal_double_with_separators(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_literal_double_with_separators(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_literal_int(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_literal_int(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_literal_int_with_separators(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_literal_int_with_separators(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_literal_list(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        left_bracket: TokenId,
        const_keyword: Option<TokenId>,
        right_bracket: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_literal_list(self, count, left_bracket, const_keyword, right_bracket);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_list_pattern(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        left_bracket: TokenId,
        right_bracket: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_list_pattern(self, count, left_bracket, right_bracket);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_literal_set_or_map(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        left_brace: TokenId,
        const_keyword: Option<TokenId>,
        right_brace: TokenId,
        has_set_entry: bool,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_literal_set_or_map(
            self,
            count,
            left_brace,
            const_keyword,
            right_brace,
            has_set_entry,
        );
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_map_pattern(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        left_brace: TokenId,
        right_brace: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_map_pattern(self, count, left_brace, right_brace);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_literal_null(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_literal_null(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_native_clause(&mut self, tokens: &mut Tokens, native_token: TokenId, has_name: bool) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_native_clause(self, native_token, has_name);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_named_argument(&mut self, tokens: &mut Tokens, colon: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_named_argument(self, colon);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_pattern_field(&mut self, tokens: &mut Tokens, colon: Option<TokenId>) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_pattern_field(self, colon);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_named_record_field(&mut self, tokens: &mut Tokens, colon: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_named_record_field(self, colon);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_arguments(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_arguments(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_constructor_reference_continuation_after_type_arguments(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_constructor_reference_continuation_after_type_arguments(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_identifier(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        identifier_context: IdentifierContext,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_identifier(self, token, identifier_context);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_type_name_in_constructor_reference(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_type_name_in_constructor_reference(self, token);
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
    fn handle_operator(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_operator(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_switch_case_no_when_clause(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_switch_case_no_when_clause(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_switch_expression_case_pattern(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_switch_expression_case_pattern(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_symbol_void(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_symbol_void(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_operator_name(
        &mut self,
        tokens: &mut Tokens,
        operator_keyword: TokenId,
        token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_operator_name(self, operator_keyword, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_invalid_operator_name(
        &mut self,
        tokens: &mut Tokens,
        operator_keyword: TokenId,
        token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_invalid_operator_name(self, operator_keyword, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_parenthesized_condition(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        case_: Option<TokenId>,
        when: Option<TokenId>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_parenthesized_condition(self, token, case_, when);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_record_pattern(&mut self, tokens: &mut Tokens, token: TokenId, count: i32) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_record_pattern(self, token, count);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_parenthesized_pattern(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_parenthesized_pattern(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_object_pattern(
        &mut self,
        tokens: &mut Tokens,
        first_identifier: TokenId,
        dot: Option<TokenId>,
        second_identifier: Option<TokenId>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_object_pattern(self, first_identifier, dot, second_identifier);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_qualified(&mut self, tokens: &mut Tokens, period: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_qualified(self, period);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_string_part(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_string_part(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_super_expression(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        context: IdentifierContext,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_super_expression(self, token, context);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_this_expression(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        context: IdentifierContext,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_this_expression(self, token, context);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_unary_postfix_assignment_expression(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_unary_postfix_assignment_expression(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_unary_prefix_expression(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_unary_prefix_expression(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_relational_pattern(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_relational_pattern(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_unary_prefix_assignment_expression(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_unary_prefix_assignment_expression(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_valued_formal_parameter(
        &mut self,
        tokens: &mut Tokens,
        equals: TokenId,
        token: TokenId,
        kind: FormalParameterKind,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_valued_formal_parameter(self, equals, token, kind);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_void_keyword(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_void_keyword(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_void_keyword_with_type_arguments(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_void_keyword_with_type_arguments(self, token);
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

    #[inline]
    fn handle_script(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_script(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_type_argument_application(
        &mut self,
        tokens: &mut Tokens,
        open_angle_bracket: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_type_argument_application(self, open_angle_bracket);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_new_as_identifier(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_new_as_identifier(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_pattern_variable_declaration_statement(
        &mut self,
        tokens: &mut Tokens,
        keyword: TokenId,
        equals: TokenId,
        semicolon: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_pattern_variable_declaration_statement(self, keyword, equals, semicolon);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_pattern_assignment(&mut self, tokens: &mut Tokens, equals: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_pattern_assignment(self, equals);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }
}
