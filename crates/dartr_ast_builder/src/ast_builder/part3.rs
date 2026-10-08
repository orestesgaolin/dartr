// Dart source: pkg/analyzer/lib/src/fasta/ast_builder.dart (lines 3100-4397)

use dartr_ast::{
    AdjacentStrings, ArgumentList, AsExpression, AssignedVariablePattern, AssignmentExpression,
    Block, BreakStatement, CastPattern, CatchClause, CatchClauseParameter, ClassMember,
    ConstructorName, ConstructorSelector, ContinueStatement, DartPattern, DeclaredIdentifier,
    DeclaredVariablePattern, DotShorthandInvocation, DotShorthandPropertyAccess, DottedName,
    EmptyFunctionBody, EmptyStatement, EnumConstantArguments, EnumConstantDeclaration, Expression,
    ExpressionFunctionBody, ExpressionStatement, ExtendsClause, ForEachParts,
    ForEachPartsWithDeclaration, ForEachPartsWithIdentifier, ForEachPartsWithPattern, ForParts,
    ForPartsWithDeclarations, ForPartsWithExpression, ForPartsWithPattern, FormalParameterList,
    FunctionBody, FunctionDeclaration, FunctionExpression, FunctionTypeAlias, GenericFunctionType,
    GenericTypeAlias, GuardedPattern, Id, Identifier, ImplementsClause, IndexExpression,
    LabelReference, MethodInvocation, NamedType, NativeClause, NodeId, NodeList,
    PatternVariableDeclaration, PrefixedIdentifier, PropertyAccess, SimpleIdentifier, Statement,
    StringLiteral, SwitchExpression, SwitchExpressionCase, SwitchMember, SwitchStatement,
    TopLevelVariableDeclaration, TryStatement, TypeAnnotation, TypeArgumentList, TypeParameter,
    TypeParameterList, VariableDeclaration, VariableDeclarationList, VariableDeclarationStatement,
    WhenClause, WhileStatement, WithClause, YieldStatement,
};
use dartr_diagnostics::{cfe_codes, diag};
use dartr_parser::experimental_features::ExperimentalFlag;
use dartr_syntax::{Keyword, TokenId, TokenType};

use super::{
    AstBuilder, ClassDeclarationBuilder, ClassLikeDeclarationBuilder, ClassLikeKind,
    EnumDeclarationBuilder, ParenthesizedCondition, PrimaryConstructorBuilder,
};
use crate::stack::{NullValue, Value};

impl AstBuilder {
    pub(crate) fn end_switch_expression(&mut self, switch_keyword: TokenId, _end_token: TokenId) {
        let right_bracket = self.pop_token();
        let cases = self.pop_nodes::<SwitchExpressionCase>();
        let left_bracket = self.pop_token();
        let condition = self.pop_parenthesized_condition();
        let right_parenthesis = condition.right_parenthesis(&self.ast.tokens);
        let cases = self.ast.new_list(cases);
        let node = self.ast.add(SwitchExpression {
            switch_keyword,
            left_parenthesis: condition.left_parenthesis,
            expression: condition.expression,
            right_parenthesis,
            left_bracket,
            cases,
            right_bracket,
        });
        self.push(node);
    }

    pub(crate) fn end_switch_expression_block(
        &mut self,
        case_count: i32,
        left_bracket: TokenId,
        right_bracket: TokenId,
    ) {
        let cases = self.pop_typed_list2::<SwitchExpressionCase>(case_count as usize);
        self.push(left_bracket);
        self.push(cases);
        self.push(right_bracket);
    }

    pub(crate) fn end_switch_expression_case(
        &mut self,
        _begin_token: TokenId,
        when: Option<TokenId>,
        arrow: TokenId,
        _end_token: TokenId,
    ) {
        let expression = self.pop_node::<Expression>();
        let mut when_clause = None;
        if let Some(when) = when {
            let expression = self.pop_node::<Expression>();
            when_clause = Some(self.ast.add(WhenClause {
                when_keyword: when,
                expression,
            }));
        }
        let pattern = self.pop_node::<DartPattern>();
        let guarded_pattern = self.ast.add(GuardedPattern {
            pattern,
            when_clause,
        });
        let node = self.ast.add(SwitchExpressionCase {
            guarded_pattern,
            arrow,
            expression,
        });
        self.push(node);
    }

    pub(crate) fn end_switch_statement(&mut self, switch_keyword: TokenId, _end_token: TokenId) {
        let right_bracket = self.pop_token();
        let members = self.pop_nodes::<SwitchMember>();
        let left_bracket = self.pop_token();
        let condition = self.pop_parenthesized_condition();
        let right_parenthesis = condition.right_parenthesis(&self.ast.tokens);
        let members = self.ast.new_list(members);
        let node = self.ast.add(SwitchStatement {
            switch_keyword,
            left_parenthesis: condition.left_parenthesis,
            expression: condition.expression,
            right_parenthesis,
            left_bracket,
            members,
            right_bracket,
        });
        self.push(node);
    }

    pub(crate) fn end_then_statement(&mut self, _begin_token: TokenId, _end_token: TokenId) {}

    pub(crate) fn end_top_level_declaration(&mut self, _end_token: TokenId) {}

