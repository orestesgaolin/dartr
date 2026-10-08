// Dart source: pkg/analyzer/lib/src/fasta/ast_builder.dart (lines 1695-3098)

use dartr_ast::{
    Annotation, ArgumentList, AwaitExpression, ClassTypeAlias, CollectionElement, Combinator,
    Configuration, ConstructorInitializer, ConstructorName, DottedName, Expression,
    ExpressionStatement, FieldFormalParameter, ForEachParts, ForElement, ForParts, ForStatement,
    FormalParameter, FormalParameterDefaultClause, FormalParameterList, FunctionBody,
    FunctionDeclaration, FunctionDeclarationStatement, FunctionExpression,
    FunctionTypedFormalParameterSuffix, GenericFunctionType, HideCombinator, Id, Identifier,
    IfElement, IfStatement, ImplementsClause, ImportDirective, InterpolationElement,
    InterpolationExpression, InterpolationString, Label, LabeledStatement, LibraryDirective,
    NamedType, NodeId, NodeList, ParenthesizedExpression, PartDirective, PartOfDirective,
    PrimaryConstructorBody, PrimaryConstructorName, RecordLiteral, RecordLiteralField,
    RecordLiteralNamedField, RecordTypeAnnotation, RecordTypeAnnotationNamedField,
    RecordTypeAnnotationNamedFields, RecordTypeAnnotationPositionalField, RegularFormalParameter,
    RethrowExpression, ReturnStatement, ShowCombinator, SimpleIdentifier, SimpleStringLiteral,
    Statement, StringInterpolation, StringLiteral, SuperFormalParameter, SwitchCase, SwitchDefault,
    SwitchMember, SwitchPatternCase, SymbolLiteral, TypeAnnotation, TypeArgumentList,
    TypeParameterList, VariableDeclaration, WhenClause, WithClause, YieldStatement,
};
use dartr_diagnostics::cfe::{CfeCode, CfeMessage};
use dartr_diagnostics::{cfe_codes, diag};
use dartr_parser::declaration_kind::DeclarationKind;
use dartr_parser::experimental_features::ExperimentalFlag;
use dartr_parser::formal_parameter_kind::FormalParameterKind;
use dartr_parser::member_kind::MemberKind;
use dartr_parser::quote::{
    analyze_quote, unescape, unescape_first_string_part, unescape_last_string_part, unescape_string,
};
use dartr_syntax::{Keyword, TokenId, TokenType};
use rustc_hash::FxHashSet;

use super::{
    AstBuilder, FunctionTypedFormalParameterData, OptionalFormalParameters, ParenthesizedCondition,
    PrimaryConstructorBuilder, RedirectingFactoryBody,
};
use crate::stack::{NullValue, Value};

impl AstBuilder {
    /// Dart `pop() as _ParenthesizedCondition`.
    fn pop_parenthesized_condition(&mut self) -> ParenthesizedCondition {
        match self.pop() {
            Value::ParenthesizedCondition(c) => c,
            other => panic!("{other:?} is not a subtype of type '_ParenthesizedCondition'"),
        }
    }

    /// Reports the unescape errors collected from a `quote.dart` call
    /// (`UnescapeErrorListener` with [location]).
    fn report_unescape_errors(&mut self, errors: Vec<(CfeMessage, u32, u32)>, location: TokenId) {
        for (message, offset, length) in errors {
            self.handle_unescape_error(message, location, offset as i32, length as i32);
        }
    }

    /// Dart `RecordLiteralField.fieldExpression`.
    fn field_expression(&self, field: Id<RecordLiteralField>) -> Id<Expression> {
        if let Some(named) = self.ast.cast::<RecordLiteralNamedField>(field) {
            self.ast[named].field_expression
        } else {
            Id::from_raw(field.raw())
        }
    }

    pub(crate) fn end_for_control_flow(&mut self, _token: TokenId) {
        let body = self.pop_node::<CollectionElement>();
        let for_loop_parts = self.pop_node::<ForParts>();
        let left_parenthesis = self.pop_token();
        let for_token = self.pop_token();
        let right_parenthesis = self.end_group(left_parenthesis);
        let node = self.ast.add(ForElement {
            await_keyword: None,
            for_keyword: for_token,
            left_parenthesis,
            for_loop_parts: for_loop_parts.upcast(),
            right_parenthesis,
            body,
        });
        self.push(node);
    }

    pub(crate) fn end_for_in(&mut self, _end_token: TokenId) {
        let body = self.pop_node::<Statement>();
        let for_loop_parts = self.pop_node::<ForEachParts>();
        let left_parenthesis = self.pop_token();
        let for_token = self.pop_token();
        let await_token = self.pop_token_opt();
        let right_parenthesis = self.end_group(left_parenthesis);
        let node = self.ast.add(ForStatement {
            await_keyword: await_token,
            for_keyword: for_token,
            left_parenthesis,
            for_loop_parts: for_loop_parts.upcast(),
            right_parenthesis,
            body,
        });
        self.push(node);
    }

    pub(crate) fn end_for_in_body(&mut self, _end_token: TokenId) {}

    pub(crate) fn end_for_in_control_flow(&mut self, _token: TokenId) {
        let body = self.pop_node::<CollectionElement>();
        let for_loop_parts = self.pop_node::<ForEachParts>();
        let left_parenthesis = self.pop_token();
        let for_token = self.pop_token();
        let await_token = self.pop_token_opt();
        let right_parenthesis = self.end_group(left_parenthesis);
        let node = self.ast.add(ForElement {
            await_keyword: await_token,
            for_keyword: for_token,
            left_parenthesis,
            for_loop_parts: for_loop_parts.upcast(),
            right_parenthesis,
            body,
        });
        self.push(node);
    }

    pub(crate) fn end_for_in_expression(&mut self, _token: TokenId) {}

