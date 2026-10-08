// Dart source: pkg/analyzer/lib/src/fasta/ast_builder.dart (lines 204-1693)

use dartr_ast::{
    AnonymousBlockBody, AnonymousExpressionBody, AnonymousMethodBody, AnonymousMethodInvocation,
    Argument, ArgumentList, AssertInitializer, AssertStatement, AwaitExpression, BinaryExpression,
    Block, BlockFunctionBody, CascadeExpression, Combinator, CompilationUnit,
    ConditionalExpression, Configuration, ConstantPattern, ConstructorName, DartPattern,
    DoStatement, DotShorthandConstructorInvocation, DotShorthandInvocation,
    DotShorthandPropertyAccess, DottedName, EmptyFunctionBody, Entity, ExportDirective, Expression,
    ExtensionOnClause, FormalParameterList, FunctionExpressionInvocation, GuardedPattern, Id,
    Identifier, ImplementsClause, InterpolationExpression, LogicalAndPattern, LogicalOrPattern,
    NamedArgument, NodeList, SimpleIdentifier, Statement, StringInterpolation, StringLiteral,
    SwitchCase, SwitchPatternCase, TypeAnnotation, TypeArgumentList, TypeParameter,
    TypeParameterList, VariableDeclaration, WhenClause,
};
use dartr_diagnostics::cfe_codes;
use dartr_parser::assert::Assert;
use dartr_parser::block_kind::BlockKind;
use dartr_parser::constructor_reference_context::ConstructorReferenceContext;
use dartr_parser::declaration_kind::DeclarationKind;
use dartr_parser::experimental_features::ExperimentalFlag;
use dartr_parser::member_kind::MemberKind;
use dartr_syntax::{TokenId, TokenType};

use super::{
    AstBuilder, ClassLikeDeclarationBuilder, ClassLikeKind, ExtensionDeclarationBuilder,
    ExtensionTypeDeclarationBuilder, Modifiers, ParenthesizedCondition, PrimaryConstructorBuilder,
};
use crate::stack::{NullValue, Value};

impl AstBuilder {
    pub(crate) fn begin_as_operator_type(&mut self, _operator: TokenId) {}

    pub(crate) fn begin_cascade(&mut self, token: TokenId) {
        let expression = self.pop_node::<Expression>();
        self.push(token);
        if self.ast.is::<CascadeExpression>(expression) {
            self.push(expression);
        } else {
            let node = self.ast.add(CascadeExpression {
                target: expression,
                cascade_sections: NodeList::EMPTY,
            });
            self.push(node);
        }
        self.push(NullValue::CascadeReceiver);
    }

    pub(crate) fn begin_class_declaration(
        &mut self,
        _begin: TokenId,
        abstract_token: Option<TokenId>,
        mut sealed_token: Option<TokenId>,
        mut base_token: Option<TokenId>,
        mut interface_token: Option<TokenId>,
        mut final_token: Option<TokenId>,
        augment_token: Option<TokenId>,
        mut mixin_token: Option<TokenId>,
        _name: TokenId,
    ) {
        debug_assert!(self.class_like_builder.is_none());
        self.push(Modifiers {
            abstract_keyword: abstract_token,
            ..Modifiers::default()
        });
        self.check_class_modifiers(
            &mut sealed_token,
            &mut base_token,
            &mut interface_token,
            &mut final_token,
            &mut mixin_token,
        );
        self.push_token_or(sealed_token, NullValue::Token);
        self.push_token_or(base_token, NullValue::Token);
        self.push_token_or(interface_token, NullValue::Token);
        self.push_token_or(final_token, NullValue::Token);
        self.push_token_or(augment_token, NullValue::Token);
        self.push_token_or(mixin_token, NullValue::Token);
    }

    /// The feature checks of `beginClassDeclaration` and
    /// `beginNamedMixinApplication` (same code in Dart).
    fn check_class_modifiers(
        &mut self,
        sealed_token: &mut Option<TokenId>,
        base_token: &mut Option<TokenId>,
        interface_token: &mut Option<TokenId>,
        final_token: &mut Option<TokenId>,
        mixin_token: &mut Option<TokenId>,
    ) {
        if !self.enable_sealed_class {
            if let Some(t) = *sealed_token {
                self.report_feature_not_enabled(ExperimentalFlag::SealedClass, t, None);
                // Pretend that 'sealed' didn't occur while this feature is incomplete.
                *sealed_token = None;
            }
        }
        if !self.enable_class_modifiers {
            for token in [base_token, interface_token, final_token, mixin_token] {
                if let Some(t) = *token {
                    self.report_feature_not_enabled(ExperimentalFlag::ClassModifiers, t, None);
                    // Pretend that the modifier didn't occur while this
                    // feature is incomplete.
                    *token = None;
                }
            }
        }
    }