    pub(crate) fn end_top_level_fields(
        &mut self,
        augment_token: Option<TokenId>,
        abstract_token: Option<TokenId>,
        external_token: Option<TokenId>,
        _static_token: Option<TokenId>,
        _covariant_token: Option<TokenId>,
        late_token: Option<TokenId>,
        var_final_or_const: Option<TokenId>,
        count: i32,
        begin_token: TokenId,
        semicolon: TokenId,
    ) {
        if let Some(external_token) = external_token {
            if late_token.is_some() {
                self.handle_recoverable_error(
                    cfe_codes::external_late_field(),
                    external_token,
                    external_token,
                );
            }
        }

        let variables = self.pop_typed_list2::<VariableDeclaration>(count as usize);
        let type_ = self.pop_node_opt::<TypeAnnotation>();
        let variables = self.ast.new_list(variables);
        let variable_list = self.ast.add(VariableDeclarationList {
            documentation_comment: None,
            metadata: NodeList::EMPTY,
            late_keyword: late_token,
            keyword: var_final_or_const,
            type_,
            variables,
        });
        let metadata = self.pop_metadata();
        let comment = self.find_comment(metadata.as_deref(), begin_token);
        let metadata = self.node_list(metadata);
        let node = self.ast.add(TopLevelVariableDeclaration {
            documentation_comment: comment,
            metadata,
            augment_keyword: augment_token,
            external_keyword: external_token,
            abstract_keyword: abstract_token,
            variables: variable_list,
            semicolon,
        });
        self.declarations.push(node.upcast());
    }

    pub(crate) fn end_top_level_method(
        &mut self,
        begin_token: TokenId,
        get_or_set: Option<TokenId>,
        _end_token: TokenId,
    ) {
        let body = self.pop_node::<FunctionBody>();
        let mut formal_parameters = self.pop_node_opt::<FormalParameterList>();
        let type_parameters = self.pop_node_opt::<TypeParameterList>();
        let name = self.pop_node::<SimpleIdentifier>();
        let return_type = self.pop_node_opt::<TypeAnnotation>();
        let modifiers = self.pop_modifiers();
        let augment_keyword = modifiers.as_ref().and_then(|m| m.augment_keyword);
        let external_keyword = modifiers.as_ref().and_then(|m| m.external_keyword);
        let metadata = self.pop_metadata();
        let comment = self.find_comment(metadata.as_deref(), begin_token);

        if get_or_set.is_some_and(|t| self.ast.tokens.ty(t) == Keyword::SET) {
            formal_parameters = self.ensure_setter_formal_parameter(name, formal_parameters);
        }

        let function_expression = self.ast.add(FunctionExpression {
            type_parameters,
            parameters: formal_parameters,
            body,
        });
        let metadata = self.node_list(metadata);
        let name = self.ast[name].token;
        let node = self.ast.add(FunctionDeclaration {
            documentation_comment: comment,
            metadata,
            augment_keyword,
            external_keyword,
            return_type,
            property_keyword: get_or_set,
            name,
            function_expression,
        });
        self.declarations.push(node.upcast());
    }

    pub(crate) fn end_try_statement(
        &mut self,
        catch_count: i32,
        try_keyword: TokenId,
        finally_keyword: Option<TokenId>,
        _end_token: TokenId,
    ) {
        let finally_block = match self.pop_if_not_null(finally_keyword) {
            Some(v) => self.value_as_node_opt::<Block>(v),
            None => None,
        };
        let catch_clauses = self.pop_typed_list2::<CatchClause>(catch_count as usize);
        let body = self.pop_node::<Block>();
        let catch_clauses = self.ast.new_list(catch_clauses);
        let node = self.ast.add(TryStatement {
            try_keyword,
            body,
            catch_clauses,
            finally_keyword,
            finally_block,
        });
        self.push(node);
    }

    pub(crate) fn end_type_arguments(
        &mut self,
        count: i32,
        left_bracket: TokenId,
        right_bracket: TokenId,
    ) {
        let arguments = self.pop_typed_list2::<TypeAnnotation>(count as usize);
        let arguments = self.ast.new_list(arguments);
        let node = self.ast.add(TypeArgumentList {
            left_bracket,
            arguments,
            right_bracket,
        });
        self.push(node);
    }

    pub(crate) fn end_typedef(
        &mut self,
        augment_token: Option<TokenId>,
        typedef_keyword: TokenId,
        equals: Option<TokenId>,
        semicolon: TokenId,
    ) {
        match equals {
            None => {
                let parameters = self.pop_node::<FormalParameterList>();
                let type_parameters = self.pop_node_opt::<TypeParameterList>();
                let name = self.pop_node::<SimpleIdentifier>();
                let return_type = self.pop_node_opt::<TypeAnnotation>();
                let metadata = self.pop_metadata();
                let comment = self.find_comment(metadata.as_deref(), typedef_keyword);
                let metadata = self.node_list(metadata);
                let name = self.ast[name].token;
                let node = self.ast.add(FunctionTypeAlias {
                    documentation_comment: comment,
                    metadata,
                    augment_keyword: augment_token,
                    typedef_keyword,
                    return_type,
                    name,
                    type_parameters,
                    parameters,
                    semicolon,
                });
                self.declarations.push(node.upcast());
            }
            Some(equals) => {
                let type_ = self.pop_node::<TypeAnnotation>();
                let code_parameters = self.pop_node_opt::<TypeParameterList>();
                let name = self.pop_node::<SimpleIdentifier>();
                let metadata = self.pop_metadata();
                let comment = self.find_comment(metadata.as_deref(), typedef_keyword);
                if !self.ast.is::<GenericFunctionType>(type_)
                    && !self.enable_non_function_type_aliases
                {
                    self.report_feature_not_enabled(
                        ExperimentalFlag::NonfunctionTypeAliases,
                        equals,
                        None,
                    );
                }
                let metadata = self.node_list(metadata);
                let name = self.ast[name].token;
                let node = self.ast.add(GenericTypeAlias {
                    documentation_comment: comment,
                    metadata,
                    augment_keyword: augment_token,
                    typedef_keyword,
                    name,
                    type_parameters: code_parameters,
                    equals,
                    type_,
                    semicolon,
                });
                self.declarations.push(node.upcast());
            }
        }
    }

    pub(crate) fn end_type_list(&mut self, count: i32) {
        let list = self.pop_typed_list::<TypeAnnotation>(count as usize);
        self.push_nodes_or(list, NullValue::TypeList);
    }

