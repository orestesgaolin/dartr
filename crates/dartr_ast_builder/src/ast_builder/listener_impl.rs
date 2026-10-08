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
    fn handle_async_modifier(
        &mut self,
        tokens: &mut Tokens,
        async_token: Option<TokenId>,
        star_token: Option<TokenId>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_async_modifier(self, async_token, star_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_class_extends(
        &mut self,
        tokens: &mut Tokens,
        extends_keyword: Option<TokenId>,
        type_count: i32,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_class_extends(self, extends_keyword, type_count);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_class_header(
        &mut self,
        tokens: &mut Tokens,
        begin: TokenId,
        class_keyword: TokenId,
        native_token: Option<TokenId>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_class_header(self, begin, class_keyword, native_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_directives_only(&mut self, tokens: &mut Tokens) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_directives_only(self);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_while_statement_body(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_while_statement_body(self, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_enum_elements(
        &mut self,
        tokens: &mut Tokens,
        elements_end_token: TokenId,
        elements_count: i32,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_enum_elements(self, elements_end_token, elements_count);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_enum_header(
        &mut self,
        tokens: &mut Tokens,
        augment_token: Option<TokenId>,
        enum_keyword: TokenId,
        left_brace: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_enum_header(self, augment_token, enum_keyword, left_brace);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_enum_element(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        augment_token: Option<TokenId>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_enum_element(self, begin_token, augment_token);
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
    fn handle_expression_statement(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_expression_statement(self, begin_token, end_token);
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
    fn handle_for_initializer_empty_statement(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_for_initializer_empty_statement(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_for_initializer_expression_statement(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        for_in: bool,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_for_initializer_expression_statement(self, token, for_in);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_for_initializer_local_variable_declaration(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        for_in: bool,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_for_initializer_local_variable_declaration(self, token, for_in);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_for_initializer_pattern_variable_assignment(
        &mut self,
        tokens: &mut Tokens,
        keyword: TokenId,
        equals: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_for_initializer_pattern_variable_assignment(self, keyword, equals);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_for_loop_parts(
        &mut self,
        tokens: &mut Tokens,
        for_keyword: TokenId,
        left_paren: TokenId,
        left_separator: TokenId,
        right_separator: TokenId,
        update_expression_count: i32,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_for_loop_parts(
            self,
            for_keyword,
            left_paren,
            left_separator,
            right_separator,
            update_expression_count,
        );
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_for_in_loop_parts(
        &mut self,
        tokens: &mut Tokens,
        await_token: Option<TokenId>,
        for_token: TokenId,
        left_parenthesis: TokenId,
        pattern_keyword: Option<TokenId>,
        in_keyword: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_for_in_loop_parts(
            self,
            await_token,
            for_token,
            left_parenthesis,
            pattern_keyword,
            in_keyword,
        );
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_function_body(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_function_body(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_typedef(
        &mut self,
        tokens: &mut Tokens,
        augment_token: Option<TokenId>,
        typedef_keyword: TokenId,
        equals: Option<TokenId>,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_typedef(self, augment_token, typedef_keyword, equals, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_class_with_clause(&mut self, tokens: &mut Tokens, with_keyword: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_class_with_clause(self, with_keyword);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_class_no_with_clause(&mut self, tokens: &mut Tokens) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_class_no_with_clause(self);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_enum_with_clause(&mut self, tokens: &mut Tokens, with_keyword: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_enum_with_clause(self, with_keyword);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_enum_no_with_clause(&mut self, tokens: &mut Tokens) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_enum_no_with_clause(self);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_type_list(&mut self, tokens: &mut Tokens, count: i32) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_type_list(self, count);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_then_statement(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_then_statement(self, begin_token, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_dotted_name(&mut self, tokens: &mut Tokens, count: i32, first_identifier: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_dotted_name(self, count, first_identifier);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_variable_initializer(&mut self, tokens: &mut Tokens, assignment_operator: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_variable_initializer(self, assignment_operator);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_initializer(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_initializer(self, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_adjacent_string_literals(
        &mut self,
        tokens: &mut Tokens,
        start_token: TokenId,
        literal_count: i32,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_adjacent_string_literals(self, start_token, literal_count);
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
    fn handle_empty_function_body(&mut self, tokens: &mut Tokens, semicolon: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_empty_function_body(self, semicolon);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_expression_function_body(
        &mut self,
        tokens: &mut Tokens,
        arrow_token: TokenId,
        end_token: Option<TokenId>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_expression_function_body(self, arrow_token, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_switch_statement(
        &mut self,
        tokens: &mut Tokens,
        switch_keyword: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_switch_statement(self, switch_keyword, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_switch_expression(
        &mut self,
        tokens: &mut Tokens,
        switch_keyword: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_switch_expression(self, switch_keyword, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_switch_expression_block(
        &mut self,
        tokens: &mut Tokens,
        case_count: i32,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_switch_expression_block(self, case_count, begin_token, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_top_level_declaration(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_top_level_declaration(self, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_top_level_fields(
        &mut self,
        tokens: &mut Tokens,
        augment_token: Option<TokenId>,
        abstract_token: Option<TokenId>,
        external_token: Option<TokenId>,
        static_token: Option<TokenId>,
        covariant_token: Option<TokenId>,
        late_token: Option<TokenId>,
        var_final_or_const: Option<TokenId>,
        count: i32,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_top_level_fields(
            self,
            augment_token,
            abstract_token,
            external_token,
            static_token,
            covariant_token,
            late_token,
            var_final_or_const,
            count,
            begin_token,
            end_token,
        );
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_top_level_method(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        get_or_set: Option<TokenId>,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_top_level_method(self, begin_token, get_or_set, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_catch_clause(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_catch_clause(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_catch_block(
        &mut self,
        tokens: &mut Tokens,
        on_keyword: Option<TokenId>,
        catch_keyword: Option<TokenId>,
        comma: Option<TokenId>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_catch_block(self, on_keyword, catch_keyword, comma);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_finally_block(&mut self, tokens: &mut Tokens, finally_keyword: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_finally_block(self, finally_keyword);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_try_statement(
        &mut self,
        tokens: &mut Tokens,
        catch_count: i32,
        try_keyword: TokenId,
        finally_keyword: Option<TokenId>,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_try_statement(self, catch_count, try_keyword, finally_keyword, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_assigned_variable_pattern(&mut self, tokens: &mut Tokens, variable: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_assigned_variable_pattern(self, variable);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_declared_variable_pattern(
        &mut self,
        tokens: &mut Tokens,
        keyword: Option<TokenId>,
        variable: TokenId,
        in_assignment_pattern: bool,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_declared_variable_pattern(
            self,
            keyword,
            variable,
            in_assignment_pattern,
        );
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_name(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_name(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_type_arguments(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_type_arguments(self, count, begin_token, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_type_arguments(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_type_arguments(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_type_variable(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        index: i32,
        extends_or_super: Option<TokenId>,
        variance: Option<TokenId>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_type_variable(self, token, index, extends_or_super, variance);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_type_variables(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_type_variables(self, begin_token, end_token);
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
    fn end_variables_declaration(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        end_token: Option<TokenId>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_variables_declaration(self, count, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_while_statement(
        &mut self,
        tokens: &mut Tokens,
        while_keyword: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_while_statement(self, while_keyword, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_as_operator(&mut self, tokens: &mut Tokens, operator: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_as_operator(self, operator);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_cast_pattern(&mut self, tokens: &mut Tokens, operator: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_cast_pattern(self, operator);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_assignment_expression(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_assignment_expression(self, token, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_implicit_formal_parameters(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_implicit_formal_parameters(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_dot_access(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        end_token: TokenId,
        is_null_aware: bool,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_dot_access(self, token, end_token, is_null_aware);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_cascade_access(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        end_token: TokenId,
        is_null_aware: bool,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_cascade_access(self, token, end_token, is_null_aware);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_const_factory(&mut self, tokens: &mut Tokens, const_keyword: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_const_factory(self, const_keyword);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_else_control_flow(&mut self, tokens: &mut Tokens, else_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_else_control_flow(self, else_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_break_statement(
        &mut self,
        tokens: &mut Tokens,
        has_target: bool,
        break_keyword: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_break_statement(self, has_target, break_keyword, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_continue_statement(
        &mut self,
        tokens: &mut Tokens,
        has_target: bool,
        continue_keyword: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_continue_statement(self, has_target, continue_keyword, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_empty_statement(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_empty_statement(self, token);
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
    fn end_switch_expression_case(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        when: Option<TokenId>,
        arrow: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_switch_expression_case(self, begin_token, when, arrow, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_formal_parameter_without_value(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_formal_parameter_without_value(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_yield_statement(
        &mut self,
        tokens: &mut Tokens,
        yield_token: TokenId,
        star_token: Option<TokenId>,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_yield_statement(self, yield_token, star_token, end_token);
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
    fn handle_dot_shorthand_context(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_dot_shorthand_context(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_dot_shorthand_head(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_dot_shorthand_head(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }
}