    pub(crate) fn begin_compilation_unit(&mut self, token: TokenId) {
        self.push(token);
    }

    pub(crate) fn begin_constant_pattern(&mut self, _const_keyword: Option<TokenId>) {}

    pub(crate) fn begin_constructor(
        &mut self,
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
        self.begin_method_impl(
            declaration_kind,
            augment_token,
            external_token,
            static_token,
            covariant_token,
            var_final_or_const,
            new_token,
            get_or_set,
            name,
            enclosing_declaration_name.map(str::to_owned),
        );
    }

    pub(crate) fn begin_enum_declaration(
        &mut self,
        _begin_token: TokenId,
        _augment_token: Option<TokenId>,
        _enum_keyword: TokenId,
        _name: TokenId,
    ) {
    }

    pub(crate) fn begin_enum_declaration_prelude(&mut self, _enum_keyword: TokenId) {}

    pub(crate) fn begin_extension_declaration(
        &mut self,
        augment_token: Option<TokenId>,
        extension_keyword: TokenId,
        name: Option<TokenId>,
    ) {
        debug_assert!(self.class_like_builder.is_none());
        let type_parameters = self.pop_node_opt::<TypeParameterList>();
        let metadata = self.pop_metadata();
        let comment = self.find_comment(metadata.as_deref(), extension_keyword);
        let left_bracket = self.detached_token(TokenType::OPEN_CURLY_BRACKET);
        let right_bracket = self.detached_token(TokenType::CLOSE_CURLY_BRACKET);
        self.class_like_builder = Some(Box::new(ClassLikeDeclarationBuilder {
            comment,
            metadata,
            type_parameters,
            empty_class_body_semicolon: None,
            left_bracket,
            members: Vec::new(),
            right_bracket,
            kind: ClassLikeKind::Extension(ExtensionDeclarationBuilder {
                augment_keyword: augment_token,
                extension_keyword,
                name,
            }),
        }));
    }

    pub(crate) fn begin_extension_type_declaration(
        &mut self,
        augment_keyword: Option<TokenId>,
        extension_keyword: TokenId,
        name: TokenId,
    ) {
        debug_assert!(self.class_like_builder.is_none());
        let type_parameters = self.pop_node_opt::<TypeParameterList>();
        let metadata = self.pop_metadata();
        let comment = self.find_comment(metadata.as_deref(), extension_keyword);
        let left_bracket = self.detached_token(TokenType::OPEN_CURLY_BRACKET);
        let right_bracket = self.detached_token(TokenType::CLOSE_CURLY_BRACKET);
        self.class_like_builder = Some(Box::new(ClassLikeDeclarationBuilder {
            comment,
            metadata,
            type_parameters,
            empty_class_body_semicolon: None,
            left_bracket,
            members: Vec::new(),
            right_bracket,
            kind: ClassLikeKind::ExtensionType(ExtensionTypeDeclarationBuilder {
                augment_keyword,
                extension_keyword,
                name,
            }),
        }));
    }

    pub(crate) fn begin_factory(
        &mut self,
        _declaration_kind: DeclarationKind,
        _last_consumed: TokenId,
        augment_token: Option<TokenId>,
        external_token: Option<TokenId>,
        const_token: Option<TokenId>,
    ) {
        self.push(Modifiers {
            augment_keyword: augment_token,
            external_keyword: external_token,
            final_const_or_var_keyword: const_token,
            ..Modifiers::default()
        });
    }

    pub(crate) fn begin_formal_parameter(
        &mut self,
        _token: TokenId,
        _kind: MemberKind,
        required_token: Option<TokenId>,
        covariant_token: Option<TokenId>,
        var_final_or_const: Option<TokenId>,
    ) {
        self.push(Modifiers {
            covariant_keyword: covariant_token,
            final_const_or_var_keyword: var_final_or_const,
            required_token,
            ..Modifiers::default()
        });
    }

    pub(crate) fn begin_formal_parameter_default_value_expression(&mut self) {}

    pub(crate) fn begin_if_control_flow(&mut self, if_token: TokenId) {
        self.push(if_token);
    }

    pub(crate) fn begin_is_operator_type(&mut self, _operator: TokenId) {}

    pub(crate) fn begin_library_augmentation(
        &mut self,
        _augment_keyword: TokenId,
        _library_keyword: TokenId,
    ) {
    }

