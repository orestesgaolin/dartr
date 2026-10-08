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
    fn end_arguments(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_arguments(self, count, begin_token, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_await_expression(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_await_expression(self, begin_token, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_block(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        begin_token: TokenId,
        end_token: TokenId,
        block_kind: BlockKind,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_block(self, count, begin_token, end_token, block_kind);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn begin_cascade(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_cascade(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_cascade(&mut self, tokens: &mut Tokens) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_cascade(self);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_case_expression(
        &mut self,
        tokens: &mut Tokens,
        case_keyword: TokenId,
        when: Option<TokenId>,
        colon: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_case_expression(self, case_keyword, when, colon);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_class_or_mixin_or_extension_body(
        &mut self,
        tokens: &mut Tokens,
        kind: DeclarationKind,
        member_count: i32,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_class_or_mixin_or_extension_body(
            self,
            kind,
            member_count,
            begin_token,
            end_token,
        );
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn begin_class_declaration(
        &mut self,
        tokens: &mut Tokens,
        begin: TokenId,
        abstract_token: Option<TokenId>,
        sealed_token: Option<TokenId>,
        base_token: Option<TokenId>,
        interface_token: Option<TokenId>,
        final_token: Option<TokenId>,
        augment_token: Option<TokenId>,
        mixin_token: Option<TokenId>,
        name: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_class_declaration(
            self,
            begin,
            abstract_token,
            sealed_token,
            base_token,
            interface_token,
            final_token,
            augment_token,
            mixin_token,
            name,
        );
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_class_declaration(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_class_declaration(self, begin_token, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn begin_mixin_declaration(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        augment_token: Option<TokenId>,
        base_token: Option<TokenId>,
        mixin_keyword: TokenId,
        name: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_mixin_declaration(
            self,
            begin_token,
            augment_token,
            base_token,
            mixin_keyword,
            name,
        );
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn begin_extension_declaration(
        &mut self,
        tokens: &mut Tokens,
        augment_token: Option<TokenId>,
        extension_keyword: TokenId,
        name: Option<TokenId>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_extension_declaration(self, augment_token, extension_keyword, name);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_extension_declaration(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        extension_keyword: TokenId,
        on_keyword: Option<TokenId>,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_extension_declaration(
            self,
            begin_token,
            extension_keyword,
            on_keyword,
            end_token,
        );
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn begin_extension_type_declaration(
        &mut self,
        tokens: &mut Tokens,
        augment_keyword: Option<TokenId>,
        extension_keyword: TokenId,
        name: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_extension_type_declaration(
            self,
            augment_keyword,
            extension_keyword,
            name,
        );
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_extension_type_declaration(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        augment_token: Option<TokenId>,
        extension_keyword: TokenId,
        type_keyword: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_extension_type_declaration(
            self,
            begin_token,
            augment_token,
            extension_keyword,
            type_keyword,
            end_token,
        );
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn begin_primary_constructor(&mut self, tokens: &mut Tokens, begin_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_primary_constructor(self, begin_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn begin_primary_constructor_body(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_primary_constructor_body(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_combinators(&mut self, tokens: &mut Tokens, count: i32) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_combinators(self, count);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn begin_compilation_unit(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_compilation_unit(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_directives_only(&mut self, tokens: &mut Tokens) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_directives_only(self);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_compilation_unit(&mut self, tokens: &mut Tokens, count: i32, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_compilation_unit(self, count, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_const_literal(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_const_literal(self, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_constructor_reference(
        &mut self,
        tokens: &mut Tokens,
        start: TokenId,
        period_before_name: Option<TokenId>,
        end_token: TokenId,
        constructor_reference_context: ConstructorReferenceContext,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_constructor_reference(
            self,
            start,
            period_before_name,
            end_token,
            constructor_reference_context,
        );
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_do_while_statement(
        &mut self,
        tokens: &mut Tokens,
        do_keyword: TokenId,
        while_keyword: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_do_while_statement(self, do_keyword, while_keyword, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_do_while_statement_body(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_do_while_statement_body(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn begin_enum_declaration_prelude(&mut self, tokens: &mut Tokens, enum_keyword: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_enum_declaration_prelude(self, enum_keyword);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn begin_enum_declaration(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        augment_token: Option<TokenId>,
        enum_keyword: TokenId,
        name: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_enum_declaration(self, begin_token, augment_token, enum_keyword, name);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_enum_declaration(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        enum_keyword: TokenId,
        left_brace: TokenId,
        member_count: i32,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_enum_declaration(
            self,
            begin_token,
            enum_keyword,
            left_brace,
            member_count,
            end_token,
        );
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_export(&mut self, tokens: &mut Tokens, export_keyword: TokenId, semicolon: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_export(self, export_keyword, semicolon);
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
    fn begin_factory(
        &mut self,
        tokens: &mut Tokens,
        declaration_kind: DeclarationKind,
        last_consumed: TokenId,
        augment_token: Option<TokenId>,
        external_token: Option<TokenId>,
        const_token: Option<TokenId>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_factory(
            self,
            declaration_kind,
            last_consumed,
            augment_token,
            external_token,
            const_token,
        );
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_factory(
        &mut self,
        tokens: &mut Tokens,
        kind: DeclarationKind,
        begin_token: TokenId,
        factory_keyword: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_factory(self, kind, begin_token, factory_keyword, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn begin_formal_parameter(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        kind: MemberKind,
        required_token: Option<TokenId>,
        covariant_token: Option<TokenId>,
        var_final_or_const: Option<TokenId>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_formal_parameter(
            self,
            token,
            kind,
            required_token,
            covariant_token,
            var_final_or_const,
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
    fn end_fields(
        &mut self,
        tokens: &mut Tokens,
        kind: DeclarationKind,
        abstract_token: Option<TokenId>,
        augment_token: Option<TokenId>,
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
        AstBuilder::end_fields(
            self,
            kind,
            abstract_token,
            augment_token,
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
    fn end_block_function_body(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_block_function_body(self, count, begin_token, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_no_function_body(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_function_body(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn begin_named_mixin_application(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        abstract_token: Option<TokenId>,
        sealed_token: Option<TokenId>,
        base_token: Option<TokenId>,
        interface_token: Option<TokenId>,
        final_token: Option<TokenId>,
        augment_token: Option<TokenId>,
        mixin_token: Option<TokenId>,
        name: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_named_mixin_application(
            self,
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
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_else_statement(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_else_statement(self, begin_token, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_conditional_uris(&mut self, tokens: &mut Tokens, count: i32) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_conditional_uris(self, count);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_conditional_uri(
        &mut self,
        tokens: &mut Tokens,
        if_keyword: TokenId,
        left_paren: TokenId,
        equal_sign: Option<TokenId>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_conditional_uri(self, if_keyword, left_paren, equal_sign);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_field_initializer(
        &mut self,
        tokens: &mut Tokens,
        assignment: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_field_initializer(self, assignment, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_initializer(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_initializer(self, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn begin_library_augmentation(
        &mut self,
        tokens: &mut Tokens,
        augment_keyword: TokenId,
        library_keyword: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_library_augmentation(self, augment_keyword, library_keyword);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn begin_literal_string(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_literal_string(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn begin_method(
        &mut self,
        tokens: &mut Tokens,
        declaration_kind: DeclarationKind,
        augment_token: Option<TokenId>,
        external_token: Option<TokenId>,
        static_token: Option<TokenId>,
        covariant_token: Option<TokenId>,
        var_final_or_const: Option<TokenId>,
        get_or_set: Option<TokenId>,
        name: TokenId,
        enclosing_declaration_name: Option<&str>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_method(
            self,
            declaration_kind,
            augment_token,
            external_token,
            static_token,
            covariant_token,
            var_final_or_const,
            get_or_set,
            name,
            enclosing_declaration_name,
        );
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn begin_constructor(
        &mut self,
        tokens: &mut Tokens,
        declaration_kind: DeclarationKind,
        augment_token: Option<TokenId>,
        external_token: Option<TokenId>,
        static_token: Option<TokenId>,
        covariant_token: Option<TokenId>,
        var_final_or_const: Option<TokenId>,
        get_or_set: Option<TokenId>,
        new_token: Option<TokenId>,
        name: TokenId,
        enclosing_declaration_name: Option<&str>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_constructor(
            self,
            declaration_kind,
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
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_constructor(
        &mut self,
        tokens: &mut Tokens,
        kind: DeclarationKind,
        begin_token: TokenId,
        new_token: Option<TokenId>,
        begin_param: TokenId,
        begin_initializers: Option<TokenId>,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_constructor(
            self,
            kind,
            begin_token,
            new_token,
            begin_param,
            begin_initializers,
            end_token,
        );
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn begin_metadata_star(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_metadata_star(self, token);
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
    fn begin_top_level_method(
        &mut self,
        tokens: &mut Tokens,
        last_consumed: TokenId,
        augment_token: Option<TokenId>,
        external_token: Option<TokenId>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_top_level_method(self, last_consumed, augment_token, external_token);
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
    fn begin_type_variable(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_type_variable(self, token);
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
    fn begin_variables_declaration(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        late_token: Option<TokenId>,
        var_final_or_const: Option<TokenId>,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_variables_declaration(self, token, late_token, var_final_or_const);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn begin_as_operator_type(&mut self, tokens: &mut Tokens, operator: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_as_operator_type(self, operator);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_as_operator_type(&mut self, tokens: &mut Tokens, operator: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_as_operator_type(self, operator);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_anonymous_method_invocation(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        function_definition: Option<TokenId>,
        end_token: TokenId,
        is_expression: bool,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_anonymous_method_invocation(
            self,
            begin_token,
            function_definition,
            end_token,
            is_expression,
        );
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_implicit_formal_parameters(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_implicit_formal_parameters(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_binary_expression(&mut self, tokens: &mut Tokens, token: TokenId, end_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_binary_expression(self, token, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_binary_pattern(&mut self, tokens: &mut Tokens, operator_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_binary_pattern(self, operator_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_conditional_expression(
        &mut self,
        tokens: &mut Tokens,
        question: TokenId,
        colon: TokenId,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_conditional_expression(self, question, colon, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_const_expression(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_const_expression(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn begin_if_control_flow(&mut self, tokens: &mut Tokens, if_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_if_control_flow(self, if_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn begin_is_operator_type(&mut self, tokens: &mut Tokens, operator: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_is_operator_type(self, operator);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_assert(
        &mut self,
        tokens: &mut Tokens,
        assert_keyword: TokenId,
        kind: Assert,
        left_parenthesis: TokenId,
        comma_token: Option<TokenId>,
        end_token: TokenId,
    ) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_assert(
            self,
            assert_keyword,
            kind,
            left_parenthesis,
            comma_token,
            end_token,
        );
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
    fn begin_pattern(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_pattern(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn begin_pattern_guard(&mut self, tokens: &mut Tokens, when: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_pattern_guard(self, when);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn begin_switch_case_when_clause(&mut self, tokens: &mut Tokens, when: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_switch_case_when_clause(self, when);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn begin_constant_pattern(&mut self, tokens: &mut Tokens, const_keyword: Option<TokenId>) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_constant_pattern(self, const_keyword);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_constant_pattern(&mut self, tokens: &mut Tokens, const_keyword: Option<TokenId>) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_constant_pattern(self, const_keyword);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn begin_formal_parameter_default_value_expression(&mut self, tokens: &mut Tokens) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_formal_parameter_default_value_expression(self);
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
    fn end_const_dot_shorthand(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_const_dot_shorthand(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }
}