    pub(crate) fn end_type_variable(
        &mut self,
        _token: TokenId,
        index: i32,
        extends_or_super: Option<TokenId>,
        variance: Option<TokenId>,
    ) {
        // TODO(kallentu): Implement variance behaviour for the analyzer.
        if !self.enable_variance {
            self.report_variance_modifier_not_enabled(variance);
        }

        let bound = self.pop_node_opt::<TypeAnnotation>();

        // Peek to leave type parameters on top of stack.
        let type_parameter = match self.peek() {
            Some(Value::Nodes(list)) => Id::<TypeParameter>::from_raw(list[index as usize]),
            other => panic!("{other:?} is not a subtype of type 'List<TypeParameterImpl>'"),
        };
        self.ast.modify(type_parameter, |n| {
            n.extends_keyword = extends_or_super;
            n.bound = bound;
            n.variance_keyword = variance;
        });
    }

    pub(crate) fn end_type_variables(&mut self, begin_token: TokenId, end_token: TokenId) {
        let type_parameters = self.pop_nodes::<TypeParameter>();
        let type_parameters = self.ast.new_list(type_parameters);
        let node = self.ast.add(TypeParameterList {
            left_bracket: begin_token,
            type_parameters,
            right_bracket: end_token,
        });
        self.push(node);
    }

    pub(crate) fn end_variable_initializer(&mut self, equals: TokenId) {
        let initializer = self.pop_node::<Expression>();
        let identifier = self.pop_node::<SimpleIdentifier>();
        self.report_error_if_super(initializer);
        // TODO(ahe): Don't push initializers, instead install them.
        let name = self.ast[identifier].token;
        let node = self.ast.add(VariableDeclaration {
            documentation_comment: None,
            metadata: NodeList::EMPTY,
            name,
            equals: Some(equals),
            initializer: Some(initializer),
        });
        self.push(node);
    }

    pub(crate) fn end_variables_declaration(&mut self, count: i32, semicolon: Option<TokenId>) {
        let variables = self.pop_typed_list2::<VariableDeclaration>(count as usize);
        let modifiers = self.pop_modifiers();
        let type_ = self.pop_node_opt::<TypeAnnotation>();
        let keyword = modifiers
            .as_ref()
            .and_then(|m| m.final_const_or_var_keyword);
        let metadata = self.pop_metadata();
        let first_begin = self.begin_token(variables[0]);
        let comment = self.find_comment(metadata.as_deref(), first_begin);

        // https://github.com/dart-lang/sdk/issues/53964
        if let Some(semicolon) = semicolon {
            if self.ast.tokens.get(semicolon).is_synthetic() && variables.len() == 1 {
                let variable = variables[0];
                if let Some(named_type) = type_.and_then(|t| self.ast.cast::<NamedType>(t)) {
                    let named = self.ast[named_type].clone();
                    if let Some(import_prefix) = named.import_prefix {
                        let prefix = self.ast[import_prefix].clone();
                        // x.^
                        // await y.foo();
                        {
                            let await_token = named.name;
                            if self.ast.tokens.ty(await_token) == Keyword::AWAIT {
                                let prefix_identifier =
                                    self.ast.add(SimpleIdentifier { token: prefix.name });
                                let synthetic = self
                                    .rewriter()
                                    .insert_synthetic_identifier(prefix.period, "");
                                let identifier =
                                    self.ast.add(SimpleIdentifier { token: synthetic });
                                let expression = self.ast.add(PrefixedIdentifier {
                                    prefix: prefix_identifier,
                                    period: prefix.period,
                                    identifier,
                                });
                                let node = self.ast.add(ExpressionStatement {
                                    expression: expression.upcast(),
                                    semicolon: Some(semicolon),
                                });
                                self.push(node);
                                self.rewriter().insert_token(semicolon, await_token);
                                let variable_name = self.ast[variable].name;
                                self.rewriter().insert_token(await_token, variable_name);
                                return;
                            }
                        }
                        // x.foo^
                        // await y.bar();
                        {
                            let await_token = self.ast[variable].name;
                            let ty = self.ast.tokens.ty(await_token);
                            if ty == Keyword::AWAIT || ty == TokenType::IDENTIFIER {
                                // We see `x.foo await;`, where `;` is synthetic.
                                // It is followed by `y.bar()`.
                                // Insert a new `;`, and (unfortunately) drop `await;`.
                                let after = self.next(semicolon);
                                self.ast.tokens.set_next(named.name, after);
                                let semicolon2 = self
                                    .rewriter()
                                    .insert_synthetic_token(named.name, TokenType::SEMICOLON);
                                let prefix_identifier =
                                    self.ast.add(SimpleIdentifier { token: prefix.name });
                                let identifier =
                                    self.ast.add(SimpleIdentifier { token: named.name });
                                let expression = self.ast.add(PrefixedIdentifier {
                                    prefix: prefix_identifier,
                                    period: prefix.period,
                                    identifier,
                                });
                                let node = self.ast.add(ExpressionStatement {
                                    expression: expression.upcast(),
                                    semicolon: Some(semicolon2),
                                });
                                self.push(node);
                                return;
                            }
                        }
                    }
                }
            }
        }

        let metadata = self.node_list(metadata);
        let variables = self.ast.new_list(variables);
        let variable_list = self.ast.add(VariableDeclarationList {
            documentation_comment: comment,
            metadata,
            late_keyword: modifiers.as_ref().and_then(|m| m.late_token),
            keyword,
            type_,
            variables,
        });
        let semicolon = match semicolon {
            Some(s) => s,
            None => self.detached_token(TokenType::SEMICOLON),
        };
        let node = self.ast.add(VariableDeclarationStatement {
            variables: variable_list,
            semicolon,
        });
        self.push(node);
    }