    pub(crate) fn begin_literal_string(&mut self, token: TokenId) {
        self.push(token);
    }

    pub(crate) fn begin_metadata_star(&mut self, _token: TokenId) {}

    pub(crate) fn begin_method(
        &mut self,
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
        self.begin_method_impl(
            declaration_kind,
            augment_token,
            external_token,
            static_token,
            covariant_token,
            var_final_or_const,
            None,
            get_or_set,
            name,
            enclosing_declaration_name.map(str::to_owned),
        );
    }

    pub(crate) fn begin_mixin_declaration(
        &mut self,
        _begin_token: TokenId,
        augment_token: Option<TokenId>,
        mut base_token: Option<TokenId>,
        _mixin_keyword: TokenId,
        _name: TokenId,
    ) {
        debug_assert!(self.class_like_builder.is_none());
        if !self.enable_class_modifiers {
            if let Some(t) = base_token {
                self.report_feature_not_enabled(ExperimentalFlag::ClassModifiers, t, None);
                // Pretend that 'base' didn't occur while this feature is incomplete.
                base_token = None;
            }
        }
        self.push_token_or(augment_token, NullValue::Token);
        self.push_token_or(base_token, NullValue::Token);
    }

    pub(crate) fn begin_named_mixin_application(
        &mut self,
        _begin_token: TokenId,
        abstract_token: Option<TokenId>,
        mut sealed_token: Option<TokenId>,
        mut base_token: Option<TokenId>,
        mut interface_token: Option<TokenId>,
        mut final_token: Option<TokenId>,
        augment_token: Option<TokenId>,
        mut mixin_token: Option<TokenId>,
        _name: TokenId,
    ) {
        self.push(Modifiers {
            abstract_keyword: abstract_token,
            ..Modifiers::default()
        });
        self.check_class_modifiers(
            &mut sealed_token,
            &mut base_token,
            &mut interface_token,
            &mut final_token,
            &mut mixin_token,
        );
        self.push_token_or(sealed_token, NullValue::Token);
        self.push_token_or(base_token, NullValue::Token);
        self.push_token_or(interface_token, NullValue::Token);
        self.push_token_or(final_token, NullValue::Token);
        self.push_token_or(augment_token, NullValue::Token);
        self.push_token_or(mixin_token, NullValue::Token);
    }

    pub(crate) fn begin_pattern(&mut self, _token: TokenId) {}

    pub(crate) fn begin_pattern_guard(&mut self, _when: TokenId) {}

    pub(crate) fn begin_primary_constructor(&mut self, _begin_token: TokenId) {}

    pub(crate) fn begin_primary_constructor_body(&mut self, _token: TokenId) {}

    pub(crate) fn begin_switch_case_when_clause(&mut self, _when: TokenId) {}

    pub(crate) fn begin_top_level_method(
        &mut self,
        _last_consumed: TokenId,
        augment_token: Option<TokenId>,
        external_token: Option<TokenId>,
    ) {
        self.push(Modifiers {
            augment_keyword: augment_token,
            external_keyword: external_token,
            ..Modifiers::default()
        });
    }

    pub(crate) fn begin_type_variable(&mut self, _token: TokenId) {
        let name = self.pop_node::<SimpleIdentifier>();
        let metadata = self.pop_metadata();

        let name_begin = self.begin_token(name);
        let comment = self.find_comment(metadata.as_deref(), name_begin);
        let metadata = self.node_list(metadata);
        let name_token = self.ast[name].token;
        let type_parameter = self.ast.add(TypeParameter {
            documentation_comment: comment,
            metadata,
            variance_keyword: None,
            name: name_token,
            extends_keyword: None,
            bound: None,
        });
        self.push(type_parameter);
    }

    pub(crate) fn begin_variables_declaration(
        &mut self,
        _token: TokenId,
        late_token: Option<TokenId>,
        var_final_or_const: Option<TokenId>,
    ) {
        if var_final_or_const.is_some() || late_token.is_some() {
            self.push(Modifiers {
                final_const_or_var_keyword: var_final_or_const,
                late_token,
                ..Modifiers::default()
            });
        } else {
            self.push(NullValue::Modifiers);
        }
    }

    // Dart lines 639-905 are in mod.rs.