    pub(crate) fn end_formal_parameter(
        &mut self,
        _var_or_final: Option<TokenId>,
        this_keyword: Option<TokenId>,
        super_keyword: Option<TokenId>,
        period_after_this_or_super: Option<TokenId>,
        name_token: TokenId,
        _initializer_start: Option<TokenId>,
        _initializer_end: Option<TokenId>,
        kind: FormalParameterKind,
        member_kind: MemberKind,
    ) {
        if let Some(super_keyword) = super_keyword {
            if !self.enable_super_parameters {
                self.report_feature_not_enabled(
                    ExperimentalFlag::SuperParameters,
                    super_keyword,
                    None,
                );
            }
        }

        let default_clause = self.pop_node_opt::<FormalParameterDefaultClause>();
        let name = self.pop_node_opt::<SimpleIdentifier>();
        let type_or_function_typed_parameter = self.pop();
        let modifiers = self.pop_modifiers();
        let keyword = modifiers
            .as_ref()
            .and_then(|m| m.final_const_or_var_keyword);
        let covariant_keyword = modifiers.as_ref().and_then(|m| m.covariant_keyword);
        let required_keyword = modifiers.as_ref().and_then(|m| m.required_token);

        let type_or_function_typed_begin_token = match &type_or_function_typed_parameter {
            Value::Node(n) => Some(self.begin_token(*n)),
            Value::FunctionTypedFormalParameterData(d) => Some(match d.return_type {
                Some(r) => self.begin_token(r),
                None => self.begin_token(d.function_typed_suffix),
            }),
            _ => None,
        };

        let metadata = self.pop_metadata();
        let token_after_metadata = modifiers
            .as_ref()
            .and_then(|m| m.begin_token(&self.ast.tokens))
            .or(this_keyword)
            .or(type_or_function_typed_begin_token)
            .unwrap_or(name_token);
        let comment = self.find_comment(metadata.as_deref(), token_after_metadata);

        let type_: Option<Id<TypeAnnotation>>;
        let mut function_typed_suffix: Option<Id<FunctionTypedFormalParameterSuffix>> = None;
        match type_or_function_typed_parameter {
            Value::FunctionTypedFormalParameterData(d) => {
                type_ = d.return_type;
                function_typed_suffix = Some(d.function_typed_suffix);
            }
            other => {
                type_ = self.value_as_node_opt::<TypeAnnotation>(other);
            }
        }

        // Dart `keyword is KeywordToken`.
        let keyword_token = keyword.filter(|&k| self.ast.tokens.ty(k).is_keyword());
        if function_typed_suffix.is_some() && (this_keyword.is_some() || super_keyword.is_some()) {
            if let Some(k) = keyword_token {
                let keyword_kind = self.ast.tokens.ty(k);
                if keyword_kind == Keyword::CONST
                    || keyword_kind == Keyword::FINAL
                    || keyword_kind == Keyword::VAR
                {
                    self.handle_recoverable_error(cfe_codes::function_typed_parameter_var(), k, k);
                }
            }
        }

        let analyzer_kind = self.to_analyzer_parameter_kind(kind);

        let metadata = self.node_list(metadata);
        let parameter: Id<FormalParameter>;
        if let Some(super_keyword) = super_keyword {
            debug_assert!(this_keyword.is_none());
            if member_kind != MemberKind::PrimaryConstructor {
                // The parser reports a special error for declaring parameters,
                // so we avoid emitting this error here for primary
                // constructors.
                if let Some(k) = keyword_token {
                    if function_typed_suffix.is_none() && self.ast.tokens.ty(k) == Keyword::VAR {
                        let lexeme = self.lexeme(k).to_string();
                        self.handle_recoverable_error(
                            cfe_codes::extraneous_modifier(&lexeme),
                            k,
                            k,
                        );
                    }
                }
            }
            let name = self.ast[name.expect("Null check operator used on a null value")].token;
            parameter = self
                .ast
                .add(SuperFormalParameter {
                    documentation_comment: comment,
                    metadata,
                    kind: analyzer_kind,
                    required_keyword,
                    covariant_keyword,
                    const_final_or_var_keyword: keyword,
                    type_,
                    super_keyword,
                    period: period_after_this_or_super
                        .expect("Null check operator used on a null value"),
                    name,
                    function_typed_suffix,
                    default_clause,
                })
                .upcast();
        } else if let Some(this_keyword) = this_keyword {
            let name = self.ast[name.expect("Null check operator used on a null value")].token;
            parameter = self
                .ast
                .add(FieldFormalParameter {
                    documentation_comment: comment,
                    metadata,
                    kind: analyzer_kind,
                    required_keyword,
                    covariant_keyword,
                    const_final_or_var_keyword: keyword,
                    type_,
                    this_keyword,
                    period: period_after_this_or_super
                        .expect("Null check operator used on a null value"),
                    name,
                    function_typed_suffix,
                    default_clause,
                })
                .upcast();
        } else {
            let name = name.map(|n| self.ast[n].token);
            parameter = self
                .ast
                .add(RegularFormalParameter {
                    documentation_comment: comment,
                    metadata,
                    kind: analyzer_kind,
                    required_keyword,
                    covariant_keyword,
                    const_final_or_var_keyword: keyword,
                    type_,
                    name,
                    function_typed_suffix,
                    default_clause,
                })
                .upcast();
        }
        self.push(parameter);
    }

    pub(crate) fn end_formal_parameter_default_value_expression(&mut self) {}

    pub(crate) fn end_formal_parameters(
        &mut self,
        count: i32,
        left_parenthesis: TokenId,
        right_parenthesis: TokenId,
        _kind: MemberKind,
    ) {
        let raw_parameters = self
            .pop_typed_list_values(count as usize)
            .unwrap_or_default();
        let mut parameters: Vec<Id<FormalParameter>> = Vec::new();
        let mut left_delimiter = None;
        let mut right_delimiter = None;
        for raw in raw_parameters {
            match raw {
                Value::OptionalFormalParameters(o) => {
                    parameters.extend(o.parameters.unwrap_or_default());
                    left_delimiter = Some(o.left_delimiter);
                    right_delimiter = Some(o.right_delimiter);
                }
                other => {
                    let p = self
                        .value_as_node_opt::<FormalParameter>(other)
                        .expect("FormalParameter");
                    parameters.push(p);
                }
            }
        }
        let parameters = self.ast.new_list(parameters);
        let node = self.ast.add(FormalParameterList {
            left_parenthesis,
            parameters,
            left_delimiter,
            right_delimiter,
            right_parenthesis,
        });
        self.push(node);
    }

