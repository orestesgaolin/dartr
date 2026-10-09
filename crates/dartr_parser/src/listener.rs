// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/listener.dart
//
// GENERATED FILE. DO NOT EDIT. Run `python3 tools/codegen/gen_listener.py`.

//! The parser event listener (Dart `Listener`).
//!
//! Every method has the arguments of the Dart method, in the same order
//! (named and optional parameters are positional here). The first argument
//! is the token arena of the parser, so that a listener can read tokens and
//! change the token stream (the analyzer `AstBuilder` uses the parser's
//! `rewriter` from inside events, see `crate::token_stream_rewriter`).
//!
//! The defaults do nothing, except the methods that report errors through
//! `handle_recoverable_error` in Dart.

#![allow(unused_variables)]

use dartr_diagnostics::cfe::{CfeCode, CfeMessage};
use dartr_syntax::{TokenId, Tokens};

use crate::assert::Assert;
use crate::block_kind::BlockKind;
use crate::constructor_reference_context::ConstructorReferenceContext;
use crate::declaration_kind::{DeclarationHeaderKind, DeclarationKind};
use crate::error_token::error_token_assertion_message;
use crate::experimental_features::{ExperimentalFlag, get_experiment_not_enabled_message};
use crate::formal_parameter_kind::FormalParameterKind;
use crate::identifier_context::IdentifierContext;
use crate::member_kind::MemberKind;