    pub(crate) fn end_anonymous_method_invocation(
        &mut self,
        begin_token: TokenId,
        function_definition: Option<TokenId>,
        _end_token: TokenId,
        is_expression: bool,
    ) {
        let expression_or_block = self.pop();
        let mut formals = self.pop_node_opt::<FormalParameterList>();
        let target = self.pop_node_opt::<Expression>();

        if let Some(f) = formals {
            let list = self.ast[f].clone();
            let parameters = self.ast.list(list.parameters).to_vec();
            if parameters.is_empty()
                || parameters.len() > 1
                || self.parameter_kind(parameters[0]).is_named()
                || self.parameter_kind(parameters[0]).is_optional()
            {
                self.handle_recoverable_error(
                    cfe_codes::anonymous_method_wrong_parameter_list(),
                    list.left_parenthesis,
                    list.right_parenthesis,
                );
                formals = None;
            }
        }

        let method_body: Id<AnonymousMethodBody> = if is_expression {
            let expression = self
                .value_as_node_opt::<Expression>(expression_or_block)
                .expect("ExpressionImpl");
            self.ast
                .add(AnonymousExpressionBody {
                    function_definition: function_definition
                        .expect("Null check operator used on a null value"),
                    expression,
                })
                .upcast()
        } else {
            let block = self
                .value_as_node_opt::<Block>(expression_or_block)
                .expect("BlockImpl");
            self.ast.add(AnonymousBlockBody { block }).upcast()
        };

        let node = self.ast.add(AnonymousMethodInvocation {
            target,
            operator: begin_token,
            parameters: formals,
            body: method_body,
        });
        self.push(node);
    }

    pub(crate) fn end_arguments(&mut self, count: i32, begin_token: TokenId, end_token: TokenId) {
        let left_parenthesis = begin_token;
        let right_parenthesis = end_token;
        let expressions = self.pop_typed_list2::<Argument>(count as usize);
        for &argument in &expressions {
            let expression = self.argument_expression(argument);
            self.report_error_if_super(expression);
        }

        let arguments = self.ast.new_list(expressions.iter().copied());
        let argument_list = self.ast.add(ArgumentList {
            left_parenthesis,
            arguments,
            right_parenthesis,
        });

        if !self.enable_named_arguments_anywhere {
            let mut has_seen_named_argument = false;
            for &expression in &expressions {
                if self.ast.is::<NamedArgument>(expression) {
                    has_seen_named_argument = true;
                } else if has_seen_named_argument {
                    // Positional argument after named argument.
                    let begin = self.begin_token(expression);
                    let end = self.end_token(expression);
                    self.handle_recoverable_error(
                        cfe_codes::positional_after_named_argument(),
                        begin,
                        end,
                    );
                }
            }
        }

        self.push(argument_list);
    }

    /// Dart `ArgumentImpl.argumentExpression`.
    fn argument_expression(&self, argument: Id<Argument>) -> Id<Expression> {
        if let Some(named) = self.ast.cast::<NamedArgument>(argument) {
            self.ast[named].argument_expression
        } else {
            self.ast
                .cast::<Expression>(argument)
                .expect("Argument is an Expression")
        }
    }

    pub(crate) fn end_as_operator_type(&mut self, _operator: TokenId) {}

    pub(crate) fn end_assert(
        &mut self,
        assert_keyword: TokenId,
        kind: Assert,
        left_parenthesis: TokenId,
        comma_token: Option<TokenId>,
        end_token: TokenId,
    ) {
        let comma = comma_token;
        let message = match self.pop_if_not_null(comma) {
            Some(v) => self.value_as_node_opt::<Expression>(v),
            None => None,
        };
        let condition = self.pop_node::<Expression>();
        match kind {
            Assert::Expression => {
                // The parser has already reported an error indicating that
                // assert cannot be used in an expression. Insert a
                // placeholder.
                let mut arguments: Vec<Id<Argument>> = vec![condition.upcast()];
                if let Some(message) = message {
                    arguments.push(message.upcast());
                }
                let function = self.ast.add(SimpleIdentifier {
                    token: assert_keyword,
                });
                let right_parenthesis = self.end_group(left_parenthesis);
                let arguments = self.ast.new_list(arguments);
                let argument_list = self.ast.add(ArgumentList {
                    left_parenthesis,
                    arguments,
                    right_parenthesis,
                });
                let node = self.ast.add(FunctionExpressionInvocation {
                    function: function.upcast(),
                    type_arguments: None,
                    argument_list,
                });
                self.push(node);
            }
            Assert::Initializer => {
                let right_parenthesis = self.end_group(left_parenthesis);
                let node = self.ast.add(AssertInitializer {
                    assert_keyword,
                    left_parenthesis,
                    condition,
                    comma,
                    message,
                    right_parenthesis,
                });
                self.push(node);
            }
            Assert::Statement => {
                let right_parenthesis = self.end_group(left_parenthesis);
                let semicolon = self.next(end_token);
                let node = self.ast.add(AssertStatement {
                    assert_keyword,
                    left_parenthesis,
                    condition,
                    comma,
                    message,
                    right_parenthesis,
                    semicolon,
                });
                self.push(node);
            }
        }
    }