    pub(crate) fn end_for_statement(&mut self, _end_token: TokenId) {
        let body = self.pop_node::<Statement>();
        let for_loop_parts = self.pop_node::<ForParts>();
        let left_paren = self.pop_token();
        let for_token = self.pop_token();
        let right_parenthesis = self.end_group(left_paren);
        let node = self.ast.add(ForStatement {
            await_keyword: None,
            for_keyword: for_token,
            left_parenthesis: left_paren,
            for_loop_parts: for_loop_parts.upcast(),
            right_parenthesis,
            body,
        });
        self.push(node);
    }

    pub(crate) fn end_for_statement_body(&mut self, _end_token: TokenId) {}

    pub(crate) fn end_function_expression(&mut self, _begin_token: TokenId, _end_token: TokenId) {
        let body = self.pop_node::<FunctionBody>();
        let parameters = self.pop_node_opt::<FormalParameterList>();
        let type_parameters = self.pop_node_opt::<TypeParameterList>();
        let node = self.ast.add(FunctionExpression {
            type_parameters,
            parameters,
            body,
        });
        self.push(node);
    }

    pub(crate) fn end_function_name(
        &mut self,
        _begin_token: TokenId,
        _token: TokenId,
        _is_function_expression: bool,
    ) {
    }

    pub(crate) fn end_function_type(
        &mut self,
        function_token: TokenId,
        question_mark: Option<TokenId>,
    ) {
        let parameters = self.pop_node::<FormalParameterList>();
        let return_type = self.pop_node_opt::<TypeAnnotation>();
        let type_parameters = self.pop_node_opt::<TypeParameterList>();
        let node = self.ast.add(GenericFunctionType {
            return_type,
            function_keyword: function_token,
            type_parameters,
            parameters,
            question: question_mark,
        });
        self.push(node);
    }

    pub(crate) fn end_function_typed_formal_parameter(
        &mut self,
        _name_token: TokenId,
        question: Option<TokenId>,
    ) {
        let formal_parameters = self.pop_node::<FormalParameterList>();
        let return_type = self.pop_node_opt::<TypeAnnotation>();
        let type_parameters = self.pop_node_opt::<TypeParameterList>();

        // Create temporary data that will be attached to the concrete
        // parameter in [endFormalParameter].
        let function_typed_suffix = self.ast.add(FunctionTypedFormalParameterSuffix {
            type_parameters,
            formal_parameters,
            question,
        });
        self.push(FunctionTypedFormalParameterData {
            return_type,
            function_typed_suffix,
        });
    }

    pub(crate) fn end_hide(&mut self, hide_keyword: TokenId) {
        let hidden_names = self.pop_nodes::<SimpleIdentifier>();
        let hidden_names = self.ast.new_list(hidden_names);
        let node = self.ast.add(HideCombinator {
            keyword: hide_keyword,
            hidden_names,
        });
        self.push(node);
    }

    pub(crate) fn end_if_control_flow(&mut self, _token: TokenId) {
        let then_element = self.pop_node::<CollectionElement>();
        let condition = self.pop_parenthesized_condition();
        let if_token = self.pop_token();
        let right_parenthesis = condition.right_parenthesis(&self.ast.tokens);
        let node = self.ast.add(IfElement {
            if_keyword: if_token,
            left_parenthesis: condition.left_parenthesis,
            expression: condition.expression,
            case_clause: condition.case_clause,
            right_parenthesis,
            then_element,
            else_keyword: None,
            else_element: None,
        });
        self.push(node);
    }

    pub(crate) fn end_if_else_control_flow(&mut self, _token: TokenId) {
        let else_element = self.pop_node::<CollectionElement>();
        let else_token = self.pop_token();
        let then_element = self.pop_node::<CollectionElement>();
        let condition = self.pop_parenthesized_condition();
        let if_token = self.pop_token();
        let right_parenthesis = condition.right_parenthesis(&self.ast.tokens);
        let node = self.ast.add(IfElement {
            if_keyword: if_token,
            left_parenthesis: condition.left_parenthesis,
            expression: condition.expression,
            case_clause: condition.case_clause,
            right_parenthesis,
            then_element,
            else_keyword: Some(else_token),
            else_element: Some(else_element),
        });
        self.push(node);
    }

    pub(crate) fn end_if_statement(
        &mut self,
        if_token: TokenId,
        else_token: Option<TokenId>,
        _end_token: TokenId,
    ) {
        let else_part = self
            .pop_if_not_null(else_token)
            .and_then(|v| self.value_as_node_opt::<Statement>(v));
        let then_part = self.pop_node::<Statement>();
        let condition = self.pop_parenthesized_condition();
        let right_parenthesis = condition.right_parenthesis(&self.ast.tokens);
        let node = self.ast.add(IfStatement {
            if_keyword: if_token,
            left_parenthesis: condition.left_parenthesis,
            expression: condition.expression,
            case_clause: condition.case_clause,
            right_parenthesis,
            then_statement: then_part,
            else_keyword: else_token,
            else_statement: else_part,
        });
        self.push(node);
    }

    pub(crate) fn end_implicit_creation_expression(
        &mut self,
        _token: TokenId,
        _open_angle_bracket: TokenId,
    ) {
        self.handle_instance_creation(None);
    }

    pub(crate) fn end_import(&mut self, import_keyword: TokenId, semicolon: Option<TokenId>) {
        let combinators = self.pop_nodes_opt::<Combinator>();
        let deferred_keyword = self.pop_token_opt();
        let as_keyword = self.pop_token_opt();
        let prefix = self.pop_node_opt::<SimpleIdentifier>();
        let configurations = self.pop_nodes_opt::<Configuration>();
        let uri = self.pop_node::<StringLiteral>();
        let metadata = self.pop_metadata();
        let comment = self.find_comment(metadata.as_deref(), import_keyword);

        let metadata = self.node_list(metadata);
        let configurations = self.node_list(configurations);
        let combinators = self.node_list(combinators);
        let semicolon = match semicolon {
            Some(s) => s,
            None => self.detached_token(TokenType::SEMICOLON),
        };
        let directive = self.ast.add(ImportDirective {
            documentation_comment: comment,
            metadata,
            import_keyword,
            uri,
            configurations,
            deferred_keyword,
            as_keyword,
            prefix,
            combinators,
            semicolon,
        });
        self.directives.push(directive.upcast());
    }