    pub(crate) fn end_while_statement(&mut self, while_keyword: TokenId, _end_token: TokenId) {
        let body = self.pop_node::<Statement>();
        let condition = self.pop_parenthesized_condition();
        let right_parenthesis = condition.right_parenthesis(&self.ast.tokens);
        let node = self.ast.add(WhileStatement {
            while_keyword,
            left_parenthesis: condition.left_parenthesis,
            condition: condition.expression,
            right_parenthesis,
            body,
        });
        self.push(node);
    }

    pub(crate) fn end_while_statement_body(&mut self, _end_token: TokenId) {}

    pub(crate) fn end_yield_statement(
        &mut self,
        yield_token: TokenId,
        star_token: Option<TokenId>,
        semicolon: TokenId,
    ) {
        let expression = self.pop_node::<Expression>();
        let node = self.ast.add(YieldStatement {
            yield_keyword: yield_token,
            star: star_token,
            expression,
            semicolon,
        });
        self.push(node);
    }

    pub(crate) fn handle_adjacent_string_literals(
        &mut self,
        _start_token: TokenId,
        literal_count: i32,
    ) {
        let strings = self.pop_typed_list2::<StringLiteral>(literal_count as usize);
        let strings = self.ast.new_list(strings);
        let node = self.ast.add(AdjacentStrings { strings });
        self.push(node);
    }

    pub(crate) fn handle_as_operator(&mut self, as_operator: TokenId) {
        let type_ = self.pop_node::<TypeAnnotation>();
        let expression = self.pop_node::<Expression>();
        self.report_error_if_super(expression);
        let node = self.ast.add(AsExpression {
            expression,
            as_operator,
            type_,
        });
        self.push(node);
    }

    pub(crate) fn handle_assigned_variable_pattern(&mut self, variable: TokenId) {
        let node = self.ast.add(AssignedVariablePattern { name: variable });
        self.push(node);
    }

    pub(crate) fn handle_assignment_expression(&mut self, token: TokenId, _end_token: TokenId) {
        let rhs = self.pop_node::<Expression>();
        let lhs = self.pop_node::<Expression>();
        if !self.is_assignable(lhs) {
            // TODO(danrubel): Update the BodyBuilder to report this error.
            let begin = self.begin_token(lhs);
            let end = self.end_token(lhs);
            self.handle_recoverable_error(cfe_codes::missing_assignable_selector(), begin, end);
        }
        self.report_error_if_super(rhs);
        let node = self.ast.add(AssignmentExpression {
            left_hand_side: lhs,
            operator: token,
            right_hand_side: rhs,
        });
        self.push(node);
        if !self.enable_triple_shift && self.ast.tokens.ty(token) == TokenType::GT_GT_GT_EQ {
            self.report_feature_not_enabled(ExperimentalFlag::TripleShift, token, None);
        }
    }

    pub(crate) fn handle_async_modifier(
        &mut self,
        async_token: Option<TokenId>,
        star_token: Option<TokenId>,
    ) {
        self.push_token_or(async_token, NullValue::FunctionBodyAsyncToken);
        self.push_token_or(star_token, NullValue::FunctionBodyStarToken);
    }

    pub(crate) fn handle_break_statement(
        &mut self,
        has_target: bool,
        break_keyword: TokenId,
        semicolon: TokenId,
    ) {
        let label_identifier = if has_target {
            Some(self.pop_token())
        } else {
            None
        };
        let label = label_identifier.map(|name| self.ast.add(LabelReference { name }));
        let node = self.ast.add(BreakStatement {
            break_keyword,
            label,
            semicolon,
        });
        self.push(node);
    }

    pub(crate) fn handle_cascade_access(
        &mut self,
        operator_token: TokenId,
        _end_token: TokenId,
        _is_null_aware: bool,
    ) {
        self.do_dot_expression(operator_token);
    }

    pub(crate) fn handle_cast_pattern(&mut self, as_operator: TokenId) {
        let type_ = self.pop_node::<TypeAnnotation>();
        let pattern = self.pop_node::<DartPattern>();
        let node = self.ast.add(CastPattern {
            pattern,
            as_token: as_operator,
            type_,
        });
        self.push(node);
    }

    pub(crate) fn handle_catch_block(
        &mut self,
        on_keyword: Option<TokenId>,
        catch_keyword: Option<TokenId>,
        comma: Option<TokenId>,
    ) {
        let body = self.pop_node::<Block>();
        let catch_parameter_list = match self.pop_if_not_null(catch_keyword) {
            Some(v) => self.value_as_node_opt::<FormalParameterList>(v),
            None => None,
        };
        let type_ = match self.pop_if_not_null(on_keyword) {
            Some(v) => self.value_as_node_opt::<TypeAnnotation>(v),
            None => None,
        };
        let mut exception = None;
        let mut stack_trace = None;
        let mut left_parenthesis = None;
        let mut right_parenthesis = None;
        if let Some(list) = catch_parameter_list {
            let list = self.ast[list].clone();
            left_parenthesis = Some(list.left_parenthesis);
            right_parenthesis = Some(list.right_parenthesis);
            let catch_parameters = self.ast.list(list.parameters).to_vec();
            if !catch_parameters.is_empty() {
                exception = self.parameter_name(catch_parameters[0]);
            }
            if catch_parameters.len() > 1 {
                stack_trace = self.parameter_name(catch_parameters[1]);
            }
        }
        let exception_parameter = exception.map(|name| self.ast.add(CatchClauseParameter { name }));
        let stack_trace_parameter =
            stack_trace.map(|name| self.ast.add(CatchClauseParameter { name }));
        let node = self.ast.add(CatchClause {
            on_keyword,
            exception_type: type_,
            catch_keyword,
            left_parenthesis,
            exception_parameter,
            comma,
            stack_trace_parameter,
            right_parenthesis,
            body,
        });
        self.push(node);
    }