    pub(crate) fn end_await_expression(&mut self, begin_token: TokenId, _end_token: TokenId) {
        let await_keyword = begin_token;
        let expression = self.pop_node::<Expression>();
        self.report_error_if_super(expression);

        let node = self.ast.add(AwaitExpression {
            await_keyword,
            expression,
        });
        self.push(node);
    }

    pub(crate) fn end_binary_expression(&mut self, token: TokenId, _end_token: TokenId) {
        let operator_token = token;
        let right = self.pop_node::<Expression>();
        let left = self.pop_node::<Expression>();
        self.report_error_if_super(right);
        let node = self.ast.add(BinaryExpression {
            left_operand: left,
            operator: operator_token,
            right_operand: right,
        });
        self.push(node);
        if !self.enable_triple_shift && self.ast.tokens.ty(operator_token) == TokenType::GT_GT_GT {
            self.report_feature_not_enabled(ExperimentalFlag::TripleShift, operator_token, None);
        }
    }

    pub(crate) fn end_binary_pattern(&mut self, operator_token: TokenId) {
        let right = self.pop_node::<DartPattern>();
        let left = self.pop_node::<DartPattern>();
        if self.lexeme(operator_token) == "&&" {
            let node = self.ast.add(LogicalAndPattern {
                left_operand: left,
                operator: operator_token,
                right_operand: right,
            });
            self.push(node);
        } else if self.lexeme(operator_token) == "||" {
            let node = self.ast.add(LogicalOrPattern {
                left_operand: left,
                operator: operator_token,
                right_operand: right,
            });
            self.push(node);
        } else {
            panic!(
                "UnimplementedError: operatorToken: {}",
                self.lexeme(operator_token)
            );
        }
    }

    pub(crate) fn end_block(
        &mut self,
        count: i32,
        begin_token: TokenId,
        end_token: TokenId,
        _block_kind: BlockKind,
    ) {
        let statements = self.pop_typed_list2::<Statement>(count as usize);
        let statements = self.ast.new_list(statements);
        let node = self.ast.add(Block {
            left_bracket: begin_token,
            statements,
            right_bracket: end_token,
        });
        self.push(node);
    }

    pub(crate) fn end_block_function_body(
        &mut self,
        count: i32,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
        let left_bracket = begin_token;
        let statements = self.pop_typed_list2::<Statement>(count as usize);
        let statements = self.ast.new_list(statements);
        let block = self.ast.add(Block {
            left_bracket,
            statements,
            right_bracket: end_token,
        });
        let star = self.pop_token_opt();
        let async_keyword = self.pop_token_opt();
        if self.parse_function_bodies {
            let node = self.ast.add(BlockFunctionBody {
                keyword: async_keyword,
                star,
                block,
            });
            self.push(node);
        } else {
            // TODO(danrubel): Skip the block rather than parsing it.
            let offset = self.ast.tokens.offset(left_bracket);
            let byte = self.ast.tokens.byte_offset(left_bracket);
            let semicolon = self
                .ast
                .tokens
                .push_synthetic(TokenType::SEMICOLON, offset, byte);
            let node = self.ast.add(EmptyFunctionBody { semicolon });
            self.push(node);
        }
    }

    pub(crate) fn end_cascade(&mut self) {
        let expression = self.pop_node::<Expression>();
        let cascade = self.pop_node::<CascadeExpression>();
        self.pop(); // Token.
        let c = self.ast[cascade].clone();
        let mut sections = self.ast.list(c.cascade_sections).to_vec();
        sections.push(expression);
        let cascade_sections = self.ast.new_list(sections);
        let node = self.ast.add(CascadeExpression {
            target: c.target,
            cascade_sections,
        });
        self.push(node);
    }