    pub(crate) fn end_initialized_identifier(&mut self, _name_token: TokenId) {
        let node = self.pop();
        // TODO(paulberry): This seems kludgy.  It would be preferable if we
        // could respond to a "handleNoVariableInitializer" event by
        // converting a SimpleIdentifier into a VariableDeclaration, and then
        // when this code was reached, node would always be a
        // VariableDeclaration.
        let variable: Id<VariableDeclaration> = match node {
            Value::Node(n) if self.ast.is::<VariableDeclaration>(n) => Id::from_raw(n),
            Value::Node(n) if self.ast.is::<SimpleIdentifier>(n) => {
                let token = self.ast[Id::<SimpleIdentifier>::from_raw(n)].token;
                self.ast.add(VariableDeclaration {
                    documentation_comment: None,
                    metadata: NodeList::EMPTY,
                    name: token,
                    equals: None,
                    initializer: None,
                })
            }
            other => self.internal_problem(&format!("Unhandled {other:?} in identifier.")),
        };
        self.push(variable);
    }

    pub(crate) fn end_initializers(&mut self, count: i32, colon: TokenId, _end_token: TokenId) {
        let initializer_objects = self
            .pop_typed_list_values(count as usize)
            .unwrap_or_default();
        if !self.is_full_ast {
            return;
        }

        self.push(colon);

        let mut initializers: Vec<Id<ConstructorInitializer>> = Vec::new();
        for initializer_object in initializer_objects {
            let initializer = self.build_initializer(&initializer_object);
            if let Some(initializer) = initializer {
                initializers.push(initializer);
            } else {
                let (begin, end) = match initializer_object {
                    Value::Node(n) => (self.begin_token(n), self.end_token(n)),
                    _ => (colon, colon),
                };
                self.handle_recoverable_error(cfe_codes::invalid_initializer(), begin, end);
            }
        }

        self.push(initializers);
    }

    pub(crate) fn end_invalid_await_expression(
        &mut self,
        await_keyword: TokenId,
        _end_token: TokenId,
        _error_code: &'static CfeCode,
    ) {
        // Dart: `endAwaitExpression(awaitKeyword, endToken)`, inlined here
        // (that event is ported in part1.rs).
        let expression = self.pop_node::<Expression>();
        self.report_error_if_super(expression);
        let node = self.ast.add(AwaitExpression {
            await_keyword,
            expression,
        });
        self.push(node);
    }

    pub(crate) fn end_invalid_yield_statement(
        &mut self,
        yield_keyword: TokenId,
        star_token: Option<TokenId>,
        end_token: TokenId,
        _error_code: &'static CfeCode,
    ) {
        // Dart: `endYieldStatement(yieldKeyword, starToken, endToken)`,
        // inlined here (that event is ported in part3.rs).
        let expression = self.pop_node::<Expression>();
        let node = self.ast.add(YieldStatement {
            yield_keyword,
            star: star_token,
            expression,
            semicolon: end_token,
        });
        self.push(node);
    }

    pub(crate) fn end_is_operator_type(&mut self, _operator: TokenId) {}

    pub(crate) fn end_labeled_statement(&mut self, label_count: i32) {
        let statement = self.pop_node::<Statement>();
        let labels = self.pop_typed_list2::<Label>(label_count as usize);
        let labels = self.ast.new_list(labels);
        let node = self.ast.add(LabeledStatement { labels, statement });
        self.push(node);
    }

    pub(crate) fn end_library_augmentation(
        &mut self,
        _augment_keyword: TokenId,
        _library_keyword: TokenId,
        _semicolon: TokenId,
    ) {
        // TODO(scheglov): remove this method
        self.pop_node::<StringLiteral>(); // uri
        self.pop_metadata(); // metadata
    }

    pub(crate) fn end_library_name(
        &mut self,
        library_keyword: TokenId,
        semicolon: TokenId,
        has_name: bool,
    ) {
        let library_name = if has_name {
            self.pop_tokens_opt()
        } else {
            None
        };

        if !has_name && !self.enable_unnamed_libraries {
            self.report_feature_not_enabled(
                ExperimentalFlag::UnnamedLibraries,
                library_keyword,
                None,
            );
        }
        let mut name = None;
        if let Some(library_name) = library_name {
            let tokens = self.ast.new_token_list(library_name);
            name = Some(self.ast.add(DottedName { tokens }));
        }
        let metadata = self.pop_metadata();
        let comment = self.find_comment(metadata.as_deref(), library_keyword);
        let metadata = self.node_list(metadata);
        let directive = self.ast.add(LibraryDirective {
            documentation_comment: comment,
            metadata,
            library_keyword,
            name,
            semicolon,
        });
        self.directives.push(directive.upcast());
    }

    pub(crate) fn end_literal_string(&mut self, interpolation_count: i32, _end_token: TokenId) {
        if interpolation_count == 0 {
            let token = self.pop_token();
            let lexeme = self.lexeme(token).to_string();
            let mut errors = Vec::new();
            let value = unescape_string(&lexeme, &mut |m, o, l| errors.push((m, o, l)));
            self.report_unescape_errors(errors, token);
            let node = self.ast.add(SimpleStringLiteral {
                literal: token,
                value: value.into(),
            });
            self.push(node);
        } else {
            let parts = self
                .pop_typed_list_values(1 + interpolation_count as usize * 2)
                .expect("parts");
            let first = match parts.first() {
                Some(Value::Token(t)) => *t,
                other => panic!("{other:?} is not a subtype of type 'Token'"),
            };
            let last = match parts.last() {
                Some(Value::Token(t)) => *t,
                other => panic!("{other:?} is not a subtype of type 'Token'"),
            };
            let first_lexeme = self.lexeme(first).to_string();
            let quote = analyze_quote(&first_lexeme);
            let mut elements: Vec<Id<InterpolationElement>> = Vec::new();
            let mut errors = Vec::new();
            let value = unescape_first_string_part(&first_lexeme, quote, &mut |m, o, l| {
                errors.push((m, o, l))
            });
            self.report_unescape_errors(errors, first);
            let element = self.ast.add(InterpolationString {
                contents: first,
                value: value.into(),
            });
            elements.push(element.upcast());
            let parts_len = parts.len();
            for part in parts.into_iter().take(parts_len - 1).skip(1) {
                match part {
                    Value::Token(part) => {
                        let lexeme = self.lexeme(part).to_string();
                        let mut errors = Vec::new();
                        let value = unescape(&lexeme, quote, &mut |m, o, l| errors.push((m, o, l)));
                        self.report_unescape_errors(errors, part);
                        let element = self.ast.add(InterpolationString {
                            contents: part,
                            value: value.into(),
                        });
                        elements.push(element.upcast());
                    }
                    Value::Node(n) if self.ast.is::<InterpolationExpression>(n) => {
                        elements.push(Id::from_raw(n));
                    }
                    other => self
                        .internal_problem(&format!("Unhandled {other:?} in string interpolation.")),
                }
            }
            let last_lexeme = self.lexeme(last).to_string();
            let last_is_synthetic = self.ast.tokens.get(last).is_synthetic();
            let mut errors = Vec::new();
            let value = unescape_last_string_part(
                &last_lexeme,
                quote,
                last_is_synthetic,
                &mut |m, o, l| errors.push((m, o, l)),
            );
            self.report_unescape_errors(errors, last);
            let element = self.ast.add(InterpolationString {
                contents: last,
                value: value.into(),
            });
            elements.push(element.upcast());
            let elements = self.ast.new_list(elements);
            let node = self.ast.add(StringInterpolation { elements });
            self.push(node);
        }
    }