    pub(crate) fn handle_class_extends(
        &mut self,
        extends_keyword: Option<TokenId>,
        type_count: i32,
    ) {
        // If more extends clauses was specified (parser has already issued an
        // error) throw them away for now and pick the first one.
        let mut type_count = type_count;
        while type_count > 1 {
            self.pop();
            type_count -= 1;
        }
        let supertype = self.pop_node_opt::<TypeAnnotation>();
        if let Some(superclass) = supertype.and_then(|t| self.ast.cast::<NamedType>(t)) {
            let node = self.ast.add(ExtendsClause {
                extends_keyword: extends_keyword
                    .expect("Null check operator used on a null value (extendsKeyword)"),
                superclass,
            });
            self.push(node);
        } else {
            self.push(NullValue::ExtendsClause);
            // TODO(brianwilkerson): Consider (a) extending `ExtendsClause` to
            //  accept any type annotation for recovery purposes, and (b)
            //  extending the parser to parse a generic function type at this
            //  location.
            if let Some(supertype) = supertype {
                self.report_at_node(diag::expected_named_type_extends(), supertype);
            }
        }
    }

    pub(crate) fn handle_class_header(
        &mut self,
        begin: TokenId,
        class_keyword: TokenId,
        native_token: Option<TokenId>,
    ) {
        debug_assert!(self.class_like_builder.is_none());

        let native_clause = native_token.map(|native_keyword| {
            let name = self.native_name;
            self.ast.add(NativeClause {
                native_keyword,
                name,
            })
        });
        let implements_clause = self.pop_node_opt::<ImplementsClause>();
        let with_clause = self.pop_node_opt::<WithClause>();
        let extends_clause = self.pop_node_opt::<ExtendsClause>();
        let primary_constructor_builder = self.pop_primary_constructor_builder();
        self.pop_token_opt(); // constKeyword
        let mixin_keyword = self.pop_token_opt();
        let augment_keyword = self.pop_token_opt();
        let final_keyword = self.pop_token_opt();
        let interface_keyword = self.pop_token_opt();
        let base_keyword = self.pop_token_opt();
        let sealed_keyword = self.pop_token_opt();
        let modifiers = self.pop_modifiers();
        let type_parameters = self.pop_node_opt::<TypeParameterList>();
        let name = self.pop_node::<SimpleIdentifier>();
        let abstract_keyword = modifiers.as_ref().and_then(|m| m.abstract_keyword);
        let metadata = self.pop_metadata();
        let comment = self.find_comment(metadata.as_deref(), begin);
        // leftBracket, members, and rightBracket
        // are set in [endClassOrMixinBody].
        let left_bracket = self.detached_token(TokenType::OPEN_CURLY_BRACKET);
        let right_bracket = self.detached_token(TokenType::CLOSE_CURLY_BRACKET);
        let name = self.ast[name].token;
        self.class_like_builder = Some(Box::new(ClassLikeDeclarationBuilder {
            comment,
            metadata,
            type_parameters,
            empty_class_body_semicolon: None,
            left_bracket,
            members: Vec::<Id<ClassMember>>::new(),
            right_bracket,
            kind: ClassLikeKind::Class(ClassDeclarationBuilder {
                augment_keyword,
                abstract_keyword,
                sealed_keyword,
                base_keyword,
                interface_keyword,
                final_keyword,
                mixin_keyword,
                class_keyword,
                primary_constructor_builder,
                name,
                extends_clause,
                with_clause,
                implements_clause,
                native_clause,
            }),
        }));
    }

    pub(crate) fn handle_class_no_with_clause(&mut self) {
        self.push(NullValue::WithClause);
    }

    pub(crate) fn handle_class_with_clause(&mut self, with_keyword: TokenId) {
        let mixin_types = self.pop_named_type_list(diag::expected_named_type_with);
        let mixin_types = self.ast.new_list(mixin_types);
        let node = self.ast.add(WithClause {
            with_keyword,
            mixin_types,
        });
        self.push(node);
    }

    pub(crate) fn handle_const_factory(&mut self, _const_keyword: TokenId) {
        // TODO(kallentu): Removal of const factory error for const function
        // feature
    }

    pub(crate) fn handle_continue_statement(
        &mut self,
        has_target: bool,
        continue_keyword: TokenId,
        semicolon: TokenId,
    ) {
        let label_identifier = if has_target {
            Some(self.pop_token())
        } else {
            None
        };
        let label = label_identifier.map(|name| self.ast.add(LabelReference { name }));
        let node = self.ast.add(ContinueStatement {
            continue_keyword,
            label,
            semicolon,
        });
        self.push(node);
    }

    pub(crate) fn handle_declared_variable_pattern(
        &mut self,
        keyword: Option<TokenId>,
        variable: TokenId,
        _in_assignment_pattern: bool,
    ) {
        let type_ = self.pop_node_opt::<TypeAnnotation>();
        let node = self.ast.add(DeclaredVariablePattern {
            keyword,
            type_,
            name: variable,
        });
        self.push(node);
    }

    pub(crate) fn handle_dot_access(
        &mut self,
        operator_token: TokenId,
        _end_token: TokenId,
        _is_null_aware: bool,
    ) {
        self.do_dot_expression(operator_token);
    }

    pub(crate) fn handle_dot_shorthand_context(&mut self, token: TokenId) {
        if !self.enabled_dot_shorthands {
            self.report_feature_not_enabled(ExperimentalFlag::DotShorthands, token, None);
        }

        let dot_shorthand = self.pop_node::<Expression>();
        if self.is_dot_shorthand_mixin(dot_shorthand.raw()) {
            self.dot_shorthands.push(dot_shorthand.raw());
        } else {
            debug_assert!(
                false,
                "'{dot_shorthand:?}' must be a 'DotShorthandMixin' because we \
                 should only call 'handleDotShorthandContext' after parsing \
                 expressions that have a context type we can cache."
            );
        }
        self.push(dot_shorthand);
    }