    pub(crate) fn end_case_expression(
        &mut self,
        case_keyword: TokenId,
        when: Option<TokenId>,
        colon: TokenId,
    ) {
        let mut when_clause = None;
        if let Some(when) = when {
            let expression = self.pop_node::<Expression>();
            when_clause = Some(self.ast.add(WhenClause {
                when_keyword: when,
                expression,
            }));
        }

        if self.is_enabled(ExperimentalFlag::Patterns) {
            let pattern = self.pop_node::<DartPattern>();
            let guarded_pattern = self.ast.add(GuardedPattern {
                pattern,
                when_clause,
            });
            let node = self.ast.add(SwitchPatternCase {
                labels: NodeList::EMPTY,
                keyword: case_keyword,
                guarded_pattern,
                colon,
                statements: NodeList::EMPTY,
            });
            self.push(node);
        } else {
            let expression = self.pop_node::<Expression>();
            let node = self.ast.add(SwitchCase {
                labels: NodeList::EMPTY,
                keyword: case_keyword,
                expression,
                colon,
                statements: NodeList::EMPTY,
            });
            self.push(node);
        }
    }

    pub(crate) fn end_class_declaration(&mut self, _begin_token: TokenId, _end_token: TokenId) {
        let builder = self.take_class_like_builder();
        let declaration = builder.build_class(&mut self.ast);
        self.declarations.push(declaration.upcast());
    }

    pub(crate) fn end_class_or_mixin_or_extension_body(
        &mut self,
        _kind: DeclarationKind,
        _member_count: i32,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
        if let Some(builder) = self.class_like_builder.as_mut() {
            builder.left_bracket = begin_token;
            builder.right_bracket = end_token;
        }
    }

    pub(crate) fn end_combinators(&mut self, count: i32) {
        let combinators = self.pop_typed_list::<Combinator>(count as usize);
        self.push_nodes_or(combinators, NullValue::Combinators);
    }

    pub(crate) fn end_compilation_unit(&mut self, _count: i32, token: TokenId) {
        let end_token = token;
        let begin_token = self.pop_token();
        self.check_empty(self.ast.tokens.offset(end_token));

        let directives = self.ast.new_list(self.directives.clone());
        let declarations = self.ast.new_list(self.declarations.clone());
        let unit = self.ast.add(CompilationUnit {
            begin_token,
            script_tag: self.script_tag,
            directives,
            declarations,
            end_token,
        });
        self.push(unit);
    }

    pub(crate) fn end_conditional_expression(
        &mut self,
        question: TokenId,
        colon: TokenId,
        _end_token: TokenId,
    ) {
        let else_expression = self.pop_node::<Expression>();
        let then_expression = self.pop_node::<Expression>();
        let condition = self.pop_node::<Expression>();
        self.report_error_if_super(else_expression);
        self.report_error_if_super(then_expression);
        let node = self.ast.add(ConditionalExpression {
            condition,
            question,
            then_expression,
            colon,
            else_expression,
        });
        self.push(node);
    }

    pub(crate) fn end_conditional_uri(
        &mut self,
        if_keyword: TokenId,
        left_paren: TokenId,
        equal_sign: Option<TokenId>,
    ) {
        let library_uri = self.pop_node::<StringLiteral>();
        let value = match self.pop_if_not_null(equal_sign) {
            Some(v) => self.value_as_node_opt::<StringLiteral>(v),
            None => None,
        };
        if let Some(value) = value {
            if self.ast.is::<StringInterpolation>(value) {
                for child in self.ast.child_entities(value) {
                    if let Entity::Node(child) = child {
                        if self.ast.is::<InterpolationExpression>(child) {
                            // This error is reported in OutlineBuilder.endLiteralString
                            let begin = self.begin_token(child);
                            let end = self.end_token(child);
                            self.handle_recoverable_error(
                                cfe_codes::interpolation_in_uri(),
                                begin,
                                end,
                            );
                            break;
                        }
                    }
                }
            }
        }
        let name = self.pop_node::<DottedName>();
        let right_parenthesis = self.end_group(left_paren);
        let node = self.ast.add(Configuration {
            if_keyword,
            left_parenthesis: left_paren,
            name,
            equal_token: equal_sign,
            value,
            right_parenthesis,
            uri: library_uri,
        });
        self.push(node);
    }

    pub(crate) fn end_conditional_uris(&mut self, count: i32) {
        let configurations = self.pop_typed_list::<Configuration>(count as usize);
        self.push_nodes_or(configurations, NullValue::ConditionalUris);
    }

    pub(crate) fn end_constant_pattern(&mut self, const_keyword: Option<TokenId>) {
        let expression = self.pop_node::<Expression>();
        let node = self.ast.add(ConstantPattern {
            const_keyword,
            expression,
        });
        self.push(node);
    }