    pub(crate) fn end_literal_symbol(&mut self, hash_token: TokenId, token_count: i32) {
        let components = self.pop_typed_list2_tokens(token_count as usize);
        let components = self.ast.new_token_list(components);
        let node = self.ast.add(SymbolLiteral {
            pound_sign: hash_token,
            components,
        });
        self.push(node);
    }

    pub(crate) fn end_local_function_declaration(&mut self, _token: TokenId) {
        let body = self.pop_node::<FunctionBody>();
        if self.is_full_ast {
            self.pop(); // constructor initializers
            self.pop(); // separator before constructor initializers
        }
        let parameters = self.pop_node::<FormalParameterList>();
        let name = self.pop_node::<SimpleIdentifier>();
        let return_type = self.pop_node_opt::<TypeAnnotation>();
        let type_parameters = self.pop_node_opt::<TypeParameterList>();
        let metadata = self.pop_metadata();
        let function_expression = self.ast.add(FunctionExpression {
            type_parameters,
            parameters: Some(parameters),
            body,
        });
        let metadata = self.node_list(metadata);
        let name = self.ast[name].token;
        let function_declaration = self.ast.add(FunctionDeclaration {
            documentation_comment: None,
            metadata,
            augment_keyword: None,
            external_keyword: None,
            return_type,
            property_keyword: None,
            name,
            function_expression,
        });
        let node = self.ast.add(FunctionDeclarationStatement {
            function_declaration,
        });
        self.push(node);
    }

    pub(crate) fn end_member(&mut self) {}

    pub(crate) fn end_metadata(
        &mut self,
        at_sign: TokenId,
        period_before_name: Option<TokenId>,
        _end_token: TokenId,
    ) {
        let argument_list = self.pop_node_opt::<ArgumentList>();
        let constructor_name = if period_before_name.is_some() {
            Some(self.pop_node::<SimpleIdentifier>())
        } else {
            None
        };
        let type_arguments = self.pop_node_opt::<TypeArgumentList>();
        if let Some(type_arguments) = type_arguments {
            if !self.is_enabled(ExperimentalFlag::GenericMetadata) {
                let begin = self.begin_token(type_arguments);
                self.report_feature_not_enabled(ExperimentalFlag::GenericMetadata, begin, None);
            }
        }
        let name = self.pop_node::<Identifier>();
        let node = self.ast.add(Annotation {
            at_sign,
            name,
            type_arguments,
            period: period_before_name,
            constructor_name,
            arguments: argument_list,
        });
        self.push(node);
    }

    pub(crate) fn end_metadata_star(&mut self, count: i32) {
        let list = self.pop_typed_list::<Annotation>(count as usize);
        self.push_nodes_or(list, NullValue::Metadata);
    }

    pub(crate) fn end_method(
        &mut self,
        _kind: DeclarationKind,
        get_or_set: Option<TokenId>,
        begin_token: TokenId,
        begin_param: TokenId,
        begin_initializers: Option<TokenId>,
        end_token: TokenId,
    ) {
        self.end_class_method(
            get_or_set,
            begin_token,
            begin_param,
            begin_initializers,
            end_token,
        );
    }

    pub(crate) fn end_mixin_declaration(&mut self, _begin_token: TokenId, _end_token: TokenId) {
        let builder = self.take_class_like_builder();
        let declaration = builder.build_mixin(&mut self.ast);
        self.declarations.push(declaration.upcast());
    }

    pub(crate) fn end_named_function_expression(&mut self, _end_token: TokenId) {
        let body = self.pop_node::<FunctionBody>();
        if self.is_full_ast {
            self.pop(); // constructor initializers
            self.pop(); // separator before constructor initializers
        }
        let parameters = self.pop_node::<FormalParameterList>();
        self.pop(); // name
        self.pop(); // returnType
        let type_parameters = self.pop_node_opt::<TypeParameterList>();
        let node = self.ast.add(FunctionExpression {
            type_parameters,
            parameters: Some(parameters),
            body,
        });
        self.push(node);
    }

    pub(crate) fn end_named_mixin_application(
        &mut self,
        begin_token: TokenId,
        class_keyword: TokenId,
        equals_token: TokenId,
        implements_keyword: Option<TokenId>,
        semicolon: TokenId,
    ) {
        let mut implements_clause = None;
        if let Some(implements_keyword) = implements_keyword {
            let interfaces = self.pop_named_type_list(diag::expected_named_type_implements);
            let interfaces = self.ast.new_list(interfaces);
            implements_clause = Some(self.ast.add(ImplementsClause {
                implements_keyword,
                interfaces,
            }));
        }
        let with_clause = self.pop_node::<WithClause>();
        let superclass_type = self.pop_node::<TypeAnnotation>();
        let superclass: Id<NamedType> = match self.ast.cast::<NamedType>(superclass_type) {
            Some(n) => n,
            None => {
                self.report_at_node(diag::expected_named_type_extends(), superclass_type);
                let begin_token = self.begin_token(superclass_type);
                let end_token = self.end_token(superclass_type);
                let mut current_token = begin_token;
                let mut count = 1;
                while current_token != end_token {
                    count += 1;
                    current_token = self.next(current_token);
                }
                let previous = self.ast.tokens.previous(begin_token);
                let name_token = self.rewriter().replace_next_tokens_with_synthetic_token(
                    previous,
                    count,
                    TokenType::IDENTIFIER,
                );
                self.ast.add(NamedType {
                    import_prefix: None,
                    name: name_token,
                    type_arguments: None,
                    question: None,
                })
            }
        };
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
        let comment = self.find_comment(metadata.as_deref(), begin_token);
        let metadata = self.node_list(metadata);
        let name = self.ast[name].token;
        let declaration = self.ast.add(ClassTypeAlias {
            documentation_comment: comment,
            metadata,
            augment_keyword,
            abstract_keyword,
            sealed_keyword,
            base_keyword,
            interface_keyword,
            final_keyword,
            mixin_keyword,
            typedef_keyword: class_keyword,
            name,
            type_parameters,
            equals: equals_token,
            superclass,
            with_clause,
            implements_clause,
            semicolon,
        });
        self.declarations.push(declaration.upcast());
    }

