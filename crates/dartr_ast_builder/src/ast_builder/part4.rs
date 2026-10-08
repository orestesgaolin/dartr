// Dart source: pkg/analyzer/lib/src/fasta/ast_builder.dart (lines 4400-5717)

use dartr_ast::{
    Block, BlockFunctionBody, BooleanLiteral, CaseClause, CollectionElement, ConstructorName,
    DartPattern, DoubleLiteral, EnumConstantDeclaration, Expression, FormalParameterDefaultClause,
    FunctionReference, GuardedPattern, Id, Identifier, ImplementsClause, ImportDirective,
    ImportPrefixReference, IndexExpression, IntegerLiteral, InterpolationExpression, IsExpression,
    Label, ListLiteral, ListPattern, ListPatternElement, MapLiteralEntry, MapPattern,
    MapPatternElement, MapPatternEntry, MixinOnClause, NamedArgument, NamedType,
    NativeFunctionBody, NodeList, NullAssertPattern, NullAwareElement, NullCheckPattern,
    NullLiteral, ObjectPattern, ParenthesizedPattern, PatternAssignment, PatternField,
    PatternFieldName, PatternVariableDeclaration, PatternVariableDeclarationStatement,
    PostfixExpression, PrefixExpression, PrefixedIdentifier, PropertyAccess,
    RecordLiteralNamedField, RecordPattern, RelationalPattern, RestPatternElement, ScriptTag,
    SetOrMapLiteral, SimpleIdentifier, SpreadElement, Statement, StringLiteral, SuperExpression,
    ThisExpression, ThrowExpression, TypeAnnotation, TypeArgumentList, TypeParameter,
    VariableDeclaration, WhenClause, WildcardPattern, WithClause,
};
use dartr_diagnostics::{cfe_codes, diag};
use dartr_parser::declaration_kind::{DeclarationHeaderKind, DeclarationKind};
use dartr_parser::experimental_features::ExperimentalFlag;
use dartr_parser::formal_parameter_kind::FormalParameterKind;
use dartr_parser::identifier_context::IdentifierContext;
use dartr_parser::util::{bool_from_token, double_from_token, int_from_token};
use dartr_syntax::{TokenId, TokenType};

use super::{
    AstBuilder, ClassLikeDeclarationBuilder, ClassLikeKind, ConstructorNameWithInvalidTypeArgs,
    MixinDeclarationBuilder, ObjectPatternFields, OperatorName, ParenthesizedCondition,
};
use crate::stack::{NullValue, Value};

impl AstBuilder {
    pub(crate) fn handle_identifier(&mut self, token: TokenId, context: IdentifierContext) {
        if context.in_symbol()
            || matches!(
                context,
                IdentifierContext::DottedName
                    | IdentifierContext::DottedNameContinuation
                    | IdentifierContext::LabelDeclaration
                    | IdentifierContext::LabelReference
            )
        {
            self.push(token);
            return;
        }

        if context.in_library_or_part_of_declaration() {
            if !context.is_continuation() {
                self.push(vec![token]);
            } else {
                self.push(token);
            }
        } else if context == IdentifierContext::EnumValueDeclaration {
            let metadata = self.pop_metadata();
            let comment = self.find_comment(metadata.as_deref(), token);

            let mut augment_keyword = None;
            if let Some(previous) = self.ast.tokens.previous(token).get() {
                if self.optional("augment", previous) {
                    augment_keyword = Some(previous);
                }
            }

            let metadata = self.node_list(metadata);
            let node = self.ast.add(EnumConstantDeclaration {
                documentation_comment: comment,
                metadata,
                augment_keyword,
                name: token,
                arguments: None,
            });
            self.push(node);
        } else {
            let identifier = self.ast.add(SimpleIdentifier { token });
            self.push(identifier);
        }
    }

    pub(crate) fn handle_identifier_list(&mut self, count: i32) {
        let list = self.pop_typed_list::<SimpleIdentifier>(count as usize);
        self.push_nodes_or(list, NullValue::IdentifierList);
    }

    pub(crate) fn handle_implements(
        &mut self,
        implements_keyword: Option<TokenId>,
        interfaces_count: i32,
    ) {
        if let Some(implements_keyword) = implements_keyword {
            self.end_type_list_impl_for_part4(interfaces_count);
            let interfaces = self.pop_named_type_list(diag::expected_named_type_implements);
            let interfaces = self.ast.new_list(interfaces);
            let node = self.ast.add(ImplementsClause {
                implements_keyword,
                interfaces,
            });
            self.push(node);
        } else {
            self.push(NullValue::IdentifierList);
        }
    }

    /// Dart `endTypeList(count)` (part3 defines `end_type_list`; this is the
    /// same code so that this part compiles on its own).
    fn end_type_list_impl_for_part4(&mut self, count: i32) {
        let list = self.pop_typed_list::<TypeAnnotation>(count as usize);
        self.push_nodes_or(list, NullValue::TypeList);
    }

    pub(crate) fn handle_import_prefix(
        &mut self,
        deferred_keyword: Option<TokenId>,
        as_keyword: Option<TokenId>,
    ) {
        match as_keyword {
            None => {
                // If asKeyword is null, then no prefix has been pushed on the
                // stack. Push a placeholder indicating that there is no
                // prefix.
                self.push(NullValue::Prefix);
                self.push(NullValue::As);
            }
            Some(as_keyword) => self.push(as_keyword),
        }
        self.push_token_or(deferred_keyword, NullValue::Deferred);
    }