    pub(crate) fn end_const_dot_shorthand(&mut self, token: TokenId) {
        if !self.enabled_dot_shorthands {
            self.report_feature_not_enabled(ExperimentalFlag::DotShorthands, token, None);
        }

        let dot_shorthand = self.pop_node::<Expression>();
        if let Some(invocation) = self.ast.cast::<DotShorthandInvocation>(dot_shorthand) {
            let d = self.ast[invocation].clone();
            let node = self.ast.add(DotShorthandConstructorInvocation {
                const_keyword: Some(token),
                period: d.period,
                constructor_name: d.member_name,
                type_arguments: d.type_arguments,
                argument_list: d.argument_list,
            });
            self.push(node);
        } else if let Some(access) = self.ast.cast::<DotShorthandPropertyAccess>(dot_shorthand) {
            let d = self.ast[access].clone();
            let name_token = self.ast[d.property_name].token;
            let argument_list = self.synthetic_argument_list(name_token);
            let node = self.ast.add(DotShorthandConstructorInvocation {
                const_keyword: Some(token),
                period: d.period,
                constructor_name: d.property_name,
                type_arguments: None,
                argument_list,
            });
            self.push(node);
        }
    }

    pub(crate) fn end_const_expression(&mut self, token: TokenId) {
        self.handle_instance_creation(Some(token));
    }

    pub(crate) fn end_const_literal(&mut self, _end_token: TokenId) {}

    pub(crate) fn end_constructor(
        &mut self,
        kind: DeclarationKind,
        begin_token: TokenId,
        _new_token: Option<TokenId>,
        begin_param: TokenId,
        begin_initializers: Option<TokenId>,
        end_token: TokenId,
    ) {
        match kind {
            DeclarationKind::Class | DeclarationKind::ExtensionType | DeclarationKind::Enum => {
                self.end_class_constructor(begin_token, begin_param, begin_initializers, end_token);
            }
            DeclarationKind::Mixin | DeclarationKind::Extension => {
                let constructor = self.build_constructor_declaration(begin_token, end_token);
                self.invalid_nodes.push(constructor.raw());
            }
            DeclarationKind::TopLevel => {
                panic!("Unsupported operation: Unexpected constructor kind {kind:?}.")
            }
        }
    }

    pub(crate) fn end_constructor_reference(
        &mut self,
        _start: TokenId,
        period_before_name: Option<TokenId>,
        _end_token: TokenId,
        _constructor_reference_context: ConstructorReferenceContext,
    ) {
        let constructor_name = self.pop_node_opt::<SimpleIdentifier>();
        let type_arguments = self.pop_node_opt::<TypeArgumentList>();
        let type_name_identifier = self.pop_node::<Identifier>();
        let type_ = self
            .ast
            .identifier_to_named_type(type_name_identifier, type_arguments, None);
        let node = self.ast.add(ConstructorName {
            type_,
            period: period_before_name,
            name: constructor_name,
        });
        self.push(node);
    }

    pub(crate) fn end_do_while_statement(
        &mut self,
        do_keyword: TokenId,
        while_keyword: TokenId,
        end_token: TokenId,
    ) {
        let semicolon = end_token;
        let condition = self.pop_parenthesized_condition();
        let body = self.pop_node::<Statement>();
        let right_parenthesis = condition.right_parenthesis(&self.ast.tokens);
        let node = self.ast.add(DoStatement {
            do_keyword,
            body,
            while_keyword,
            left_parenthesis: condition.left_parenthesis,
            condition: condition.expression,
            right_parenthesis,
            semicolon,
        });
        self.push(node);
    }

    /// Dart `pop() as _ParenthesizedCondition`.
    fn pop_parenthesized_condition(&mut self) -> ParenthesizedCondition {
        match self.pop() {
            Value::ParenthesizedCondition(c) => c,
            other => panic!("{other:?} is not a subtype of type '_ParenthesizedCondition'"),
        }
    }

    pub(crate) fn end_do_while_statement_body(&mut self, _token: TokenId) {}

    pub(crate) fn end_else_statement(&mut self, _begin_token: TokenId, _end_token: TokenId) {}

    pub(crate) fn end_enum_declaration(
        &mut self,
        _begin_token: TokenId,
        _enum_keyword: TokenId,
        _left_brace: TokenId,
        _member_count: i32,
        _end_token: TokenId,
    ) {
        let builder = self.take_class_like_builder();
        let declaration = builder.build_enum(&mut self.ast);
        self.declarations.push(declaration.upcast());
    }