    pub(crate) fn end_new_expression(&mut self, new_keyword: TokenId) {
        self.handle_instance_creation(Some(new_keyword));
    }

    pub(crate) fn end_optional_formal_parameters(
        &mut self,
        count: i32,
        left_delimiter: TokenId,
        right_delimiter: TokenId,
        _kind: MemberKind,
    ) {
        let parameters = self.pop_typed_list2::<FormalParameter>(count as usize);
        self.push(OptionalFormalParameters {
            parameters: Some(parameters),
            left_delimiter,
            right_delimiter,
        });
    }

    pub(crate) fn end_parenthesized_expression(&mut self, left_parenthesis: TokenId) {
        let expression = self.pop_node::<Expression>();
        self.report_error_if_super(expression);
        let right_parenthesis = self.end_group(left_parenthesis);
        let node = self.ast.add(ParenthesizedExpression {
            left_parenthesis,
            expression,
            right_parenthesis,
        });
        self.push(node);
    }

    pub(crate) fn end_part(&mut self, part_keyword: TokenId, semicolon: TokenId) {
        let uri = self.pop_node::<StringLiteral>();
        let metadata = self.pop_metadata();
        let comment = self.find_comment(metadata.as_deref(), part_keyword);
        let metadata = self.node_list(metadata);
        let directive = self.ast.add(PartDirective {
            documentation_comment: comment,
            metadata,
            part_keyword,
            uri,
            semicolon,
        });
        self.directives.push(directive.upcast());
    }

    pub(crate) fn end_part_of(
        &mut self,
        part_keyword: TokenId,
        of_keyword: TokenId,
        semicolon: TokenId,
        _has_name: bool,
    ) {
        let library_name_or_uri = self.pop();
        let mut name = None;
        let mut uri = None;
        match library_name_or_uri {
            Value::Node(n) if self.ast.is::<StringLiteral>(n) => {
                uri = Some(Id::<StringLiteral>::from_raw(n));
            }
            Value::Tokens(library_name) => {
                let tokens = self.ast.new_token_list(library_name);
                let dotted = self.ast.add(DottedName { tokens });
                name = Some(dotted);
                if self.is_enabled(ExperimentalFlag::EnhancedParts) {
                    self.report_at_node(diag::part_of_name(), dotted);
                }
            }
            other => panic!("{other:?} is not a subtype of type 'List<Token>'"),
        }
        let metadata = self.pop_metadata();
        let comment = self.find_comment(metadata.as_deref(), part_keyword);
        let metadata = self.node_list(metadata);
        let directive = self.ast.add(PartOfDirective {
            documentation_comment: comment,
            metadata,
            part_keyword,
            of_keyword,
            uri,
            library_name: name,
            semicolon,
        });
        self.directives.push(directive.upcast());
    }

    pub(crate) fn end_pattern(&mut self, _token: TokenId) {}

    pub(crate) fn end_pattern_guard(&mut self, when: TokenId) {
        let expression = self.pop_node::<Expression>();
        let node = self.ast.add(WhenClause {
            when_keyword: when,
            expression,
        });
        self.push(node);
    }

    pub(crate) fn end_primary_constructor(
        &mut self,
        kind: DeclarationKind,
        begin_token: TokenId,
        _end_token: TokenId,
        const_keyword: Option<TokenId>,
        has_constructor_name: bool,
    ) {
        let formal_parameter_list = match self.pop_node_opt::<FormalParameterList>() {
            Some(l) => l,
            None => {
                let extension_type_name = self.ast.tokens.previous(begin_token);
                assert!(
                    extension_type_name.is_some(),
                    "Null check operator used on a null value"
                );
                self.synthetic_formal_parameter_list(extension_type_name)
            }
        };

        match kind {
            DeclarationKind::TopLevel | DeclarationKind::Mixin | DeclarationKind::Extension => {
                // Invalid. Error reported in the parser.
            }
            DeclarationKind::ExtensionType => {
                // Always valid.
            }
            DeclarationKind::Class | DeclarationKind::Enum => {
                if !self.is_enabled(ExperimentalFlag::PrimaryConstructors) {
                    self.report_feature_not_enabled(
                        ExperimentalFlag::PrimaryConstructors,
                        begin_token,
                        None,
                    );
                }
            }
        }

        let mut constructor_name = None;
        if has_constructor_name {
            let name_identifier = self.pop_node::<SimpleIdentifier>();
            let name = self.ast[name_identifier].token;
            constructor_name = Some(self.ast.add(PrimaryConstructorName {
                period: begin_token,
                name,
            }));
        }

        self.push_token_or(const_keyword, NullValue::Token);

        self.push(PrimaryConstructorBuilder {
            const_keyword,
            constructor_name,
            formal_parameter_list,
        });
    }

    pub(crate) fn end_primary_constructor_body(
        &mut self,
        begin_token: TokenId,
        _begin_initializers: Option<TokenId>,
        _end_token: TokenId,
    ) {
        let body = self.pop_node::<FunctionBody>();
        let initializers = self
            .pop_nodes_opt::<ConstructorInitializer>()
            .unwrap_or_default();
        let colon = self.pop_token_opt();
        let metadata = self.pop_metadata();
        let comment = self.find_comment(metadata.as_deref(), begin_token);

        let metadata = self.node_list(metadata);
        let initializers = self.ast.new_list(initializers);
        let member = self.ast.add(PrimaryConstructorBody {
            documentation_comment: comment,
            metadata,
            this_keyword: begin_token,
            colon,
            initializers,
            body,
        });
        self.class_like_builder
            .as_mut()
            .expect("Null check operator used on a null value (_classLikeBuilder)")
            .members
            .push(member.upcast());
    }