    pub(crate) fn handle_dot_shorthand_head(&mut self, period_token: TokenId) {
        if !self.enabled_dot_shorthands {
            self.report_feature_not_enabled(ExperimentalFlag::DotShorthands, period_token, None);
        }

        let operand = self.pop_node::<Expression>();
        if let Some(property_name) = self.ast.cast::<SimpleIdentifier>(operand) {
            let node = self.ast.add(DotShorthandPropertyAccess {
                period: period_token,
                property_name,
            });
            self.push(node);
        } else if let Some(invocation) = self.ast.cast::<MethodInvocation>(operand) {
            let m = self.ast[invocation].clone();
            let node = self.ast.add(DotShorthandInvocation {
                period: period_token,
                member_name: m.method_name,
                type_arguments: m.type_arguments,
                argument_list: m.argument_list,
            });
            self.push(node);
        } else {
            self.push(operand);
        }
    }

    pub(crate) fn handle_dotted_name(&mut self, count: i32, _first_identifier: TokenId) {
        let identifiers = self.pop_typed_list2_tokens(count as usize);
        let mut tokens = Vec::new();
        if !identifiers.is_empty() {
            // TODO(scheglov): The parser does not use [handleQualified] for
            // [handleDottedName], so no periods in [identifiers].
            // We must walk the token stream.
            let mut t = identifiers[0];
            let end = *identifiers.last().unwrap();
            while t != end {
                tokens.push(t);
                t = self.next(t);
            }
            tokens.push(end);
        }
        let tokens = self.ast.new_token_list(tokens);
        let node = self.ast.add(DottedName { tokens });
        self.push(node);
    }

    pub(crate) fn handle_else_control_flow(&mut self, else_token: TokenId) {
        self.push(else_token);
    }

    pub(crate) fn handle_empty_function_body(&mut self, semicolon: TokenId) {
        // TODO(scheglov): Change the parser to not produce these modifiers.
        self.pop(); // star
        self.pop(); // async
        let node = self.ast.add(EmptyFunctionBody { semicolon });
        self.push(node);
    }

    pub(crate) fn handle_empty_statement(&mut self, semicolon: TokenId) {
        let node = self.ast.add(EmptyStatement { semicolon });
        self.push(node);
    }

    pub(crate) fn handle_enum_element(
        &mut self,
        _begin_token: TokenId,
        augment_token: Option<TokenId>,
    ) {
        let argument_list = self.pop_node_opt::<ArgumentList>();
        let tmp_constructor = self.pop_node_opt::<ConstructorName>();
        let mut constant = self.pop_node::<EnumConstantDeclaration>();

        let tmp = tmp_constructor.map(|c| self.ast[c].clone());
        if !self.enable_enhanced_enums
            && (argument_list.is_some()
                || tmp.as_ref().is_some_and(|c| {
                    self.ast[c.type_].type_arguments.is_some() || c.name.is_some()
                }))
        {
            let token = match argument_list {
                Some(a) => self.begin_token(a),
                None => self.begin_token(tmp_constructor.unwrap()),
            };
            self.report_feature_not_enabled(ExperimentalFlag::EnhancedEnums, token, None);
        }

        let mut type_arguments = None;
        let mut constructor_selector = None;
        if let Some(tmp) = tmp {
            type_arguments = self.ast[tmp.type_].type_arguments;
            if let (Some(period), Some(name)) = (tmp.period, tmp.name) {
                constructor_selector = Some(self.ast.add(ConstructorSelector { period, name }));
            }
        }

        // Replace the constant to include arguments.
        if let Some(argument_list) = argument_list {
            let c = self.ast[constant].clone();
            let arguments = self.ast.add(EnumConstantArguments {
                type_arguments,
                constructor_selector,
                argument_list,
            });
            let metadata = self.ast.list(c.metadata).to_vec();
            let metadata = self.ast.new_list(metadata);
            constant = self.ast.add(EnumConstantDeclaration {
                documentation_comment: c.documentation_comment,
                metadata,
                augment_keyword: augment_token,
                name: c.name,
                arguments: Some(arguments),
            });
        }

        self.push(constant);
    }

    pub(crate) fn handle_enum_elements(
        &mut self,
        elements_end_token: TokenId,
        elements_count: i32,
    ) {
        let constants = self.pop_typed_list2::<EnumConstantDeclaration>(elements_count as usize);
        let is_semicolon = self.optional(";", elements_end_token);
        {
            let builder = self
                .class_like_builder
                .as_mut()
                .expect("Null check operator used on a null value (_classLikeBuilder)");
            let ClassLikeKind::Enum(e) = &mut builder.kind else {
                panic!("type is not a subtype of type '_EnumDeclarationBuilder'");
            };
            e.constants.extend(constants);
            if is_semicolon {
                e.semicolon = Some(elements_end_token);
            }
        }

        if !self.enable_enhanced_enums && is_semicolon {
            self.report_feature_not_enabled(
                ExperimentalFlag::EnhancedEnums,
                elements_end_token,
                None,
            );
        }
    }

    pub(crate) fn handle_enum_header(
        &mut self,
        augment_token: Option<TokenId>,
        enum_keyword: TokenId,
        left_brace: TokenId,
    ) {
        let implements_clause = self.pop_node_opt::<ImplementsClause>();
        let with_clause = self.pop_node_opt::<WithClause>();
        let primary_constructor_builder = self.pop_primary_constructor_builder();
        self.pop_token_opt(); // constKeyword
        let type_parameters = self.pop_node_opt::<TypeParameterList>();
        let name = self.pop_node::<SimpleIdentifier>();
        let metadata = self.pop_metadata();
        let comment = self.find_comment(metadata.as_deref(), enum_keyword);

        if !self.enable_enhanced_enums
            && (with_clause.is_some() || implements_clause.is_some() || type_parameters.is_some())
        {
            let token = if let Some(w) = with_clause {
                self.ast[w].with_keyword
            } else if let Some(i) = implements_clause {
                self.ast[i].implements_keyword
            } else {
                self.begin_token(type_parameters.unwrap())
            };
            self.report_feature_not_enabled(ExperimentalFlag::EnhancedEnums, token, None);
        }

        let end_group = self.ast.tokens.get(left_brace).end_group;
        let right_bracket = if end_group.is_some() {
            end_group
        } else {
            left_brace
        };
        let name = self.ast[name].token;
        self.class_like_builder = Some(Box::new(ClassLikeDeclarationBuilder {
            comment,
            metadata,
            type_parameters,
            empty_class_body_semicolon: None,
            left_bracket: left_brace,
            members: Vec::new(),
            right_bracket,
            kind: ClassLikeKind::Enum(EnumDeclarationBuilder {
                augment_keyword: augment_token,
                enum_keyword,
                primary_constructor_builder,
                name,
                with_clause,
                implements_clause,
                constants: Vec::new(),
                semicolon: None,
            }),
        }));
    }