    pub(crate) fn end_export(&mut self, export_keyword: TokenId, semicolon: TokenId) {
        let combinators = self.pop_nodes_opt::<Combinator>();
        let configurations = self.pop_nodes_opt::<Configuration>();
        let uri = self.pop_node::<StringLiteral>();
        let metadata = self.pop_metadata();
        let comment = self.find_comment(metadata.as_deref(), export_keyword);
        let metadata = self.node_list(metadata);
        let configurations = self.node_list(configurations);
        let combinators = self.node_list(combinators);
        let directive = self.ast.add(ExportDirective {
            documentation_comment: comment,
            metadata,
            export_keyword,
            uri,
            configurations,
            combinators,
            semicolon,
        });
        self.directives.push(directive.upcast());
    }

    pub(crate) fn end_extension_declaration(
        &mut self,
        _begin_token: TokenId,
        _extension_keyword: TokenId,
        on_keyword: Option<TokenId>,
        _end_token: TokenId,
    ) {
        let builder = self.take_class_like_builder();
        assert!(
            matches!(builder.kind, ClassLikeKind::Extension(_)),
            "not an extension builder"
        );

        let mut on_clause = None;
        if let Some(on_keyword) = on_keyword {
            let extended_type = self.pop_node::<TypeAnnotation>();
            on_clause = Some(self.ast.add(ExtensionOnClause {
                on_keyword,
                extended_type,
            }));
        }

        self.pop(); // Error recovery: Primary constructor.
        self.pop(); // Error recovery: Const token.

        let declaration = builder.build_extension(&mut self.ast, None, on_clause);
        self.declarations.push(declaration.upcast());
    }

    pub(crate) fn end_extension_type_declaration(
        &mut self,
        _begin_token: TokenId,
        _augment_token: Option<TokenId>,
        _extension_keyword: TokenId,
        type_keyword: TokenId,
        _end_token: TokenId,
    ) {
        let implements_clause = self.pop_node_opt::<ImplementsClause>();
        let mut primary_constructor_builder = match self.pop() {
            Value::PrimaryConstructorBuilder(b) => Some(*b),
            Value::Null(_) => None,
            other => panic!("{other:?} is not a subtype of type '_PrimaryConstructorBuilder?'"),
        };
        let const_keyword = self.pop_token_opt();

        if self.enable_inline_class {
            let builder = self.take_class_like_builder();
            let ClassLikeKind::ExtensionType(e) = &builder.kind else {
                panic!("not an extension type builder");
            };
            if e.augment_keyword.is_none() && primary_constructor_builder.is_none() {
                let formal_parameter_list = self.synthetic_formal_parameter_list(e.name);
                primary_constructor_builder = Some(PrimaryConstructorBuilder {
                    const_keyword: None,
                    constructor_name: None,
                    formal_parameter_list,
                });
            }

            let declaration = builder.build_extension_type(
                &mut self.ast,
                type_keyword,
                const_keyword,
                primary_constructor_builder.as_ref(),
                implements_clause,
            );
            self.declarations.push(declaration.upcast());
        } else {
            self.report_feature_not_enabled(ExperimentalFlag::InlineClass, type_keyword, None);
        }

        self.class_like_builder = None;
    }

    pub(crate) fn end_factory(
        &mut self,
        kind: DeclarationKind,
        begin_token: TokenId,
        factory_keyword: TokenId,
        end_token: TokenId,
    ) {
        match kind {
            DeclarationKind::Class | DeclarationKind::Enum | DeclarationKind::ExtensionType => {
                self.end_factory_method(begin_token, factory_keyword, end_token);
            }
            DeclarationKind::Mixin | DeclarationKind::Extension => {
                let constructor = self.build_factory_constructor_declaration(
                    begin_token,
                    factory_keyword,
                    end_token,
                );
                self.invalid_nodes.push(constructor.raw());
            }
            DeclarationKind::TopLevel => {
                panic!("Unsupported operation: Unexpected factory kind {kind:?}.")
            }
        }
    }

    pub(crate) fn end_field_initializer(&mut self, assignment: TokenId, _end_token: TokenId) {
        let equals = assignment;
        let initializer = self.pop_node::<Expression>();
        let name = self.pop_node::<SimpleIdentifier>();
        self.report_error_if_super(initializer);
        let name = self.ast[name].token;
        let node = self.ast.add(VariableDeclaration {
            documentation_comment: None,
            metadata: NodeList::EMPTY,
            name,
            equals: Some(equals),
            initializer: Some(initializer),
        });
        self.push(node);
    }

    pub(crate) fn end_fields(
        &mut self,
        _kind: DeclarationKind,
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
        self.end_class_fields(
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
    }
}