    pub(crate) fn end_record_literal(
        &mut self,
        left_parenthesis: TokenId,
        count: i32,
        const_keyword: Option<TokenId>,
    ) {
        let fields = self
            .pop_typed_list::<RecordLiteralField>(count as usize)
            .unwrap_or_default();
        let right_parenthesis = self.end_group(left_parenthesis);

        if self.enable_records {
            let fields = self.ast.new_list(fields);
            let node = self.ast.add(RecordLiteral {
                const_keyword,
                left_parenthesis,
                fields,
                right_parenthesis,
            });
            self.push(node);
        } else {
            self.report_feature_not_enabled(ExperimentalFlag::Records, left_parenthesis, None);

            let expression = match fields.first() {
                Some(&f) => self.field_expression(f),
                None => {
                    let token = self
                        .rewriter()
                        .insert_synthetic_identifier(left_parenthesis, "");
                    self.ast.add(SimpleIdentifier { token }).upcast()
                }
            };

            let node = self.ast.add(ParenthesizedExpression {
                left_parenthesis,
                expression,
                right_parenthesis,
            });
            self.push(node);
        }
    }

    pub(crate) fn end_record_type(
        &mut self,
        left_bracket: TokenId,
        question_mark: Option<TokenId>,
        count: i32,
        _has_named_fields: bool,
    ) {
        let mut named_fields = None;
        let mut elements = self
            .pop_typed_list_values(count as usize)
            .unwrap_or_default();
        if let Some(Value::Node(last)) = elements.last() {
            if let Some(n) = self.ast.cast::<RecordTypeAnnotationNamedFields>(*last) {
                elements.pop();
                named_fields = Some(n);
            }
        }
        let mut positional_fields: Vec<Id<RecordTypeAnnotationPositionalField>> = Vec::new();
        for elem in elements {
            let f = self
                .value_as_node_opt::<RecordTypeAnnotationPositionalField>(elem)
                .expect("RecordTypeAnnotationPositionalField");
            positional_fields.push(f);
        }

        if self.enable_records {
            let positional_fields = self.ast.new_list(positional_fields);
            let right_parenthesis = self.end_group(left_bracket);
            let node = self.ast.add(RecordTypeAnnotation {
                left_parenthesis: left_bracket,
                positional_fields,
                named_fields,
                right_parenthesis,
                question: question_mark,
            });
            self.push(node);
        } else {
            self.report_feature_not_enabled(ExperimentalFlag::Records, left_bracket, None);

            let name = self
                .rewriter()
                .insert_synthetic_identifier(left_bracket, "");
            let node = self.ast.add(NamedType {
                import_prefix: None,
                name,
                type_arguments: None,
                question: question_mark,
            });
            self.push(node);
        }
    }

    pub(crate) fn end_record_type_entry(&mut self) {
        let name = self.pop_node_opt::<SimpleIdentifier>();
        let type_ = self.pop_node::<TypeAnnotation>();
        let metadata = self.pop_metadata();

        let metadata = self.node_list(metadata);
        let name = name.map(|n| self.ast[n].token);
        let node = self.ast.add(RecordTypeAnnotationPositionalField {
            metadata,
            type_,
            name,
        });
        self.push(node);
    }

    pub(crate) fn end_record_type_named_fields(&mut self, count: i32, left_bracket: TokenId) {
        let elements = self
            .pop_typed_list::<RecordTypeAnnotationPositionalField>(count as usize)
            .unwrap_or_default();
        let mut fields: Vec<Id<RecordTypeAnnotationNamedField>> = Vec::new();
        for elem in elements {
            let e = self.ast[elem].clone();
            let metadata: Vec<Id<Annotation>> = self.ast.list(e.metadata).to_vec();
            let metadata = self.ast.new_list(metadata);
            let field = self.ast.add(RecordTypeAnnotationNamedField {
                metadata,
                type_: e.type_,
                name: e.name.expect("Null check operator used on a null value"),
            });
            fields.push(field);
        }
        let fields = self.ast.new_list(fields);
        let right_bracket = self.end_group(left_bracket);
        let node = self.ast.add(RecordTypeAnnotationNamedFields {
            left_bracket,
            fields,
            right_bracket,
        });
        self.push(node);
    }

    pub(crate) fn end_redirecting_factory_body(
        &mut self,
        equal_token: TokenId,
        _end_token: TokenId,
    ) {
        let constructor_name = self.pop_node::<ConstructorName>();
        let star_token = self.pop_token_opt();
        let async_token = self.pop_token_opt();
        self.push(RedirectingFactoryBody {
            async_keyword: async_token,
            star_keyword: star_token,
            equal_token,
            constructor_name,
        });
    }

    pub(crate) fn end_rethrow_statement(&mut self, rethrow_token: TokenId, semicolon: TokenId) {
        let expression = self.ast.add(RethrowExpression {
            rethrow_keyword: rethrow_token,
        });
        // TODO(scheglov): According to the specification, 'rethrow' is a
        // statement.
        let node = self.ast.add(ExpressionStatement {
            expression: expression.upcast(),
            semicolon: Some(semicolon),
        });
        self.push(node);
    }

    pub(crate) fn end_return_statement(
        &mut self,
        has_expression: bool,
        return_keyword: TokenId,
        semicolon: TokenId,
    ) {
        let expression = if has_expression {
            Some(self.pop_node::<Expression>())
        } else {
            None
        };
        let node = self.ast.add(ReturnStatement {
            return_keyword,
            expression,
            semicolon,
        });
        self.push(node);
    }

    pub(crate) fn end_show(&mut self, show_keyword: TokenId) {
        let shown_names = self.pop_nodes::<SimpleIdentifier>();
        let shown_names = self.ast.new_list(shown_names);
        let node = self.ast.add(ShowCombinator {
            keyword: show_keyword,
            shown_names,
        });
        self.push(node);
    }