    pub(crate) fn handle_enum_no_with_clause(&mut self) {
        self.push(NullValue::WithClause);
    }

    pub(crate) fn handle_enum_with_clause(&mut self, with_keyword: TokenId) {
        let mixin_types = self.pop_named_type_list(diag::expected_named_type_with);
        let mixin_types = self.ast.new_list(mixin_types);
        let node = self.ast.add(WithClause {
            with_keyword,
            mixin_types,
        });
        self.push(node);
    }

    pub(crate) fn handle_expression_function_body(
        &mut self,
        arrow_token: TokenId,
        semicolon: Option<TokenId>,
    ) {
        let expression = self.pop_node::<Expression>();
        self.report_error_if_super(expression);
        let star = self.pop_token_opt();
        let async_keyword = self.pop_token_opt();
        if self.parse_function_bodies {
            let node = self.ast.add(ExpressionFunctionBody {
                keyword: async_keyword,
                star,
                function_definition: arrow_token,
                expression,
                semicolon,
            });
            self.push(node);
        } else {
            let node = self.ast.add(EmptyFunctionBody {
                semicolon: semicolon.expect("Null check operator used on a null value"),
            });
            self.push(node);
        }
    }

    pub(crate) fn handle_expression_statement(
        &mut self,
        _begin_token: TokenId,
        semicolon: TokenId,
    ) {
        let expression = self.pop_node::<Expression>();
        self.report_error_if_super(expression);
        if let Some(identifier) = self.ast.cast::<SimpleIdentifier>(expression) {
            // Dart `expression.token.keyword?.isBuiltInOrPseudo == false`.
            let ty = self.ast.tokens.ty(self.ast[identifier].token);
            if ty.is_keyword() && !(ty.is_built_in() || ty.is_pseudo()) {
                // This error is also reported by the body builder.
                let begin = self.begin_token(expression);
                let end = self.end_token(expression);
                self.handle_recoverable_error(cfe_codes::expected_statement(), begin, end);
            }
        }
        if let Some(assignment) = self.ast.cast::<AssignmentExpression>(expression) {
            let left = self.ast[assignment].left_hand_side;
            if !self.is_assignable(left) {
                // This error is also reported by the body builder.
                let begin = self.begin_token(left);
                let end = self.end_token(left);
                self.handle_recoverable_error(
                    cfe_codes::illegal_assignment_to_non_assignable(),
                    begin,
                    end,
                );
            }
        }
        let node = self.ast.add(ExpressionStatement {
            expression,
            semicolon: Some(semicolon),
        });
        self.push(node);
    }

    pub(crate) fn handle_finally_block(&mut self, _finally_keyword: TokenId) {
        // The finally block is popped in "endTryStatement".
    }

    pub(crate) fn handle_for_initializer_empty_statement(&mut self, _token: TokenId) {
        self.push(NullValue::Expression);
    }

    pub(crate) fn handle_for_initializer_expression_statement(
        &mut self,
        _token: TokenId,
        _for_in: bool,
    ) {
    }

    pub(crate) fn handle_for_initializer_local_variable_declaration(
        &mut self,
        _token: TokenId,
        _for_in: bool,
    ) {
    }

    pub(crate) fn handle_for_initializer_pattern_variable_assignment(
        &mut self,
        keyword: TokenId,
        equals: TokenId,
    ) {
        let expression = self.pop_node::<Expression>();
        let pattern = self.pop_node::<DartPattern>();
        let metadata = self.pop_metadata();
        let metadata = self.node_list(metadata);
        let node = self.ast.add(PatternVariableDeclaration {
            documentation_comment: None,
            metadata,
            keyword,
            pattern,
            equals,
            expression,
        });
        self.push(node);
    }