    pub(crate) fn handle_indexed_expression(
        &mut self,
        question: Option<TokenId>,
        open_square_bracket: TokenId,
        close_square_bracket: TokenId,
    ) {
        let index = self.pop_node::<Expression>();
        let target = self.pop_node_opt::<Expression>();
        self.report_error_if_super(index);
        match target {
            None => {
                let receiver = self.pop_node::<dartr_ast::CascadeExpression>();
                let token = match self.peek() {
                    Some(Value::Token(t)) => *t,
                    other => panic!("{other:?} is not a subtype of type 'Token'"),
                };
                self.push(receiver);
                let expression = self.ast.add(IndexExpression {
                    target: None,
                    period: Some(token),
                    question,
                    left_bracket: open_square_bracket,
                    index,
                    right_bracket: close_square_bracket,
                });
                self.push(expression);
            }
            Some(target) => {
                let node = self.ast.add(IndexExpression {
                    target: Some(target),
                    period: None,
                    question,
                    left_bracket: open_square_bracket,
                    index,
                    right_bracket: close_square_bracket,
                });
                self.push(node);
            }
        }
    }

    pub(crate) fn handle_interpolation_expression(
        &mut self,
        left_bracket: TokenId,
        right_bracket: Option<TokenId>,
    ) {
        let expression = self.pop_node::<Expression>();
        let node = self.ast.add(InterpolationExpression {
            left_bracket,
            expression,
            right_bracket,
        });
        self.push(node);
    }

    pub(crate) fn handle_invalid_expression(&mut self, _token: TokenId) {}

    pub(crate) fn handle_invalid_function_body(&mut self, token: TokenId) {
        let left_bracket = token;
        let right_bracket = self.end_group(left_bracket);
        let block = self.ast.add(Block {
            left_bracket,
            statements: NodeList::<Statement>::EMPTY,
            right_bracket,
        });
        let star = self.pop_token_opt();
        let async_keyword = self.pop_token_opt();
        let node = self.ast.add(BlockFunctionBody {
            keyword: async_keyword,
            star,
            block,
        });
        self.push(node);
    }

    pub(crate) fn handle_invalid_member(&mut self, _end_token: TokenId) {
        self.pop(); // metadata star
    }

    pub(crate) fn handle_invalid_operator_name(
        &mut self,
        operator_keyword: TokenId,
        token: TokenId,
    ) {
        let name = self.ast.add(SimpleIdentifier { token });
        self.push(OperatorName {
            operator_keyword,
            name,
        });
    }

    pub(crate) fn handle_invalid_top_level_block(&mut self, _token: TokenId) {
        // TODO(danrubel): Consider improved recovery by adding this block
        // as part of a synthetic top level function.
        self.pop(); // block
    }

    pub(crate) fn handle_invalid_top_level_declaration(&mut self, _end_token: TokenId) {
        self.pop(); // metadata star
        // TODO(danrubel): consider creating a AST node
        // representing the invalid declaration to better support code
        // completion, quick fixes, etc, rather than discarding the metadata
        // and token
    }

    pub(crate) fn handle_invalid_type_arguments(&mut self, _token: TokenId) {
        let invalid_type_args = self.pop_node::<TypeArgumentList>();
        let node = self.pop();
        match node {
            Value::Node(n) if self.ast.is::<ConstructorName>(n) => {
                self.push(ConstructorNameWithInvalidTypeArgs {
                    name: Id::from_raw(n),
                    invalid_type_args,
                });
            }
            other => panic!(
                "UnimplementedError: node is an instance of {other:?} in handleInvalidTypeArguments"
            ),
        }
    }

    pub(crate) fn handle_is_operator(&mut self, is_operator: TokenId, not: Option<TokenId>) {
        let type_ = self.pop_node::<TypeAnnotation>();
        let expression = self.pop_node::<Expression>();
        self.report_error_if_super(expression);

        let node = self.ast.add(IsExpression {
            expression,
            is_operator,
            not_operator: not,
            type_,
        });
        self.push(node);
    }

    pub(crate) fn handle_label(&mut self, token: TokenId) {
        let colon = token;
        let name = self.pop_token();
        let node = self.ast.add(Label { name, colon });
        self.push(node);
    }

    pub(crate) fn handle_list_pattern(
        &mut self,
        count: i32,
        left_bracket: TokenId,
        right_bracket: TokenId,
    ) {
        let elements = self.pop_typed_list2::<ListPatternElement>(count as usize);
        let type_arguments = self.pop_node_opt::<TypeArgumentList>();
        let elements = self.ast.new_list(elements);
        let node = self.ast.add(ListPattern {
            type_arguments,
            left_bracket,
            elements,
            right_bracket,
        });
        self.push(node);
    }

    pub(crate) fn handle_literal_bool(&mut self, token: TokenId) {
        let value = bool_from_token(&self.ast.tokens, token);
        let node = self.ast.add(BooleanLiteral {
            literal: token,
            value,
        });
        self.push(node);
    }

    pub(crate) fn handle_literal_double(&mut self, token: TokenId) {
        // Dart `double.parse(token.lexeme)`.
        let value = double_from_token(&self.ast.tokens, token, false);
        let node = self.ast.add(DoubleLiteral {
            literal: token,
            value,
        });
        self.push(node);
    }

    pub(crate) fn handle_literal_double_with_separators(&mut self, token: TokenId) {
        if !self.enable_digit_separators {
            self.report_feature_not_enabled(ExperimentalFlag::DigitSeparators, token, None);
        }

        let value = double_from_token(&self.ast.tokens, token, true);
        let node = self.ast.add(DoubleLiteral {
            literal: token,
            value,
        });
        self.push(node);
    }

