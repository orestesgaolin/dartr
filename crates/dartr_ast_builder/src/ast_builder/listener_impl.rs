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
    fn end_invalid_await_expression(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        end_token: TokenId,
        error_code: &'static CfeCode,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_invalid_await_expression(self, begin_token, end_token, error_code);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_mixin_declaration(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_mixin_declaration(self, begin_token, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_primary_constructor(
        &mut self,
        tokens: &mut Tokens,
        kind: DeclarationKind,
        begin_token: TokenId,
        end_token: TokenId,
        const_keyword: Option<TokenId>,
        has_constructor_name: bool,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_primary_constructor(
            self,
            kind,
            begin_token,
            end_token,
            const_keyword,
            has_constructor_name,
        );
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_primary_constructor_body(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        begin_initializers: Option<TokenId>,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_primary_constructor_body(self, begin_token, begin_initializers, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

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
    fn end_formal_parameter(
        &mut self,
        tokens: &mut Tokens,
        var_or_final: Option<TokenId>,
        this_keyword: Option<TokenId>,
        super_keyword: Option<TokenId>,
        period_after_this_or_super: Option<TokenId>,
        name_token: TokenId,
        initializer_start: Option<TokenId>,
        initializer_end: Option<TokenId>,
        kind: FormalParameterKind,
        member_kind: MemberKind,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_formal_parameter(
            self,
            var_or_final,
            this_keyword,
            super_keyword,
            period_after_this_or_super,
            name_token,
            initializer_start,
            initializer_end,
            kind,
            member_kind,
        );
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
    fn end_formal_parameters(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        begin_token: TokenId,
        end_token: TokenId,
        kind: MemberKind,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_formal_parameters(self, count, begin_token, end_token, kind);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_for_statement(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_for_statement(self, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_for_statement_body(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_for_statement_body(self, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_for_in(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_for_in(self, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_for_in_expression(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_for_in_expression(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_for_in_body(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_for_in_body(self, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_named_function_expression(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_named_function_expression(self, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_local_function_declaration(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_local_function_declaration(self, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_function_body(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_function_body(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_function_name(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        token: TokenId,
        is_function_expression: bool,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_function_name(self, begin_token, token, is_function_expression);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_named_mixin_application(
        &mut self,
        tokens: &mut Tokens,
        begin: TokenId,
        class_keyword: TokenId,
        equals: TokenId,
        implements_keyword: Option<TokenId>,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_named_mixin_application(
            self,
            begin,
            class_keyword,
            equals,
            implements_keyword,
            end_token,
        );
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_hide(&mut self, tokens: &mut Tokens, hide_keyword: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_hide(self, hide_keyword);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_if_statement(
        &mut self,
        tokens: &mut Tokens,
        if_token: TokenId,
        else_token: Option<TokenId>,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_if_statement(self, if_token, else_token, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_import(
        &mut self,
        tokens: &mut Tokens,
        import_keyword: TokenId,
        semicolon: Option<TokenId>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_import(self, import_keyword, semicolon);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_implicit_creation_expression(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        open_angle_bracket: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_implicit_creation_expression(self, token, open_angle_bracket);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_initialized_identifier(&mut self, tokens: &mut Tokens, name_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_initialized_identifier(self, name_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_initializer(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_initializer(self, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_initializers(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_initializers(self, count, begin_token, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_labeled_statement(&mut self, tokens: &mut Tokens, label_count: i32) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_labeled_statement(self, label_count);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_library_augmentation(
        &mut self,
        tokens: &mut Tokens,
        augment_keyword: TokenId,
        library_keyword: TokenId,
        semicolon: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_library_augmentation(self, augment_keyword, library_keyword, semicolon);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_library_name(
        &mut self,
        tokens: &mut Tokens,
        library_keyword: TokenId,
        semicolon: TokenId,
        has_name: bool,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_library_name(self, library_keyword, semicolon, has_name);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_literal_string(
        &mut self,
        tokens: &mut Tokens,
        interpolation_count: i32,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_literal_string(self, interpolation_count, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_member(&mut self, tokens: &mut Tokens) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_member(self);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_method(
        &mut self,
        tokens: &mut Tokens,
        kind: DeclarationKind,
        get_or_set: Option<TokenId>,
        begin_token: TokenId,
        begin_param: TokenId,
        begin_initializers: Option<TokenId>,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_method(
            self,
            kind,
            get_or_set,
            begin_token,
            begin_param,
            begin_initializers,
            end_token,
        );
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_metadata_star(&mut self, tokens: &mut Tokens, count: i32) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_metadata_star(self, count);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_metadata(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        period_before_name: Option<TokenId>,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_metadata(self, begin_token, period_before_name, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_optional_formal_parameters(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        begin_token: TokenId,
        end_token: TokenId,
        kind: MemberKind,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_optional_formal_parameters(self, count, begin_token, end_token, kind);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_part(&mut self, tokens: &mut Tokens, part_keyword: TokenId, semicolon: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_part(self, part_keyword, semicolon);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_part_of(
        &mut self,
        tokens: &mut Tokens,
        part_keyword: TokenId,
        of_keyword: TokenId,
        semicolon: TokenId,
        has_name: bool,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_part_of(self, part_keyword, of_keyword, semicolon, has_name);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_redirecting_factory_body(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_redirecting_factory_body(self, begin_token, end_token);
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
    fn end_return_statement(
        &mut self,
        tokens: &mut Tokens,
        has_expression: bool,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_return_statement(self, has_expression, begin_token, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_show(&mut self, tokens: &mut Tokens, show_keyword: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_show(self, show_keyword);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_switch_block(
        &mut self,
        tokens: &mut Tokens,
        case_count: i32,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_switch_block(self, case_count, begin_token, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_literal_symbol(
        &mut self,
        tokens: &mut Tokens,
        hash_token: TokenId,
        identifier_count: i32,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_literal_symbol(self, hash_token, identifier_count);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_rethrow_statement(
        &mut self,
        tokens: &mut Tokens,
        rethrow_token: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_rethrow_statement(self, rethrow_token, end_token);
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
    fn end_record_type(
        &mut self,
        tokens: &mut Tokens,
        left_bracket: TokenId,
        question_mark: Option<TokenId>,
        count: i32,
        has_named_fields: bool,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_record_type(self, left_bracket, question_mark, count, has_named_fields);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_record_type_entry(&mut self, tokens: &mut Tokens) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_record_type_entry(self);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_record_type_named_fields(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        left_bracket: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_record_type_named_fields(self, count, left_bracket);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_function_type(
        &mut self,
        tokens: &mut Tokens,
        function_token: TokenId,
        question_mark: Option<TokenId>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_function_type(self, function_token, question_mark);
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
    fn end_function_expression(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_function_expression(self, begin_token, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_implicit_formal_parameters(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_implicit_formal_parameters(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_for_control_flow(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_for_control_flow(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_for_in_control_flow(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_for_in_control_flow(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_if_control_flow(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_if_control_flow(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_if_else_control_flow(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_if_else_control_flow(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_function_typed_formal_parameter(
        &mut self,
        tokens: &mut Tokens,
        name_token: TokenId,
        question: Option<TokenId>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_function_typed_formal_parameter(self, name_token, question);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_is_operator_type(&mut self, tokens: &mut Tokens, operator: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_is_operator_type(self, operator);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_new_expression(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_new_expression(self, token);
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
    fn end_record_literal(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        count: i32,
        const_keyword: Option<TokenId>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_record_literal(self, token, count, const_keyword);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_pattern(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_pattern(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_pattern_guard(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_pattern_guard(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_parenthesized_expression(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_parenthesized_expression(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_switch_case_when_clause(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_switch_case_when_clause(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_switch_case(
        &mut self,
        tokens: &mut Tokens,
        label_count: i32,
        expression_count: i32,
        default_keyword: Option<TokenId>,
        colon_after_default: Option<TokenId>,
        statement_count: i32,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_switch_case(
            self,
            label_count,
            expression_count,
            default_keyword,
            colon_after_default,
            statement_count,
            begin_token,
            end_token,
        );
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_formal_parameter_default_value_expression(&mut self, tokens: &mut Tokens) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_formal_parameter_default_value_expression(self);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_invalid_yield_statement(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        star_token: Option<TokenId>,
        end_token: TokenId,
        error_code: &'static CfeCode,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_invalid_yield_statement(
            self,
            begin_token,
            star_token,
            end_token,
            error_code,
        );
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
