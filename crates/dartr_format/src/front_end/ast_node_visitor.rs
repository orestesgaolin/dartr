// Dart source: dart_style lib/src/front_end/ast_node_visitor.dart

use dartr_ast::*;
use dartr_syntax::{LineInfo, TokenId};

use crate::ast_extensions::*;
use crate::back_end::code_writer::Indent;
use crate::piece::{
    AssignPiece, AssignPiece3Dot7, CaseExpressionPiece, ConstructorPiece, ControlFlowPiece,
    GroupingPiece, InfixPiece, LeadingCommentPiece, ListStyle, PieceId, Pieces, State,
    TypeParameterBoundPiece, VariablePiece,
};
use crate::source_code::SourceCode;

use super::chain_builder::ChainBuilder;
use super::comment_writer::CommentWriter;
use super::delimited_list_builder::DelimitedListBuilder;
use super::expression_contents::ExpressionContents;
use super::formatting_style::FormattingStyle;
use super::piece_factory::{BinaryOperation, NodeContext, comma_after_token, node_can_block_split};
use super::piece_writer::PieceWriter;
use super::sequence_builder::SequenceBuilder;
use super::type_builder::{TypeBuilder, TypeClauses};

/// Visits every token of the AST and produces a tree of [Piece]s that
/// corresponds to it and contains every token and comment in the original
/// source.
///
/// Dart divides the functionality into the mixin `PieceFactory` and the
/// class `PieceWriter`; here they are all methods of this struct.
pub struct AstNodeVisitor<'a> {
    pub ast: &'a Ast,

    pub style: FormattingStyle,

    pub comments: CommentWriter<'a>,

    /// The arena of all pieces.
    pub arena: Pieces,

    /// The state of Dart `PieceWriter`.
    pub(crate) writer: PieceWriter<'a>,

    /// Dart `PieceFactory._contents`.
    pub(crate) contents: ExpressionContents,

    /// The context set by the surrounding AstNode when visiting a child, or
    /// [NodeContext::None] if the parent node doesn't set a context.
    pub(crate) parent_context: NodeContext,
}