    pub(crate) fn handle_literal_int(&mut self, token: TokenId) {
        let value = int_from_token(&self.ast.tokens, token, false);
        let node = self.ast.add(IntegerLiteral {
            literal: token,
            value,
        });
        self.push(node);
    }

    pub(crate) fn handle_literal_int_with_separators(&mut self, token: TokenId) {
        if !self.enable_digit_separators {
            self.report_feature_not_enabled(ExperimentalFlag::DigitSeparators, token, None);
        }

        let value = int_from_token(&self.ast.tokens, token, true);
        let node = self.ast.add(IntegerLiteral {
            literal: token,
            value,
        });
        self.push(node);
    }

    pub(crate) fn handle_literal_list(
        &mut self,
        count: i32,
        left_bracket: TokenId,
        const_keyword: Option<TokenId>,
        right_bracket: TokenId,
    ) {
        let elements = self.pop_collection_elements(count as usize);
        let type_arguments = self.pop_node_opt::<TypeArgumentList>();

        let elements = self.ast.new_list::<CollectionElement>(elements);
        let node = self.ast.add(ListLiteral {
            const_keyword,
            type_arguments,
            left_bracket,
            elements,
            right_bracket,
        });
        self.push(node);
    }

    pub(crate) fn handle_literal_map_entry(
        &mut self,
        colon: TokenId,
        _end_token: TokenId,
        mut null_aware_key_token: Option<TokenId>,
        mut null_aware_value_token: Option<TokenId>,
    ) {
        if !self.enable_null_aware_elements
            && (null_aware_key_token.is_some() || null_aware_value_token.is_some())
        {
            let start = null_aware_key_token.or(null_aware_value_token).unwrap();
            self.report_feature_not_enabled(ExperimentalFlag::NullAwareElements, start, None);
            null_aware_key_token = None;
            null_aware_value_token = None;
        }

        let value = self.pop_node::<Expression>();
        let key = self.pop_node::<Expression>();
        let node = self.ast.add(MapLiteralEntry {
            key_question: null_aware_key_token,
            key,
            separator: colon,
            value_question: null_aware_value_token,
            value,
        });
        self.push(node);
    }

    pub(crate) fn handle_literal_null(&mut self, token: TokenId) {
        let node = self.ast.add(NullLiteral { literal: token });
        self.push(node);
    }

    pub(crate) fn handle_literal_set_or_map(
        &mut self,
        count: i32,
        left_brace: TokenId,
        const_keyword: Option<TokenId>,
        right_brace: TokenId,
        // TODO(danrubel): hasSetEntry parameter exists for replicating
        // existing behavior and will be removed once unified collection has
        // been enabled
        _has_set_entry: bool,
    ) {
        let elements = self.pop_collection_elements(count as usize);

        let type_arguments = self.pop_node_opt::<TypeArgumentList>();
        let elements = self.ast.new_list::<CollectionElement>(elements);
        let node = self.ast.add(SetOrMapLiteral {
            const_keyword,
            type_arguments,
            left_bracket: left_brace,
            elements,
            right_bracket: right_brace,
        });
        self.push(node);
    }

    pub(crate) fn handle_map_pattern(
        &mut self,
        count: i32,
        left_brace: TokenId,
        right_brace: TokenId,
    ) {
        let elements = self.pop_typed_list2::<MapPatternElement>(count as usize);
        let type_arguments = self.pop_node_opt::<TypeArgumentList>();
        let elements = self.ast.new_list(elements);
        let node = self.ast.add(MapPattern {
            type_arguments,
            left_bracket: left_brace,
            elements,
            right_bracket: right_brace,
        });
        self.push(node);
    }

    pub(crate) fn handle_map_pattern_entry(&mut self, colon: TokenId, _end_token: TokenId) {
        let value = self.pop_node::<DartPattern>();
        let key = self.pop_node::<Expression>();
        let node = self.ast.add(MapPatternEntry {
            key,
            separator: colon,
            value,
        });
        self.push(node);
    }