    pub(crate) fn end_switch_block(
        &mut self,
        case_count: i32,
        left_bracket: TokenId,
        right_bracket: TokenId,
    ) {
        // Dart `popTypedList2<List<SwitchMemberImpl>>(caseCount)`.
        let members_list = self.pop_list_values(case_count as usize);
        let mut members: Vec<Id<SwitchMember>> = Vec::new();
        for list in members_list {
            match list {
                Some(Value::Nodes(nodes)) => {
                    members.extend(nodes.into_iter().map(Id::<SwitchMember>::from_raw));
                }
                other => panic!("{other:?} is not a subtype of type 'List<SwitchMemberImpl>'"),
            }
        }

        let mut labels: FxHashSet<String> = FxHashSet::default();
        for &member in &members {
            for label in self.switch_member_labels(member) {
                let name = self.ast[label].name;
                let lexeme = self.lexeme(name).to_string();
                if !labels.insert(lexeme.clone()) {
                    let begin = self.begin_token(label);
                    self.handle_recoverable_error(
                        cfe_codes::duplicate_label_in_switch_statement(&lexeme),
                        begin,
                        begin,
                    );
                }
            }
        }

        self.push(left_bracket);
        self.push(members);
        self.push(right_bracket);
    }

    /// Dart `SwitchMember.labels`.
    fn switch_member_labels(&self, member: Id<SwitchMember>) -> Vec<Id<Label>> {
        let n: NodeId = member.raw();
        let list = if let Some(c) = self.ast.cast::<SwitchCase>(n) {
            self.ast[c].labels
        } else if let Some(d) = self.ast.cast::<SwitchDefault>(n) {
            self.ast[d].labels
        } else if let Some(p) = self.ast.cast::<SwitchPatternCase>(n) {
            self.ast[p].labels
        } else {
            panic!("UnimplementedError: SwitchMember {:?}", self.ast.kind(n))
        };
        self.ast.list(list).to_vec()
    }

    /// A copy of [list] or the elements of [current].
    fn new_list_or_copy<T: ?Sized>(
        &mut self,
        list: Option<Vec<Id<T>>>,
        current: NodeList<T>,
    ) -> NodeList<T> {
        match list {
            Some(l) => self.ast.new_list(l),
            None => {
                let l = self.ast.list(current).to_vec();
                self.ast.new_list(l)
            }
        }
    }

    /// Dart `updateSwitchMember` (local function of `endSwitchCase`).
    fn update_switch_member(
        &mut self,
        member: Id<SwitchMember>,
        labels: Option<Vec<Id<Label>>>,
        statements: Option<Vec<Id<Statement>>>,
    ) -> Id<SwitchMember> {
        let n: NodeId = member.raw();
        if let Some(c) = self.ast.cast::<SwitchCase>(n) {
            let m = self.ast[c].clone();
            let labels = self.new_list_or_copy(labels, m.labels);
            let statements = self.new_list_or_copy(statements, m.statements);
            self.ast
                .add(SwitchCase {
                    labels,
                    keyword: m.keyword,
                    expression: m.expression,
                    colon: m.colon,
                    statements,
                })
                .upcast()
        } else if let Some(d) = self.ast.cast::<SwitchDefault>(n) {
            let m = self.ast[d].clone();
            let labels = self.new_list_or_copy(labels, m.labels);
            let statements = self.new_list_or_copy(statements, m.statements);
            self.ast
                .add(SwitchDefault {
                    labels,
                    keyword: m.keyword,
                    colon: m.colon,
                    statements,
                })
                .upcast()
        } else if let Some(p) = self.ast.cast::<SwitchPatternCase>(n) {
            let m = self.ast[p].clone();
            let labels = self.new_list_or_copy(labels, m.labels);
            let statements = self.new_list_or_copy(statements, m.statements);
            self.ast
                .add(SwitchPatternCase {
                    labels,
                    keyword: m.keyword,
                    guarded_pattern: m.guarded_pattern,
                    colon: m.colon,
                    statements,
                })
                .upcast()
        } else {
            panic!("UnimplementedError: ({:?})", self.ast.kind(n))
        }
    }

    /// Dart `popLabels` (local function of `endSwitchCase`).
    fn pop_labels(&mut self, label_count: &mut i32) -> Vec<Id<Label>> {
        let mut labels = Vec::new();
        while let Some(Value::Node(n)) = self.peek() {
            let n = *n;
            if !self.ast.is::<Label>(n) {
                break;
            }
            self.pop();
            labels.insert(0, Id::<Label>::from_raw(n));
            *label_count -= 1;
        }
        labels
    }

    pub(crate) fn end_switch_case(
        &mut self,
        label_count: i32,
        expression_count: i32,
        default_keyword: Option<TokenId>,
        colon_after_default: Option<TokenId>,
        statement_count: i32,
        _begin_token: TokenId,
        _end_token: TokenId,
    ) {
        let mut label_count = label_count;
        let statements = self.pop_typed_list2::<Statement>(statement_count as usize);
        let members: Vec<Option<Id<SwitchMember>>>;

        if label_count == 0 && default_keyword.is_none() {
            // Common situation: case with no default and no labels.
            members = self
                .pop_typed_list2::<SwitchMember>(expression_count as usize)
                .into_iter()
                .map(Some)
                .collect();
        } else {
            // Labels and case statements may be intertwined
            let mut list: Vec<Option<Id<SwitchMember>>>;
            if let Some(default_keyword) = default_keyword {
                let labels = self.pop_labels(&mut label_count);
                let labels = self.ast.new_list(labels);
                let member = self.ast.add(SwitchDefault {
                    labels,
                    keyword: default_keyword,
                    colon: colon_after_default.expect("Null check operator used on a null value"),
                    statements: NodeList::EMPTY,
                });
                list = vec![None; expression_count as usize + 1];
                list[expression_count as usize] = Some(member.upcast());
            } else {
                list = vec![None; expression_count as usize];
            }
            for index in (0..expression_count as usize).rev() {
                let member = self.pop_node::<SwitchMember>();
                let labels = self.pop_labels(&mut label_count);
                list[index] = Some(self.update_switch_member(member, Some(labels), None));
            }
            debug_assert_eq!(label_count, 0);
            members = list;
        }

        let mut members2: Vec<Id<SwitchMember>> = members.into_iter().flatten().collect();
        if let Some(&last) = members2.last() {
            let updated = self.update_switch_member(last, None, Some(statements));
            *members2.last_mut().unwrap() = updated;
        }
        self.push(members2);
    }

    pub(crate) fn end_switch_case_when_clause(&mut self, _token: TokenId) {}
}