impl<'a> AstNodeVisitor<'a> {
    /// Create a new visitor that will be called to visit the code in
    /// [source].
    pub fn new(
        style: FormattingStyle,
        ast: &'a Ast,
        line_info: &'a LineInfo,
        source: &'a SourceCode,
    ) -> AstNodeVisitor<'a> {
        AstNodeVisitor {
            ast,
            style,
            comments: CommentWriter::new(ast, line_info),
            arena: Pieces::new(),
            writer: PieceWriter::new(source),
            contents: ExpressionContents::default(),
            parent_context: NodeContext::None,
        }
    }

    /// Visits [node] and returns the formatted result.
    ///
    /// Returns a [SourceCode] containing the resulting formatted source and
    /// updated selection, if any.
    pub fn run(mut self, source: &SourceCode, node: NodeId) -> SourceCode {
        let ast = self.ast;
        // Always treat the code being formatted as contained in a sequence,
        // even if we aren't formatting an entire compilation unit. That way,
        // comments before and after the node are handled properly.
        let mut sequence = SequenceBuilder::new();

        if let Some(unit) = ast.cast::<CompilationUnit>(node) {
            let unit = &ast[unit];
            if let Some(script_tag) = unit.script_tag {
                sequence.visit(&mut self, script_tag.raw());
                sequence.add_blank(&mut self);
            }

            // Put a blank line between the library tag and the other
            // directives.
            let mut directives = ast.list_raw(unit.directives);
            if let Some(&first) = directives.first() {
                if ast.kind(first) == NodeKind::LibraryDirective {
                    sequence.visit(&mut self, first);
                    sequence.add_blank(&mut self);
                    directives = &directives[1..];
                }
            }

            let mut preceding_section = DirectiveSection::None;
            for &directive in directives {
                // Separate sections of different import categories with blank
                // lines.
                let mut needs_blank = false;
                if self.style.separate_directive_sections() {
                    let section = DirectiveSection::parse(ast, directive);
                    if section != DirectiveSection::None
                        && preceding_section != DirectiveSection::None
                        && section != preceding_section
                    {
                        needs_blank = true;
                    }

                    preceding_section = section;
                }

                sequence.visit_with(&mut self, directive, Indent::None, needs_blank);
            }

            // Add a blank line between directives and declarations.
            sequence.add_blank(&mut self);

            let mut needs_blank = false;
            for &declaration in ast.list_raw(unit.declarations) {
                // "Type" declarations that have braced bodies are surrounded
                // by blank lines.
                let blank_around_mixin = self.style.blank_line_around_mixin_and_extension_types();
                let add_blank_lines = match ast.kind(declaration) {
                    NodeKind::ClassDeclaration => {
                        let body = ast[ast.cast::<ClassDeclaration>(declaration).unwrap()].body;
                        ast.is::<BlockClassBody>(body)
                    }
                    NodeKind::EnumDeclaration => {
                        let body = ast[ast.cast::<EnumDeclaration>(declaration).unwrap()].body;
                        ast.is::<BlockEnumBody>(body)
                    }
                    NodeKind::ExtensionDeclaration => {
                        let body = ast[ast.cast::<ExtensionDeclaration>(declaration).unwrap()].body;
                        ast.is::<BlockClassBody>(body)
                    }
                    NodeKind::ExtensionTypeDeclaration => {
                        let body =
                            ast[ast.cast::<ExtensionTypeDeclaration>(declaration).unwrap()].body;
                        ast.is::<BlockClassBody>(body) && blank_around_mixin
                    }
                    NodeKind::MixinDeclaration => {
                        let body = ast[ast.cast::<MixinDeclaration>(declaration).unwrap()].body;
                        ast.is::<BlockClassBody>(body) && blank_around_mixin
                    }
                    _ => false,
                };

                sequence.visit_with(
                    &mut self,
                    declaration,
                    Indent::None,
                    needs_blank || add_blank_lines,
                );

                // Add a blank line after type or function declarations with
                // bodies.
                needs_blank = add_blank_lines || has_non_empty_body(ast, declaration);
            }
        } else {
            // Just formatting a single statement.
            sequence.visit(&mut self, node);
        }

        // Write any comments at the end of the code.
        sequence.add_comments_before(&mut self, ast.tokens.next(ast.end_token(node)));

        let unit_piece = sequence.build(&mut self);

        // Finish writing and return the complete result.
        let style = self.style.clone();
        self.finish(&style, source, unit_piece)
    }

    /// Visits [node] in [context].
    pub fn visit_node(&mut self, node: NodeId, context: NodeContext) {
        let previous_context = self.parent_context;
        self.parent_context = context;

        // If there are comments before this node, then some of them may be
        // leading comments. If so, capture them now. We do this here so that
        // the comments are wrapped around the outermost possible Piece for
        // the AST node. For example:
        //
        //     // Comment
        //     a.b && c || d ? e : f;
        //
        // Here, the node that actually owns the token before the comment is
        // `a`, which is an identifier expression inside a property access
        // inside an `&&` expression inside an `||` expression inside `?:`
        // expression. If we attach the comment to the identifier expression,
        // then the newline from the comment will force all of those
        // surrounding pieces to split.
        //
        // Instead, we hoist the comment out of all of those and then have
        // comment precede them all so that they don't split.
        let first_token = first_non_comment_token(self.ast, node);
        if has_preceding_comments(self.ast, first_token) {
            let comments = self.take_comments_before(first_token);
            let mut piece = self.build(|v| {
                v.accept(node);
            });

            // Check again because the preceding comments may not necessarily
            // end up as leading comments.
            if !comments.is_empty() {
                piece = self.arena.add(LeadingCommentPiece::new(comments, piece));
            }

            self.add(piece);
        } else {
            // No preceding comments, so just visit it inline.
            self.accept(node);
        }

        self.parent_context = previous_context;
    }

    /// Dart `node.accept(this)`.
    fn accept(&mut self, node: NodeId) {
        let ast = self.ast;
        macro_rules! dispatch {
            ($($kind:ident => $method:ident),* $(,)?) => {
                match ast.kind(node) {
                    $(NodeKind::$kind => self.$method(Id::<$kind>::from_raw(node)),)*
                    kind => panic!("unexpected node in the tall style formatter: {kind:?}"),
                }
            };
        }
        dispatch!(
            AdjacentStrings => visit_adjacent_strings,
            Annotation => visit_annotation,
            ArgumentList => visit_argument_list,
            AsExpression => visit_as_expression,
            AssertInitializer => visit_assert_initializer,
            AssertStatement => visit_assert_statement,
            AssignedVariablePattern => visit_assigned_variable_pattern,
            AssignmentExpression => visit_assignment_expression,
            AwaitExpression => visit_await_expression,
            BinaryExpression => visit_binary_expression,
            Block => visit_block,
            BlockFunctionBody => visit_block_function_body,
            BooleanLiteral => visit_boolean_literal,
            BreakStatement => visit_break_statement,
            CascadeExpression => visit_cascade_expression,
            CastPattern => visit_cast_pattern,
            CatchClauseParameter => visit_catch_clause_parameter,
            ClassDeclaration => visit_class_declaration,
            ClassTypeAlias => visit_class_type_alias,
            ConditionalExpression => visit_conditional_expression,
            Configuration => visit_configuration,
            ConstantPattern => visit_constant_pattern,
            ConstructorDeclaration => visit_constructor_declaration,
            ConstructorFieldInitializer => visit_constructor_field_initializer,
            ConstructorName => visit_constructor_name,
            ConstructorSelector => visit_constructor_selector,
            ContinueStatement => visit_continue_statement,
            DeclaredIdentifier => visit_declared_identifier,
            DeclaredVariablePattern => visit_declared_variable_pattern,
            DoStatement => visit_do_statement,
            DotShorthandConstructorInvocation => visit_dot_shorthand_constructor_invocation,
            DotShorthandInvocation => visit_dot_shorthand_invocation,
            DotShorthandPropertyAccess => visit_dot_shorthand_property_access,
            DottedName => visit_dotted_name,
            DoubleLiteral => visit_double_literal,
            EmptyFunctionBody => visit_empty_function_body,
            EmptyStatement => visit_empty_statement,
            EnumConstantDeclaration => visit_enum_constant_declaration,
            EnumDeclaration => visit_enum_declaration,
            ExportDirective => visit_export_directive,
            ExpressionFunctionBody => visit_expression_function_body,
            ExpressionStatement => visit_expression_statement,
            ExtensionDeclaration => visit_extension_declaration,
            ExtensionTypeDeclaration => visit_extension_type_declaration,
            FieldDeclaration => visit_field_declaration,
            FieldFormalParameter => visit_field_formal_parameter,
            FormalParameterList => visit_formal_parameter_list,
            ForElement => visit_for_element,
            ForStatement => visit_for_statement,
            FunctionDeclaration => visit_function_declaration,
            FunctionDeclarationStatement => visit_function_declaration_statement,
            FunctionExpression => visit_function_expression,
            FunctionExpressionInvocation => visit_function_expression_invocation,
            FunctionReference => visit_function_reference,
            FunctionTypeAlias => visit_function_type_alias,
            GenericFunctionType => visit_generic_function_type,
            GenericTypeAlias => visit_generic_type_alias,
            IfElement => visit_if_element,
            IfStatement => visit_if_statement,
            ImportDirective => visit_import_directive,
            IndexExpression => visit_index_expression,
            InstanceCreationExpression => visit_instance_creation_expression,
            IntegerLiteral => visit_integer_literal,
            InterpolationExpression => visit_interpolation_expression,
            InterpolationString => visit_interpolation_string,
            IsExpression => visit_is_expression,
            Label => visit_label,
            LabelReference => visit_label_reference,
            LabeledStatement => visit_labeled_statement,
            LibraryDirective => visit_library_directive,
            ListLiteral => visit_list_literal,
            ListPattern => visit_list_pattern,
            LogicalAndPattern => visit_logical_and_pattern,
            LogicalOrPattern => visit_logical_or_pattern,
            MapLiteralEntry => visit_map_literal_entry,
            MapPattern => visit_map_pattern,
            MapPatternEntry => visit_map_pattern_entry,
            MethodDeclaration => visit_method_declaration,
            MethodInvocation => visit_method_invocation,
            MixinDeclaration => visit_mixin_declaration,
            NamedArgument => visit_named_argument,
            NamedType => visit_named_type,
            NativeClause => visit_native_clause,
            NativeFunctionBody => visit_native_function_body,
            NullAssertPattern => visit_null_assert_pattern,
            NullAwareElement => visit_null_aware_element,
            NullCheckPattern => visit_null_check_pattern,
            NullLiteral => visit_null_literal,
            ObjectPattern => visit_object_pattern,
            ParenthesizedExpression => visit_parenthesized_expression,
            ParenthesizedPattern => visit_parenthesized_pattern,
            PartDirective => visit_part_directive,
            PartOfDirective => visit_part_of_directive,
            PatternAssignment => visit_pattern_assignment,
            PatternField => visit_pattern_field,
            PatternFieldName => visit_pattern_field_name,
            PatternVariableDeclaration => visit_pattern_variable_declaration,
            PatternVariableDeclarationStatement => visit_pattern_variable_declaration_statement,
            PostfixExpression => visit_postfix_expression,
            PrefixedIdentifier => visit_prefixed_identifier,
            PrefixExpression => visit_prefix_expression,
            PrimaryConstructorBody => visit_primary_constructor_body,
            PrimaryConstructorName => visit_primary_constructor_name,
            PropertyAccess => visit_property_access,
            RedirectingConstructorInvocation => visit_redirecting_constructor_invocation,
            RecordLiteral => visit_record_literal,
            RecordLiteralNamedField => visit_record_literal_named_field,
            RecordPattern => visit_record_pattern,
            RecordTypeAnnotation => visit_record_type_annotation,
            RecordTypeAnnotationNamedField => visit_record_type_annotation_named_field,
            RecordTypeAnnotationPositionalField => visit_record_type_annotation_positional_field,
            RelationalPattern => visit_relational_pattern,
            RethrowExpression => visit_rethrow_expression,
            RestPatternElement => visit_rest_pattern_element,
            ReturnStatement => visit_return_statement,
            ScriptTag => visit_script_tag,
            SetOrMapLiteral => visit_set_or_map_literal,
            RegularFormalParameter => visit_regular_formal_parameter,
            SimpleIdentifier => visit_simple_identifier,
            SimpleStringLiteral => visit_simple_string_literal,
            SpreadElement => visit_spread_element,
            StringInterpolation => visit_string_interpolation,
            SuperConstructorInvocation => visit_super_constructor_invocation,
            SuperExpression => visit_super_expression,
            SuperFormalParameter => visit_super_formal_parameter,
            SwitchExpression => visit_switch_expression,
            SwitchExpressionCase => visit_switch_expression_case,
            SwitchStatement => visit_switch_statement,
            SymbolLiteral => visit_symbol_literal,
            ThisExpression => visit_this_expression,
            ThrowExpression => visit_throw_expression,
            TopLevelVariableDeclaration => visit_top_level_variable_declaration,
            TryStatement => visit_try_statement,
            TypeArgumentList => visit_type_argument_list,
            TypeParameter => visit_type_parameter,
            TypeParameterList => visit_type_parameter_list,
            VariableDeclarationList => visit_variable_declaration_list,
            VariableDeclarationStatement => visit_variable_declaration_statement,
            WhenClause => visit_when_clause,
            WhileStatement => visit_while_statement,
            WildcardPattern => visit_wildcard_pattern,
            YieldStatement => visit_yield_statement,
        )
    }

    fn visit_adjacent_strings(&mut self, node: Id<AdjacentStrings>) {
        let ast = self.ast;
        let indent = if self.style.is_3_dot_7() {
            indent_strings_3_dot_7(ast, node)
        } else {
            indent_strings(ast, node)
        };
        let mut operands = Vec::new();
        for &string in ast.list_raw(ast[node].strings) {
            operands.push(self.node_piece(string));
        }
        let is_3_dot_7 = self.style.is_3_dot_7();
        let piece = self.arena.add(InfixPiece::new(
            operands,
            is_3_dot_7,
            if indent { Indent::Infix } else { Indent::None },
        ));

        // Adjacent strings always split.
        self.arena.pin(piece, State::SPLIT);

        self.add(piece);
    }

    fn visit_annotation(&mut self, node: Id<Annotation>) {
        let n = &self.ast[node];
        self.token(n.at_sign);
        self.visit(n.name);
        self.visit(n.type_arguments);
        self.token(n.period);
        self.visit(n.constructor_name);
        self.visit(n.arguments);
    }

    fn visit_argument_list(&mut self, node: Id<ArgumentList>) {
        self.write_argument_list(node);
    }

    fn visit_as_expression(&mut self, node: Id<AsExpression>) {
        let n = &self.ast[node];
        self.write_type_test(n.expression, n.as_operator, n.type_, None);
    }

    fn visit_assert_initializer(&mut self, node: Id<AssertInitializer>) {
        let n = &self.ast[node];
        self.token(n.assert_keyword);
        let mut arguments = vec![n.condition.raw()];
        arguments.extend(n.message.map(Id::raw));
        self.write_arguments(n.left_parenthesis, &arguments, n.right_parenthesis);
    }

    fn visit_assert_statement(&mut self, node: Id<AssertStatement>) {
        let n = &self.ast[node];
        self.token(n.assert_keyword);
        let mut arguments = vec![n.condition.raw()];
        arguments.extend(n.message.map(Id::raw));
        self.write_arguments(n.left_parenthesis, &arguments, n.right_parenthesis);
        self.token(n.semicolon);
    }

    fn visit_assigned_variable_pattern(&mut self, node: Id<AssignedVariablePattern>) {
        self.token(self.ast[node].name);
    }

    fn visit_assignment_expression(&mut self, node: Id<AssignmentExpression>) {
        let n = &self.ast[node];
        self.write_assignment(
            n.left_hand_side.raw(),
            n.operator,
            n.right_hand_side.raw(),
            false,
            NodeContext::None,
            NodeContext::None,
        );
    }

    fn visit_await_expression(&mut self, node: Id<AwaitExpression>) {
        let n = &self.ast[node];
        self.write_prefix(Some(n.await_keyword), n.expression, true);
    }

    fn visit_binary_expression(&mut self, node: Id<BinaryExpression>) {
        let ast = self.ast;
        // In 3.7 style, flatten binary operands in assignment contexts. (In
        // 3.8 and later, indentation merging accomplishes the same thing.)
        let indent = !self.style.is_3_dot_7() || self.parent_context != NodeContext::Assignment;

        fn destructure(ast: &Ast, node: NodeId) -> Option<BinaryOperation> {
            let n = &ast[ast.cast::<BinaryExpression>(node)?];
            Some((n.left_operand.raw(), n.operator, n.right_operand.raw()))
        }

        let precedence = ast.tokens.ty(ast[node].operator).precedence();
        self.write_infix_chain(node.raw(), destructure, Some(precedence), indent);
    }

    fn visit_block(&mut self, node: Id<Block>) {
        self.write_block(node, false);
    }

    fn visit_block_function_body(&mut self, node: Id<BlockFunctionBody>) {
        let n = &self.ast[node];
        self.space();
        self.write_function_body_modifiers(n.keyword, n.star);
        self.visit(n.block);
    }

    fn visit_boolean_literal(&mut self, node: Id<BooleanLiteral>) {
        self.token(self.ast[node].literal);
    }

    fn visit_break_statement(&mut self, node: Id<BreakStatement>) {
        let n = &self.ast[node];
        self.write_break(n.break_keyword, n.label, n.semicolon);
    }

    fn visit_cascade_expression(&mut self, node: Id<CascadeExpression>) {
        let builder = ChainBuilder::new(self, node.raw());
        let piece = builder.build_cascade(self);
        self.add(piece);
    }

    fn visit_cast_pattern(&mut self, node: Id<CastPattern>) {
        let n = &self.ast[node];
        self.write_infix(n.pattern.raw(), n.as_token, n.type_.raw(), false, None, Indent::Infix);
    }

    fn visit_catch_clause_parameter(&mut self, node: Id<CatchClauseParameter>) {
        self.token(self.ast[node].name);
    }

    fn visit_class_declaration(&mut self, node: Id<ClassDeclaration>) {
        let ast = self.ast;
        let n = &ast[node];
        let keywords = [
            n.abstract_keyword,
            n.base_keyword,
            n.interface_keyword,
            n.final_keyword,
            n.sealed_keyword,
            n.mixin_keyword,
            Some(n.class_keyword),
        ];
        let builder = TypeBuilder::new(
            ast,
            n.metadata,
            &keywords,
            None,
            None,
            Some(n.name_part),
            TypeClauses {
                extends_clause: n.extends_clause,
                with_clause: n.with_clause,
                implements_clause: n.implements_clause,
                native_clause: n.native_clause,
                ..TypeClauses::default()
            },
        );
        builder.build_class_body(self, n.body);
    }

    fn visit_class_type_alias(&mut self, node: Id<ClassTypeAlias>) {
        let ast = self.ast;
        let n = &ast[node];
        let keywords = [
            n.abstract_keyword,
            n.base_keyword,
            n.interface_keyword,
            n.final_keyword,
            n.sealed_keyword,
            n.mixin_keyword,
            Some(n.typedef_keyword),
        ];
        let builder = TypeBuilder::new(
            ast,
            n.metadata,
            &keywords,
            Some(n.name),
            n.type_parameters,
            None,
            TypeClauses {
                with_clause: Some(n.with_clause),
                implements_clause: n.implements_clause,
                ..TypeClauses::default()
            },
        );
        builder.build_mixin_application_class(self, n.equals, n.superclass, n.semicolon);
    }

    fn visit_conditional_expression(&mut self, node: Id<ConditionalExpression>) {
        let ast = self.ast;
        // Flatten a series of else-if-like chained conditionals into a single
        // long infix piece. This produces a flattened style like:
        //
        //     condition
        //     ? thenBranch
        //     : condition2
        //     ? thenBranch2
        //     : elseBranch;
        //
        // This (arguably) looks nicer. More importantly, it means that all
        // but the last operand can be formatted separately, which is
        // important to avoid pathological performance in the solved with
        // long nested conditional chains.
        let mut operands = vec![self.node_piece(ast[node].condition)];

        fn add_operand(
            v: &mut AstNodeVisitor<'_>,
            operands: &mut Vec<PieceId>,
            operator: TokenId,
            operand: Id<Expression>,
        ) {
            if !v.style.is_3_dot_7() {
                // If there are comments before a branch, then hoist them so
                // they aren't indented with the branch body.
                let comments = v.take_comments_before(operator);
                operands.extend(comments);
            }

            let piece = v.build(|v| {
                v.token(operator);
                v.space();
                v.visit_in(operand, NodeContext::ConditionalBranch);
            });
            operands.push(piece);
        }

        let mut conditional = &ast[node];
        loop {
            add_operand(self, &mut operands, conditional.question, conditional.then_expression);

            let else_branch = conditional.else_expression;
            if let Some(else_if) = ast.cast::<ConditionalExpression>(else_branch) {
                let else_if = &ast[else_if];
                add_operand(self, &mut operands, conditional.colon, else_if.condition);
                conditional = else_if;
            } else {
                add_operand(self, &mut operands, conditional.colon, conditional.else_expression);
                break;
            }
        }

        let is_3_dot_7 = self.style.is_3_dot_7();
        let piece = self.arena.add(InfixPiece::conditional(operands, is_3_dot_7));

        // If conditional expressions are directly nested, force them all to
        // split, both parents and children.
        let n = &ast[node];
        if self.parent_context == NodeContext::ConditionalBranch
            || ast.is::<ConditionalExpression>(n.then_expression)
            || ast.is::<ConditionalExpression>(n.else_expression)
        {
            self.arena.pin(piece, State::SPLIT);
        }

        self.add(piece);
    }

    fn visit_configuration(&mut self, node: Id<Configuration>) {
        let ast = self.ast;
        let n = &ast[node];
        self.token(n.if_keyword);
        self.space();
        self.token(n.left_parenthesis);

        if let Some(equals) = n.equal_token {
            // Hoist comments so that they don't force the `==` to split.
            self.hoist_leading_comments(first_non_comment_token(ast, n.name.raw()), |v| {
                let left = v.build(|v| {
                    v.visit(n.name);
                    v.space();
                    v.token(equals);
                });
                let right = v.node_piece(n.value.unwrap());
                let is_3_dot_7 = v.style.is_3_dot_7();
                v.arena
                    .add(InfixPiece::new(vec![left, right], is_3_dot_7, Indent::Expression))
            });
        } else {
            self.visit(n.name);
        }

        self.token(n.right_parenthesis);
        self.space();
        self.visit(n.uri);
    }

    fn visit_constant_pattern(&mut self, node: Id<ConstantPattern>) {
        let n = &self.ast[node];
        if self.style.is_3_dot_7() {
            self.write_prefix(n.const_keyword, n.expression, true);
        } else if let Some(const_keyword) = n.const_keyword {
            self.token_with(const_keyword, false, true, false);
            self.visit(n.expression);
        } else {
            self.visit(n.expression);
        }
    }

    fn visit_constructor_declaration(&mut self, node: Id<ConstructorDeclaration>) {
        let ast = self.ast;
        let n = &ast[node];
        self.with_metadata(ast.list_raw(n.metadata), false, |v| {
            let header = v.build(|v| {
                v.modifier(n.external_keyword);
                v.modifier(n.const_keyword);

                // If there is no type name, then this is an unnamed
                // constructor using `new` or `factory` to declare the
                // constructor. In that case, don't put a space after the
                // keyword.
                if n.type_name.is_some() || n.name.is_some() {
                    v.modifier(n.new_keyword);
                    v.modifier(n.factory_keyword);
                    v.visit(n.type_name);
                } else {
                    v.token(n.new_keyword);
                    v.token(n.factory_keyword);
                }

                v.token(n.period);
                v.token(n.name);
            });

            let parameters = v.node_piece(n.parameters);

            let mut redirect = None;
            let mut initializer_separator = None;
            let mut initializers = None;
            if let Some(constructor) = n.redirected_constructor {
                let separator = v.build(|v| {
                    v.token(n.separator);
                    v.space();
                });

                if v.style.is_3_dot_7() {
                    let right = v.node_piece_with(constructor, false, NodeContext::Assignment);
                    redirect = Some(v.arena.add(AssignPiece3Dot7::new(
                        separator, right, None, false, false, false,
                    )));
                } else {
                    let right = v.node_piece(constructor);
                    redirect = Some(v.arena.add(AssignPiece::new(separator, right)));
                }
            } else if !n.initializers.is_empty() {
                initializer_separator = Some(v.token_piece(n.separator.unwrap()));
                initializers = Some(v.create_comma_separated(ast.list_raw(n.initializers)));
            }

            let body = v.node_piece(n.body);

            let parameter_list = &ast[n.parameters];
            let piece = v.arena.add(ConstructorPiece::new(
                header,
                parameters,
                body,
                can_split(ast, parameter_list.parameters, parameter_list.right_parenthesis),
                parameter_list.right_delimiter.is_some(),
                redirect,
                initializer_separator,
                initializers,
            ));
            v.add(piece);
        });
    }

    fn visit_constructor_field_initializer(&mut self, node: Id<ConstructorFieldInitializer>) {
        let n = &self.ast[node];
        self.token(n.this_keyword);
        self.token(n.period);
        self.write_assignment(
            n.field_name.raw(),
            n.equals,
            n.expression.raw(),
            false,
            NodeContext::None,
            NodeContext::None,
        );
    }

    fn visit_constructor_name(&mut self, node: Id<ConstructorName>) {
        let ast = self.ast;
        let n = &ast[node];
        let ty = &ast[n.type_];
        if let Some(import_prefix) = ty.import_prefix {
            let import_prefix = &ast[import_prefix];
            self.token(import_prefix.name);
            self.token(import_prefix.period);
        }

        // The name of the type being constructed.
        self.token(ty.name);
        self.visit(ty.type_arguments);
        self.token(ty.question);

        // If this is a named constructor, the name.
        if n.name.is_some() {
            self.token(n.period);
            self.visit(n.name);
        }
    }

    fn visit_constructor_selector(&mut self, node: Id<ConstructorSelector>) {
        let n = &self.ast[node];
        self.token(n.period);
        self.visit(n.name);
    }

    fn visit_continue_statement(&mut self, node: Id<ContinueStatement>) {
        let n = &self.ast[node];
        self.write_break(n.continue_keyword, n.label, n.semicolon);
    }

    fn visit_declared_identifier(&mut self, node: Id<DeclaredIdentifier>) {
        let ast = self.ast;
        let n = &ast[node];
        self.write_parameter(
            n.type_,
            Some(n.name),
            ast.list_raw(n.metadata),
            &[n.keyword],
            None,
            None,
            None,
        );
    }

    fn visit_declared_variable_pattern(&mut self, node: Id<DeclaredVariablePattern>) {
        let n = &self.ast[node];
        self.write_pattern_variable(n.keyword, n.type_, n.name);
    }

    fn visit_do_statement(&mut self, node: Id<DoStatement>) {
        let n = &self.ast[node];
        self.token(n.do_keyword);
        self.space();
        self.visit(n.body);
        self.space();
        self.token(n.while_keyword);
        self.space();
        self.token(n.left_parenthesis);
        self.visit(n.condition);
        self.token(n.right_parenthesis);
        self.token(n.semicolon);
    }

    fn visit_dot_shorthand_constructor_invocation(
        &mut self,
        node: Id<DotShorthandConstructorInvocation>,
    ) {
        let n = &self.ast[node];
        self.modifier(n.const_keyword);
        self.token(n.period);
        self.visit(n.constructor_name);
        self.visit(n.type_arguments);
        self.visit(n.argument_list);
    }

    fn visit_dot_shorthand_invocation(&mut self, node: Id<DotShorthandInvocation>) {
        let n = &self.ast[node];
        self.token(n.period);
        self.visit(n.member_name);
        self.visit(n.type_arguments);
        self.visit(n.argument_list);
    }

    fn visit_dot_shorthand_property_access(&mut self, node: Id<DotShorthandPropertyAccess>) {
        let n = &self.ast[node];
        self.token(n.period);
        self.visit(n.property_name);
    }

    fn visit_dotted_name(&mut self, node: Id<DottedName>) {
        let ast = self.ast;
        for &token in ast.token_list(ast[node].tokens) {
            self.token(token);
        }
    }

    fn visit_double_literal(&mut self, node: Id<DoubleLiteral>) {
        self.token(self.ast[node].literal);
    }

    fn visit_empty_function_body(&mut self, node: Id<EmptyFunctionBody>) {
        self.token(self.ast[node].semicolon);
    }

    fn visit_empty_statement(&mut self, node: Id<EmptyStatement>) {
        self.token(self.ast[node].semicolon);
    }

    fn visit_enum_constant_declaration(&mut self, node: Id<EnumConstantDeclaration>) {
        let piece = self.create_enum_constant(node, false, None);
        self.add(piece);
    }

    fn visit_enum_declaration(&mut self, node: Id<EnumDeclaration>) {
        let ast = self.ast;
        let n = &ast[node];
        let keywords = [Some(n.enum_keyword)];
        let builder = TypeBuilder::new(
            ast,
            n.metadata,
            &keywords,
            None,
            None,
            Some(n.name_part),
            TypeClauses {
                with_clause: n.with_clause,
                implements_clause: n.implements_clause,
                ..TypeClauses::default()
            },
        );
        builder.build_enum(self, node);
    }

    fn visit_export_directive(&mut self, node: Id<ExportDirective>) {
        let n = &self.ast[node];
        self.write_import(
            n.metadata,
            n.uri,
            n.configurations,
            n.combinators,
            n.semicolon,
            n.export_keyword,
            None,
            None,
            None,
        );
    }

    fn visit_expression_function_body(&mut self, node: Id<ExpressionFunctionBody>) {
        let ast = self.ast;
        let n = &ast[node];
        let operator_piece = self.build(|v| {
            v.space();
            v.write_function_body_modifiers(n.keyword, n.star);
            v.token(n.function_definition);
        });

        let expression = self.node_piece_with(n.expression, false, NodeContext::Assignment);

        if self.style.is_3_dot_7() {
            let piece = self.arena.add(AssignPiece3Dot7::new(
                operator_piece,
                expression,
                None,
                false,
                can_block_split(ast, n.expression),
                block_format_type(ast, n.expression) == BlockFormat::Invocation,
            ));
            self.add(piece);
        } else {
            let assign_piece = self.arena.add(AssignPiece::with_avoid_split(
                operator_piece,
                expression,
                // Prefer splitting at `=>` and keeping the expression together
                // unless it's a collection literal.
                is_homogeneous_collection_body(ast, n.expression.raw()),
            ));

            // If a `=>` is directly nested inside another, force the outer one
            // to split. This is for performance reasons. A series of nested
            // AssignPieces can have combinatorial performance otherwise.
            // TODO(rnystrom): Figure out a better way to handle this. We could
            // possibly collapse a series of curried function expressions into
            // a single piece similar to how we handle nested conditional
            // expressions. In practice, outside of a few libraries that lean
            // heavily on currying, this is very rare.
            if let Some(function) = ast.cast::<FunctionExpression>(n.expression) {
                if ast.is::<ExpressionFunctionBody>(ast[function].body) {
                    self.arena.pin(assign_piece, State::SPLIT);
                }
            }

            self.add(assign_piece);
        }

        self.token(n.semicolon);
    }

    fn visit_expression_statement(&mut self, node: Id<ExpressionStatement>) {
        let n = &self.ast[node];
        self.visit(n.expression);

        // Treat the ";" as soft so that a long expression containing soft
        // code at the end isn't split unnecessarily.
        self.token_soft(n.semicolon);
    }

    fn visit_extension_declaration(&mut self, node: Id<ExtensionDeclaration>) {
        let ast = self.ast;
        let n = &ast[node];
        let keywords = [Some(n.extension_keyword)];
        let builder = TypeBuilder::new(
            ast,
            n.metadata,
            &keywords,
            n.name,
            n.type_parameters,
            None,
            TypeClauses {
                extension_on_clause: n.on_clause,
                ..TypeClauses::default()
            },
        );
        builder.build_class_body(self, n.body);
    }

    fn visit_extension_type_declaration(&mut self, node: Id<ExtensionTypeDeclaration>) {
        let ast = self.ast;
        let n = &ast[node];
        let keywords = [Some(n.extension_keyword), Some(n.type_keyword)];
        let builder = TypeBuilder::new(
            ast,
            n.metadata,
            &keywords,
            None,
            None,
            Some(n.name_part),
            TypeClauses {
                implements_clause: n.implements_clause,
                ..TypeClauses::default()
            },
        );
        builder.build_class_body(self, n.body);
    }

    fn visit_field_declaration(&mut self, node: Id<FieldDeclaration>) {
        let ast = self.ast;
        let n = &ast[node];
        self.with_metadata(ast.list_raw(n.metadata), false, |v| {
            v.modifier(n.external_keyword);
            v.modifier(n.static_keyword);
            v.modifier(n.abstract_keyword);
            v.modifier(n.covariant_keyword);
            v.visit(n.fields);
            v.token(n.semicolon);
        });
    }

    fn visit_field_formal_parameter(&mut self, node: Id<FieldFormalParameter>) {
        let ast = self.ast;
        let n = &ast[node];
        if let Some(suffix) = n.function_typed_suffix {
            // A function-typed field formal like:
            //
            //     C(this.fn(parameter));
            let suffix = &ast[suffix];
            self.write_function_type(
                n.type_,
                n.name,
                suffix.type_parameters,
                suffix.formal_parameters,
                suffix.question,
                Some(node.raw()),
                Some(n.this_keyword),
                Some(n.period),
            );
        } else {
            self.write_formal_parameter(
                node.raw(),
                n.type_,
                Some(n.name),
                n.const_final_or_var_keyword,
                Some(n.this_keyword),
                Some(n.period),
            );
        }
    }

    fn visit_formal_parameter_list(&mut self, node: Id<FormalParameterList>) {
        let ast = self.ast;
        let n = &ast[node];
        let parameters = ast.list_raw(n.parameters);

        // Find the first non-mandatory parameter (if there are any).
        let first_optional = parameters
            .iter()
            .position(|&p| {
                let kind = super::piece_factory::formal_parameter_parts(ast, p)
                    .map(|parts| parts.kind)
                    .unwrap_or_default();
                kind.is_named() || kind.is_optional_positional()
            })
            .map(|i| i as i64)
            .unwrap_or(-1);

        // If the parameter list is completely empty, write the brackets
        // inline so that we generate fewer separate pieces.
        if !can_split(ast, n.parameters, n.right_parenthesis) {
            // Treat the "()" as soft so that if a function expression follows
            // a long string literal (typically after the description in a
            // `test()` call), then we allow soft overflow for the string and
            // the rest of the line.
            self.token_soft(n.left_parenthesis);
            self.token_soft(n.right_parenthesis);
            return;
        }

        // Extension type representation clauses prior to Dart 3.13 don't
        // allow a trailing comma.
        let mut list_style = ListStyle::default();
        if !self.style.allow_trailing_comma_in_representation_clause()
            && ast
                .parent(node)
                .and_then(|p| ast.parent(p))
                .is_some_and(|p| ast.kind(p) == NodeKind::ExtensionTypeDeclaration)
        {
            list_style = ListStyle::with_commas(crate::piece::Commas::NonTrailing);
        }

        let mut builder = DelimitedListBuilder::new(list_style);

        let left = self.build(|v| {
            v.token(n.left_parenthesis);
            // If all parameters are optional, put the `[` or `{` right after
            // `(`.
            if !parameters.is_empty() && first_optional == 0 {
                v.token(n.left_delimiter);
            }
        });
        builder.add_left_bracket(left);

        for (i, &parameter) in parameters.iter().enumerate() {
            // If this is the first optional parameter, put the delimiter
            // before it.
            if first_optional > 0 && i as i64 == first_optional {
                builder.left_delimiter(self, n.left_delimiter.unwrap());
            }

            builder.visit(self, parameter);
        }

        builder.right_bracket(self, n.right_parenthesis, n.right_delimiter, None);
        let force_split = self
            .style
            .preserve_trailing_comma_before(ast, n.right_delimiter.unwrap_or(n.right_parenthesis));
        let block_shaped = self.style.block_format_parameter_lists();
        let piece = builder.build_with(self, force_split, block_shaped);
        self.add(piece);
    }

    fn visit_for_element(&mut self, node: Id<ForElement>) {
        let ast = self.ast;
        let n = &ast[node];
        self.write_for(
            n.await_keyword,
            n.for_keyword,
            n.left_parenthesis,
            n.for_loop_parts,
            n.right_parenthesis,
            n.body.raw(),
            is_spread_collection(ast, n.body.raw()),
            is_control_flow_element(ast, n.body.raw()),
        );
    }

    fn visit_for_statement(&mut self, node: Id<ForStatement>) {
        let ast = self.ast;
        let n = &ast[node];
        self.write_for(
            n.await_keyword,
            n.for_keyword,
            n.left_parenthesis,
            n.for_loop_parts,
            n.right_parenthesis,
            n.body.raw(),
            ast.is::<Block>(n.body),
            false,
        );
    }

    fn visit_function_declaration(&mut self, node: Id<FunctionDeclaration>) {
        let ast = self.ast;
        let n = &ast[node];
        let function = &ast[n.function_expression];
        self.write_function(
            ast.list_raw(n.metadata),
            &[n.external_keyword],
            n.return_type,
            None,
            n.property_keyword,
            Some(n.name),
            function.type_parameters,
            function.parameters,
            function.body,
        );
    }

    fn visit_function_declaration_statement(&mut self, node: Id<FunctionDeclarationStatement>) {
        let piece = self.node_piece(self.ast[node].function_declaration);
        self.add(piece);
    }

    fn visit_function_expression(&mut self, node: Id<FunctionExpression>) {
        let n = &self.ast[node];
        self.write_function(&[], &[], None, None, None, None, n.type_parameters, n.parameters, n.body);
    }

    fn visit_function_expression_invocation(&mut self, node: Id<FunctionExpressionInvocation>) {
        let n = &self.ast[node];
        self.visit(n.function);
        self.visit(n.type_arguments);
        self.visit(n.argument_list);
    }

    fn visit_function_reference(&mut self, node: Id<FunctionReference>) {
        let n = &self.ast[node];
        self.visit(n.function);
        self.visit(n.type_arguments);
    }

    fn visit_function_type_alias(&mut self, node: Id<FunctionTypeAlias>) {
        let ast = self.ast;
        let n = &ast[node];
        self.with_metadata(ast.list_raw(n.metadata), false, |v| {
            v.token(n.typedef_keyword);
            v.space();
            v.visit_with(n.return_type, false, true, NodeContext::None);
            v.token(n.name);
            v.visit(n.type_parameters);
            v.visit(n.parameters);
            v.token(n.semicolon);
        });
    }

    fn visit_generic_function_type(&mut self, node: Id<GenericFunctionType>) {
        let n = &self.ast[node];
        self.write_function_type(
            n.return_type,
            n.function_keyword,
            n.type_parameters,
            n.parameters,
            n.question,
            None,
            None,
            None,
        );
    }

    fn visit_generic_type_alias(&mut self, node: Id<GenericTypeAlias>) {
        let ast = self.ast;
        let n = &ast[node];
        self.with_metadata(ast.list_raw(n.metadata), false, |v| {
            v.token(n.typedef_keyword);
            v.space();
            v.token(n.name);
            v.visit(n.type_parameters);
            v.space();
            let equals = v.token_piece(n.equals);
            let ty = v.node_piece(n.type_);
            let piece = if v.style.is_3_dot_7() {
                v.arena
                    .add(AssignPiece3Dot7::new(equals, ty, None, false, false, false))
            } else {
                v.arena.add(AssignPiece::new(equals, ty))
            };
            v.add(piece);
            v.token(n.semicolon);
        });
    }

    fn visit_if_element(&mut self, node: Id<IfElement>) {
        let ast = self.ast;
        let mut piece = ControlFlowPiece::new(false);
        let mut pin_split = false;

        // Recurses through the else branches to flatten them into a linear
        // if-else chain handled by a single [IfPiece].
        fn traverse(
            v: &mut AstNodeVisitor<'_>,
            piece: &mut ControlFlowPiece,
            pin_split: &mut bool,
            preceding_else: Option<TokenId>,
            if_element: Id<IfElement>,
        ) {
            let ast = v.ast;
            let if_element = &ast[if_element];
            let spread_then = spread_collection(ast, if_element.then_element.raw());

            let condition = v.build(|v| {
                v.token_with(preceding_else, false, true, false);
                v.write_if_condition(
                    if_element.if_keyword,
                    if_element.left_parenthesis,
                    if_element.expression,
                    if_element.case_clause,
                    if_element.right_parenthesis,
                );

                // Make the `...` part of the header so that IfPiece can
                // correctly constrain the inner collection literal's ListPiece
                // to split.
                if let Some(spread) = spread_then {
                    v.space();
                    v.token(ast[spread].spread_operator);
                }
            });

            let then_element = match spread_then {
                Some(spread) => v.node_piece(ast[spread].expression),
                None => v.node_piece(if_element.then_element),
            };

            // If the then branch of an if element is itself a control flow
            // element, then force the outer if to always split.
            if is_control_flow_element(ast, if_element.then_element.raw()) {
                *pin_split = true;
            }

            piece.add(condition, then_element, spread_then.is_some());

            match if_element.else_element {
                Some(else_element) => {
                    if let Some(else_if) = ast.cast::<IfElement>(else_element) {
                        // Hit an else-if, so flatten it into the chain with
                        // the `else` becoming part of the next section's
                        // header.
                        traverse(v, piece, pin_split, if_element.else_keyword, else_if);
                    } else {
                        let spread_else = spread_collection(ast, else_element.raw());

                        // Any other kind of else body ends the chain, with the
                        // header for the last section just being the `else`
                        // keyword.
                        let header = v.build(|v| {
                            v.token(if_element.else_keyword.unwrap());

                            // Make the `...` part of the header so that IfPiece
                            // can correctly constrain the inner collection
                            // literal's ListPiece to split.
                            if let Some(spread) = spread_else {
                                v.space();
                                v.token(ast[spread].spread_operator);
                            }
                        });

                        let statement = match spread_else {
                            Some(spread) => v.node_piece(ast[spread].expression),
                            None => v.node_piece(else_element),
                        };

                        piece.add(header, statement, spread_else.is_some());

                        // If the else branch of an if element is itself a
                        // control flow element, then force the outer if to
                        // always split.
                        if is_control_flow_element(ast, if_element.then_element.raw()) {
                            *pin_split = true;
                        }
                    }
                }
                None => {} // Nothing to do.
            }
        }

        traverse(self, &mut piece, &mut pin_split, None, node);
        let _ = ast;
        let piece = self.arena.add(piece);
        if pin_split {
            self.arena.pin(piece, State::SPLIT);
        }
        self.add(piece);
    }

    fn visit_if_statement(&mut self, node: Id<IfStatement>) {
        let ast = self.ast;
        let mut piece = ControlFlowPiece::new(true);

        // Recurses through the else branches to flatten them into a linear
        // if-else chain handled by a single [IfPiece].
        fn traverse(
            v: &mut AstNodeVisitor<'_>,
            piece: &mut ControlFlowPiece,
            preceding_else: Option<TokenId>,
            if_statement: Id<IfStatement>,
        ) {
            let ast = v.ast;
            let if_statement = &ast[if_statement];
            let condition = v.build(|v| {
                v.token_with(preceding_else, false, true, false);
                v.write_if_condition(
                    if_statement.if_keyword,
                    if_statement.left_parenthesis,
                    if_statement.expression,
                    if_statement.case_clause,
                    if_statement.right_parenthesis,
                );
                v.space();
            });

            // Edge case: When the then branch is a block and there is an else
            // clause after it, we want to force the block to split even if
            // empty, like:
            //
            //     if (condition) {
            //     } else {
            //       body;
            //     }
            let then_statement = match ast.cast::<Block>(if_statement.then_statement) {
                Some(then_block) if if_statement.else_statement.is_some() => v.build(|v| {
                    v.write_block(then_block, true);
                }),
                _ => v.node_piece(if_statement.then_statement),
            };

            piece.add(
                condition,
                then_statement,
                ast.is::<Block>(if_statement.then_statement),
            );

            if let Some(else_statement) = if_statement.else_statement {
                if let Some(else_if) = ast.cast::<IfStatement>(else_statement) {
                    // Hit an else-if, so flatten it into the chain with the
                    // `else` becoming part of the next section's header.
                    traverse(v, piece, if_statement.else_keyword, else_if);
                } else {
                    // Any other kind of else body ends the chain, with the
                    // header for the last section just being the `else`
                    // keyword.
                    let header = v.build(|v| {
                        v.token_with(if_statement.else_keyword, false, true, false);
                    });
                    let statement = v.node_piece(else_statement);
                    piece.add(header, statement, ast.is::<Block>(else_statement));
                }
            }
        }

        traverse(self, &mut piece, None, node);
        let piece = self.arena.add(piece);

        // If statements almost always split at the clauses unless the if is a
        // simple if with only a single unbraced then statement and no else
        // clause, like:
        //
        //     if (condition) print("ok");
        let n = &ast[node];
        if ast.is::<Block>(n.then_statement) || n.else_statement.is_some() {
            self.arena.pin(piece, State::SPLIT);
        }

        self.add(piece);
    }

    fn visit_import_directive(&mut self, node: Id<ImportDirective>) {
        let n = &self.ast[node];
        self.write_import(
            n.metadata,
            n.uri,
            n.configurations,
            n.combinators,
            n.semicolon,
            n.import_keyword,
            n.deferred_keyword,
            n.as_keyword,
            n.prefix,
        );
    }

    fn visit_index_expression(&mut self, node: Id<IndexExpression>) {
        self.visit(self.ast[node].target);
        self.write_index_expression(node);
    }

    fn visit_instance_creation_expression(&mut self, node: Id<InstanceCreationExpression>) {
        let ast = self.ast;
        let n = &ast[node];
        self.token_with(n.keyword, false, true, false);

        let constructor = &ast[n.constructor_name];
        let ty = &ast[constructor.type_];
        if let Some(import_prefix) = ty.import_prefix {
            let import_prefix = &ast[import_prefix];
            self.token(import_prefix.name);
            self.token(import_prefix.period);
        }

        // The type being constructed.
        self.token(ty.name);
        self.visit(ty.type_arguments);

        // If this is a named constructor call, the name.
        if let Some(name) = constructor.name {
            self.token(constructor.period);
            self.visit(name);
        }

        self.visit(n.argument_list);
    }

    fn visit_integer_literal(&mut self, node: Id<IntegerLiteral>) {
        self.token(self.ast[node].literal);
    }

    fn visit_interpolation_expression(&mut self, node: Id<InterpolationExpression>) {
        let n = &self.ast[node];
        let piece = self.build(|v| {
            v.token(n.left_bracket);
            v.visit(n.expression);
            v.token(n.right_bracket);
        });

        // Don't allow splitting inside interpolated expressions (except for
        // mandatory splits from comments and sequences). Splits inside
        // interpolations almost never look good. It's usually better to just
        // let the lines overflow. More importantly, a single string literal
        // with many interpolations can easily lead to combinatorial
        // performance in the solver.
        // TODO(rnystrom): Traversing the entire interpolation Piece tree and
        // pinning it feels sort of inelegant. Is there a cleaner approach?
        let mut stack = vec![piece];
        let mut children = Vec::new();
        while let Some(piece) = stack.pop() {
            self.arena.prevent_split(piece);
            children.clear();
            self.arena.for_each_child(piece, &mut |child| children.push(child));
            stack.extend(children.iter().rev());
        }

        if self.style.is_3_dot_7() {
            self.add(piece);
        } else {
            // Wrap in grouping so that an infix split inside the interpolation
            // doesn't get collapsed in the surrounding context, like:
            //
            //     // Wrong:
            //     var x =
            //         '${"a"
            //         "b"}';
            //
            //     // Right:
            //     var x =
            //         '${"a"
            //             "b"}';
            let grouping = self.arena.add(GroupingPiece::new(piece));
            self.add(grouping);
        }
    }

    fn visit_interpolation_string(&mut self, node: Id<InterpolationString>) {
        let contents = self.ast[node].contents;
        if self.parent_context == NodeContext::MultilineStringInterpolation {
            self.multiline_token(contents, false);
        } else {
            self.token(contents);
        }
    }

    fn visit_is_expression(&mut self, node: Id<IsExpression>) {
        let n = &self.ast[node];
        self.write_type_test(n.expression, n.is_operator, n.type_, n.not_operator);
    }

    fn visit_label(&mut self, node: Id<Label>) {
        let n = &self.ast[node];
        self.token(n.name);
        self.token(n.colon);
    }

    fn visit_label_reference(&mut self, node: Id<LabelReference>) {
        self.token(self.ast[node].name);
    }

    fn visit_labeled_statement(&mut self, node: Id<LabeledStatement>) {
        let ast = self.ast;
        let n = &ast[node];
        let mut sequence = SequenceBuilder::new();
        for &label in ast.list_raw(n.labels) {
            sequence.visit(self, label);
        }

        sequence.visit(self, n.statement.raw());
        let piece = sequence.build(self);
        self.add(piece);
    }

    fn visit_library_directive(&mut self, node: Id<LibraryDirective>) {
        let ast = self.ast;
        let n = &ast[node];
        self.with_metadata(ast.list_raw(n.metadata), false, |v| {
            v.token(n.library_keyword);
            v.visit_with(n.name, true, false, NodeContext::None);
            v.token(n.semicolon);
        });
    }

    fn visit_list_literal(&mut self, node: Id<ListLiteral>) {
        let ast = self.ast;
        let n = &ast[node];
        self.write_collection(
            n.left_bracket,
            ast.list_raw(n.elements),
            n.right_bracket,
            n.const_keyword,
            n.type_arguments,
            ListStyle::default(),
            true,
            true,
        );
    }

    fn visit_list_pattern(&mut self, node: Id<ListPattern>) {
        let ast = self.ast;
        let n = &ast[node];
        self.write_collection(
            n.left_bracket,
            ast.list_raw(n.elements),
            n.right_bracket,
            None,
            n.type_arguments,
            ListStyle::default(),
            false,
            false,
        );
    }

    fn visit_logical_and_pattern(&mut self, node: Id<LogicalAndPattern>) {
        let ast = self.ast;
        let mut indent = true;
        if self.style.is_3_dot_7() {
            // If a logical and pattern occurs inside a map pattern entry, we
            // want to format the operands in parallel, like:
            //
            //     var {
            //       key:
            //         operand1 &&
            //         operand2,
            //     } = value;
            indent = self.parent_context != NodeContext::Assignment;
        }

        fn destructure(ast: &Ast, node: NodeId) -> Option<BinaryOperation> {
            let n = &ast[ast.cast::<LogicalAndPattern>(node)?];
            Some((n.left_operand.raw(), n.operator, n.right_operand.raw()))
        }

        let precedence = ast.tokens.ty(ast[node].operator).precedence();
        self.write_infix_chain(node.raw(), destructure, Some(precedence), indent);
    }

    fn visit_logical_or_pattern(&mut self, node: Id<LogicalOrPattern>) {
        let ast = self.ast;
        // If a logical or pattern is the outermost pattern in a switch
        // expression case, we flatten the operands like parallel cases:
        //
        //     e = switch (obj) {
        //       operand1 ||
        //       operand2 => value,
        //     };
        let mut indent = self.parent_context != NodeContext::SwitchExpressionCase;

        if self.style.is_3_dot_7() {
            // If a logical or pattern occurs inside a map pattern entry, we
            // want to format the operands in parallel, like:
            //
            //     var {
            //       key:
            //         operand1 ||
            //         operand2,
            //     } = value;
            if self.parent_context == NodeContext::Assignment {
                indent = false;
            }
        }

        fn destructure(ast: &Ast, node: NodeId) -> Option<BinaryOperation> {
            let n = &ast[ast.cast::<LogicalOrPattern>(node)?];
            Some((n.left_operand.raw(), n.operator, n.right_operand.raw()))
        }

        let precedence = ast.tokens.ty(ast[node].operator).precedence();
        self.write_infix_chain(node.raw(), destructure, Some(precedence), indent);
    }

    fn visit_map_literal_entry(&mut self, node: Id<MapLiteralEntry>) {
        let n = &self.ast[node];
        if self.style.is_3_dot_7() {
            self.write_assignment(
                n.key.raw(),
                n.separator,
                n.value.raw(),
                false,
                NodeContext::None,
                NodeContext::None,
            );
        } else {
            let left_piece = self.build(|v| {
                v.token(n.key_question);
                v.visit(n.key);
                v.token(n.separator);
            });

            let right_piece = self.build(|v| {
                v.token(n.value_question);
                v.visit(n.value);
            });

            let piece = self.arena.add(AssignPiece::new(left_piece, right_piece));
            self.add(piece);
        }
    }

    fn visit_map_pattern(&mut self, node: Id<MapPattern>) {
        let ast = self.ast;
        let n = &ast[node];
        self.write_collection(
            n.left_bracket,
            ast.list_raw(n.elements),
            n.right_bracket,
            None,
            n.type_arguments,
            ListStyle::default(),
            false,
            false,
        );
    }

    fn visit_map_pattern_entry(&mut self, node: Id<MapPatternEntry>) {
        let n = &self.ast[node];
        self.write_assignment(
            n.key.raw(),
            n.separator,
            n.value.raw(),
            false,
            NodeContext::None,
            NodeContext::None,
        );
    }

    fn visit_method_declaration(&mut self, node: Id<MethodDeclaration>) {
        let ast = self.ast;
        let n = &ast[node];
        self.write_function(
            ast.list_raw(n.metadata),
            &[n.external_keyword, n.modifier_keyword],
            n.return_type,
            None,
            n.operator_keyword.or(n.property_keyword),
            Some(n.name),
            n.type_parameters,
            n.parameters,
            n.body,
        );
    }

    fn visit_method_invocation(&mut self, node: Id<MethodInvocation>) {
        let ast = self.ast;
        let n = &ast[node];
        // If there's no target, this is a "bare" function call like
        // "foo(1, 2)", or a section in a cascade.
        //
        // If it looks like a constructor or static call, we want to keep the
        // target and method together instead of including the method in the
        // subsequent method chain.
        if n.target.is_none() || looks_like_static_call(ast, node.raw()) {
            self.visit(n.target);
            self.token(n.operator);
            self.visit(n.method_name);
            self.visit(n.type_arguments);
            self.visit(n.argument_list);
            return;
        }

        self.write_chain(node.raw());
    }

    fn visit_mixin_declaration(&mut self, node: Id<MixinDeclaration>) {
        let ast = self.ast;
        let n = &ast[node];
        let keywords = [n.base_keyword, Some(n.mixin_keyword)];
        let builder = TypeBuilder::new(
            ast,
            n.metadata,
            &keywords,
            Some(n.name),
            n.type_parameters,
            None,
            TypeClauses {
                mixin_on_clause: n.on_clause,
                implements_clause: n.implements_clause,
                ..TypeClauses::default()
            },
        );
        builder.build_class_body(self, n.body);
    }

    fn visit_named_argument(&mut self, node: Id<NamedArgument>) {
        let n = &self.ast[node];
        self.write_token_assignment(
            n.name,
            n.colon,
            n.argument_expression.raw(),
            false,
            NodeContext::NamedExpression,
        );
    }

    fn visit_named_type(&mut self, node: Id<NamedType>) {
        let ast = self.ast;
        let n = &ast[node];
        if let Some(import_prefix) = n.import_prefix {
            let import_prefix = &ast[import_prefix];
            self.token(import_prefix.name);
            self.token(import_prefix.period);
        }
        self.token(n.name);
        self.visit(n.type_arguments);
        self.token(n.question);
    }

    fn visit_native_clause(&mut self, node: Id<NativeClause>) {
        let n = &self.ast[node];
        self.token(n.native_keyword);
        self.visit_with(n.name, true, false, NodeContext::None);
    }

    fn visit_native_function_body(&mut self, node: Id<NativeFunctionBody>) {
        let n = &self.ast[node];
        self.space();
        self.token(n.native_keyword);
        self.visit_with(n.string_literal, true, false, NodeContext::None);
        self.token(n.semicolon);
    }

    fn visit_null_assert_pattern(&mut self, node: Id<NullAssertPattern>) {
        let n = &self.ast[node];
        self.write_postfix(n.pattern.raw(), n.operator);
    }

    fn visit_null_aware_element(&mut self, node: Id<NullAwareElement>) {
        let n = &self.ast[node];
        self.write_prefix(Some(n.question), n.value, false);
    }

    fn visit_null_check_pattern(&mut self, node: Id<NullCheckPattern>) {
        let n = &self.ast[node];
        self.write_postfix(n.pattern.raw(), n.operator);
    }

    fn visit_null_literal(&mut self, node: Id<NullLiteral>) {
        self.token(self.ast[node].literal);
    }

    fn visit_object_pattern(&mut self, node: Id<ObjectPattern>) {
        let ast = self.ast;
        let n = &ast[node];
        // If the object pattern is completely empty, write it inline so that
        // we create fewer pieces.
        if !can_split(ast, n.fields, n.right_parenthesis) {
            self.visit(n.type_);
            self.token(n.left_parenthesis);
            self.token(n.right_parenthesis);
            return;
        }

        let mut builder = DelimitedListBuilder::new(ListStyle::default());

        let left = self.build(|v| {
            v.visit(n.type_);
            v.token(n.left_parenthesis);
        });
        builder.add_left_bracket(left);

        for &field in ast.list_raw(n.fields) {
            builder.visit(self, field);
        }
        builder.right_bracket(self, n.right_parenthesis, None, None);
        let force_split = self.style.preserve_trailing_comma_before(ast, n.right_parenthesis);
        let piece = builder.build_with(self, force_split, true);
        self.add(piece);
    }

    fn visit_parenthesized_expression(&mut self, node: Id<ParenthesizedExpression>) {
        let n = &self.ast[node];
        self.write_parenthesized(n.left_parenthesis, n.expression.raw(), n.right_parenthesis);
    }

    fn visit_parenthesized_pattern(&mut self, node: Id<ParenthesizedPattern>) {
        let n = &self.ast[node];
        self.write_parenthesized(n.left_parenthesis, n.pattern.raw(), n.right_parenthesis);
    }

    fn visit_part_directive(&mut self, node: Id<PartDirective>) {
        let ast = self.ast;
        let n = &ast[node];
        self.with_metadata(ast.list_raw(n.metadata), false, |v| {
            v.token(n.part_keyword);
            v.space();
            v.visit(n.uri);
            v.token(n.semicolon);
        });
    }

    fn visit_part_of_directive(&mut self, node: Id<PartOfDirective>) {
        let ast = self.ast;
        let n = &ast[node];
        self.with_metadata(ast.list_raw(n.metadata), false, |v| {
            v.token(n.part_keyword);
            v.space();
            v.token(n.of_keyword);
            v.space();

            // Part-of may have either a name or a URI. Only one of these will
            // be non-null. We visit both since visit() ignores null.
            v.visit(n.library_name);
            v.visit(n.uri);
            v.token(n.semicolon);
        });
    }

    fn visit_pattern_assignment(&mut self, node: Id<PatternAssignment>) {
        let n = &self.ast[node];
        self.write_assignment(
            n.pattern.raw(),
            n.equals,
            n.expression.raw(),
            false,
            NodeContext::None,
            NodeContext::None,
        );
    }

    fn visit_pattern_field(&mut self, node: Id<PatternField>) {
        let n = &self.ast[node];
        self.visit(n.name);
        self.visit(n.pattern);
    }

    fn visit_pattern_field_name(&mut self, node: Id<PatternFieldName>) {
        let n = &self.ast[node];
        self.token(n.name);
        self.token(n.colon);
        if n.name.is_some() {
            self.space();
        }
    }

    fn visit_pattern_variable_declaration(&mut self, node: Id<PatternVariableDeclaration>) {
        let ast = self.ast;
        let n = &ast[node];
        // If the variable is part of a for loop, it looks weird to force the
        // metadata to split since it's in a sort of expression-ish location:
        //
        //     for (@meta var (x, y) in pairs) ...
        let inline_metadata = self.parent_context == NodeContext::ForLoopVariable;
        self.with_metadata(ast.list_raw(n.metadata), inline_metadata, |v| {
            v.token(n.keyword);
            v.space();
            v.write_assignment(
                n.pattern.raw(),
                n.equals,
                n.expression.raw(),
                false,
                NodeContext::None,
                NodeContext::None,
            );
        });
    }

    fn visit_pattern_variable_declaration_statement(
        &mut self,
        node: Id<PatternVariableDeclarationStatement>,
    ) {
        let n = &self.ast[node];
        self.visit(n.declaration);
        self.token(n.semicolon);
    }

    fn visit_postfix_expression(&mut self, node: Id<PostfixExpression>) {
        let n = &self.ast[node];
        self.write_postfix(n.operand.raw(), n.operator);
    }

    fn visit_prefixed_identifier(&mut self, node: Id<PrefixedIdentifier>) {
        self.write_chain(node.raw());
    }

    fn visit_prefix_expression(&mut self, node: Id<PrefixExpression>) {
        let ast = self.ast;
        let n = &ast[node];
        // Edge case: If we have two adjacent `-` or `--` operators, insert a
        // space between them so that we don't inadvertently collapse `- -`
        // into `--`.
        let is_minus = |token: TokenId| matches!(ast.tokens.lexeme(token), "-" | "--");
        let space = is_minus(n.operator)
            && ast
                .cast::<PrefixExpression>(n.operand)
                .is_some_and(|operand| is_minus(ast[operand].operator));

        self.write_prefix(Some(n.operator), n.operand, space);
    }

    fn visit_primary_constructor_body(&mut self, node: Id<PrimaryConstructorBody>) {
        let ast = self.ast;
        let n = &ast[node];
        self.with_metadata(ast.list_raw(n.metadata), false, |v| {
            let header = v.build(|v| {
                v.token(n.this_keyword);
            });

            let mut colon = None;
            let mut initializers = None;
            if !n.initializers.is_empty() {
                colon = Some(v.token_piece(n.colon.unwrap()));
                initializers = Some(v.create_comma_separated(ast.list_raw(n.initializers)));
            }

            let body = v.node_piece(n.body);

            let piece = v
                .arena
                .add(ConstructorPiece::this_block(header, body, colon, initializers));
            v.add(piece);
        });
    }

    fn visit_primary_constructor_name(&mut self, node: Id<PrimaryConstructorName>) {
        let n = &self.ast[node];
        self.token(n.period);
        self.token(n.name);
    }

    fn visit_property_access(&mut self, node: Id<PropertyAccess>) {
        let n = &self.ast[node];
        // If there's no target, this is a section in a cascade.
        if n.target.is_none() {
            self.token(n.operator);
            self.visit(n.property_name);
            return;
        }

        self.write_chain(node.raw());
    }

    fn visit_redirecting_constructor_invocation(
        &mut self,
        node: Id<RedirectingConstructorInvocation>,
    ) {
        let n = &self.ast[node];
        self.token(n.this_keyword);
        self.token(n.period);
        self.visit(n.constructor_name);
        self.visit(n.argument_list);
    }

    fn visit_record_literal(&mut self, node: Id<RecordLiteral>) {
        let ast = self.ast;
        let n = &ast[node];
        self.write_record(
            n.left_parenthesis,
            ast.list_raw(n.fields),
            n.right_parenthesis,
            n.const_keyword,
            true,
        );
    }

    fn visit_record_literal_named_field(&mut self, node: Id<RecordLiteralNamedField>) {
        let n = &self.ast[node];
        self.write_token_assignment(
            n.name,
            n.colon,
            n.field_expression.raw(),
            false,
            NodeContext::NamedExpression,
        );
    }

    fn visit_record_pattern(&mut self, node: Id<RecordPattern>) {
        let ast = self.ast;
        let n = &ast[node];
        self.write_record(
            n.left_parenthesis,
            ast.list_raw(n.fields),
            n.right_parenthesis,
            None,
            false,
        );
    }

    fn visit_record_type_annotation(&mut self, node: Id<RecordTypeAnnotation>) {
        let ast = self.ast;
        let n = &ast[node];
        let named_fields = n.named_fields.map(|fields| &ast[fields]);
        let positional_fields = ast.list_raw(n.positional_fields);

        // Single positional record types always have a trailing comma and are
        // not forced to split.
        let is_single_positional = positional_fields.len() == 1 && named_fields.is_none();

        let list_style = if is_single_positional {
            ListStyle::with_commas(crate::piece::Commas::AlwaysTrailing)
        } else {
            ListStyle::with_commas(crate::piece::Commas::Trailing)
        };
        let mut builder = DelimitedListBuilder::new(list_style);

        // If all parameters are optional, put the `{` right after `(`.
        let left = self.build(|v| {
            v.token(n.left_parenthesis);
            if positional_fields.is_empty() {
                if let Some(named_fields) = named_fields {
                    v.token(named_fields.left_bracket);
                }
            }
        });
        builder.add_left_bracket(left);

        for &positional_field in positional_fields {
            builder.visit(self, positional_field);
        }

        let mut right_delimiter = None;
        if let Some(named_fields) = named_fields {
            // If we have both positional fields and named fields, then we need
            // to add the left bracket delimiter before the first named field.
            if !positional_fields.is_empty() {
                builder.left_delimiter(self, named_fields.left_bracket);
            }
            for &named_field in ast.list_raw(named_fields.fields) {
                builder.visit(self, named_field);
            }
            right_delimiter = Some(named_fields.right_bracket);
        }

        builder.right_bracket(self, n.right_parenthesis, right_delimiter, None);
        let force_split = !is_single_positional
            && self
                .style
                .preserve_trailing_comma_before(ast, right_delimiter.unwrap_or(n.right_parenthesis));
        let piece = builder.build_with(self, force_split, true);
        self.add(piece);
        self.token(n.question);
    }

    fn visit_record_type_annotation_named_field(
        &mut self,
        node: Id<RecordTypeAnnotationNamedField>,
    ) {
        let n = &self.ast[node];
        self.write_record_type_field(n.metadata, n.type_, Some(n.name));
    }

    fn visit_record_type_annotation_positional_field(
        &mut self,
        node: Id<RecordTypeAnnotationPositionalField>,
    ) {
        let n = &self.ast[node];
        self.write_record_type_field(n.metadata, n.type_, n.name);
    }

    fn visit_relational_pattern(&mut self, node: Id<RelationalPattern>) {
        let n = &self.ast[node];
        self.token(n.operator);
        self.space();
        self.visit(n.operand);
    }

    fn visit_rethrow_expression(&mut self, node: Id<RethrowExpression>) {
        self.token(self.ast[node].rethrow_keyword);
    }

    fn visit_rest_pattern_element(&mut self, node: Id<RestPatternElement>) {
        let n = &self.ast[node];
        self.write_prefix(Some(n.operator), n.pattern, false);
    }

    fn visit_return_statement(&mut self, node: Id<ReturnStatement>) {
        let n = &self.ast[node];
        self.token(n.return_keyword);
        self.visit_with(n.expression, true, false, NodeContext::None);
        self.token(n.semicolon);
    }

    fn visit_script_tag(&mut self, node: Id<ScriptTag>) {
        self.token(self.ast[node].script_tag);
    }

    fn visit_set_or_map_literal(&mut self, node: Id<SetOrMapLiteral>) {
        let ast = self.ast;
        let n = &ast[node];
        self.write_collection(
            n.left_bracket,
            ast.list_raw(n.elements),
            n.right_bracket,
            n.const_keyword,
            n.type_arguments,
            ListStyle::default(),
            true,
            true,
        );
    }

    fn visit_regular_formal_parameter(&mut self, node: Id<RegularFormalParameter>) {
        let ast = self.ast;
        let n = &ast[node];
        if let Some(suffix) = n.function_typed_suffix {
            let suffix = &ast[suffix];
            self.write_function_type(
                n.type_,
                n.name.unwrap(),
                suffix.type_parameters,
                suffix.formal_parameters,
                suffix.question,
                Some(node.raw()),
                None,
                None,
            );
        } else {
            self.write_formal_parameter(
                node.raw(),
                n.type_,
                n.name,
                n.const_final_or_var_keyword,
                None,
                None,
            );
        }
    }

    fn visit_simple_identifier(&mut self, node: Id<SimpleIdentifier>) {
        self.token(self.ast[node].token);
    }

    fn visit_simple_string_literal(&mut self, node: Id<SimpleStringLiteral>) {
        let ast = self.ast;
        let literal = ast[node].literal;
        if lexeme_is_multiline(ast.tokens.lexeme(literal)) {
            self.multiline_token(literal, true);
        } else {
            self.token_soft(literal);
        }
    }

    fn visit_spread_element(&mut self, node: Id<SpreadElement>) {
        let n = &self.ast[node];
        self.write_prefix(Some(n.spread_operator), n.expression, false);
    }

    fn visit_string_interpolation(&mut self, node: Id<StringInterpolation>) {
        let ast = self.ast;
        let context = if string_is_multiline(ast, node.raw()) {
            NodeContext::MultilineStringInterpolation
        } else {
            NodeContext::None
        };
        for &element in ast.list_raw(ast[node].elements) {
            self.visit_in(element, context);
        }
    }

    fn visit_super_constructor_invocation(&mut self, node: Id<SuperConstructorInvocation>) {
        let n = &self.ast[node];
        self.token(n.super_keyword);
        self.token(n.period);
        self.visit(n.constructor_name);
        self.visit(n.argument_list);
    }

    fn visit_super_expression(&mut self, node: Id<SuperExpression>) {
        self.token(self.ast[node].super_keyword);
    }

    fn visit_super_formal_parameter(&mut self, node: Id<SuperFormalParameter>) {
        let ast = self.ast;
        let n = &ast[node];
        if let Some(suffix) = n.function_typed_suffix {
            // A function-typed super parameter like:
            //
            //     C(super.fn(parameter));
            let suffix = &ast[suffix];
            self.write_function_type(
                n.type_,
                n.name,
                suffix.type_parameters,
                suffix.formal_parameters,
                suffix.question,
                Some(node.raw()),
                Some(n.super_keyword),
                Some(n.period),
            );
        } else {
            self.write_formal_parameter(
                node.raw(),
                n.type_,
                Some(n.name),
                n.const_final_or_var_keyword,
                Some(n.super_keyword),
                Some(n.period),
            );
        }
    }

    fn visit_switch_expression(&mut self, node: Id<SwitchExpression>) {
        let ast = self.ast;
        let n = &ast[node];
        let mut list = DelimitedListBuilder::new(ListStyle {
            space_when_unsplit: true,
            ..ListStyle::default()
        });

        let left = self.build(|v| {
            v.write_control_flow_start(
                n.switch_keyword,
                n.left_parenthesis,
                n.expression.raw(),
                n.right_parenthesis,
            );
            v.space();
            v.token(n.left_bracket);
        });
        list.add_left_bracket(left);

        for &member in ast.list_raw(n.cases) {
            list.visit(self, member);
        }

        list.right_bracket(self, n.right_bracket, None, None);
        let piece = list.build_with(self, !n.cases.is_empty(), true);
        self.add(piece);
    }

    fn visit_switch_expression_case(&mut self, node: Id<SwitchExpressionCase>) {
        let ast = self.ast;
        let n = &ast[node];
        let guarded_pattern = &ast[n.guarded_pattern];
        let pattern_piece =
            self.node_piece_with(guarded_pattern.pattern, false, NodeContext::SwitchExpressionCase);

        let guard_piece = self.optional_node_piece(guarded_pattern.when_clause);
        let arrow_piece = self.token_piece(n.arrow);
        let body_piece = self.node_piece(n.expression);

        let piece = self.arena.add(CaseExpressionPiece::new(
            pattern_piece,
            guard_piece,
            arrow_piece,
            body_piece,
            pattern_can_block_split(ast, guarded_pattern.pattern.raw()),
            ast.is::<LogicalOrPattern>(guarded_pattern.pattern),
            !self.style.is_3_dot_7() || can_block_split(ast, n.expression),
        ));
        self.add(piece);
    }

    fn visit_switch_statement(&mut self, node: Id<SwitchStatement>) {
        let ast = self.ast;
        let n = &ast[node];
        self.write_control_flow_start(
            n.switch_keyword,
            n.left_parenthesis,
            n.expression.raw(),
            n.right_parenthesis,
        );
        self.space();

        let mut sequence = SequenceBuilder::new();
        sequence.left_bracket(self, n.left_bracket, false);

        for &member in ast.list_raw(n.members) {
            let (labels, keyword, colon, statements) = switch_member_parts(ast, member);
            for &label in ast.list_raw(labels) {
                sequence.visit(self, label);
            }

            sequence.add_comments_before(self, keyword);

            let case_piece = self.build(|v| {
                v.token(keyword);

                if let Some(case) = ast.cast::<SwitchCase>(member) {
                    v.space();
                    v.visit(ast[case].expression);
                } else if let Some(case) = ast.cast::<SwitchPatternCase>(member) {
                    v.space();

                    let guarded_pattern = &ast[ast[case].guarded_pattern];
                    let pattern_piece = v.node_piece(guarded_pattern.pattern);

                    if let Some(when_clause) = guarded_pattern.when_clause {
                        let when_piece = v.node_piece(when_clause);
                        let is_3_dot_7 = v.style.is_3_dot_7();
                        let piece = v.arena.add(InfixPiece::new(
                            vec![pattern_piece, when_piece],
                            is_3_dot_7,
                            Indent::Expression,
                        ));
                        v.add(piece);
                    } else {
                        v.add(pattern_piece);
                    }
                }

                v.token(colon);
            });

            // Don't allow any blank lines between the `case` line and the
            // first statement in the case (or the next case if this case has
            // no body).
            sequence.add_with(self, case_piece, Indent::None, false);

            for &statement in ast.list_raw(statements) {
                sequence.visit_with(self, statement, Indent::Block, false);
            }
        }

        sequence.right_bracket(self, n.right_bracket);
        let piece = sequence.build(self);
        self.add(piece);
    }

    fn visit_symbol_literal(&mut self, node: Id<SymbolLiteral>) {
        let ast = self.ast;
        let n = &ast[node];
        self.token(n.pound_sign);
        let components = ast.token_list(n.components);
        for (i, &component) in components.iter().enumerate() {
            // The '.' separator.
            if i > 0 {
                self.token(ast.tokens.previous(component));
            }

            self.token(component);
        }
    }

    fn visit_this_expression(&mut self, node: Id<ThisExpression>) {
        self.token(self.ast[node].this_keyword);
    }

    fn visit_throw_expression(&mut self, node: Id<ThrowExpression>) {
        let n = &self.ast[node];
        self.write_prefix(Some(n.throw_keyword), n.expression, true);
    }

    fn visit_top_level_variable_declaration(&mut self, node: Id<TopLevelVariableDeclaration>) {
        let ast = self.ast;
        let n = &ast[node];
        self.with_metadata(ast.list_raw(n.metadata), false, |v| {
            v.modifier(n.external_keyword);
            v.visit(n.variables);
            v.token(n.semicolon);
        });
    }

    fn visit_try_statement(&mut self, node: Id<TryStatement>) {
        self.write_try(node);
    }

    fn visit_type_argument_list(&mut self, node: Id<TypeArgumentList>) {
        let ast = self.ast;
        let n = &ast[node];
        self.write_type_list(n.left_bracket, ast.list_raw(n.arguments), n.right_bracket);
    }

    fn visit_type_parameter(&mut self, node: Id<TypeParameter>) {
        let ast = self.ast;
        let n = &ast[node];
        self.with_metadata(ast.list_raw(n.metadata), true, |v| {
            if let Some(bound) = n.bound {
                let type_parameter_piece = v.build(|v| {
                    v.token(n.name);
                });

                let bound_piece = v.build(|v| {
                    v.token(n.extends_keyword);
                    v.space();
                    v.visit(bound);
                });

                let piece = v
                    .arena
                    .add(TypeParameterBoundPiece::new(type_parameter_piece, bound_piece));
                v.add(piece);
            } else {
                // No bound.
                v.token(n.name);
            }
        });
    }

    fn visit_type_parameter_list(&mut self, node: Id<TypeParameterList>) {
        let ast = self.ast;
        let n = &ast[node];
        self.write_type_list(n.left_bracket, ast.list_raw(n.type_parameters), n.right_bracket);
    }

    fn visit_variable_declaration_list(&mut self, node: Id<VariableDeclarationList>) {
        let ast = self.ast;
        let n = &ast[node];
        // If the variable is part of a for loop, it looks weird to force the
        // metadata to split since it's in a sort of expression-ish location:
        //
        //     for (@meta var x in list) ...
        let inline_metadata = self.parent_context == NodeContext::ForLoopVariable;
        self.with_metadata(ast.list_raw(n.metadata), inline_metadata, |v| {
            let header = v.build(|v| {
                v.modifier(n.late_keyword);
                v.modifier(n.keyword);
                v.visit(n.type_);
            });

            let mut variables = Vec::new();

            for &variable in ast.list(n.variables) {
                let variable_node = variable.raw();
                let variable = &ast[variable];
                if let (Some(equals), Some(initializer)) = (variable.equals, variable.initializer)
                {
                    if v.style.is_3_dot_7() {
                        let variable_piece = v.token_piece(variable.name);

                        let equals_piece = v.build(|v| {
                            v.space();
                            v.token(equals);
                        });

                        let initializer_piece =
                            v.node_piece_with(initializer, true, NodeContext::Assignment);

                        variables.push(v.arena.add(AssignPiece3Dot7::new(
                            equals_piece,
                            initializer_piece,
                            Some(variable_piece),
                            false,
                            can_block_split(ast, initializer),
                            false,
                        )));
                    } else {
                        let variable_piece = v.build(|v| {
                            v.token(variable.name);
                            v.space();
                            v.token(equals);
                        });

                        let initializer_piece =
                            v.node_piece_with(initializer, true, NodeContext::None);

                        variables.push(
                            v.arena
                                .add(AssignPiece::new(variable_piece, initializer_piece)),
                        );
                    }
                } else {
                    let _ = variable_node;
                    variables.push(v.token_piece_with(variable.name, None, true, false));
                }
            }

            let is_3_dot_7 = v.style.is_3_dot_7();
            let piece = v.arena.add(VariablePiece::new(
                header,
                variables,
                n.type_.is_some(),
                is_3_dot_7,
            ));
            v.add(piece);
        });
    }

    fn visit_variable_declaration_statement(&mut self, node: Id<VariableDeclarationStatement>) {
        let n = &self.ast[node];
        self.visit(n.variables);
        self.token(n.semicolon);
    }

    fn visit_when_clause(&mut self, node: Id<WhenClause>) {
        let n = &self.ast[node];
        self.write_prefix(Some(n.when_keyword), n.expression, true);
    }

    fn visit_while_statement(&mut self, node: Id<WhileStatement>) {
        let ast = self.ast;
        let n = &ast[node];
        let condition = self.build(|v| {
            v.write_control_flow_start(
                n.while_keyword,
                n.left_parenthesis,
                n.condition.raw(),
                n.right_parenthesis,
            );
            v.space();
        });

        let body = self.node_piece(n.body);

        let mut piece = ControlFlowPiece::new(true);
        piece.add(condition, body, ast.is::<Block>(n.body));
        let piece = self.arena.add(piece);
        self.add(piece);
    }

    fn visit_wildcard_pattern(&mut self, node: Id<WildcardPattern>) {
        let n = &self.ast[node];
        self.write_pattern_variable(n.keyword, n.type_, n.name);
    }

    fn visit_yield_statement(&mut self, node: Id<YieldStatement>) {
        let n = &self.ast[node];
        self.token(n.yield_keyword);
        self.token(n.star);
        self.space();
        self.visit(n.expression);
        self.token(n.semicolon);
    }
}