    pub(crate) fn handle_for_in_loop_parts(
        &mut self,
        await_token: Option<TokenId>,
        for_token: TokenId,
        left_parenthesis: TokenId,
        pattern_keyword: Option<TokenId>,
        in_keyword: TokenId,
    ) {
        let iterable = self.pop_node::<Expression>();
        let variable_or_declaration = match self.pop() {
            Value::Node(n) => n,
            other => panic!("Null check operator used on a null value ({other:?})"),
        };
        self.report_error_if_super(iterable);

        let for_loop_parts: Id<ForEachParts>;
        if let Some(pattern_keyword) = pattern_keyword {
            let metadata = self.pop_metadata();
            let metadata = self.node_list(metadata);
            let pattern = self
                .ast
                .cast::<DartPattern>(variable_or_declaration)
                .expect("type is not a subtype of type 'DartPatternImpl'");
            for_loop_parts = self
                .ast
                .add(ForEachPartsWithPattern {
                    metadata,
                    keyword: pattern_keyword,
                    pattern,
                    in_keyword,
                    iterable,
                })
                .upcast();
        } else if let Some(statement) = self
            .ast
            .cast::<VariableDeclarationStatement>(variable_or_declaration)
        {
            let variable_list = self.ast[self.ast[statement].variables].clone();
            let metadata = self.ast.list(variable_list.metadata).to_vec();
            let metadata = self.ast.new_list(metadata);
            let first = self.ast.list(variable_list.variables)[0];
            let name = self.ast[first].name;
            let loop_variable = self.ast.add(DeclaredIdentifier {
                documentation_comment: variable_list.documentation_comment,
                metadata,
                keyword: variable_list.keyword,
                type_: variable_list.type_,
                name,
            });
            for_loop_parts = self
                .ast
                .add(ForEachPartsWithDeclaration {
                    loop_variable,
                    in_keyword,
                    iterable,
                })
                .upcast();
        } else {
            let identifier = match self.ast.cast::<SimpleIdentifier>(variable_or_declaration) {
                Some(identifier) => identifier,
                None => {
                    // Parser has already reported the error.
                    let next = self.next(left_parenthesis);
                    if !self.ast.tokens.get(next).is_identifier() {
                        self.rewriter()
                            .insert_synthetic_identifier(left_parenthesis, "");
                    }
                    let token = self.next(left_parenthesis);
                    self.ast.add(SimpleIdentifier { token })
                }
            };
            for_loop_parts = self
                .ast
                .add(ForEachPartsWithIdentifier {
                    identifier,
                    in_keyword,
                    iterable,
                })
                .upcast();
        }

        self.push_token_or(await_token, NullValue::AwaitToken);
        self.push(for_token);
        self.push(left_parenthesis);
        self.push(for_loop_parts);
    }

    pub(crate) fn handle_for_loop_parts(
        &mut self,
        for_keyword: TokenId,
        left_paren: TokenId,
        left_separator: TokenId,
        _right_separator: TokenId,
        update_expression_count: i32,
    ) {
        let updates = self.pop_typed_list2::<Expression>(update_expression_count as usize);
        let condition_statement = self.pop_node::<Statement>();
        let initializer_part = self.pop();

        for &update in &updates {
            self.report_error_if_super(update);
        }

        let mut condition = None;
        let right_separator;
        if let Some(statement) = self.ast.cast::<ExpressionStatement>(condition_statement) {
            let s = self.ast[statement].clone();
            condition = Some(s.expression);
            right_separator = s
                .semicolon
                .expect("Null check operator used on a null value");
        } else {
            let empty = self
                .ast
                .cast::<EmptyStatement>(condition_statement)
                .expect("type is not a subtype of type 'EmptyStatementImpl'");
            right_separator = self.ast[empty].semicolon;
        }

        let updaters = self.ast.new_list(updates);
        let initializer_node = match initializer_part {
            Value::Node(n) => Some(n),
            Value::Null(_) => None,
            other => panic!("{other:?} is not a for loop initializer"),
        };
        let for_loop_parts: Id<ForParts>;
        if let Some(statement) =
            initializer_node.and_then(|n| self.ast.cast::<VariableDeclarationStatement>(n))
        {
            let variables = self.ast[statement].variables;
            for_loop_parts = self
                .ast
                .add(ForPartsWithDeclarations {
                    variables,
                    left_separator,
                    condition,
                    right_separator,
                    updaters,
                })
                .upcast();
        } else if let Some(declaration) =
            initializer_node.and_then(|n| self.ast.cast::<PatternVariableDeclaration>(n))
        {
            for_loop_parts = self
                .ast
                .add(ForPartsWithPattern {
                    variables: declaration,
                    left_separator,
                    condition,
                    right_separator,
                    updaters,
                })
                .upcast();
        } else {
            let initialization = initializer_node.map(|n| {
                self.ast
                    .cast::<Expression>(n)
                    .expect("type is not a subtype of type 'ExpressionImpl?'")
            });
            for_loop_parts = self
                .ast
                .add(ForPartsWithExpression {
                    initialization,
                    left_separator,
                    condition,
                    right_separator,
                    updaters,
                })
                .upcast();
        }

        self.push(for_keyword);
        self.push(left_paren);
        self.push(for_loop_parts);
    }

    pub(crate) fn handle_formal_parameter_without_value(&mut self, _token: TokenId) {
        self.push(NullValue::ParameterDefaultValue);
    }

    // ---------------------------------------------------------------------
    // Helpers of this part.

    /// Dart `pop() as _ParenthesizedCondition`.
    fn pop_parenthesized_condition(&mut self) -> ParenthesizedCondition {
        match self.pop() {
            Value::ParenthesizedCondition(c) => c,
            other => panic!("{other:?} is not a subtype of type '_ParenthesizedCondition'"),
        }
    }

    /// Dart `pop(NullValues.PrimaryConstructor) as
    /// _PrimaryConstructorBuilder?`.
    fn pop_primary_constructor_builder(&mut self) -> Option<Box<PrimaryConstructorBuilder>> {
        match self.pop() {
            Value::PrimaryConstructorBuilder(b) => Some(b),
            Value::Null(_) => None,
            other => panic!("{other:?} is not a subtype of type '_PrimaryConstructorBuilder?'"),
        }
    }

    /// Dart `ExpressionImpl.isAssignable`: identifiers, index expressions
    /// and property accesses.
    fn is_assignable(&self, expression: Id<Expression>) -> bool {
        self.ast.is::<Identifier>(expression)
            || self.ast.is::<IndexExpression>(expression)
            || self.ast.is::<PropertyAccess>(expression)
    }

    /// Dart `node is DotShorthandMixin`.
    fn is_dot_shorthand_mixin(&self, node: NodeId) -> bool {
        use dartr_ast::NodeKind as K;
        matches!(
            self.ast.kind(node),
            K::AnonymousMethodInvocation
                | K::DotShorthandConstructorInvocation
                | K::DotShorthandInvocation
                | K::DotShorthandPropertyAccess
                | K::FunctionExpressionInvocation
                | K::FunctionReference
                | K::IndexExpression
                | K::MethodInvocation
                | K::PostfixExpression
                | K::PropertyAccess
        )
    }
}