    pub(crate) fn handle_mixin_header(&mut self, mixin_keyword: TokenId) {
        debug_assert!(self.class_like_builder.is_none());

        let implements_clause = self.pop_node_opt::<ImplementsClause>();
        let on_clause = self.pop_node_opt::<MixinOnClause>();
        self.pop(); // Error recovery: Primary constructor.
        self.pop(); // Error recovery: Const token.
        let base_keyword = self.pop_token_opt();
        let augment_keyword = self.pop_token_opt();
        let type_parameters = self.pop_node_opt::<dartr_ast::TypeParameterList>();
        let name = self.pop_node::<SimpleIdentifier>();
        let metadata = self.pop_metadata();

        let begin = base_keyword.unwrap_or(mixin_keyword);
        let comment = self.find_comment(metadata.as_deref(), begin);

        let name = self.ast[name].token;
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
            kind: ClassLikeKind::Mixin(MixinDeclarationBuilder {
                augment_keyword,
                base_keyword,
                mixin_keyword,
                name,
                on_clause,
                implements_clause,
            }),
        }));
    }

    pub(crate) fn handle_mixin_on(&mut self, on_keyword: Option<TokenId>, type_count: i32) {
        if let Some(on_keyword) = on_keyword {
            self.end_type_list_impl_for_part4(type_count);
            let on_types = self.pop_named_type_list(diag::expected_named_type_on);
            let superclass_constraints = self.ast.new_list(on_types);
            let node = self.ast.add(MixinOnClause {
                on_keyword,
                superclass_constraints,
            });
            self.push(node);
        } else {
            self.push(NullValue::IdentifierList);
        }
    }

    pub(crate) fn handle_mixin_with_clause(&mut self, _with_keyword: TokenId) {
        // This is an error case. An error has been issued already.
        // Possibly the data could be used for help though.
        self.pop_named_type_list(diag::expected_named_type_with);
    }

    pub(crate) fn handle_named_argument(&mut self, colon: TokenId) {
        let expression = self.pop_node::<Expression>();
        let name = self.pop_node::<SimpleIdentifier>();

        let name = self.ast[name].token;
        let node = self.ast.add(NamedArgument {
            name,
            colon,
            argument_expression: expression,
        });
        self.push(node);
    }

    pub(crate) fn handle_named_mixin_application_with_clause(&mut self, with_keyword: TokenId) {
        let mixin_types = self.pop_named_type_list(diag::expected_named_type_with);
        let mixin_types = self.ast.new_list(mixin_types);
        let node = self.ast.add(WithClause {
            with_keyword,
            mixin_types,
        });
        self.push(node);
    }

    pub(crate) fn handle_named_record_field(&mut self, colon: TokenId) {
        let expression = self.pop_node::<Expression>();
        let name = self.pop_node::<SimpleIdentifier>();

        let name = self.ast[name].token;
        let node = self.ast.add(RecordLiteralNamedField {
            name,
            colon,
            field_expression: expression,
        });
        self.push(node);
    }

    pub(crate) fn handle_native_clause(&mut self, _native_token: TokenId, has_name: bool) {
        if has_name {
            self.native_name = Some(self.pop_node::<StringLiteral>());
        } else {
            self.native_name = None;
        }
    }

    pub(crate) fn handle_native_function_body(
        &mut self,
        native_token: TokenId,
        semicolon: TokenId,
    ) {
        // TODO(danrubel): Change the parser to not produce these modifiers.
        self.pop(); // star
        self.pop(); // async
        let node = self.ast.add(NativeFunctionBody {
            native_keyword: native_token,
            string_literal: self.native_name,
            semicolon,
        });
        self.push(node);
    }

    pub(crate) fn handle_new_as_identifier(&mut self, token: TokenId) {
        if !self.enable_constructor_tearoffs {
            self.report_feature_not_enabled(ExperimentalFlag::ConstructorTearoffs, token, None);
        }
    }

    pub(crate) fn handle_no_class_body(&mut self, semicolon_token: TokenId) {
        if let Some(builder) = self.class_like_builder.as_mut() {
            builder.empty_class_body_semicolon = Some(semicolon_token);
        }
    }

    pub(crate) fn handle_no_constructor_reference_continuation_after_type_arguments(
        &mut self,
        _token: TokenId,
    ) {
        self.push(NullValue::ConstructorReferenceContinuationAfterTypeArguments);
    }

    pub(crate) fn handle_no_enum_body(&mut self, semicolon_token: TokenId) {
        if let Some(builder) = self.class_like_builder.as_mut() {
            builder.empty_class_body_semicolon = Some(semicolon_token);
        }
    }

    pub(crate) fn handle_no_extension_body(&mut self, semicolon_token: TokenId) {
        if let Some(builder) = self.class_like_builder.as_mut() {
            builder.empty_class_body_semicolon = Some(semicolon_token);
        }
    }

    pub(crate) fn handle_no_extension_type_body(&mut self, semicolon_token: TokenId) {
        if let Some(builder) = self.class_like_builder.as_mut() {
            builder.empty_class_body_semicolon = Some(semicolon_token);
        }
    }

    pub(crate) fn handle_no_field_initializer(&mut self, _token: TokenId) {
        let name = self.pop_node::<SimpleIdentifier>();
        let name = self.ast[name].token;
        let node = self.ast.add(VariableDeclaration {
            documentation_comment: None,
            metadata: NodeList::EMPTY,
            name,
            equals: None,
            initializer: None,
        });
        self.push(node);
    }

    pub(crate) fn handle_no_identifier(
        &mut self,
        _token: TokenId,
        _identifier_context: IdentifierContext,
    ) {
        self.push(NullValue::Identifier);
    }

    pub(crate) fn handle_no_initializers(&mut self) {
        if !self.is_full_ast {
            return;
        }
        self.push(NullValue::ConstructorInitializerSeparator);
        self.push(NullValue::ConstructorInitializers);
    }

    pub(crate) fn handle_no_mixin_body(&mut self, semicolon_token: TokenId) {
        if let Some(builder) = self.class_like_builder.as_mut() {
            builder.empty_class_body_semicolon = Some(semicolon_token);
        }
    }

    pub(crate) fn handle_non_null_assert_expression(&mut self, bang: TokenId) {
        let operand = self.pop_node::<Expression>();
        let node = self.ast.add(PostfixExpression {
            operand,
            operator: bang,
        });
        self.push(node);
    }

    pub(crate) fn handle_no_primary_constructor(
        &mut self,
        _kind: DeclarationKind,
        _token: TokenId,
        const_keyword: Option<TokenId>,
    ) {
        self.push_token_or(const_keyword, NullValue::Token);
        self.push(NullValue::PrimaryConstructor);
    }

    pub(crate) fn handle_no_type_name_in_constructor_reference(&mut self, _token: TokenId) {
        let name = match self.class_like_builder.as_ref().map(|b| &b.kind) {
            Some(ClassLikeKind::Enum(e)) => e.name,
            _ => panic!(
                "type '_ClassLikeDeclarationBuilder?' is not a subtype of type '_EnumDeclarationBuilder'"
            ),
        };
        let node = self.ast.add(SimpleIdentifier { token: name });
        self.push(node);
    }

    pub(crate) fn handle_no_variable_initializer(&mut self, _token: TokenId) {}

    pub(crate) fn handle_null_assert_pattern(&mut self, bang: TokenId) {
        let pattern = self.pop_node::<DartPattern>();
        let node = self.ast.add(NullAssertPattern {
            pattern,
            operator: bang,
        });
        self.push(node);
    }

    pub(crate) fn handle_null_aware_element(&mut self, null_aware_token: TokenId) {
        if !self.enable_null_aware_elements {
            self.report_feature_not_enabled(
                ExperimentalFlag::NullAwareElements,
                null_aware_token,
                None,
            );
        } else {
            let expression = self.pop_node::<Expression>();
            let node = self.ast.add(NullAwareElement {
                question: null_aware_token,
                value: expression,
            });
            self.push(node);
        }
    }

    pub(crate) fn handle_null_check_pattern(&mut self, question: TokenId) {
        if !self.is_enabled(ExperimentalFlag::Patterns) {
            // TODO(paulberry): report the appropriate error
            panic!("UnimplementedError: Patterns not enabled");
        }
        let pattern = self.pop_node::<DartPattern>();
        let node = self.ast.add(NullCheckPattern {
            pattern,
            operator: question,
        });
        self.push(node);
    }

    pub(crate) fn handle_object_pattern(
        &mut self,
        first_identifier: TokenId,
        dot: Option<TokenId>,
        second_identifier: Option<TokenId>,
    ) {
        let arguments = match self.pop() {
            Value::ObjectPatternFields(f) => f,
            other => panic!("{other:?} is not a subtype of type '_ObjectPatternFields'"),
        };
        let type_arguments = self.pop_node_opt::<TypeArgumentList>();

        let named_type = match (dot, second_identifier) {
            (Some(dot), Some(second_identifier)) => {
                let import_prefix = self.ast.add(ImportPrefixReference {
                    name: first_identifier,
                    period: dot,
                });
                self.ast.add(NamedType {
                    import_prefix: Some(import_prefix),
                    name: second_identifier,
                    type_arguments,
                    question: None,
                })
            }
            _ => self.ast.add(NamedType {
                import_prefix: None,
                name: first_identifier,
                type_arguments,
                question: None,
            }),
        };

        let fields = self.ast.new_list(arguments.fields.iter().copied());
        let node = self.ast.add(ObjectPattern {
            type_: named_type,
            left_parenthesis: arguments.left_parenthesis,
            fields,
            right_parenthesis: arguments.right_parenthesis,
        });
        self.push(node);
    }

    pub(crate) fn handle_object_pattern_fields(
        &mut self,
        count: i32,
        begin_token: TokenId,
        end_token: TokenId,
    ) {
        let fields = self.pop_typed_list2::<PatternField>(count as usize);
        self.push(ObjectPatternFields {
            left_parenthesis: begin_token,
            right_parenthesis: end_token,
            fields,
        });
    }

    pub(crate) fn handle_operator(&mut self, token: TokenId) {
        self.push(token);
    }

    pub(crate) fn handle_operator_name(&mut self, operator_keyword: TokenId, token: TokenId) {
        let name = self.ast.add(SimpleIdentifier { token });
        self.push(OperatorName {
            operator_keyword,
            name,
        });
    }

    pub(crate) fn handle_parenthesized_condition(
        &mut self,
        token: TokenId,
        case_: Option<TokenId>,
        when: Option<TokenId>,
    ) {
        let left_parenthesis = token;
        let mut case_clause = None;
        if let Some(case_) = case_ {
            let when_clause = match when {
                Some(_) => Some(self.pop_node::<WhenClause>()),
                None => None,
            };
            let pattern = self.pop_node::<DartPattern>();
            let guarded_pattern = self.ast.add(GuardedPattern {
                pattern,
                when_clause,
            });
            case_clause = Some(self.ast.add(CaseClause {
                case_keyword: case_,
                guarded_pattern,
            }));
        }
        let condition = self.pop_node::<Expression>();
        self.report_error_if_super(condition);
        self.push(ParenthesizedCondition {
            left_parenthesis,
            expression: condition,
            case_clause,
        });
    }

    pub(crate) fn handle_parenthesized_pattern(&mut self, token: TokenId) {
        let left_parenthesis = token;
        let pattern = self.pop_node::<DartPattern>();
        let right_parenthesis = self.end_group(left_parenthesis);
        let node = self.ast.add(ParenthesizedPattern {
            left_parenthesis,
            pattern,
            right_parenthesis,
        });
        self.push(node);
    }

    pub(crate) fn handle_pattern_assignment(&mut self, equals: TokenId) {
        let expression = self.pop_node::<Expression>();
        let pattern = self.pop_node::<DartPattern>();
        let node = self.ast.add(PatternAssignment {
            pattern,
            equals,
            expression,
        });
        self.push(node);
    }

    pub(crate) fn handle_pattern_field(&mut self, colon: Option<TokenId>) {
        let pattern = self.pop_node::<DartPattern>();
        let mut field_name = None;
        if let Some(colon) = colon {
            let name = self
                .pop_node_opt::<SimpleIdentifier>()
                .map(|n| self.ast[n].token);
            field_name = Some(self.ast.add(PatternFieldName { name, colon }));
        }
        let node = self.ast.add(PatternField {
            name: field_name,
            pattern,
        });
        self.push(node);
    }

    pub(crate) fn handle_pattern_variable_declaration_statement(
        &mut self,
        keyword: TokenId,
        equals: TokenId,
        semicolon: TokenId,
    ) {
        let expression = self.pop_node::<Expression>();
        let pattern = self.pop_node::<DartPattern>();
        let metadata = self.pop_metadata();
        let comment = self.find_comment(metadata.as_deref(), keyword);
        let metadata = self.node_list(metadata);
        let declaration = self.ast.add(PatternVariableDeclaration {
            documentation_comment: comment,
            metadata,
            keyword,
            pattern,
            equals,
            expression,
        });
        let node = self.ast.add(PatternVariableDeclarationStatement {
            declaration,
            semicolon,
        });
        self.push(node);
    }

    pub(crate) fn handle_qualified(&mut self, period: TokenId) {
        let identifier = self.pop();
        let prefix = self.pop();
        match prefix {
            Value::Tokens(mut list) => {
                // We're just accumulating components into a list.
                list.push(period);
                match identifier {
                    Value::Token(t) => list.push(t),
                    other => panic!("{other:?} is not a subtype of type 'Token'"),
                }
                self.push(list);
            }
            Value::Node(n) if self.ast.is::<SimpleIdentifier>(n) => {
                // TODO(paulberry): resolve [identifier].  Note that
                // BodyBuilder handles this situation using
                // SendAccessGenerator.
                let identifier = self
                    .value_as_node_opt::<SimpleIdentifier>(identifier)
                    .expect("Null check operator used on a null value");
                let node = self.ast.add(PrefixedIdentifier {
                    prefix: Id::from_raw(n),
                    period,
                    identifier,
                });
                self.push(node);
            }
            _ => {
                // TODO(paulberry): implement.
                self.log_event("Qualified with >1 dot");
            }
        }
    }

    pub(crate) fn handle_record_pattern(&mut self, token: TokenId, count: i32) {
        let fields = self.pop_typed_list2::<PatternField>(count as usize);
        let fields = self.ast.new_list(fields);
        let right_parenthesis = self.end_group(token);
        let node = self.ast.add(RecordPattern {
            left_parenthesis: token,
            fields,
            right_parenthesis,
        });
        self.push(node);
    }

    pub(crate) fn handle_recover_declaration_header(&mut self, kind: DeclarationHeaderKind) {
        let implements_clause = self.pop_node_opt::<ImplementsClause>();
        let with_clause = self.pop_node_opt::<WithClause>();
        let extends_clause = self.pop_node_opt::<dartr_ast::ExtendsClause>();
        match kind {
            DeclarationHeaderKind::Class => {
                let (existing_extends, existing_with, existing_implements) = match self
                    .class_like_builder
                    .as_ref()
                    .map(|b| &b.kind)
                {
                    Some(ClassLikeKind::Class(c)) => {
                        (c.extends_clause, c.with_clause, c.implements_clause)
                    }
                    _ => panic!(
                        "type '_ClassLikeDeclarationBuilder?' is not a subtype of type '_ClassDeclarationBuilder'"
                    ),
                };
                let mut new_extends = existing_extends;
                let mut new_with = existing_with;
                let mut new_implements = existing_implements;
                if let Some(extends_clause) = extends_clause {
                    // Dart `declaration.extendsClause?.superclass == null`:
                    // the superclass of an extends clause is never null.
                    if existing_extends.is_none() {
                        new_extends = Some(extends_clause);
                    }
                }
                if let Some(with_clause) = with_clause {
                    match existing_with {
                        None => new_with = Some(with_clause),
                        Some(existing) => {
                            let e = self.ast[existing].clone();
                            let w = self.ast[with_clause].clone();
                            let mut types = self.ast.list(e.mixin_types).to_vec();
                            types.extend_from_slice(self.ast.list(w.mixin_types));
                            let mixin_types = self.ast.new_list(types);
                            new_with = Some(self.ast.add(WithClause {
                                with_keyword: e.with_keyword,
                                mixin_types,
                            }));
                        }
                    }
                }
                if let Some(implements_clause) = implements_clause {
                    match existing_implements {
                        None => new_implements = Some(implements_clause),
                        Some(existing) => {
                            let e = self.ast[existing].clone();
                            let i = self.ast[implements_clause].clone();
                            let mut types = self.ast.list(e.interfaces).to_vec();
                            types.extend_from_slice(self.ast.list(i.interfaces));
                            let interfaces = self.ast.new_list(types);
                            new_implements = Some(self.ast.add(ImplementsClause {
                                implements_keyword: e.implements_keyword,
                                interfaces,
                            }));
                        }
                    }
                }
                if let Some(ClassLikeKind::Class(c)) =
                    self.class_like_builder.as_mut().map(|b| &mut b.kind)
                {
                    c.extends_clause = new_extends;
                    c.with_clause = new_with;
                    c.implements_clause = new_implements;
                }
            }
            DeclarationHeaderKind::ExtensionType => {
                // TODO(scheglov): Support header recovery on extension type
                //  declaration.
            }
        }
    }

    pub(crate) fn handle_recover_import(&mut self, semicolon: Option<TokenId>) {
        let combinators = self.pop_nodes_opt::<dartr_ast::Combinator>();
        let deferred_keyword = self.pop_token_opt();
        let as_keyword = self.pop_token_opt();
        let prefix = self.pop_node_opt::<SimpleIdentifier>();
        let configurations = self.pop_nodes_opt::<dartr_ast::Configuration>();

        let directive = *self.directives.last().expect("Bad state: No element");
        match self.ast.cast::<ImportDirective>(directive) {
            Some(import) => {
                // TODO(scheglov): This code would be easier if we used one
                // object.
                let d = self.ast[import].clone();
                let mut merged_as_keyword = d.as_keyword;
                let mut merged_prefix = d.prefix;
                if d.as_keyword.is_none() && as_keyword.is_some() {
                    merged_as_keyword = as_keyword;
                    merged_prefix = prefix;
                }

                let mut all_configurations = self.ast.list(d.configurations).to_vec();
                all_configurations.extend(configurations.unwrap_or_default());
                let mut all_combinators = self.ast.list(d.combinators).to_vec();
                all_combinators.extend(combinators.unwrap_or_default());
                let metadata = self.ast.list(d.metadata).to_vec();
                let metadata = self.ast.new_list(metadata);
                let configurations = self.ast.new_list(all_configurations);
                let combinators = self.ast.new_list(all_combinators);
                let node = self.ast.add(ImportDirective {
                    documentation_comment: d.documentation_comment,
                    metadata,
                    import_keyword: d.import_keyword,
                    uri: d.uri,
                    configurations,
                    deferred_keyword: d.deferred_keyword.or(deferred_keyword),
                    as_keyword: merged_as_keyword,
                    prefix: merged_prefix,
                    combinators,
                    semicolon: semicolon.unwrap_or(d.semicolon),
                });
                *self.directives.last_mut().unwrap() = node.upcast();
            }
            None => panic!("UnimplementedError: {:?}", self.ast.kind(directive)),
        }
    }

    pub(crate) fn handle_recover_mixin_header(&mut self) {
        let (existing_on, existing_implements) = match self
            .class_like_builder
            .as_ref()
            .map(|b| &b.kind)
        {
            Some(ClassLikeKind::Mixin(m)) => (m.on_clause, m.implements_clause),
            _ => panic!(
                "type '_ClassLikeDeclarationBuilder?' is not a subtype of type '_MixinDeclarationBuilder'"
            ),
        };
        let implements_clause = self.pop_node_opt::<ImplementsClause>();
        let on_clause = self.pop_node_opt::<MixinOnClause>();

        let mut new_on = existing_on;
        let mut new_implements = existing_implements;
        if let Some(on_clause) = on_clause {
            match existing_on {
                None => new_on = Some(on_clause),
                Some(existing) => {
                    let e = self.ast[existing].clone();
                    let o = self.ast[on_clause].clone();
                    let mut types = self.ast.list(e.superclass_constraints).to_vec();
                    types.extend_from_slice(self.ast.list(o.superclass_constraints));
                    let superclass_constraints = self.ast.new_list(types);
                    new_on = Some(self.ast.add(MixinOnClause {
                        on_keyword: e.on_keyword,
                        superclass_constraints,
                    }));
                }
            }
        }
        if let Some(implements_clause) = implements_clause {
            match existing_implements {
                None => new_implements = Some(implements_clause),
                Some(existing) => {
                    let e = self.ast[existing].clone();
                    let i = self.ast[implements_clause].clone();
                    let mut types = self.ast.list(e.interfaces).to_vec();
                    types.extend_from_slice(self.ast.list(i.interfaces));
                    let interfaces = self.ast.new_list(types);
                    // Dart uses the `implements` keyword of the new clause.
                    new_implements = Some(self.ast.add(ImplementsClause {
                        implements_keyword: i.implements_keyword,
                        interfaces,
                    }));
                }
            }
        }
        if let Some(ClassLikeKind::Mixin(m)) = self.class_like_builder.as_mut().map(|b| &mut b.kind)
        {
            m.on_clause = new_on;
            m.implements_clause = new_implements;
        }
    }

    pub(crate) fn handle_relational_pattern(&mut self, token: TokenId) {
        let operand = self.pop_node::<Expression>();
        let node = self.ast.add(RelationalPattern {
            operator: token,
            operand,
        });
        self.push(node);
    }

    pub(crate) fn handle_rest_pattern(&mut self, dots: TokenId, has_sub_pattern: bool) {
        let sub_pattern = if has_sub_pattern {
            Some(self.pop_node::<DartPattern>())
        } else {
            None
        };
        let node = self.ast.add(RestPatternElement {
            operator: dots,
            pattern: sub_pattern,
        });
        self.push(node);
    }

    pub(crate) fn handle_script(&mut self, token: TokenId) {
        self.script_tag = Some(self.ast.add(ScriptTag { script_tag: token }));
    }

    pub(crate) fn handle_send(&mut self, _begin_token: TokenId, _end_token: TokenId) {
        let argument_list = self.pop_node_opt::<dartr_ast::ArgumentList>();
        let type_arguments = self.pop_node_opt::<TypeArgumentList>();
        if let Some(argument_list) = argument_list {
            self.do_invocation(type_arguments, argument_list);
        } else {
            self.do_property_get();
        }
    }

    pub(crate) fn handle_spread_expression(&mut self, spread_token: TokenId) {
        let expression = self.pop_node::<Expression>();
        let node = self.ast.add(SpreadElement {
            spread_operator: spread_token,
            expression,
        });
        self.push(node);
    }

    pub(crate) fn handle_string_part(&mut self, token: TokenId) {
        self.push(token);
    }

    pub(crate) fn handle_super_expression(&mut self, token: TokenId, _context: IdentifierContext) {
        let node = self.ast.add(SuperExpression {
            super_keyword: token,
        });
        self.push(node);
    }

    pub(crate) fn handle_switch_case_no_when_clause(&mut self, _token: TokenId) {}

    pub(crate) fn handle_switch_expression_case_pattern(&mut self, _token: TokenId) {}

    pub(crate) fn handle_symbol_void(&mut self, token: TokenId) {
        self.push(token);
    }

    pub(crate) fn handle_this_expression(&mut self, token: TokenId, _context: IdentifierContext) {
        let node = self.ast.add(ThisExpression {
            this_keyword: token,
        });
        self.push(node);
    }

    pub(crate) fn handle_throw_expression(&mut self, throw_token: TokenId, _end_token: TokenId) {
        let expression = self.pop_node::<Expression>();
        let node = self.ast.add(ThrowExpression {
            throw_keyword: throw_token,
            expression,
        });
        self.push(node);
    }

    pub(crate) fn handle_type(&mut self, _begin_token: TokenId, question_mark: Option<TokenId>) {
        let arguments = self.pop_node_opt::<TypeArgumentList>();
        let name = self.pop_node::<Identifier>();

        let node = self
            .ast
            .identifier_to_named_type(name, arguments, question_mark);
        self.push(node);
    }

    pub(crate) fn handle_type_argument_application(&mut self, _open_angle_bracket: TokenId) {
        let type_arguments = self.pop_node::<TypeArgumentList>();
        let receiver = self.pop_node::<Expression>();
        if !self.enable_constructor_tearoffs {
            let t = self.ast[type_arguments].clone();
            self.report_feature_not_enabled(
                ExperimentalFlag::ConstructorTearoffs,
                t.left_bracket,
                Some(t.right_bracket),
            );
        }
        self.report_error_if_super(receiver);
        let node = self.ast.add(FunctionReference {
            function: receiver,
            type_arguments: Some(type_arguments),
        });
        self.push(node);
    }

    pub(crate) fn handle_type_variables_defined(&mut self, _token: TokenId, count: i32) {
        debug_assert!(count > 0);
        match self.pop_typed_list::<TypeParameter>(count as usize) {
            Some(list) => self.push(list),
            // Dart `push(null)` is an internal problem.
            None => self.internal_problem("Unhandled null in push."),
        }
    }

    pub(crate) fn handle_unary_postfix_assignment_expression(&mut self, token: TokenId) {
        let operator = token;
        let expression = self.pop_node::<Expression>();
        if !self.is_assignable(expression) {
            // This error is also reported by the body builder.
            self.handle_recoverable_error(
                cfe_codes::illegal_assignment_to_non_assignable(),
                operator,
                operator,
            );
        }
        let node = self.ast.add(PostfixExpression {
            operand: expression,
            operator,
        });
        self.push(node);
    }

    pub(crate) fn handle_unary_prefix_assignment_expression(&mut self, token: TokenId) {
        let operator = token;
        let expression = self.pop_node::<Expression>();
        if !self.is_assignable(expression) {
            // This error is also reported by the body builder.
            let end = self.end_token(expression);
            self.handle_recoverable_error(cfe_codes::missing_assignable_selector(), end, end);
        }
        let node = self.ast.add(PrefixExpression {
            operator,
            operand: expression,
        });
        self.push(node);
    }

    pub(crate) fn handle_unary_prefix_expression(&mut self, token: TokenId) {
        let operator = token;
        let operand = self.pop_node::<Expression>();
        let ty = self.ast.tokens.ty(operator);
        if !(ty == TokenType::MINUS || ty == TokenType::TILDE) {
            self.report_error_if_super(operand);
        }

        let node = self.ast.add(PrefixExpression { operator, operand });
        self.push(node);
    }

    pub(crate) fn handle_valued_formal_parameter(
        &mut self,
        equals: TokenId,
        _token: TokenId,
        _kind: FormalParameterKind,
    ) {
        let value = self.pop_node::<Expression>();
        let node = self.ast.add(FormalParameterDefaultClause {
            separator: equals,
            value,
        });
        self.push(node);
    }

    pub(crate) fn handle_void_keyword(&mut self, token: TokenId) {
        // TODO(paulberry): is this sufficient, or do we need to hook the
        // "void" keyword up to an element?
        self.handle_identifier(token, IdentifierContext::TypeReference);
        self.handle_no_type_arguments(token);
        self.handle_type(token, None);
    }

    pub(crate) fn handle_void_keyword_with_type_arguments(&mut self, token: TokenId) {
        let arguments = self.pop_node::<TypeArgumentList>();

        // TODO(paulberry): is this sufficient, or do we need to hook the
        // "void" keyword up to an element?
        self.handle_identifier(token, IdentifierContext::TypeReference);
        self.push(arguments);
        self.handle_type(token, None);
    }

    pub(crate) fn handle_wildcard_pattern(&mut self, keyword: Option<TokenId>, wildcard: TokenId) {
        debug_assert!(self.is_enabled(ExperimentalFlag::Patterns));
        // Note: if `default` appears in a switch expression, parser error
        // recovery treats it as a wildcard pattern.
        let type_ = self.pop_node_opt::<TypeAnnotation>();
        let node = self.ast.add(WildcardPattern {
            keyword,
            type_,
            name: wildcard,
        });
        self.push(node);
    }

    /// Dart `ExpressionImpl.isAssignable`: identifiers, index expressions
    /// and property accesses.
    fn is_assignable(&self, expression: Id<Expression>) -> bool {
        self.ast.is::<Identifier>(expression)
            || self.ast.is::<IndexExpression>(expression)
            || self.ast.is::<PropertyAccess>(expression)
    }
}