/// The labels, keyword, colon and statements of a Dart `SwitchMember`.
fn switch_member_parts(
    ast: &Ast,
    member: NodeId,
) -> (NodeList<Label>, TokenId, TokenId, NodeList<Statement>) {
    if let Some(case) = ast.cast::<SwitchCase>(member) {
        let case = &ast[case];
        return (case.labels, case.keyword, case.colon, case.statements);
    }
    if let Some(case) = ast.cast::<SwitchPatternCase>(member) {
        let case = &ast[case];
        return (case.labels, case.keyword, case.colon, case.statements);
    }
    let default = &ast[ast.cast::<SwitchDefault>(member).unwrap()];
    (default.labels, default.keyword, default.colon, default.statements)
}

/// Dart `StringLiteral.stringValue`.
fn string_value(ast: &Ast, node: NodeId) -> Option<String> {
    if let Some(simple) = ast.cast::<SimpleStringLiteral>(node) {
        return Some(ast[simple].value.to_string());
    }
    if let Some(adjacent) = ast.cast::<AdjacentStrings>(node) {
        let mut result = String::new();
        for &string in ast.list_raw(ast[adjacent].strings) {
            result.push_str(&string_value(ast, string)?);
        }
        return Some(result);
    }
    None
}

/// The sections of imports and exports that should have blank lines between
/// them.
///
/// "Effective Dart" says that "dart:", "package:", and other imports and
/// exports should be grouped into their own sections with blank lines
/// between them. The formatter doesn't reorder directives, but it can
/// enforce a blank line between sections.
///
/// See: https://dart.dev/effective-dart/style#ordering
#[derive(Clone, Copy, PartialEq, Eq)]
enum DirectiveSection {
    /// A "dart:" URI.
    Dart,

    /// A "package:" URI.
    Package,

    /// A relative or other kind of URI.
    Other,

    /// A directive that doesn't have a URI.
    None,
}

impl DirectiveSection {
    /// Returns the section that [directive] belongs to.
    fn parse(ast: &Ast, directive: NodeId) -> DirectiveSection {
        let uri = if let Some(import) = ast.cast::<ImportDirective>(directive) {
            ast[import].uri
        } else if let Some(export) = ast.cast::<ExportDirective>(directive) {
            ast[export].uri
        } else {
            return DirectiveSection::None;
        };

        let Some(uri) = string_value(ast, uri.raw()) else {
            return DirectiveSection::None;
        };

        if uri.starts_with("dart:") {
            return DirectiveSection::Dart;
        }
        if uri.starts_with("package:") {
            return DirectiveSection::Package;
        }
        DirectiveSection::Other
    }
}

#[allow(dead_code)]
fn _unused(ast: &Ast, node: NodeId) -> (Option<TokenId>, bool) {
    (comma_after_token(ast, node), node_can_block_split(ast, node))
}

#[allow(dead_code)]
fn _unused_piece(_: PieceId) {}