/// A parser event listener (Dart `Listener`).
///
/// Events are methods that begin with one of: `begin`, `end`, or `handle`.
///
/// Events starting with `begin` and `end` come in pairs. Normally, a
/// `begin_foo` event is followed by an `end_foo` event. There's a few
/// exceptions documented below.
///
/// Events starting with `handle` are used when isn't possible to have a
/// begin event.
pub trait Listener {
    fn begin_arguments(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_arguments(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
    }

    /// Called after the parser has consumed a sequence of patternFields that
    /// forms the arguments to an objectPattern
    fn handle_object_pattern_fields(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
    }

    /// Handle async modifiers `async`, `async*`, `sync`.
    fn handle_async_modifier(
        &mut self,
        tokens: &mut Tokens,
        async_token: Option<TokenId>,
        star_token: Option<TokenId>,
    ) {
    }

    /// Ended by either `endAwaitExpression` or `endInvalidAwaitExpression`.
    fn begin_await_expression(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// One of the two possible corresponding end events for
    /// `beginAwaitExpression`.
    fn end_await_expression(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
    }

    /// One of the two possible corresponding end events for
    /// `beginAwaitExpression`.
    fn end_invalid_await_expression(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        end_token: TokenId,
        error_code: &'static CfeCode,
    ) {
    }

    fn begin_block(&mut self, tokens: &mut Tokens, token: TokenId, block_kind: BlockKind) {}

    fn end_block(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        begin_token: TokenId,
        end_token: TokenId,
        block_kind: BlockKind,
    ) {
    }

    /// Called to handle a block that has been parsed but is not associated
    /// with any top level function declaration. Substructures:
    /// - block
    fn handle_invalid_top_level_block(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn begin_cascade(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_cascade(&mut self, tokens: &mut Tokens) {}

    fn begin_case_expression(&mut self, tokens: &mut Tokens, case_keyword: TokenId) {}

    fn end_case_expression(
        &mut self,
        tokens: &mut Tokens,
        case_keyword: TokenId,
        when: Option<TokenId>,
        colon: TokenId,
    ) {
    }

    /// Handle the start of the body of a class, mixin or extension declaration
    /// beginning at `token`. The actual kind of declaration is indicated by
    /// `kind`.
    fn begin_class_or_mixin_or_extension_body(
        &mut self,
        tokens: &mut Tokens,
        kind: DeclarationKind,
        token: TokenId,
    ) {
    }

    /// Handle the end of the body of a class, mixin or extension declaration.
    /// The only substructures are the class, mixin or extension members.
    ///
    /// The actual kind of declaration is indicated by `kind`.
    fn end_class_or_mixin_or_extension_body(
        &mut self,
        tokens: &mut Tokens,
        kind: DeclarationKind,
        member_count: i32,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
    }

    /// Called before parsing a class declaration, mixin declaration, or named
    /// mixin application.
    ///
    /// At this point only the `class` or `mixin` keyword have been seen,
    /// so we know a declaration is coming but not its name or type
    /// parameter declarations.
    ///
    /// Ended by `endTopLevelDeclaration`.
    fn begin_class_or_mixin_or_named_mixin_application_prelude(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
    ) {
    }

    /// Handle the beginning of a class declaration.
    /// `begin` may be the 'class' token, or may point to modifiers
    /// (or extraneous modifiers in the case of recovery) preceding `name`.
    ///
    /// At this point we have parsed the name and type parameter declarations.
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
    }

    /// Handle an extends clause in a class declaration. Substructures:
    /// - supertype (may be a mixin application)
    /// The typeCount is for error recovery: Invalid code might have more than one
    /// class specified in the extends clause. A parser error has already been
    /// issued.
    fn handle_class_extends(
        &mut self,
        tokens: &mut Tokens,
        extends_keyword: Option<TokenId>,
        type_count: i32,
    ) {
    }

    /// Handle an implements clause in a class, mixin or enum declaration.
    /// Substructures:
    /// - implemented types
    fn handle_implements(
        &mut self,
        tokens: &mut Tokens,
        implements_keyword: Option<TokenId>,
        interfaces_count: i32,
    ) {
    }

    /// Handle the header of a class declaration.  Substructures:
    /// - metadata
    /// - modifiers
    /// - class name
    /// - type variables
    /// - supertype
    /// - with clause
    /// - implemented types
    /// - native clause
    fn handle_class_header(
        &mut self,
        tokens: &mut Tokens,
        begin: TokenId,
        class_keyword: TokenId,
        native_token: Option<TokenId>,
    ) {
    }

    /// Handle recovery associated with a class or extension type header.
    /// This may be called multiple times after `handleClassHeader`
    /// to recover information about the previous class header.
    /// The substructures are a subset of
    /// and in the same order as `handleClassHeader`:
    /// - supertype
    /// - with clause
    /// - implemented types
    fn handle_recover_declaration_header(
        &mut self,
        tokens: &mut Tokens,
        kind: DeclarationHeaderKind,
    ) {
    }

    /// Handle the end of a class declaration.  Substructures:
    /// - class header
    /// - class body
    fn end_class_declaration(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
    }

    /// Handle `;` as a class body.
    fn handle_no_class_body(&mut self, tokens: &mut Tokens, semicolon_token: TokenId) {}

    /// Handle `;` as an extension type body.
    fn handle_no_extension_type_body(&mut self, tokens: &mut Tokens, semicolon_token: TokenId) {}

    /// Handle the beginning of a mixin declaration.
    fn begin_mixin_declaration(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        augment_token: Option<TokenId>,
        base_token: Option<TokenId>,
        mixin_keyword: TokenId,
        name: TokenId,
    ) {
    }

    /// Handle an on clause in a mixin declaration. Substructures:
    /// - implemented types
    fn handle_mixin_on(
        &mut self,
        tokens: &mut Tokens,
        on_keyword: Option<TokenId>,
        type_count: i32,
    ) {
    }

    /// Handle the header of a mixin declaration.  Substructures:
    /// - metadata
    /// - mixin name
    /// - type variables
    /// - on types
    /// - implemented types
    fn handle_mixin_header(&mut self, tokens: &mut Tokens, mixin_keyword: TokenId) {}

    /// Handle recovery associated with a mixin header.
    /// This may be called multiple times after `handleMixinHeader`
    /// to recover information about the previous mixin header.
    /// For otherwise legal input the substructures are a subset of
    /// and in the same order as `handleMixinHeader`
    /// - on types
    /// - implemented types
    /// but also covers the illegal
    /// - with clause
    fn handle_recover_mixin_header(&mut self, tokens: &mut Tokens) {}

    /// Handle `;` as a mixin body.
    fn handle_no_mixin_body(&mut self, tokens: &mut Tokens, semicolon_token: TokenId) {}

    /// Handle the end of a mixin declaration.  Substructures:
    /// - mixin header
    /// - class or mixin body
    fn end_mixin_declaration(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
    }

    /// Begins a not-further-categorized top-level declaration.
    ///
    /// Ended by `endTopLevelDeclaration`.
    fn begin_uncategorized_top_level_declaration(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Handle the beginning of an extension methods declaration.  Substructures:
    /// - metadata
    ///
    /// At this point only the `extension` keyword have been seen, so we know a
    /// declaration is coming but not its name or type parameter declarations.
    ///
    /// Ended by `endTopLevelDeclaration`.
    fn begin_extension_declaration_prelude(
        &mut self,
        tokens: &mut Tokens,
        extension_keyword: TokenId,
    ) {
    }

    /// Handle the beginning of an extension methods declaration.  Substructures:
    /// - type variables
    ///
    /// At this point we have parsed the name and type parameter declarations.
    fn begin_extension_declaration(
        &mut self,
        tokens: &mut Tokens,
        augment_token: Option<TokenId>,
        extension_keyword: TokenId,
        name: Option<TokenId>,
    ) {
    }

    /// Handle the end of an extension methods declaration.  Substructures:
    /// - substructures from `beginExtensionDeclaration`
    /// - on type
    /// - body
    fn end_extension_declaration(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        extension_keyword: TokenId,
        on_keyword: Option<TokenId>,
        end_token: TokenId,
    ) {
    }

    /// Handle `;` as an extension body.
    fn handle_no_extension_body(&mut self, tokens: &mut Tokens, semicolon_token: TokenId) {}

    /// Handle the beginning of an extension type declaration.  Substructures:
    /// - type variables
    ///
    /// At this point we have parsed the name and type parameter declarations.
    fn begin_extension_type_declaration(
        &mut self,
        tokens: &mut Tokens,
        augment_keyword: Option<TokenId>,
        extension_keyword: TokenId,
        name: TokenId,
    ) {
    }

    /// Handle the end of an extension methods declaration.  Substructures:
    /// - substructures from `beginExtensionTypeDeclaration`
    /// - primary constructor formals
    /// - implements clause
    /// - body
    fn end_extension_type_declaration(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        augment_token: Option<TokenId>,
        extension_keyword: TokenId,
        type_keyword: TokenId,
        end_token: TokenId,
    ) {
    }

    /// Handle the start of a primary constructor declaration.
    fn begin_primary_constructor(&mut self, tokens: &mut Tokens, begin_token: TokenId) {}

    /// Handle the end of a primary constructor declaration. `constKeyword` is the
    /// 'const' keyword, if present, in
    ///
    ///   class const Class() {}
    ///   enum const Enum() {}
    ///   extension type const ExtensionType() {}
    ///
    /// Substructures:
    /// - constructor name (if `hasConstructorName` is `true`)
    /// - formals
    fn end_primary_constructor(
        &mut self,
        tokens: &mut Tokens,
        kind: DeclarationKind,
        begin_token: TokenId,
        end_token: TokenId,
        const_keyword: Option<TokenId>,
        has_constructor_name: bool,
    ) {
    }

    /// Handle the omission of a primary constructor declaration.
    fn handle_no_primary_constructor(
        &mut self,
        tokens: &mut Tokens,
        kind: DeclarationKind,
        token: TokenId,
        const_keyword: Option<TokenId>,
    ) {
    }

    fn begin_primary_constructor_body(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// - metadata
    /// - initializers
    /// - async marker
    /// - body
    fn end_primary_constructor_body(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        begin_initializers: Option<TokenId>,
        end_token: TokenId,
    ) {
    }

    fn begin_combinators(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_combinators(&mut self, tokens: &mut Tokens, count: i32) {}

    fn begin_compilation_unit(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// This method exists for analyzer compatibility only
    /// and will be removed once analyzer/cfe integration is complete.
    ///
    /// This is called when `Parser.parseDirectives` has parsed all directives
    /// and is skipping the remainder of the file.  Substructures:
    /// - metadata
    fn handle_directives_only(&mut self, tokens: &mut Tokens) {}

    fn end_compilation_unit(&mut self, tokens: &mut Tokens, count: i32, token: TokenId) {}

    fn begin_const_literal(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_const_literal(&mut self, tokens: &mut Tokens, end_token: TokenId) {}

    fn begin_constructor_reference(&mut self, tokens: &mut Tokens, start: TokenId) {}

    fn end_constructor_reference(
        &mut self,
        tokens: &mut Tokens,
        start: TokenId,
        period_before_name: Option<TokenId>,
        end_token: TokenId,
        constructor_reference_context: ConstructorReferenceContext,
    ) {
    }

    fn begin_do_while_statement(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_do_while_statement(
        &mut self,
        tokens: &mut Tokens,
        do_keyword: TokenId,
        while_keyword: TokenId,
        end_token: TokenId,
    ) {
    }

    fn begin_do_while_statement_body(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_do_while_statement_body(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn begin_while_statement_body(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_while_statement_body(&mut self, tokens: &mut Tokens, end_token: TokenId) {}

    /// Handle the beginning of an enum declaration.  Substructures:
    /// - metadata
    ///
    /// At this point only the `enum` keyword have been seen, so we know a
    /// declaration is coming but not its name or type parameter declarations.
    ///
    /// Ended by `endTopLevelDeclaration`.
    fn begin_enum_declaration_prelude(&mut self, tokens: &mut Tokens, enum_keyword: TokenId) {}

    /// Handle the beginning of an enum declaration.
    /// `beginToken` may be the `enumKeyword`, or may point to modifiers
    /// (or extraneous modifiers in the case of recovery) preceding `name`.
    ///
    /// At this point we have parsed the name and type parameter declarations.
    fn begin_enum_declaration(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        augment_token: Option<TokenId>,
        enum_keyword: TokenId,
        name: TokenId,
    ) {
    }

    /// Handle the end of an enum declaration.  Substructures:
    /// - `memberCount` times:
    ///   - Enum member
    fn end_enum_declaration(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        enum_keyword: TokenId,
        left_brace: TokenId,
        member_count: i32,
        end_token: TokenId,
    ) {
    }

    /// Handle the enum elements. Substructures:
    /// - `elementsCount` times:
    ///   - Enum element
    fn handle_enum_elements(
        &mut self,
        tokens: &mut Tokens,
        elements_end_token: TokenId,
        elements_count: i32,
    ) {
    }

    /// Handle the header of an enum declaration.  Substructures:
    /// - Metadata
    /// - Enum name (identifier)
    /// - type variables
    /// - with clause
    /// - implemented types
    fn handle_enum_header(
        &mut self,
        tokens: &mut Tokens,
        augment_token: Option<TokenId>,
        enum_keyword: TokenId,
        left_brace: TokenId,
    ) {
    }

    /// Handle the start of an enum body.
    fn begin_enum_body(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Handle the end of an enum body.
    fn end_enum_body(&mut self, tokens: &mut Tokens, begin_token: TokenId, end_token: TokenId) {}

    /// Handle `;` as an enum body.
    fn handle_no_enum_body(&mut self, tokens: &mut Tokens, semicolon_token: TokenId) {}

    /// Handle the enum element. Substructures:
    /// - Metadata
    /// - Enum value (identifier)
    fn handle_enum_element(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        augment_token: Option<TokenId>,
    ) {
    }

    fn begin_export(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Handle the end of an export directive.  Substructures:
    /// - metadata
    /// - uri
    /// - conditional uris
    /// - combinators
    fn end_export(&mut self, tokens: &mut Tokens, export_keyword: TokenId, semicolon: TokenId) {}

    /// Called by `Parser` after parsing an extraneous expression as error
    /// recovery. For a stack-based listener, the suggested action is to discard
    /// an expression from the stack.
    fn handle_extraneous_expression(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        message: CfeMessage,
    ) {
    }

    fn handle_expression_statement(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
    }

    fn begin_factory(
        &mut self,
        tokens: &mut Tokens,
        declaration_kind: DeclarationKind,
        last_consumed: TokenId,
        augment_token: Option<TokenId>,
        external_token: Option<TokenId>,
        const_token: Option<TokenId>,
    ) {
    }

    fn end_factory(
        &mut self,
        tokens: &mut Tokens,
        kind: DeclarationKind,
        begin_token: TokenId,
        factory_keyword: TokenId,
        end_token: TokenId,
    ) {
    }

    fn begin_formal_parameter(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        kind: MemberKind,
        required_token: Option<TokenId>,
        covariant_token: Option<TokenId>,
        var_final_or_const: Option<TokenId>,
    ) {
    }

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
    }

    fn handle_no_formal_parameters(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        kind: MemberKind,
    ) {
    }

    fn begin_formal_parameters(&mut self, tokens: &mut Tokens, token: TokenId, kind: MemberKind) {}

    fn end_formal_parameters(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        begin_token: TokenId,
        end_token: TokenId,
        kind: MemberKind,
    ) {
    }

    /// Handle the end of a class field declaration.  Substructures:
    /// - Metadata
    /// - Modifiers
    /// - Type
    /// - Variable declarations (count times)
    ///
    /// Started by `beginFields`.
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
    }

    /// Marks that the grammar term `forInitializerStatement` has been parsed and
    /// it was an empty statement.
    fn handle_for_initializer_empty_statement(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Marks that the grammar term `forInitializerStatement` has been parsed and
    /// it was an expression statement.
    fn handle_for_initializer_expression_statement(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        for_in: bool,
    ) {
    }

    /// Marks that the grammar term `forInitializerStatement` has been parsed and
    /// it was a `localVariableDeclaration` of the form
    /// `metadata initializedVariableDeclaration ';'`.
    fn handle_for_initializer_local_variable_declaration(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        for_in: bool,
    ) {
    }

    /// Marks that the grammar term `forInitializerStatement` has been parsed and
    /// it was a `localVariableDeclaration` of the form
    /// `metadata patternVariableDeclaration ';'`.
    fn handle_for_initializer_pattern_variable_assignment(
        &mut self,
        tokens: &mut Tokens,
        keyword: TokenId,
        equals: TokenId,
    ) {
    }

    /// Marks the start of a for statement which is ended by either
    /// `endForStatement` or `endForIn`.
    fn begin_for_statement(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Marks the end of parsing the control structure of a for statement
    /// or for control flow entry up to and including the closing parenthesis.
    /// `for` `(` initialization `;` condition `;` updaters `)`
    fn handle_for_loop_parts(
        &mut self,
        tokens: &mut Tokens,
        for_keyword: TokenId,
        left_paren: TokenId,
        left_separator: TokenId,
        right_separator: TokenId,
        update_expression_count: i32,
    ) {
    }

    fn end_for_statement(&mut self, tokens: &mut Tokens, end_token: TokenId) {}

    fn begin_for_statement_body(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_for_statement_body(&mut self, tokens: &mut Tokens, end_token: TokenId) {}

    /// Marks the end of parsing the control structure of a for-in statement
    /// or for control flow entry up to and including the closing parenthesis.
    /// If `patternKeyword` is `null`, this takes the form:
    ///   `for` `(` (type)? identifier `in` iterator `)`
    /// If `patternKeyword` is not `null`, it is either a `var` or `final` token,
    /// and this takes the form:
    ///   `for` `(` patternKeyword pattern `in` iterator `)`
    fn handle_for_in_loop_parts(
        &mut self,
        tokens: &mut Tokens,
        await_token: Option<TokenId>,
        for_token: TokenId,
        left_parenthesis: TokenId,
        pattern_keyword: Option<TokenId>,
        in_keyword: TokenId,
    ) {
    }

    fn end_for_in(&mut self, tokens: &mut Tokens, end_token: TokenId) {}

    fn begin_for_in_expression(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_for_in_expression(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn begin_for_in_body(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_for_in_body(&mut self, tokens: &mut Tokens, end_token: TokenId) {}

    /// Handle the beginning of a named function expression which isn't legal
    /// syntax in Dart.  Useful for recovering from JavaScript code being pasted
    /// into a Dart program, as it will interpret `function foo() {}` as a named
    /// function expression with return type `function` and name `foo`.
    ///
    /// Substructures:
    /// - Type variables
    fn begin_named_function_expression(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// A named function expression which isn't legal syntax in Dart.
    /// Useful for recovering from JavaScript code being pasted into a Dart
    /// program, as it will interpret `function foo() {}` as a named function
    /// expression with return type `function` and name `foo`.
    ///
    /// Substructures:
    /// - Type variables
    /// - Modifiers
    /// - Return type
    /// - Name
    /// - Formals
    /// - Initializers
    /// - Async modifier
    /// - Function body (block or arrow expression).
    fn end_named_function_expression(&mut self, tokens: &mut Tokens, end_token: TokenId) {}

    /// Handle the beginning of a local function declaration.  Substructures:
    /// - Metadata
    /// - Type variables
    fn begin_local_function_declaration(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// A function declaration.
    ///
    /// Substructures:
    /// - Metadata
    /// - Type variables
    /// - Return type
    /// - Name
    /// - Type variables
    /// - Formals
    /// - Initializers
    /// - Async modifier
    /// - Function body (block or arrow expression).
    fn end_local_function_declaration(&mut self, tokens: &mut Tokens, end_token: TokenId) {}

    /// This method is invoked when the parser sees that a function has a
    /// block function body.  This method is not invoked for empty or expression
    /// function bodies, see the corresponding methods `handleEmptyFunctionBody`
    /// and `handleExpressionFunctionBody`.
    fn begin_block_function_body(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// This method is invoked by the parser after it finished parsing a block
    /// function body.  This method is not invoked for empty or expression
    /// function bodies, see the corresponding methods `handleEmptyFunctionBody`
    /// and `handleExpressionFunctionBody`.  The `beginToken` is the '{' token,
    /// and the `endToken` is the '}' token of the block.  The number of
    /// statements is given as the `count` parameter.
    fn end_block_function_body(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
    }

    fn handle_no_function_body(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Handle the end of a function body that was skipped by the parser.
    ///
    /// The boolean `isExpressionBody` indicates whether the function body that
    /// was skipped used "=>" syntax.
    fn handle_function_body_skipped(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        end_token: TokenId,
        is_expression_body: bool,
    ) {
    }

    fn begin_function_name(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// The end of the function name in either a local function declaration, like
    /// 'local' in:
    ///
    /// ```text
    ///     void m() {
    ///       void local() {}
    ///     }
    /// ```
    ///
    /// or an erroneous function expression, like 'local' in:
    ///
    /// ```text
    ///     void m() {
    ///       var f = void local() {};
    ///     }
    /// ```
    ///
    /// The boolean `isFunctionExpression` indicates that we are in the latter
    /// case.
    fn end_function_name(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        token: TokenId,
        is_function_expression: bool,
    ) {
    }

    fn begin_typedef(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Handle the end of a typedef declaration.
    ///
    /// If `equals` is null, then we have the following substructures:
    /// - Metadata
    /// - Return type
    /// - Name (identifier)
    /// - Alias type variables
    /// - Formal parameters
    ///
    /// If `equals` is not null, then the have the following substructures:
    /// - Metadata
    /// - Name (identifier)
    /// - Alias type variables
    /// - Type (FunctionTypeAnnotation)
    fn end_typedef(
        &mut self,
        tokens: &mut Tokens,
        augment_token: Option<TokenId>,
        typedef_keyword: TokenId,
        equals: Option<TokenId>,
        end_token: TokenId,
    ) {
    }

    /// Handle the end of a class with clause (e.g. "with B, C").
    /// Substructures:
    /// - mixin types (TypeList)
    fn handle_class_with_clause(&mut self, tokens: &mut Tokens, with_keyword: TokenId) {}

    /// Handle the absence of a class with clause.
    fn handle_class_no_with_clause(&mut self, tokens: &mut Tokens) {}

    /// Handle the end of an enum with clause (e.g. "with B, C").
    /// Substructures:
    /// - mixin types (TypeList)
    ///
    /// This method is separated from `handleClassWithClause` to simplify
    /// handling the different objects in the context.
    fn handle_enum_with_clause(&mut self, tokens: &mut Tokens, with_keyword: TokenId) {}

    /// Handle the absence of an enum with clause.
    fn handle_enum_no_with_clause(&mut self, tokens: &mut Tokens) {}

    /// Handle the end of a mixin with clause (e.g. "with B, C").
    /// Substructures:
    /// - mixin types (TypeList)
    ///
    /// This method is separated from `handleClassWithClause` and
    /// `handleEnumWithClause` as it is an error state.
    fn handle_mixin_with_clause(&mut self, tokens: &mut Tokens, with_keyword: TokenId) {}

    /// Handle the beginning of a named mixin application.
    /// `beginToken` may be the same as `name`, or may point to modifiers
    /// (or extraneous modifiers in the case of recovery) preceding `name`.
    ///
    /// At this point we have parsed the name and type parameter declarations.
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
    }

    /// Handle a named mixin application with clause (e.g. "A with B, C").
    /// Substructures:
    /// - supertype
    /// - mixin types (TypeList)
    fn handle_named_mixin_application_with_clause(
        &mut self,
        tokens: &mut Tokens,
        with_keyword: TokenId,
    ) {
    }

    /// Handle the end of a named mixin declaration.  Substructures:
    /// - metadata
    /// - modifiers
    /// - class name
    /// - type variables
    /// - supertype
    /// - with clause
    /// - implemented types (TypeList)
    ///
    /// TODO(paulberry,ahe): it seems inconsistent that for a named mixin
    /// application, the implemented types are a TypeList, whereas for a class
    /// declaration, each implemented type is listed separately on the stack, and
    /// the number of implemented types is passed as a parameter.
    ///
    /// TODO(jensj): Rename `begin` to `beginToken` for consistency.
    fn end_named_mixin_application(
        &mut self,
        tokens: &mut Tokens,
        begin: TokenId,
        class_keyword: TokenId,
        equals: TokenId,
        implements_keyword: Option<TokenId>,
        end_token: TokenId,
    ) {
    }

    fn begin_hide(&mut self, tokens: &mut Tokens, hide_keyword: TokenId) {}

    /// Handle the end of a "hide" combinator.  Substructures:
    /// - hidden names (IdentifierList)
    fn end_hide(&mut self, tokens: &mut Tokens, hide_keyword: TokenId) {}

    fn handle_identifier_list(&mut self, tokens: &mut Tokens, count: i32) {}

    fn begin_type_list(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_type_list(&mut self, tokens: &mut Tokens, count: i32) {}

    fn begin_if_statement(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_if_statement(
        &mut self,
        tokens: &mut Tokens,
        if_token: TokenId,
        else_token: Option<TokenId>,
        end_token: TokenId,
    ) {
    }

    fn begin_then_statement(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_then_statement(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
    }

    fn begin_else_statement(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// The `beginToken` is the `else` token.
    fn end_else_statement(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
    }

    fn begin_import(&mut self, tokens: &mut Tokens, import_keyword: TokenId) {}

    /// Signals that the current import is deferred and/or has a prefix
    /// depending upon whether `deferredKeyword` and `asKeyword`
    /// are not `null` respectively. Substructures:
    /// - prefix identifier (only if asKeyword != null)
    fn handle_import_prefix(
        &mut self,
        tokens: &mut Tokens,
        deferred_keyword: Option<TokenId>,
        as_keyword: Option<TokenId>,
    ) {
    }

    /// Handle the end of an import directive.  Substructures:
    /// - metadata
    /// - uri
    /// - conditional uris
    /// - prefix identifier
    /// - combinators
    fn end_import(
        &mut self,
        tokens: &mut Tokens,
        import_keyword: TokenId,
        semicolon: Option<TokenId>,
    ) {
    }

    /// Handle recovery associated with an import directive.
    /// This may be called multiple times after `endImport`
    /// to recover information about the previous import directive.
    /// The substructures are a subset of and in the same order as `endImport`:
    /// - conditional uris
    /// - prefix identifier
    /// - combinators
    fn handle_recover_import(&mut self, tokens: &mut Tokens, semicolon: Option<TokenId>) {}

    fn begin_conditional_uris(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_conditional_uris(&mut self, tokens: &mut Tokens, count: i32) {}

    fn begin_conditional_uri(&mut self, tokens: &mut Tokens, if_keyword: TokenId) {}

    /// Handle the end of a conditional URI construct.  Substructures:
    /// - Dotted name
    /// - Condition (literal string; only if `equalSign` != null)
    /// - URI (literal string)
    fn end_conditional_uri(
        &mut self,
        tokens: &mut Tokens,
        if_keyword: TokenId,
        left_paren: TokenId,
        equal_sign: Option<TokenId>,
    ) {
    }

    fn handle_dotted_name(&mut self, tokens: &mut Tokens, count: i32, first_identifier: TokenId) {}

    fn begin_implicit_creation_expression(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_implicit_creation_expression(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        open_angle_bracket: TokenId,
    ) {
    }

    fn begin_initialized_identifier(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_initialized_identifier(&mut self, tokens: &mut Tokens, name_token: TokenId) {}

    fn begin_field_initializer(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Handle the end of a field initializer.  Substructures:
    /// - Initializer expression
    fn end_field_initializer(
        &mut self,
        tokens: &mut Tokens,
        assignment: TokenId,
        end_token: TokenId,
    ) {
    }

    /// Handle the lack of a field initializer.
    fn handle_no_field_initializer(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn begin_variable_initializer(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Handle the end of a variable initializer. Substructures:
    /// - Initializer expression.
    fn end_variable_initializer(&mut self, tokens: &mut Tokens, assignment_operator: TokenId) {}

    /// Used when a variable has no initializer.
    fn handle_no_variable_initializer(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn begin_initializer(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_initializer(&mut self, tokens: &mut Tokens, end_token: TokenId) {}

    fn begin_initializers(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_initializers(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
    }

    fn handle_no_initializers(&mut self, tokens: &mut Tokens) {}

    /// Called after the listener has recovered from an invalid expression. The
    /// parser will resume parsing from `token`. Exactly where the parser will
    /// resume parsing is unspecified.
    fn handle_invalid_expression(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Called after the listener has recovered from an invalid function
    /// body. The parser expected an open curly brace `{` and will resume parsing
    /// from `token` as if a function body had preceded it.
    fn handle_invalid_function_body(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Called after the listener has recovered from an invalid type. The parser
    /// expected an identifier, and will resume parsing type arguments from
    /// `token`.
    fn handle_invalid_type_reference(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn handle_label(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn begin_labeled_statement(&mut self, tokens: &mut Tokens, token: TokenId, label_count: i32) {}

    fn end_labeled_statement(&mut self, tokens: &mut Tokens, label_count: i32) {}

    fn begin_library_augmentation(
        &mut self,
        tokens: &mut Tokens,
        augment_keyword: TokenId,
        library_keyword: TokenId,
    ) {
    }

    /// Handle the end of a library augmentation directive.  Substructures:
    /// - metadata
    /// - uri
    fn end_library_augmentation(
        &mut self,
        tokens: &mut Tokens,
        augment_keyword: TokenId,
        library_keyword: TokenId,
        semicolon: TokenId,
    ) {
    }

    fn begin_library_name(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Handle the end of a library directive.  Substructures:
    /// - Metadata
    /// - Library name (a qualified identifier)
    fn end_library_name(
        &mut self,
        tokens: &mut Tokens,
        library_keyword: TokenId,
        semicolon: TokenId,
        has_name: bool,
    ) {
    }

    /// Called after parsing a map entry. Either the key or the value or both can
    /// start with the null-aware token `?`. In that case, `nullAwareKeyToken` and
    /// `nullAwareValueToken` are set appropriately. Substructures:
    /// - expression
    /// - expression
    fn handle_literal_map_entry(
        &mut self,
        tokens: &mut Tokens,
        colon: TokenId,
        end_token: TokenId,
        null_aware_key_token: Option<TokenId>,
        null_aware_value_token: Option<TokenId>,
    ) {
    }

    /// Called after the parser has consumed a mapPatternEntry, consisting of an
    /// expression, a colon, and a pattern.
    fn handle_map_pattern_entry(
        &mut self,
        tokens: &mut Tokens,
        colon: TokenId,
        end_token: TokenId,
    ) {
    }

    fn begin_literal_string(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn handle_interpolation_expression(
        &mut self,
        tokens: &mut Tokens,
        left_bracket: TokenId,
        right_bracket: Option<TokenId>,
    ) {
    }

    fn end_literal_string(
        &mut self,
        tokens: &mut Tokens,
        interpolation_count: i32,
        end_token: TokenId,
    ) {
    }

    fn handle_adjacent_string_literals(
        &mut self,
        tokens: &mut Tokens,
        start_token: TokenId,
        literal_count: i32,
    ) {
    }

    /// Called for class-like members (class, mixin, extension), but each member
    /// should also have a more specific begin/end pair, e.g.
    /// `beginFactory`/`endFactory`.
    fn begin_member(&mut self, tokens: &mut Tokens) {}

    /// Handle an invalid member declaration. Substructures:
    /// - metadata
    fn handle_invalid_member(&mut self, tokens: &mut Tokens, end_token: TokenId) {}

    /// This event is added for convenience to the listener.
    /// Members will actually be begin/end'ed by more specific
    /// events as well.
    /// Normally listeners should probably override
    /// `endFields`, `endMethod` or `endConstructor` instead.
    fn end_member(&mut self, tokens: &mut Tokens) {}

    /// Handle the beginning of a class-like method declaration.  Substructures:
    /// - metadata
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
    }

    /// Handle the end of a method declaration in a class, enum, mixin, extension
    /// or extension type.  Substructures:
    /// - metadata
    /// - return type
    /// - method name (identifier, possibly qualified)
    /// - type variables
    /// - formal parameters
    /// - initializers
    /// - async marker
    /// - body
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
    }

    /// Handle the beginning of a constructor declaration.  Substructures:
    /// - metadata
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
    }

    /// Handle the end of a constructor declaration.  Substructures:
    /// - metadata
    /// - return type
    /// - method name (identifier, possibly qualified)
    /// - type variables
    /// - formal parameters
    /// - initializers
    /// - async marker
    /// - body
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
    }

    fn begin_metadata_star(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_metadata_star(&mut self, tokens: &mut Tokens, count: i32) {}

    fn begin_metadata(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Handle the end of a metadata annotation.  Substructures:
    /// - Identifier
    /// - Type arguments
    /// - Constructor name (only if `periodBeforeName` is not `null`)
    /// - Arguments
    fn end_metadata(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        period_before_name: Option<TokenId>,
        end_token: TokenId,
    ) {
    }

    fn begin_optional_formal_parameters(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_optional_formal_parameters(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        begin_token: TokenId,
        end_token: TokenId,
        kind: MemberKind,
    ) {
    }

    fn begin_part(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Handle the end of a part directive.  Substructures:
    /// - metadata
    /// - uri
    fn end_part(&mut self, tokens: &mut Tokens, part_keyword: TokenId, semicolon: TokenId) {}

    fn begin_part_of(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Handle the end of a "part of" directive.  Substructures:
    /// - Metadata
    /// - Library name (a qualified identifier)
    ///
    /// If `hasName` is true, this part refers to its library by name, otherwise,
    /// by URI.
    fn end_part_of(
        &mut self,
        tokens: &mut Tokens,
        part_keyword: TokenId,
        of_keyword: TokenId,
        semicolon: TokenId,
        has_name: bool,
    ) {
    }

    fn begin_redirecting_factory_body(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_redirecting_factory_body(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
    }

    fn begin_return_statement(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Handle the end of a `native` function.
    /// The `handleNativeClause` event is sent prior to this event.
    fn handle_native_function_body(
        &mut self,
        tokens: &mut Tokens,
        native_token: TokenId,
        semicolon: TokenId,
    ) {
    }

    /// Called after the `handleNativeClause` event when the parser determines
    /// that the native clause should be discarded / ignored.
    /// For example, this method is called a native clause is followed by
    /// a function body.
    fn handle_native_function_body_ignored(
        &mut self,
        tokens: &mut Tokens,
        native_token: TokenId,
        semicolon: TokenId,
    ) {
    }

    /// Handle the end of a `native` function that was skipped by the parser.
    /// The `handleNativeClause` event is sent prior to this event.
    fn handle_native_function_body_skipped(
        &mut self,
        tokens: &mut Tokens,
        native_token: TokenId,
        semicolon: TokenId,
    ) {
    }

    /// This method is invoked when a function has the empty body.
    fn handle_empty_function_body(&mut self, tokens: &mut Tokens, semicolon: TokenId) {}

    /// This method is invoked when parser finishes parsing the corresponding
    /// expression of the expression function body.
    fn handle_expression_function_body(
        &mut self,
        tokens: &mut Tokens,
        arrow_token: TokenId,
        end_token: Option<TokenId>,
    ) {
    }

    fn end_return_statement(
        &mut self,
        tokens: &mut Tokens,
        has_expression: bool,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
    }

    fn handle_send(&mut self, tokens: &mut Tokens, begin_token: TokenId, end_token: TokenId) {}

    fn begin_show(&mut self, tokens: &mut Tokens, show_keyword: TokenId) {}

    /// Handle the end of a "show" combinator.  Substructures:
    /// - shown names (IdentifierList)
    fn end_show(&mut self, tokens: &mut Tokens, show_keyword: TokenId) {}

    fn begin_switch_statement(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_switch_statement(
        &mut self,
        tokens: &mut Tokens,
        switch_keyword: TokenId,
        end_token: TokenId,
    ) {
    }

    fn begin_switch_expression(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_switch_expression(
        &mut self,
        tokens: &mut Tokens,
        switch_keyword: TokenId,
        end_token: TokenId,
    ) {
    }

    fn begin_switch_block(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_switch_block(
        &mut self,
        tokens: &mut Tokens,
        case_count: i32,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
    }

    fn begin_switch_expression_block(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_switch_expression_block(
        &mut self,
        tokens: &mut Tokens,
        case_count: i32,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
    }

    fn begin_literal_symbol(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_literal_symbol(
        &mut self,
        tokens: &mut Tokens,
        hash_token: TokenId,
        identifier_count: i32,
    ) {
    }

    fn handle_throw_expression(
        &mut self,
        tokens: &mut Tokens,
        throw_token: TokenId,
        end_token: TokenId,
    ) {
    }

    fn begin_rethrow_statement(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_rethrow_statement(
        &mut self,
        tokens: &mut Tokens,
        rethrow_token: TokenId,
        end_token: TokenId,
    ) {
    }

    /// This event is added for convenience for the listener.
    /// All top-level declarations will actually be begin/end'ed by more specific
    /// events as well, e.g. `beginClassDeclaration`/`endClassDeclaration`,
    /// `beginEnumDeclaration`/`endEnumDeclaration` etc.
    ///
    /// Normally listeners should probably override
    /// `endClassDeclaration`, `endNamedMixinApplication`, `endEnumDeclaration`,
    /// `endTypedef`, `endLibraryName`, `endImport`, `endExport`,
    /// `endPart`, `endPartOf`, `endTopLevelFields`, or `endTopLevelMethod`
    /// instead.
    ///
    /// Started by one of `beginExtensionDeclarationPrelude`,
    /// `beginClassOrMixinOrNamedMixinApplicationPrelude`, `beginTopLevelMember`
    /// or `beginUncategorizedTopLevelDeclaration`.
    fn end_top_level_declaration(&mut self, tokens: &mut Tokens, end_token: TokenId) {}

    /// Called by the `Parser` when it recovers from an invalid top level
    /// declaration, where `endToken` is the last token in the declaration
    /// This is called after the begin/end metadata star events,
    /// and is followed by `endTopLevelDeclaration`.
    ///
    /// Substructures:
    /// - metadata
    fn handle_invalid_top_level_declaration(&mut self, tokens: &mut Tokens, end_token: TokenId) {}

    /// Marks the beginning of a top level field or method declaration.
    /// See also `endTopLevelFields` and `endTopLevelMethod`.
    ///
    /// Ended by `endTopLevelDeclaration`.
    fn begin_top_level_member(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Marks the beginning of a fields declaration.
    /// Note that this is ended with `endTopLevelFields` or `endFields`.
    fn begin_fields(
        &mut self,
        tokens: &mut Tokens,
        declaration_kind: DeclarationKind,
        augment_token: Option<TokenId>,
        abstract_token: Option<TokenId>,
        external_token: Option<TokenId>,
        static_token: Option<TokenId>,
        covariant_token: Option<TokenId>,
        late_token: Option<TokenId>,
        var_final_or_const: Option<TokenId>,
        last_consumed: TokenId,
    ) {
    }

    /// Handle the end of a top level variable declaration.  Substructures:
    /// - Metadata
    /// - Type
    /// - Repeated `count` times:
    ///   - Variable name (identifier)
    ///   - Field initializer
    ///
    /// Started by `beginFields`.
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
    }

    fn begin_top_level_method(
        &mut self,
        tokens: &mut Tokens,
        last_consumed: TokenId,
        augment_token: Option<TokenId>,
        external_token: Option<TokenId>,
    ) {
    }

    /// Handle the end of a top level method.  Substructures:
    /// - metadata
    /// - modifiers
    /// - return type
    /// - identifier
    /// - type variables
    /// - formal parameters
    /// - async marker
    /// - body
    fn end_top_level_method(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        get_or_set: Option<TokenId>,
        end_token: TokenId,
    ) {
    }

    fn begin_try_statement(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn begin_catch_clause(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_catch_clause(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn handle_catch_block(
        &mut self,
        tokens: &mut Tokens,
        on_keyword: Option<TokenId>,
        catch_keyword: Option<TokenId>,
        comma: Option<TokenId>,
    ) {
    }

    fn handle_finally_block(&mut self, tokens: &mut Tokens, finally_keyword: TokenId) {}

    fn end_try_statement(
        &mut self,
        tokens: &mut Tokens,
        catch_count: i32,
        try_keyword: TokenId,
        finally_keyword: Option<TokenId>,
        end_token: TokenId,
    ) {
    }

    fn handle_type(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        question_mark: Option<TokenId>,
    ) {
    }

    /// Called when parser encounters a '!'
    /// used as a non-null postfix assertion in an expression.
    fn handle_non_null_assert_expression(&mut self, tokens: &mut Tokens, bang: TokenId) {}

    /// Called after the parser has consumed a null-assert pattern, consisting of
    /// a pattern followed by a `!` operator.
    fn handle_null_assert_pattern(&mut self, tokens: &mut Tokens, bang: TokenId) {}

    /// Called after the parser has consumed a null-check pattern, consisting of a
    /// pattern followed by a `?` operator.
    fn handle_null_check_pattern(&mut self, tokens: &mut Tokens, question: TokenId) {}

    /// Called after the parser has consumed an assigned variable pattern,
    /// consisting of a variable name identifier (other than `_`).
    ///
    /// This method will only be called for a variable pattern that is part of a
    /// `patternAssignment` (and hence should refer to a previously declared
    /// variable rather than declaring a fresh one).
    fn handle_assigned_variable_pattern(&mut self, tokens: &mut Tokens, variable: TokenId) {}

    /// Called after the parser has consumed a declared variable pattern,
    /// consisting of an optional `var` or `final` keyword, an optional type
    /// annotation, and a variable name identifier (other than `_`).
    ///
    /// The flag `inAssignmentPattern` indicates whether this variable pattern is
    /// part of a `patternAssignment`.  If this is `true`, it indicates that the
    /// parser has recovered from an error (since declared variable patterns are
    /// not allowed inside a `patternAssignment`).  The error has already been
    /// reported.
    fn handle_declared_variable_pattern(
        &mut self,
        tokens: &mut Tokens,
        keyword: Option<TokenId>,
        variable: TokenId,
        in_assignment_pattern: bool,
    ) {
    }

    /// Called after the parser has consumed a wildcard pattern, consisting of an
    /// optional `var` or `final` keyword, an optional type annotation, and the
    /// identifier `_`.
    fn handle_wildcard_pattern(
        &mut self,
        tokens: &mut Tokens,
        keyword: Option<TokenId>,
        wildcard: TokenId,
    ) {
    }

    fn handle_no_name(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn begin_record_type(&mut self, tokens: &mut Tokens, left_bracket: TokenId) {}

    /// Handle the end of a record type declaration.
    ///
    /// Substructures:
    /// - RecordTypeEntry*
    /// - RecordTypeNamedFields?
    ///
    /// Notice that `count` is:
    /// - the number of RecordTypeEntries if `hasNamedFields` is `false`, or
    /// - the number of RecordTypeEntries + 1 if `hasNamedFields` is `true`.
    fn end_record_type(
        &mut self,
        tokens: &mut Tokens,
        left_bracket: TokenId,
        question_mark: Option<TokenId>,
        count: i32,
        has_named_fields: bool,
    ) {
    }

    fn begin_record_type_entry(&mut self, tokens: &mut Tokens) {}

    /// Handle the end of the record type entries.
    ///
    /// Substructures:
    /// - metadata
    /// - type
    /// - identifier
    fn end_record_type_entry(&mut self, tokens: &mut Tokens) {}

    fn begin_record_type_named_fields(&mut self, tokens: &mut Tokens, left_bracket: TokenId) {}

    /// Handle the end of the record type named fields.
    ///
    /// Substructures:
    /// - RecordTypeEntry*
    fn end_record_type_named_fields(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        left_bracket: TokenId,
    ) {
    }

    fn begin_function_type(&mut self, tokens: &mut Tokens, begin_token: TokenId) {}

    /// Handle the end of a generic function type declaration.
    ///
    /// Substructures:
    /// - Type variables
    /// - Return type
    /// - Formal parameters
    fn end_function_type(
        &mut self,
        tokens: &mut Tokens,
        function_token: TokenId,
        question_mark: Option<TokenId>,
    ) {
    }

    fn begin_type_arguments(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_type_arguments(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
    }

    /// After endTypeArguments has been called,
    /// this event is called if those type arguments are invalid.
    fn handle_invalid_type_arguments(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn handle_no_type_arguments(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Handle the begin of a type formal parameter (e.g. "X extends Y").
    /// Substructures:
    /// - Metadata
    /// - Name (identifier)
    fn begin_type_variable(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Called when `beginTypeVariable` has been called for all of the variables
    /// in a group, and before `endTypeVariable` has been called for any of the
    /// variables in that same group.
    fn handle_type_variables_defined(&mut self, tokens: &mut Tokens, token: TokenId, count: i32) {}

    /// Handle the end of a type formal parameter (e.g. "X extends Y")
    /// where `index` is the index of the type variable in the list of
    /// type variables being declared.
    ///
    /// Substructures:
    /// - Type bound
    ///
    /// See `beginTypeVariable` for additional substructures.
    fn end_type_variable(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        index: i32,
        extends_or_super: Option<TokenId>,
        variance: Option<TokenId>,
    ) {
    }

    fn begin_type_variables(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_type_variables(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
    }

    fn report_variance_modifier_not_enabled(
        &mut self,
        tokens: &mut Tokens,
        variance: Option<TokenId>,
    ) {
        if let Some(variance) = variance {
            self.handle_experiment_not_enabled(
                tokens,
                ExperimentalFlag::Variance,
                variance,
                variance,
            );
        }
    }

    fn begin_function_expression(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Handle the end of a function expression (e.g. "() { ... }").
    /// Substructures:
    /// - Type variables
    /// - Formal parameters
    /// - Async marker
    /// - Body
    fn end_function_expression(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
    }

    /// Handle the start of a variables declaration.  Substructures:
    /// - Metadata
    /// - Type
    fn begin_variables_declaration(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        late_token: Option<TokenId>,
        var_final_or_const: Option<TokenId>,
    ) {
    }

    fn end_variables_declaration(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        end_token: Option<TokenId>,
    ) {
    }

    fn begin_while_statement(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_while_statement(
        &mut self,
        tokens: &mut Tokens,
        while_keyword: TokenId,
        end_token: TokenId,
    ) {
    }

    fn begin_as_operator_type(&mut self, tokens: &mut Tokens, operator: TokenId) {}

    fn end_as_operator_type(&mut self, tokens: &mut Tokens, operator: TokenId) {}

    fn handle_as_operator(&mut self, tokens: &mut Tokens, operator: TokenId) {}

    /// Called after the parser has consumed a cast pattern, consisting of a
    /// pattern, `as` operator, and type annotation.
    fn handle_cast_pattern(&mut self, tokens: &mut Tokens, operator: TokenId) {}

    fn handle_assignment_expression(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        end_token: TokenId,
    ) {
    }

    fn begin_anonymous_method_invocation(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Called after the parser has consumed an anonymous method invocation.
    /// Substructures:
    /// - The target of the invocation.
    /// - A formal parameter list (can be implicit, see
    ///   `handleImplicitFormalParameters`).
    /// - The body of the anonymous method (either an expression or a block).
    fn end_anonymous_method_invocation(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        function_definition: Option<TokenId>,
        end_token: TokenId,
        is_expression: bool,
    ) {
    }

    /// Called when an anonymous method invocation does not have
    /// an explicit formal parameter list.
    fn handle_implicit_formal_parameters(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Called when the parser encounters a binary operator, in between the LHS
    /// and RHS subexpressions.
    ///
    /// Not called when the binary operator is `.`, `?.`, or `..`.
    fn begin_binary_expression(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_binary_expression(&mut self, tokens: &mut Tokens, token: TokenId, end_token: TokenId) {}

    /// Called when the parser has consumed the operator of a binary pattern.
    fn begin_binary_pattern(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Called when the parser has consumed a binary pattern, consisting of a LHS
    /// pattern, `&&` or `||` operator, and a RHS pattern.
    fn end_binary_pattern(&mut self, tokens: &mut Tokens, operator_token: TokenId) {}

    /// Called for property access through `.` and `?.`.
    ///
    /// `isNullAware` is `true` if the access uses `?.`.
    fn handle_dot_access(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        end_token: TokenId,
        is_null_aware: bool,
    ) {
    }

    /// Called for cascade access through `..` and `?..`.
    ///
    /// `isNullAware` is `true` if the access uses `?..`.
    fn handle_cascade_access(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        end_token: TokenId,
        is_null_aware: bool,
    ) {
    }

    /// Called when the parser encounters a `?` operator and begins parsing a
    /// conditional expression.
    fn begin_conditional_expression(&mut self, tokens: &mut Tokens, question: TokenId) {}

    /// Called when the parser encounters a `:` operator in a conditional
    /// expression.
    fn handle_conditional_expression_colon(&mut self, tokens: &mut Tokens) {}

    /// Called when the parser finishes processing a conditional expression.
    fn end_conditional_expression(
        &mut self,
        tokens: &mut Tokens,
        question: TokenId,
        colon: TokenId,
        end_token: TokenId,
    ) {
    }

    fn begin_const_expression(&mut self, tokens: &mut Tokens, const_keyword: TokenId) {}

    fn end_const_expression(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn handle_const_factory(&mut self, tokens: &mut Tokens, const_keyword: TokenId) {}

    /// Called before parsing a "for" control flow list, set, or map entry.
    /// Ended by either `endForControlFlow` or `endForInControlFlow`.
    fn begin_for_control_flow(
        &mut self,
        tokens: &mut Tokens,
        await_token: Option<TokenId>,
        for_token: TokenId,
    ) {
    }

    /// Called after parsing a "for" control flow list, set, or map entry.
    /// One of the two possible corresponding end events for
    /// `beginForControlFlow`.
    fn end_for_control_flow(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Called after parsing a "for-in" control flow list, set, or map entry.
    /// One of the two possible corresponding end events for
    /// `beginForControlFlow`.
    fn end_for_in_control_flow(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Called before parsing an `if` control flow list, set, or map entry.
    /// Ended by either `endIfControlFlow` or `endIfElseControlFlow`.
    fn begin_if_control_flow(&mut self, tokens: &mut Tokens, if_token: TokenId) {}

    /// Called before parsing the `then` portion of an `if` control flow list,
    /// set, or map entry.
    fn handle_then_control_flow(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Called before parsing the `else` portion of an `if` control flow list,
    /// set, or map entry.
    fn handle_else_control_flow(&mut self, tokens: &mut Tokens, else_token: TokenId) {}

    /// Called after parsing an `if` control flow list, set, or map entry.
    /// Substructures:
    /// - if conditional expression
    /// - expression
    /// One of the two possible corresponding end events for
    /// `beginIfControlFlow`.
    fn end_if_control_flow(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Called after parsing an if-else control flow list, set, or map entry.
    /// Substructures:
    /// - if conditional expression
    /// - then expression
    /// - else expression
    /// One of the two possible corresponding end events for
    /// `beginIfControlFlow`.
    fn end_if_else_control_flow(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Called after parsing a list, set, or map entry that starts with
    /// one of the spread collection tokens `...` or `...?`.  Substructures:
    /// - expression
    fn handle_spread_expression(&mut self, tokens: &mut Tokens, spread_token: TokenId) {}

    /// Called after parsing a list or set element that starts with the null-aware
    /// token `?`. Substructures:
    /// - expression
    fn handle_null_aware_element(&mut self, tokens: &mut Tokens, null_aware_token: TokenId) {}

    /// Called after parsing an element of a list or map pattern that starts with
    /// `...`.  Substructures:
    /// - pattern (if hasSubPattern is `true`)
    fn handle_rest_pattern(&mut self, tokens: &mut Tokens, dots: TokenId, has_sub_pattern: bool) {}

    /// Handle the start of a function typed formal parameter.  Substructures:
    /// - type variables
    fn begin_function_typed_formal_parameter(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Handle the end of a function typed formal parameter.  Substructures:
    /// - type variables
    /// - return type
    /// - formal parameters
    fn end_function_typed_formal_parameter(
        &mut self,
        tokens: &mut Tokens,
        name_token: TokenId,
        question: Option<TokenId>,
    ) {
    }

    /// Handle an identifier token.
    ///
    /// `context` indicates what kind of construct the identifier appears in.
    fn handle_identifier(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        context: IdentifierContext,
    ) {
    }

    fn handle_indexed_expression(
        &mut self,
        tokens: &mut Tokens,
        question: Option<TokenId>,
        open_square_bracket: TokenId,
        close_square_bracket: TokenId,
    ) {
    }

    fn begin_is_operator_type(&mut self, tokens: &mut Tokens, operator: TokenId) {}

    fn end_is_operator_type(&mut self, tokens: &mut Tokens, operator: TokenId) {}

    fn handle_is_operator(
        &mut self,
        tokens: &mut Tokens,
        is_operator: TokenId,
        not: Option<TokenId>,
    ) {
    }

    fn handle_literal_bool(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn handle_break_statement(
        &mut self,
        tokens: &mut Tokens,
        has_target: bool,
        break_keyword: TokenId,
        end_token: TokenId,
    ) {
    }

    fn handle_continue_statement(
        &mut self,
        tokens: &mut Tokens,
        has_target: bool,
        continue_keyword: TokenId,
        end_token: TokenId,
    ) {
    }

    fn handle_empty_statement(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn begin_assert(&mut self, tokens: &mut Tokens, assert_keyword: TokenId, kind: Assert) {}

    fn end_assert(
        &mut self,
        tokens: &mut Tokens,
        assert_keyword: TokenId,
        kind: Assert,
        left_parenthesis: TokenId,
        comma_token: Option<TokenId>,
        end_token: TokenId,
    ) {
    }

    /// Called with either the token containing a double literal, or an
    /// immediately preceding "unary minus" token.
    fn handle_literal_double(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Called with either the token containing a double literal with separators,
    /// or an immediately preceding "unary minus" token.
    fn handle_literal_double_with_separators(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Called with either the token containing an integer literal, or an
    /// immediately preceding "unary minus" token.
    fn handle_literal_int(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Called with either the token containing an integer literal with
    /// separators, or an immediately preceding "unary minus" token.
    fn handle_literal_int_with_separators(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn handle_literal_list(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        left_bracket: TokenId,
        const_keyword: Option<TokenId>,
        right_bracket: TokenId,
    ) {
    }

    /// Called after the parser has consumed a list pattern, consisting of a `[`,
    /// a comma-separated sequence of patterns, and a `]`.
    fn handle_list_pattern(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        left_bracket: TokenId,
        right_bracket: TokenId,
    ) {
    }

    fn handle_literal_set_or_map(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        left_brace: TokenId,
        const_keyword: Option<TokenId>,
        right_brace: TokenId,
        has_set_entry: bool,
    ) {
    }

    /// Called after the parser has consumed a map pattern, consisting of a `{`,
    /// a comma-separated sequence of mapPatternEntry, and a `}`.
    fn handle_map_pattern(
        &mut self,
        tokens: &mut Tokens,
        count: i32,
        left_brace: TokenId,
        right_brace: TokenId,
    ) {
    }

    fn handle_literal_null(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn handle_native_clause(&mut self, tokens: &mut Tokens, native_token: TokenId, has_name: bool) {
    }

    fn handle_named_argument(&mut self, tokens: &mut Tokens, colon: TokenId) {}

    fn handle_positional_argument(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Called after the parser has consumed a patternField, consisting of an
    /// optional identifier, optional `:`, and a pattern.
    fn handle_pattern_field(&mut self, tokens: &mut Tokens, colon: Option<TokenId>) {}

    fn handle_named_record_field(&mut self, tokens: &mut Tokens, colon: TokenId) {}

    fn handle_positional_record_field(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn begin_new_expression(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_new_expression(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn handle_no_arguments(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn handle_no_constructor_reference_continuation_after_type_arguments(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
    ) {
    }

    fn handle_no_identifier(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        identifier_context: IdentifierContext,
    ) {
    }

    fn handle_no_type_name_in_constructor_reference(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
    ) {
    }

    fn handle_no_type(&mut self, tokens: &mut Tokens, last_consumed: TokenId) {}

    fn handle_no_type_variables(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn handle_operator(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Invoked when a pattern switch case doesn't have the 'when' clause
    fn handle_switch_case_no_when_clause(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn handle_switch_expression_case_pattern(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn handle_symbol_void(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Handle the end of a construct of the form "operator <token>".
    fn handle_operator_name(
        &mut self,
        tokens: &mut Tokens,
        operator_keyword: TokenId,
        token: TokenId,
    ) {
    }

    /// Handle the end of a construct of the form "operator <token>"
    /// where <token> is not a valid operator token.
    fn handle_invalid_operator_name(
        &mut self,
        tokens: &mut Tokens,
        operator_keyword: TokenId,
        token: TokenId,
    ) {
    }

    /// Handle the condition in a control structure:
    /// - if statement
    /// - do while loop
    /// - switch statement
    /// - while loop
    fn handle_parenthesized_condition(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        case_: Option<TokenId>,
        when: Option<TokenId>,
    ) {
    }

    /// Starts a pattern
    fn begin_pattern(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Starts a pattern guard, the expression that follows the 'when' keyword
    fn begin_pattern_guard(&mut self, tokens: &mut Tokens, when: TokenId) {}

    /// Starts a parenthesized expression or a record literal. Will be ended with
    /// either `endParenthesizedExpression` or `endRecordLiteral`.
    fn begin_parenthesized_expression_or_record_literal(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
    ) {
    }

    /// Starts a guard expression in a switch case, after the 'when' keyword
    fn begin_switch_case_when_clause(&mut self, tokens: &mut Tokens, when: TokenId) {}

    /// Ends a record literal with `count` entries.
    fn end_record_literal(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        count: i32,
        const_keyword: Option<TokenId>,
    ) {
    }

    /// Called after the parser has consumed a record pattern, consisting of a
    /// `(`, a comma-separated sequence of patternFields, and a `)`.
    fn handle_record_pattern(&mut self, tokens: &mut Tokens, token: TokenId, count: i32) {}

    /// Ends a pattern
    fn end_pattern(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// End a pattern guard, the expression that follows the 'when' keyword
    fn end_pattern_guard(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// End a parenthesized expression.
    /// These may be within the condition expression of a control structure
    /// but will not be the condition of a control structure.
    fn end_parenthesized_expression(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Starts a guard expression in a switch case, after the 'when' keyword
    fn end_switch_case_when_clause(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Called after the parser has consumed a parenthesized pattern, consisting
    /// of a `(`, a pattern, and a `)`.
    fn handle_parenthesized_pattern(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Called after the parser has consumed a constant pattern, consisting of an
    /// optional `const` and an expression.
    ///
    /// Note that some expressions can legally begin with `const`, so there is
    /// ambiguity as to whether to associate the `const` keyword with the constant
    /// pattern or the constant expression.  This ambiguity is resolved in favor
    /// of associating the `const` keyword with the constant pattern.  So for
    /// example, in `case const []` the `const` keyword is passed to
    /// `beginConstantPattern` and `endConstantPattern` rather than
    /// `handleLiteralList`.
    fn begin_constant_pattern(&mut self, tokens: &mut Tokens, const_keyword: Option<TokenId>) {}

    /// Called after the parser has consumed a constant pattern, consisting of an
    /// optional `const` and an expression.
    ///
    /// Note that some expressions can legally begin with `const`, so there is
    /// ambiguity as to whether to associate the `const` keyword with the constant
    /// pattern or the constant expression.  This ambiguity is resolved in favor
    /// of associating the `const` keyword with the constant pattern.  So for
    /// example, in `case const []` the `const` keyword is passed to
    /// `beginConstantPattern` and `endConstantPattern` rather than
    /// `handleLiteralList`.
    fn end_constant_pattern(&mut self, tokens: &mut Tokens, const_keyword: Option<TokenId>) {}

    /// Called after the parser has consumed an object pattern, consisting of
    /// an identifier, optional dot and second identifier, optional type
    /// arguments, and a parenthesized list of object pattern fields (see
    /// `handleObjectPatternFields`).
    fn handle_object_pattern(
        &mut self,
        tokens: &mut Tokens,
        first_identifier: TokenId,
        dot: Option<TokenId>,
        second_identifier: Option<TokenId>,
    ) {
    }

    /// Handle a construct of the form "identifier.identifier" occurring in a part
    /// of the grammar where expressions in general are not allowed.
    /// Substructures:
    /// - Qualified identifier (before the period)
    /// - Identifier (after the period)
    fn handle_qualified(&mut self, tokens: &mut Tokens, period: TokenId) {}

    fn handle_string_part(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn handle_super_expression(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        context: IdentifierContext,
    ) {
    }

    fn begin_switch_case(
        &mut self,
        tokens: &mut Tokens,
        label_count: i32,
        expression_count: i32,
        begin_token: TokenId,
    ) {
    }

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
    }

    fn begin_switch_expression_case(&mut self, tokens: &mut Tokens) {}

    fn end_switch_expression_case(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        when: Option<TokenId>,
        arrow: TokenId,
        end_token: TokenId,
    ) {
    }

    fn handle_this_expression(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        context: IdentifierContext,
    ) {
    }

    fn handle_unary_postfix_assignment_expression(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn handle_unary_prefix_expression(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Called after the parser has consumed a relational pattern, consisting of
    /// an equality operator or relational operator, followed by an expression.
    fn handle_relational_pattern(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn handle_unary_prefix_assignment_expression(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn begin_formal_parameter_default_value_expression(&mut self, tokens: &mut Tokens) {}

    fn end_formal_parameter_default_value_expression(&mut self, tokens: &mut Tokens) {}

    fn handle_valued_formal_parameter(
        &mut self,
        tokens: &mut Tokens,
        equals: TokenId,
        token: TokenId,
        kind: FormalParameterKind,
    ) {
    }

    fn handle_formal_parameter_without_value(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn handle_void_keyword(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// The parser saw a void with type arguments (e.g. void<int>).
    /// This is not valid - an error has already been emitted.
    fn handle_void_keyword_with_type_arguments(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Ended by either `endYieldStatement` or `endInvalidYieldStatement`.
    fn begin_yield_statement(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// One of the two possible corresponding end events for
    /// `beginYieldStatement`.
    fn end_yield_statement(
        &mut self,
        tokens: &mut Tokens,
        yield_token: TokenId,
        star_token: Option<TokenId>,
        end_token: TokenId,
    ) {
    }

    /// One of the two possible corresponding end events for
    /// `beginYieldStatement`.
    fn end_invalid_yield_statement(
        &mut self,
        tokens: &mut Tokens,
        begin_token: TokenId,
        star_token: Option<TokenId>,
        end_token: TokenId,
        error_code: &'static CfeCode,
    ) {
    }

    /// The parser noticed a syntax error, but was able to recover from it. The
    /// error should be reported using the `message`, and the code between the
    /// beginning of the `startToken` and the end of the `endToken` should be
    /// highlighted. The `startToken` and `endToken` can be the same token.
    fn handle_recoverable_error(
        &mut self,
        tokens: &mut Tokens,
        message: CfeMessage,
        start_token: TokenId,
        end_token: TokenId,
    ) {
    }

    /// The parser noticed a use of the experimental feature by the flag
    /// `experimentalFlag` that was not enabled, but was able to recover from it.
    /// The error should be reported and the code between the beginning of the
    /// `beginToken` and the end of the `endToken` should be highlighted. The
    /// `beginToken` and `endToken` can be the same token.
    fn handle_experiment_not_enabled(
        &mut self,
        tokens: &mut Tokens,
        experimental_flag: ExperimentalFlag,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
        self.handle_recoverable_error(
            tokens,
            get_experiment_not_enabled_message(experimental_flag),
            begin_token,
            end_token,
        );
    }

    /// The parser encountered an `ErrorToken` representing an error
    /// from the scanner but recovered from it. By default, the error is reported
    /// by calling `handleRecoverableError` with the message associated
    /// with the error `token`.
    fn handle_error_token(&mut self, tokens: &mut Tokens, token: TokenId) {
        let message = error_token_assertion_message(tokens, token);
        self.handle_recoverable_error(tokens, message, token, token);
    }

    fn handle_unescape_error(
        &mut self,
        tokens: &mut Tokens,
        message: CfeMessage,
        location: TokenId,
        string_offset: i32,
        length: i32,
    ) {
        let _ = (string_offset, length);
        self.handle_recoverable_error(tokens, message, location, location);
    }

    /// Signals to the listener that the previous statement contained a semantic
    /// error (described by the given `message`). This method can also be called
    /// after `handleExpressionFunctionBody`, in which case it signals that the
    /// implicit return statement of the function contained a semantic error.
    fn handle_invalid_statement(
        &mut self,
        tokens: &mut Tokens,
        token: TokenId,
        message: CfeMessage,
    ) {
        self.handle_recoverable_error(tokens, message, token, token);
    }

    fn handle_script(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// An expression was encountered consisting of type arguments applied to a
    /// subexpression.  This could validly represent any of the following:
    /// - A type literal (`var x = List<int>;`)
    /// - A function tear-off with type arguments (`var x = f<int>;` or
    ///   `var x = importPrefix.f<int>;`)
    /// - A static method tear-off with type arguments (`var x = ClassName.m<int>`
    ///   or `var x = importPrefix.ClassName.m<int>;`)
    /// - An instance method tear-off with type arguments (`var x = EXPR.m<int>;`)
    ///
    /// Or, in the event of invalid code, it could represent type arguments
    /// erroneously applied to some other expression type (e.g.
    /// `var x = (f)<int>;`).  The client is responsible for reporting an error if
    /// this occurs.
    fn handle_type_argument_application(
        &mut self,
        tokens: &mut Tokens,
        open_angle_bracket: TokenId,
    ) {
    }

    /// A `new` token was found in a place where an identifier was expected, and
    /// the "constructor tearoffs" feature permits `new` to be used as an
    /// identifier name.  It is the client's responsibility to report an
    /// appropriate error if the "constructor tearoffs" feature is not enabled.
    fn handle_new_as_identifier(&mut self, tokens: &mut Tokens, token: TokenId) {}

    /// Called after the parser has processed a variable declaration statement,
    /// consisting of `METADATA KEYWORD PATTERN EQUALS EXPRESSION SEMICOLON`.
    ///
    /// KEYWORD is either `var` or `final`, and PATTERN may only be one of the
    /// patterns accepted by the `outerPattern` grammar rule defined in the
    /// patterns spec.
    fn handle_pattern_variable_declaration_statement(
        &mut self,
        tokens: &mut Tokens,
        keyword: TokenId,
        equals: TokenId,
        semicolon: TokenId,
    ) {
    }

    /// Called after the parser has processed a pattern assignment consisting of
    /// `PATTERN EQUALS EXPRESSION`.
    ///
    /// PATTERN may only be one of the patterns accepted by the `outerPattern`
    /// grammar rule defined in the patterns spec.
    fn handle_pattern_assignment(&mut self, tokens: &mut Tokens, equals: TokenId) {}

    fn handle_dot_shorthand_context(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn handle_dot_shorthand_head(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn begin_const_dot_shorthand(&mut self, tokens: &mut Tokens, token: TokenId) {}

    fn end_const_dot_shorthand(&mut self, tokens: &mut Tokens, token: TokenId) {}
}
