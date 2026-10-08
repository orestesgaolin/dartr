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
    fn handle_invalid_top_level_block(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_invalid_top_level_block(self, token);
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
    fn handle_no_extension_body(&mut self, tokens: &mut Tokens, semicolon_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_extension_body(self, semicolon_token);
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
    fn begin_primary_constructor_body(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_primary_constructor_body(self, token);
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
    fn end_while_statement_body(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_while_statement_body(self, end_token);
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
    fn handle_no_enum_body(&mut self, tokens: &mut Tokens, semicolon_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_enum_body(self, semicolon_token);
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
    fn handle_mixin_with_clause(&mut self, tokens: &mut Tokens, with_keyword: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_mixin_with_clause(self, with_keyword);
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
    fn handle_identifier_list(&mut self, tokens: &mut Tokens, count: i32) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_identifier_list(self, count);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_type_list(&mut self, tokens: &mut Tokens, count: i32) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_type_list(self, count);
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
    fn handle_recover_import(&mut self, tokens: &mut Tokens, semicolon: Option<TokenId>) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_recover_import(self, semicolon);
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
    fn handle_dotted_name(&mut self, tokens: &mut Tokens, count: i32, first_identifier: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_dotted_name(self, count, first_identifier);
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
    fn handle_no_field_initializer(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_no_field_initializer(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_variable_initializer(&mut self, tokens: &mut Tokens, assignment_operator: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_variable_initializer(self, assignment_operator);
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
    fn end_labeled_statement(&mut self, tokens: &mut Tokens, label_count: i32) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_labeled_statement(self, label_count);
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
    fn begin_literal_string(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_literal_string(self, token);
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
    fn handle_invalid_member(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_invalid_member(self, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_member(&mut self, tokens: &mut Tokens) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_member(self);
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
    fn handle_send(&mut self, tokens: &mut Tokens, begin_token: TokenId, end_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_send(self, begin_token, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_show(&mut self, tokens: &mut Tokens, show_keyword: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_show(self, show_keyword);
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
    fn end_top_level_declaration(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_top_level_declaration(self, end_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_invalid_top_level_declaration(&mut self, tokens: &mut Tokens, end_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_invalid_top_level_declaration(self, end_token);
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
    fn begin_type_variable(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_type_variable(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_type_variables_defined(&mut self, tokens: &mut Tokens, token: TokenId, count: i32) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_type_variables_defined(self, token, count);
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
    fn handle_const_factory(&mut self, tokens: &mut Tokens, const_keyword: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_const_factory(self, const_keyword);
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
    fn begin_if_control_flow(&mut self, tokens: &mut Tokens, if_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_if_control_flow(self, if_token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn handle_else_control_flow(&mut self, tokens: &mut Tokens, else_token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_else_control_flow(self, else_token);
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
    fn begin_is_operator_type(&mut self, tokens: &mut Tokens, operator: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_is_operator_type(self, operator);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_is_operator_type(&mut self, tokens: &mut Tokens, operator: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_is_operator_type(self, operator);
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
    fn handle_record_pattern(&mut self, tokens: &mut Tokens, token: TokenId, count: i32) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_record_pattern(self, token, count);
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
    fn handle_parenthesized_pattern(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_parenthesized_pattern(self, token);
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
    fn begin_formal_parameter_default_value_expression(&mut self, tokens: &mut Tokens) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::begin_formal_parameter_default_value_expression(self);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }

    #[inline]
    fn end_formal_parameter_default_value_expression(&mut self, tokens: &mut Tokens) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_formal_parameter_default_value_expression(self);
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
    fn handle_formal_parameter_without_value(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::handle_formal_parameter_without_value(self, token);
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

    #[inline]
    fn end_const_dot_shorthand(&mut self, tokens: &mut Tokens, token: TokenId) {
        std::mem::swap(tokens, &mut self.ast.tokens);
        AstBuilder::end_const_dot_shorthand(self, token);
        std::mem::swap(tokens, &mut self.ast.tokens);
    }
}
