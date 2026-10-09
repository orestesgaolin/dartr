// Dart source: dart_style lib/src/short/source_visitor.dart

use dartr_ast::*;
use dartr_syntax::{LineInfo, TokenId, TokenType};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ast_extensions::{
    can_split, cascade_allow_inline, comma_after, contains_line_comments, has_comma_after,
    has_non_empty_body, has_preceding_comments, is_collection_literal, is_control_flow_element,
    is_function_expression_body, is_spread_collection, is_trailing_comma_argument,
    list_has_comma_after, looks_like_static_call, nodes_have_comma_after,
    spread_collection_bracket,
};
use crate::comment_type::CommentType;
use crate::constants::{Cost, Indent};
use crate::dart_formatter::DartFormatter;
use crate::source_code::SourceCode;
use crate::text::utf16_len;

use super::argument_list_visitor::ArgumentListVisitor;
use super::arena::{ChunkId, RuleId};
use super::call_chain_visitor::CallChainVisitor;
use super::chunk_builder::ChunkBuilder;
use super::rule::argument::{NamedRule, PositionalRule};
use super::rule::combinator::CombinatorRule;
use super::rule::type_argument::TypeArgumentRule;
use super::source_comment::SourceComment;

type Callback<'a> = Option<fn(&mut SourceVisitor<'a>)>;

/// Dart `Argument.argumentExpression`: the expression of a named argument,
/// or the argument itself.
pub fn argument_expression(ast: &Ast, argument: NodeId) -> NodeId {
    match ast.cast::<NamedArgument>(argument) {
        Some(named) => ast[named].argument_expression.raw(),
        None => argument,
    }
}

/// Dart `FormalParameter.isNamed || FormalParameter.isOptionalPositional`.
fn is_named_or_optional_positional(ast: &Ast, parameter: NodeId) -> bool {
    let kind = match ast.kind(parameter) {
        NodeKind::RegularFormalParameter => {
            ast[Id::<RegularFormalParameter>::from_raw(parameter)].kind
        }
        NodeKind::FieldFormalParameter => ast[Id::<FieldFormalParameter>::from_raw(parameter)].kind,
        NodeKind::SuperFormalParameter => ast[Id::<SuperFormalParameter>::from_raw(parameter)].kind,
        _ => ParameterKind::Required,
    };
    kind.is_named() || kind.is_optional_positional()
}

/// Visits every token of the AST and passes all of the relevant bits to a
/// [ChunkBuilder].
pub struct SourceVisitor<'a> {
    /// The builder for the block that is currently being visited.
    pub builder: ChunkBuilder,

    pub ast: &'a Ast,

    /// Cached line info for calculating blank lines.
    line_info: &'a LineInfo,

    /// The source being formatted.
    source: &'a SourceCode,

    /// The UTF-16 code units of the source text, computed when a selection
    /// end is searched.
    source_units: Option<Vec<u16>>,

    /// The most recently written token.
    ///
    /// This is used to determine how many lines are between a pair of tokens in
    /// the original source in places where a user can control whether or not a
    /// blank line or newline is left in the output.
    last_token: TokenId,

    /// `true` if the visitor has written past the beginning of the selection in
    /// the original source text.
    passed_selection_start: bool,

    /// `true` if the visitor has written past the end of the selection in the
    /// original source text.
    passed_selection_end: bool,

    /// The character offset of the end of the selection, if there is a selection.
    ///
    /// This is calculated and cached by [find_selection_end].
    selection_end: Option<i64>,

    /// A stack that tracks forcing nested collections to split.
    ///
    /// Each entry corresponds to a collection currently being visited and the
    /// value is whether or not it should be forced to split. Every time a
    /// collection is entered, it sets all of the existing elements to `true`
    /// then it pushes `false` for itself.
    ///
    /// When done visiting the elements, it removes its value. If it was set to
    /// `true`, we know we visited a nested collection so we force this one to
    /// split.
    collection_splits: Vec<bool>,

    /// The mapping for blocks that are managed by the argument list that contains
    /// them.
    ///
    /// When a block expression, such as a collection literal or a multiline
    /// string, appears inside an [ArgumentSublist], the argument list provides a
    /// rule for the body to split to ensure that all blocks split in unison. It
    /// also tracks the chunk before the argument that determines whether or not
    /// the block body is indented like an expression or a statement.
    ///
    /// Before a block argument is visited, [ArgumentSublist] binds itself to the
    /// beginning token of each block it controls. When we later visit that
    /// literal, we use the token to find that association.
    ///
    /// This mapping is also used for spread collection literals that appear
    /// inside control flow elements to ensure that when a "then" collection
    /// splits, the corresponding "else" one does too.
    block_rules: FxHashMap<TokenId, RuleId>,
    block_previous_chunks: FxHashMap<TokenId, ChunkId>,

    /// Tracks tokens whose preceding comments have already been handled and
    /// written and thus don't need to be written when the token is.
    suppress_preceding_comments_and_new_lines: FxHashSet<TokenId>,
}

impl<'a> SourceVisitor<'a> {
    /// Initialize a newly created visitor to write source code representing
    /// the visited nodes to the given [writer].
    pub fn new(
        formatter: &DartFormatter,
        ast: &'a Ast,
        line_info: &'a LineInfo,
        source: &'a SourceCode,
    ) -> Self {
        SourceVisitor {
            builder: ChunkBuilder::new(formatter.page_width as i32, formatter.indent as i32),
            ast,
            line_info,
            source,
            source_units: None,
            last_token: TokenId::NONE,
            passed_selection_start: false,
            passed_selection_end: false,
            selection_end: None,
            collection_splits: Vec::new(),
            block_rules: FxHashMap::default(),
            block_previous_chunks: FxHashMap::default(),
            suppress_preceding_comments_and_new_lines: FxHashSet::default(),
        }
    }

    /// Runs the visitor on [node], formatting its contents.
    ///
    /// Returns a [SourceCode] containing the resulting formatted source and
    /// updated selection, if any.
    ///
    /// This is the only method that should be called externally. Everything else
    /// is effectively private.
    pub fn run(mut self, node: NodeId, line_ending: &str) -> SourceCode {
        self.visit(node);

        // Output trailing comments.
        let ast = self.ast;
        self.write_preceding_comments_and_newlines(ast.tokens.next(ast.end_token(node)));

        // Finish writing and return the complete result.
        let source = self.source;
        self.builder.end(line_ending, source)
    }

    /// Dart `node.accept(this)`: dispatches to the `visit_*` method of the
    /// node's kind.
    pub fn visit(&mut self, node: impl Into<NodeId>) {
        let node: NodeId = node.into();
        macro_rules! dispatch {
            ($($kind:ident => $method:ident),* $(,)?) => {
                match self.ast.kind(node) {
                    $(NodeKind::$kind => self.$method(Id::from_raw(node)),)*
                    NodeKind::ArgumentList => self.visit_argument_list(Id::from_raw(node), true),
                    NodeKind::FormalParameterList => {
                        self.visit_formal_parameter_list(Id::from_raw(node), true)
                    }
                    NodeKind::NamedArgument => self.visit_named_argument(Id::from_raw(node), None),
                    NodeKind::Comment | NodeKind::CommentReference => {}
                    kind => panic!("SourceVisitor does not support {kind:?}"),
                }
            };
        }
        dispatch! {
            AdjacentStrings => visit_adjacent_strings,
            Annotation => visit_annotation,
            AsExpression => visit_as_expression,
            AssertInitializer => visit_assert_initializer,
            AssertStatement => visit_assert_statement,
            AssignedVariablePattern => visit_assigned_variable_pattern,
            AssignmentExpression => visit_assignment_expression,
            AwaitExpression => visit_await_expression,
            BinaryExpression => visit_binary_expression,
            Block => visit_block,
            BlockClassBody => visit_block_class_body,
            BlockEnumBody => visit_block_enum_body,
            BlockFunctionBody => visit_block_function_body,
            BooleanLiteral => visit_boolean_literal,
            BreakStatement => visit_break_statement,
            CascadeExpression => visit_cascade_expression,
            CastPattern => visit_cast_pattern,
            CatchClause => visit_catch_clause,
            CatchClauseParameter => visit_catch_clause_parameter,
            ClassDeclaration => visit_class_declaration,
            ClassTypeAlias => visit_class_type_alias,
            CompilationUnit => visit_compilation_unit,
            ConditionalExpression => visit_conditional_expression,
            Configuration => visit_configuration,
            ConstantPattern => visit_constant_pattern,
            ConstructorDeclaration => visit_constructor_declaration,
            ConstructorFieldInitializer => visit_constructor_field_initializer,
            ConstructorName => visit_constructor_name,
            ContinueStatement => visit_continue_statement,
            DeclaredIdentifier => visit_declared_identifier,
            DeclaredVariablePattern => visit_declared_variable_pattern,
            DoStatement => visit_do_statement,
            DottedName => visit_dotted_name,
            DoubleLiteral => visit_double_literal,
            EmptyClassBody => visit_empty_class_body,
            EmptyEnumBody => visit_empty_enum_body,
            EmptyFunctionBody => visit_empty_function_body,
            EmptyStatement => visit_empty_statement,
            EnumConstantDeclaration => visit_enum_constant_declaration,
            EnumDeclaration => visit_enum_declaration,
            ExportDirective => visit_export_directive,
            ExpressionFunctionBody => visit_expression_function_body,
            ExpressionStatement => visit_expression_statement,
            ExtendsClause => visit_extends_clause,
            ExtensionDeclaration => visit_extension_declaration,
            ExtensionTypeDeclaration => visit_extension_type_declaration,
            FieldDeclaration => visit_field_declaration,
            FieldFormalParameter => visit_field_formal_parameter,
            FormalParameterDefaultClause => visit_formal_parameter_default_clause,
            ForElement => visit_for_element,
            ForStatement => visit_for_statement,
            ForEachPartsWithDeclaration => visit_for_each_parts_with_declaration,
            ForEachPartsWithIdentifier => visit_for_each_parts_with_identifier,
            ForEachPartsWithPattern => visit_for_each_parts_with_pattern,
            ForPartsWithDeclarations => visit_for_parts_with_declarations,
            ForPartsWithExpression => visit_for_parts_with_expression,
            ForPartsWithPattern => visit_for_parts_with_pattern,
            FunctionDeclaration => visit_function_declaration,
            FunctionDeclarationStatement => visit_function_declaration_statement,
            FunctionExpression => visit_function_expression,
            FunctionExpressionInvocation => visit_function_expression_invocation,
            FunctionReference => visit_function_reference,
            FunctionTypeAlias => visit_function_type_alias,
            FunctionTypedFormalParameterSuffix => visit_function_typed_formal_parameter_suffix,
            GenericFunctionType => visit_generic_function_type,
            GenericTypeAlias => visit_generic_type_alias,
            HideCombinator => visit_hide_combinator,
            IfElement => visit_if_element,
            IfStatement => visit_if_statement,
            ImplementsClause => visit_implements_clause,
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
            NameWithTypeParameters => visit_name_with_type_parameters,
            NamedType => visit_named_type,
            NativeClause => visit_native_clause,
            NativeFunctionBody => visit_native_function_body,
            NullAssertPattern => visit_null_assert_pattern,
            NullCheckPattern => visit_null_check_pattern,
            NullLiteral => visit_null_literal,
            ObjectPattern => visit_object_pattern,
            MixinOnClause => visit_mixin_on_clause,
            ParenthesizedExpression => visit_parenthesized_expression,
            ParenthesizedPattern => visit_parenthesized_pattern,
            PartDirective => visit_part_directive,
            PartOfDirective => visit_part_of_directive,
            PatternAssignment => visit_pattern_assignment,
            PatternField => visit_pattern_field,
            PatternVariableDeclaration => visit_pattern_variable_declaration,
            PatternVariableDeclarationStatement => visit_pattern_variable_declaration_statement,
            PostfixExpression => visit_postfix_expression,
            PrefixedIdentifier => visit_prefixed_identifier,
            PrefixExpression => visit_prefix_expression,
            PrimaryConstructorDeclaration => visit_primary_constructor_declaration,
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
            ShowCombinator => visit_show_combinator,
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
            VariableDeclaration => visit_variable_declaration,
            VariableDeclarationList => visit_variable_declaration_list,
            VariableDeclarationStatement => visit_variable_declaration_statement,
            WhileStatement => visit_while_statement,
            WildcardPattern => visit_wildcard_pattern,
            WithClause => visit_with_clause,
            YieldStatement => visit_yield_statement,
        }
    }

    fn visit_adjacent_strings(&mut self, node: Id<AdjacentStrings>) {
        let ast = self.ast;

        // We generally want to indent adjacent strings because it can be confusing
        // otherwise when they appear in a list of expressions, like:
        //
        //     [
        //       "one",
        //       "two"
        //       "three",
        //       "four"
        //     ]
        //
        // Especially when these stings are longer, it can be hard to tell that
        // "three" is a continuation of the previous argument.
        //
        // However, the indentation is distracting in argument lists that don't
        // suffer from this ambiguity:
        //
        //     test(
        //         "A very long test description..."
        //             "this indentation looks bad.", () { ... });
        //
        // To balance these, we omit the indentation when an adjacent string
        // expression is the only string in an argument list.
        let mut should_nest = true;

        let parent = ast.parent(node).unwrap();
        if let Some(parent) = ast.cast::<ArgumentList>(parent) {
            should_nest = false;

            for &argument in ast.list_raw(ast[parent].arguments) {
                if argument == node.raw() {
                    continue;
                }
                if ast.is::<StringLiteral>(argument) {
                    should_nest = true;
                    break;
                }
            }
        } else if ast.is::<Assertion>(parent) {
            // Treat asserts like argument lists.
            should_nest = false;
            let (condition, message) = match ast.kind(parent) {
                NodeKind::AssertInitializer => {
                    let assertion = &ast[Id::<AssertInitializer>::from_raw(parent)];
                    (assertion.condition.raw(), assertion.message.map(|m| m.raw()))
                }
                _ => {
                    let assertion = &ast[Id::<AssertStatement>::from_raw(parent)];
                    (assertion.condition.raw(), assertion.message.map(|m| m.raw()))
                }
            };
            if condition != node.raw() && ast.is::<StringLiteral>(condition) {
                should_nest = true;
            }

            if let Some(message) = message {
                if message != node.raw() && ast.is::<StringLiteral>(message) {
                    should_nest = true;
                }
            }
        } else if ast.kind(parent) == NodeKind::VariableDeclaration
            || ast.cast::<AssignmentExpression>(parent).is_some_and(|assignment| {
                ast[assignment].right_hand_side.raw() == node.raw()
                    && ast
                        .parent(assignment)
                        .is_some_and(|p| ast.kind(p) == NodeKind::ExpressionStatement)
            })
        {
            // Don't add extra indentation in a variable initializer or assignment:
            //
            //     var variable =
            //         "no extra"
            //         "indent";
            should_nest = false;
        } else if matches!(
            ast.kind(parent),
            NodeKind::NamedArgument | NodeKind::ExpressionFunctionBody
        ) {
            should_nest = false;
        }

        self.builder.start_span_normal();
        self.builder.start_rule(None);
        if should_nest {
            self.builder.nest();
        }
        self.visit_nodes(
            ast.list_raw(ast[node].strings),
            None,
            Some(Self::split_or_newline),
            None,
        );
        if should_nest {
            self.builder.unnest();
        }
        self.builder.end_rule();
        self.builder.end_span();
    }

    fn visit_annotation(&mut self, node: Id<Annotation>) {
        let n = &self.ast[node];
        self.token(n.at_sign);
        self.visit(n.name);

        self.builder.nest();
        self.visit_opt(n.type_arguments);
        self.token_opt(n.period);
        self.visit_opt(n.constructor_name);

        if let Some(arguments) = n.arguments {
            self.visit_argument_list(arguments, false);
        }

        self.builder.unnest();
    }

    /// Visits an argument list.
    ///
    /// This is a bit complex to handle the rules for formatting positional and
    /// named arguments. The goals, in rough order of descending priority are:
    ///
    /// 1. Keep everything on the first line.
    /// 2. Keep the named arguments together on the next line.
    /// 3. Keep everything together on the second line.
    /// 4. Split between one or more positional arguments, trying to keep as many
    ///    on earlier lines as possible.
    /// 5. Split the named arguments each onto their own line.
    pub fn visit_argument_list(&mut self, node: Id<ArgumentList>, nest_expression: bool) {
        let ast = self.ast;
        let n = &ast[node];

        // Corner case: handle empty argument lists.
        if n.arguments.is_empty() {
            self.token(n.left_parenthesis);

            // If there is a comment inside the parens, do allow splitting before it.
            if has_preceding_comments(ast, n.right_parenthesis) {
                self.solo_zero_split();
            }

            self.token(n.right_parenthesis);
            return;
        }

        // If the argument list has a trailing comma, format it like a collection
        // literal where each argument goes on its own line, they are indented +2,
        // and the ")" ends up on its own line.
        if list_has_comma_after(ast, n.arguments) {
            self.visit_collection_literal(
                n.left_parenthesis,
                ast.list_raw(n.arguments),
                n.right_parenthesis,
                None,
                None,
                false,
                false,
            );
            return;
        }

        if nest_expression {
            self.builder.nest();
        }
        ArgumentListVisitor::new(self, node).visit(self);
        if nest_expression {
            self.builder.unnest();
        }
    }

    fn visit_as_expression(&mut self, node: Id<AsExpression>) {
        let n = &self.ast[node];
        self.builder.start_span_normal();
        self.builder.nest();
        self.visit(n.expression);
        self.solo_split(Cost::NORMAL);
        self.token(n.as_operator);
        self.space();
        self.visit(n.type_);
        self.builder.unnest();
        self.builder.end_span();
    }

    fn visit_assert_initializer(&mut self, node: Id<AssertInitializer>) {
        let ast = self.ast;
        let n = &ast[node];
        self.token(n.assert_keyword);

        let mut arguments = vec![n.condition.raw()];
        if let Some(message) = n.message {
            arguments.push(message.raw());
        }

        // If the argument list has a trailing comma, format it like a collection
        // literal where each argument goes on its own line, they are indented +2,
        // and the ")" ends up on its own line.
        if nodes_have_comma_after(ast, &arguments) {
            self.visit_collection_literal(
                n.left_parenthesis,
                &arguments,
                n.right_parenthesis,
                None,
                None,
                false,
                false,
            );
            return;
        }

        self.builder.nest();
        let visitor = ArgumentListVisitor::for_arguments(
            self,
            n.left_parenthesis,
            n.right_parenthesis,
            arguments,
        );
        visitor.visit(self);
        self.builder.unnest();
    }

    fn visit_assert_statement(&mut self, node: Id<AssertStatement>) {
        let ast = self.ast;
        let n = &ast[node];
        self.simple_statement(Some(n.semicolon), |v| {
            v.token(n.assert_keyword);

            let mut arguments = vec![n.condition.raw()];
            if let Some(message) = n.message {
                arguments.push(message.raw());
            }

            // If the argument list has a trailing comma, format it like a collection
            // literal where each argument goes on its own line, they are indented +2,
            // and the ")" ends up on its own line.
            if nodes_have_comma_after(ast, &arguments) {
                v.visit_collection_literal(
                    n.left_parenthesis,
                    &arguments,
                    n.right_parenthesis,
                    None,
                    None,
                    false,
                    false,
                );
                return;
            }

            let visitor = ArgumentListVisitor::for_arguments(
                v,
                n.left_parenthesis,
                n.right_parenthesis,
                arguments,
            );
            visitor.visit(v);
        });
    }

    fn visit_assigned_variable_pattern(&mut self, node: Id<AssignedVariablePattern>) {
        self.token(self.ast[node].name);
    }

    fn visit_assignment_expression(&mut self, node: Id<AssignmentExpression>) {
        let n = &self.ast[node];
        self.builder.nest();

        self.visit(n.left_hand_side);
        self.visit_assignment(n.operator, n.right_hand_side.raw(), false);

        self.builder.unnest();
    }

    fn visit_await_expression(&mut self, node: Id<AwaitExpression>) {
        let n = &self.ast[node];
        self.token(n.await_keyword);
        self.space();
        self.visit(n.expression);
    }

    fn visit_binary_expression(&mut self, node: Id<BinaryExpression>) {
        let ast = self.ast;

        // If a binary operator sequence appears immediately after a `=>`, don't
        // add an extra level of nesting. Instead, let the subsequent operands line
        // up with the first, as in:
        //
        //     method() =>
        //         argument &&
        //         argument &&
        //         argument;
        let nest = ast
            .parent(node)
            .is_none_or(|parent| ast.kind(parent) != NodeKind::ExpressionFunctionBody);

        let precedence = ast.tokens.ty(ast[node].operator).precedence();
        self.visit_binary(
            node.raw(),
            |ast, e| {
                let binary = ast.cast::<BinaryExpression>(e)?;
                let b = &ast[binary];
                Some((b.left_operand.raw(), b.operator, b.right_operand.raw()))
            },
            Some(precedence),
            nest,
        );
    }

    fn visit_block(&mut self, node: Id<Block>) {
        let ast = self.ast;
        let n = &ast[node];

        // Treat empty blocks specially. In most cases, they are not allowed to
        // split. However, an empty block as the then statement of an if with an
        // else is always split.
        if !can_split(ast, n.statements, n.right_bracket) {
            self.token(n.left_bracket);
            if self.split_empty_block(node) {
                self.newline();
            }
            self.token(n.right_bracket);
            return;
        }

        self.visit_body(n.left_bracket, ast.list_raw(n.statements), n.right_bracket);
    }

    fn visit_block_class_body(&mut self, node: Id<BlockClassBody>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_body(n.left_bracket, ast.list_raw(n.members), n.right_bracket);
    }

    fn visit_block_enum_body(&mut self, node: Id<BlockEnumBody>) {
        let ast = self.ast;
        let n = &ast[node];
        self.begin_body(n.left_bracket, true);

        let constants = ast.list_raw(n.constants);
        self.visit_comma_separated_nodes(constants, Some(Self::split_or_two_newlines));

        // If there is a trailing comma, always force the constants to split.
        let last_constant = *constants.last().unwrap();
        let trailing_comma = comma_after(ast, last_constant);
        if trailing_comma.is_some() {
            self.builder.force_rules();
        }

        // The ";" after the constants, which may occur after a trailing comma.
        let after_constants = ast.tokens.next(ast.end_token(last_constant));
        let mut semicolon = None;
        if ast.tokens.ty(after_constants) == TokenType::SEMICOLON {
            semicolon = Some(after_constants);
        } else if let Some(trailing_comma) = trailing_comma {
            if ast.tokens.ty(ast.tokens.next(trailing_comma)) == TokenType::SEMICOLON {
                semicolon = Some(ast.tokens.next(after_constants));
            }
        }

        if let Some(semicolon) = semicolon {
            // If there is both a trailing comma and a semicolon, move the semicolon
            // to the next line. This doesn't look great but it's less bad than being
            // next to the comma.
            // TODO(rnystrom): If the formatter starts making non-whitespace changes
            // like adding/removing trailing commas, then it should fix this too.
            if trailing_comma.is_some() {
                self.newline();
            }

            self.token(semicolon);

            // Put a blank line between the constants and members.
            if !n.members.is_empty() {
                self.two_newlines();
            }
        }

        self.visit_body_contents(ast.list_raw(n.members));

        self.end_body(
            n.right_bracket,
            semicolon.is_some()
                || trailing_comma.is_some()
                || !n.members.is_empty()
                || contains_line_comments(ast, constants, None),
        );
    }

    fn visit_block_function_body(&mut self, node: Id<BlockFunctionBody>) {
        let n = &self.ast[node];

        // Space after the parameter list.
        self.space();

        // The "async" or "sync" keyword.
        self.token_opt(n.keyword);

        // The "*" in "async*" or "sync*".
        self.token_opt(n.star);
        if n.keyword.is_some() {
            self.space();
        }

        self.visit(n.block);
    }

    fn visit_boolean_literal(&mut self, node: Id<BooleanLiteral>) {
        self.token(self.ast[node].literal);
    }

    fn visit_break_statement(&mut self, node: Id<BreakStatement>) {
        let n = &self.ast[node];
        self.simple_statement(Some(n.semicolon), |v| {
            v.token(n.break_keyword);
            v.visit_with(n.label, Some(Self::space), None);
        });
    }

    fn visit_cascade_expression(&mut self, node: Id<CascadeExpression>) {
        let ast = self.ast;
        let n = &ast[node];
        let sections = ast.list_raw(n.cascade_sections);

        // Optimized path if we know the cascade will split.
        if sections.len() > 1 {
            self.visit_split_cascade(node);
            return;
        }

        // Whether a split in the cascade target expression forces the cascade to
        // move to the next line. It looks weird to move the cascade down if the
        // target expression is a collection, so we don't:
        //
        //     var list = [
        //       stuff
        //     ]
        //       ..add(more);
        let target = n.target.raw();
        let mut split_if_target_splits = true;
        if sections.len() > 1 {
            // Always split if there are multiple cascade sections.
        } else if is_collection_literal(ast, target) {
            split_if_target_splits = false;
        } else if let Some(arguments) = invocation_argument_list(ast, target) {
            // If the target is a call with a trailing comma in the argument list,
            // treat it like a collection literal.
            split_if_target_splits = !list_has_comma_after(ast, ast[arguments].arguments);
        } else if let Some(creation) = ast.cast::<InstanceCreationExpression>(target) {
            // If the target is a call with a trailing comma in the argument list,
            // treat it like a collection literal.
            let arguments = ast[creation].argument_list;
            split_if_target_splits = !list_has_comma_after(ast, ast[arguments].arguments);
        }

        let allow_inline = cascade_allow_inline(ast, node);
        if split_if_target_splits {
            let rule = self.new_rule_or_hard(allow_inline);
            self.builder.start_lazy_rule(Some(rule));
        }

        self.visit(target);

        self.builder.nest_expression(Some(Indent::CASCADE), true);
        self.builder.start_block_argument_nesting();

        // If the cascade section shouldn't cause the cascade to split, end the
        // rule early so it isn't affected by it.
        if !split_if_target_splits {
            let rule = self.new_rule_or_hard(allow_inline);
            self.builder.start_rule(Some(rule));
        }

        self.zero_split();

        if !split_if_target_splits {
            self.builder.end_rule();
        }

        self.visit_nodes(sections, None, Some(Self::zero_split_callback), None);

        if split_if_target_splits {
            self.builder.end_rule();
        }

        self.builder.end_block_argument_nesting();
        self.builder.unnest();
    }

    /// Dart `allowInline ? Rule() : Rule.hard()`.
    fn new_rule_or_hard(&mut self, allow_inline: bool) -> RuleId {
        if allow_inline {
            self.builder.arena.new_rule()
        } else {
            self.builder.arena.new_hard_rule()
        }
    }

    /// Format the cascade using a nested block instead of a single inline
    /// expression.
    ///
    /// If the cascade has multiple sections, we know each section will be on its
    /// own line and we know there will be at least one trailing section following
    /// a preceding one. That let's us treat all of the earlier sections as a
    /// separate block like we do with collections and functions, instead of a
    /// monolithic expression. Using a block in turn makes big cascades much
    /// faster to format (like 10x) since the block formatting is memoized and
    /// each cascade section in it is formatted independently.
    ///
    /// The tricky part is that block formatting assumes the entire line will be
    /// part of the block. This is not true of the last section in a cascade,
    /// which may have other trailing code, like the `;` here:
    ///
    ///     var x = someLeadingExpression
    ///       ..firstCascade()
    ///       ..secondCascade()
    ///       ..thirdCascade()
    ///       ..fourthCascade();
    ///
    /// To handle that, we don't put the last section in the block and instead
    /// format it with the surrounding expression. So, from the formatter's
    /// view, the above casade is formatted like:
    ///
    ///     var x = someLeadingExpression
    ///       [ begin block ]
    ///       ..firstCascade()
    ///       ..secondCascade()
    ///       ..thirdCascade()
    ///       [ end block ]
    ///       ..fourthCascade();
    ///
    /// This somewhere between clever and hacky, but it works and allows cascades
    /// of essentially unbounded length to be formatted quickly.
    fn visit_split_cascade(&mut self, node: Id<CascadeExpression>) {
        let ast = self.ast;
        let n = &ast[node];
        let sections = ast.list_raw(n.cascade_sections);

        // Rule to split the block.
        let rule = self.builder.arena.new_hard_rule();
        self.builder.start_lazy_rule(Some(rule));
        self.visit(n.target);

        self.builder.nest_expression(Some(Indent::CASCADE), true);
        self.builder.start_block_argument_nesting();

        // If there are comments before the first section, keep them outside of the
        // block. That way code like:
        //
        //     receiver // comment
        //       ..cascade();
        //
        // Keeps the comment on the first line.
        let first_comment_token = ast.begin_token(sections[0]);
        self.write_preceding_comments_and_newlines(first_comment_token);
        self.suppress_preceding_comments_and_new_lines
            .insert(first_comment_token);

        // Process the inner cascade sections as a separate block. This way the
        // entire cascade expression isn't line split as a single monolithic unit,
        // which is very slow.
        self.builder.start_block(None, false, false);

        for &section in &sections[..sections.len() - 1] {
            self.newline();
            self.visit(section);
        }

        // Put comments before the last section inside the block.
        let last_section = *sections.last().unwrap();
        let last_comment_token = ast.begin_token(last_section);
        self.write_preceding_comments_and_newlines(last_comment_token);
        self.suppress_preceding_comments_and_new_lines
            .insert(last_comment_token);

        self.builder.end_block(true);

        // The last section is outside of the block.
        self.visit(last_section);

        self.builder.end_rule();
        self.builder.end_block_argument_nesting();
        self.builder.unnest();
    }

    fn visit_cast_pattern(&mut self, node: Id<CastPattern>) {
        let n = &self.ast[node];
        self.builder.start_span_normal();
        self.builder.nest();
        self.visit(n.pattern);
        self.solo_split(Cost::NORMAL);
        self.token(n.as_token);
        self.space();
        self.visit(n.type_);
        self.builder.unnest();
        self.builder.end_span();
    }

    fn visit_catch_clause(&mut self, node: Id<CatchClause>) {
        let n = &self.ast[node];
        self.token_with(n.on_keyword, None, Some(Self::space));
        self.visit_opt(n.exception_type);

        if n.catch_keyword.is_some() {
            if n.exception_type.is_some() {
                self.space();
            }
            self.token_opt(n.catch_keyword);
            self.space();
            self.token_opt(n.left_parenthesis);
            self.visit_opt(n.exception_parameter);
            self.token_with(n.comma, None, Some(Self::space));
            self.visit_opt(n.stack_trace_parameter);
            self.token_opt(n.right_parenthesis);
            self.space();
        } else {
            self.space();
        }
        self.visit(n.body);
    }

    fn visit_catch_clause_parameter(&mut self, node: Id<CatchClauseParameter>) {
        self.token(self.ast[node].name);
    }

    fn visit_class_declaration(&mut self, node: Id<ClassDeclaration>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_metadata(n.metadata);

        self.builder.nest();
        self.modifier(n.abstract_keyword);
        self.modifier(n.base_keyword);
        self.modifier(n.interface_keyword);
        self.modifier(n.final_keyword);
        self.modifier(n.sealed_keyword);
        self.modifier(n.mixin_keyword);
        self.token(n.class_keyword);
        self.space();
        self.visit(n.name_part);
        self.visit_opt(n.extends_clause);
        self.visit_clauses(n.with_clause, n.implements_clause);
        self.visit_with(n.native_clause, Some(Self::space), None);
        if ast.kind(n.body) != NodeKind::EmptyClassBody {
            self.space();
        }
        self.builder.unnest();

        self.visit(n.body);
    }

    fn visit_class_type_alias(&mut self, node: Id<ClassTypeAlias>) {
        let n = &self.ast[node];
        self.visit_metadata(n.metadata);

        self.simple_statement(Some(n.semicolon), |v| {
            v.modifier(n.abstract_keyword);
            v.modifier(n.base_keyword);
            v.modifier(n.interface_keyword);
            v.modifier(n.final_keyword);
            v.modifier(n.sealed_keyword);
            v.modifier(n.mixin_keyword);
            v.token(n.typedef_keyword);
            v.space();
            v.token(n.name);
            v.visit_opt(n.type_parameters);
            v.space();
            v.token(n.equals);
            v.space();

            v.visit(n.superclass);
            v.visit_clauses(Some(n.with_clause), n.implements_clause);
        });
    }

    fn visit_compilation_unit(&mut self, node: Id<CompilationUnit>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_opt(n.script_tag);

        // Put a blank line between the library tag and the other directives.
        let mut directives = ast.list_raw(n.directives);
        if !directives.is_empty() && ast.kind(directives[0]) == NodeKind::LibraryDirective {
            self.visit(directives[0]);
            self.two_newlines();

            directives = &directives[1..];
        }

        self.visit_nodes(directives, None, Some(Self::one_or_two_newlines), None);

        let mut needs_double = true;
        for &declaration in ast.list_raw(n.declarations) {
            let has_body = matches!(
                ast.kind(declaration),
                NodeKind::ClassDeclaration
                    | NodeKind::EnumDeclaration
                    | NodeKind::ExtensionDeclaration
            );

            // Add a blank line before types with bodies.
            if has_body {
                needs_double = true;
            }

            if needs_double {
                self.two_newlines();
            } else {
                // Variables and arrow-bodied members can be more tightly packed if
                // the user wants to group things together.
                self.one_or_two_newlines();
            }

            self.visit(declaration);

            needs_double = false;
            if has_body {
                // Add a blank line after types declarations with bodies.
                needs_double = true;
            } else if let Some(function) = ast.cast::<FunctionDeclaration>(declaration) {
                // Add a blank line after non-empty block functions.
                let body = ast[ast[function].function_expression].body;
                if let Some(body) = ast.cast::<BlockFunctionBody>(body) {
                    needs_double = !ast[ast[body].block].statements.is_empty();
                }
            }
        }
    }

    fn visit_conditional_expression(&mut self, node: Id<ConditionalExpression>) {
        let ast = self.ast;
        let n = &ast[node];

        // TODO(rnystrom): Consider revisiting whether users prefer this after 2.13.
        self.builder.nest();

        // Start lazily so we don't force the operator to split if a line comment
        // appears before the first operand. If we split after one clause in a
        // conditional, always split after both.
        self.builder.start_lazy_rule(None);
        self.visit(n.condition);

        // Push any block arguments all the way past the leading "?" and ":".
        self.builder.nest_expression(Some(Indent::BLOCK), true);
        self.builder.start_block_argument_nesting();
        self.builder.unnest();

        self.builder.start_span_normal();

        self.split();
        self.token(n.question);
        self.space();
        self.builder.nest();
        self.visit(n.then_expression);
        self.builder.unnest();

        self.split();
        self.token(n.colon);
        self.space();
        self.visit(n.else_expression);

        // If conditional expressions are directly nested, force them all to split.
        // This line here forces the child, which implicitly forces the surrounding
        // parent rules to split too.
        if ast
            .parent(node)
            .is_some_and(|parent| ast.kind(parent) == NodeKind::ConditionalExpression)
        {
            self.builder.force_rules();
        }

        self.builder.end_rule();
        self.builder.end_span();
        self.builder.end_block_argument_nesting();

        // TODO(rnystrom): Consider revisiting whether users prefer this after 2.13.
        self.builder.unnest();
    }

    fn visit_configuration(&mut self, node: Id<Configuration>) {
        let n = &self.ast[node];
        self.token(n.if_keyword);
        self.space();
        self.token(n.left_parenthesis);
        self.visit(n.name);

        if n.equal_token.is_some() {
            self.builder.nest();
            self.space();
            self.token_opt(n.equal_token);
            self.solo_split(Cost::NORMAL);
            self.visit_opt(n.value);
            self.builder.unnest();
        }

        self.token(n.right_parenthesis);
        self.space();
        self.visit(n.uri);
    }

    fn visit_constant_pattern(&mut self, node: Id<ConstantPattern>) {
        let n = &self.ast[node];
        self.token_with(n.const_keyword, None, Some(Self::space));
        self.visit(n.expression);
    }

    fn visit_constructor_declaration(&mut self, node: Id<ConstructorDeclaration>) {
        let n = &self.ast[node];
        self.visit_metadata(n.metadata);

        self.modifier(n.external_keyword);
        self.modifier(n.const_keyword);
        self.modifier(n.new_keyword);
        self.modifier(n.factory_keyword);
        self.visit_opt(n.type_name);
        self.token_opt(n.period);
        self.token_opt(n.name);

        // Make the rule for the ":" span both the preceding parameter list and
        // the entire initialization list. This ensures that we split before the
        // ":" if the parameters and initialization list don't all fit on one line.
        if !n.initializers.is_empty() {
            self.builder.start_rule(None);
        }

        // If the redirecting constructor happens to wrap, we want to make sure
        // the parameter list gets more deeply indented.
        if n.redirected_constructor.is_some() {
            self.builder.nest();
        }

        self.visit_function_body(
            None,
            Some(n.parameters),
            n.body,
            Some(&mut |v: &mut Self| {
                // Check for redirects or initializer lists.
                if n.redirected_constructor.is_some() {
                    v.visit_constructor_redirects(node);
                    v.builder.unnest();
                } else if !n.initializers.is_empty() {
                    v.visit_constructor_initializers(node);

                    // End the rule for ":" after all of the initializers.
                    v.builder.end_rule();
                }
            }),
        );
    }

    fn visit_constructor_redirects(&mut self, node: Id<ConstructorDeclaration>) {
        let ast = self.ast;
        let n = &ast[node];
        self.token_with(n.separator /* = */, Some(Self::space), None);
        self.solo_split(Cost::NORMAL);
        self.visit_comma_separated_nodes(ast.list_raw(n.initializers), None);
        self.visit_opt(n.redirected_constructor);
    }

    fn visit_constructor_initializers(&mut self, node: Id<ConstructorDeclaration>) {
        let ast = self.ast;
        let n = &ast[node];
        let parameters = ast.list_raw(ast[n.parameters].parameters);
        let has_trailing_comma = nodes_have_comma_after(ast, parameters);
        let initializers = ast.list_raw(n.initializers);

        if has_trailing_comma {
            // Since the ")", "])", or "})" on the preceding line doesn't take up
            // much space, it looks weird to move the ":" onto it's own line. Instead,
            // keep it and the first initializer on the current line but add enough
            // space before it to line it up with any subsequent initializers.
            //
            //     Foo(
            //       parameter,
            //     )   : field = value,
            //           super();
            self.space();
            if initializers.len() > 1 {
                let mut padding = "  ";
                if is_named_or_optional_positional(ast, *parameters.last().unwrap()) {
                    padding = " ";
                }
                self.write_text(padding, n.separator.unwrap(), None, true);
            }

            // ":".
            self.token_opt(n.separator);
            self.space();

            self.builder.indent(Some(6));
        } else {
            // Shift the itself ":" forward.
            self.builder.indent(Some(Indent::CONSTRUCTOR_INITIALIZER));

            // If the parameters or initializers split, put the ":" on its own line.
            self.split();

            // ":".
            self.token_opt(n.separator);
            self.space();

            // Try to line up the initializers with the first one that follows the ":"
            //
            //     Foo(notTrailing)
            //         : initializer = value,
            //           super(); // +2 from previous line.
            //
            //     Foo(
            //       trailing,
            //     ) : initializer = value,
            //         super(); // +4 from previous line.
            //
            // This doesn't work if there is a trailing comma in an optional
            // parameter, but we don't want to do a weird +5 alignment:
            //
            //     Foo({
            //       trailing,
            //     }) : initializer = value,
            //         super(); // Doesn't quite line up. :(
            self.builder.indent(Some(2));
        }

        for (i, &initializer) in initializers.iter().enumerate() {
            if i > 0 {
                // Preceding comma.
                self.token(ast.tokens.previous(ast.begin_token(initializer)));
                self.newline();
            }

            self.visit(initializer);
        }

        self.builder.unindent();
        if !has_trailing_comma {
            self.builder.unindent();
        }
    }

    fn visit_constructor_field_initializer(&mut self, node: Id<ConstructorFieldInitializer>) {
        let n = &self.ast[node];
        self.builder.nest();

        self.token_opt(n.this_keyword);
        self.token_opt(n.period);
        self.visit(n.field_name);

        self.visit_assignment(n.equals, n.expression.raw(), false);

        self.builder.unnest();
    }

    fn visit_constructor_name(&mut self, node: Id<ConstructorName>) {
        let n = &self.ast[node];
        self.visit(n.type_);
        self.token_opt(n.period);
        self.visit_opt(n.name);
    }

    fn visit_continue_statement(&mut self, node: Id<ContinueStatement>) {
        let n = &self.ast[node];
        self.simple_statement(Some(n.semicolon), |v| {
            v.token(n.continue_keyword);
            v.visit_with(n.label, Some(Self::space), None);
        });
    }

    fn visit_declared_identifier(&mut self, node: Id<DeclaredIdentifier>) {
        let n = &self.ast[node];
        self.modifier(n.keyword);
        self.visit_with(n.type_, None, Some(Self::space));
        self.token(n.name);
    }

    fn visit_declared_variable_pattern(&mut self, node: Id<DeclaredVariablePattern>) {
        let n = &self.ast[node];
        self.visit_variable_pattern(n.keyword, n.type_, n.name);
    }

    fn visit_do_statement(&mut self, node: Id<DoStatement>) {
        let n = &self.ast[node];
        self.builder.nest();
        self.token(n.do_keyword);
        self.space();
        self.builder.unnest_now(false);
        self.visit(n.body);

        self.builder.nest();
        self.space();
        self.token(n.while_keyword);
        self.space();
        self.token(n.left_parenthesis);
        self.solo_zero_split();
        self.visit(n.condition);
        self.token(n.right_parenthesis);
        self.token(n.semicolon);
        self.builder.unnest();
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

    fn visit_empty_class_body(&mut self, node: Id<EmptyClassBody>) {
        self.token(self.ast[node].semicolon);
    }

    fn visit_empty_enum_body(&mut self, node: Id<EmptyEnumBody>) {
        self.token(self.ast[node].semicolon);
    }

    fn visit_empty_function_body(&mut self, node: Id<EmptyFunctionBody>) {
        self.token(self.ast[node].semicolon);
    }

    fn visit_empty_statement(&mut self, node: Id<EmptyStatement>) {
        self.token(self.ast[node].semicolon);
    }

    fn visit_enum_constant_declaration(&mut self, node: Id<EnumConstantDeclaration>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_metadata(n.metadata);
        self.token(n.name);

        if let Some(arguments) = n.arguments {
            let arguments = &ast[arguments];
            self.builder.nest();
            self.visit_opt(arguments.type_arguments);

            if let Some(constructor) = arguments.constructor_selector {
                let constructor = &ast[constructor];
                self.token(constructor.period);
                self.visit(constructor.name);
            }

            self.visit_argument_list(arguments.argument_list, false);
            self.builder.unnest();
        }
    }

    fn visit_enum_declaration(&mut self, node: Id<EnumDeclaration>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_metadata(n.metadata);

        self.builder.nest();
        self.token(n.enum_keyword);
        self.space();
        self.visit(n.name_part);
        self.visit_clauses(n.with_clause, n.implements_clause);

        if ast.kind(n.body) != NodeKind::EmptyEnumBody {
            self.space();
        }
        self.builder.unnest();

        self.visit(n.body);
    }

    fn visit_export_directive(&mut self, node: Id<ExportDirective>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_directive_metadata(node.raw(), n.metadata);
        self.simple_statement(Some(n.semicolon), |v| {
            v.token(n.export_keyword);
            v.space();
            v.visit(n.uri);

            v.visit_configurations(ast.list_raw(n.configurations));
            v.visit_combinators(ast.list_raw(n.combinators));
        });
    }

    fn visit_expression_function_body(&mut self, node: Id<ExpressionFunctionBody>) {
        let ast = self.ast;
        let n = &ast[node];

        // Space after the parameter list.
        self.space();

        // The "async" or "sync" keyword and "*".
        self.token_opt(n.keyword);
        self.token_opt(n.star);
        if n.keyword.is_some() || n.star.is_some() {
            self.space();
        }

        // Try to keep the "(...) => " with the start of the body for anonymous
        // functions.
        let is_function_expression_body = is_function_expression_body(ast, node.raw());
        if is_function_expression_body {
            self.builder.start_span_normal();
        }

        self.token(n.function_definition); // "=>".

        // Split after the "=>", using the rule created before the parameters
        // by _visitBody().
        self.split();

        // If the body is a binary operator expression, then we want to force the
        // split at `=>` if the operators split. See visitBinaryExpression().
        let is_binary = ast.kind(n.expression) == NodeKind::BinaryExpression;
        if !is_binary {
            self.builder.end_rule();
        }

        if is_function_expression_body {
            self.builder.end_span();
        }

        // If this function invocation appears in an argument list with trailing
        // comma, don't add extra nesting to preserve normal indentation.
        let mut is_arg_with_trailing_comma = false;
        if let Some(parent) = ast.parent(node) {
            if ast.kind(parent) == NodeKind::FunctionExpression {
                is_arg_with_trailing_comma = is_trailing_comma_argument(ast, parent);
            }
        }

        if !is_arg_with_trailing_comma {
            self.builder.start_block_argument_nesting();
        }
        self.builder.start_span_normal();
        self.visit(n.expression);
        self.builder.end_span();
        if !is_arg_with_trailing_comma {
            self.builder.end_block_argument_nesting();
        }

        if is_binary {
            self.builder.end_rule();
        }

        self.token_opt(n.semicolon);
    }

    fn visit_expression_statement(&mut self, node: Id<ExpressionStatement>) {
        let n = &self.ast[node];
        self.simple_statement(n.semicolon, |v| {
            v.visit(n.expression);
        });
    }

    fn visit_extends_clause(&mut self, node: Id<ExtendsClause>) {
        let n = &self.ast[node];
        self.solo_split(Cost::NORMAL);
        self.token(n.extends_keyword);
        self.space();
        self.visit(n.superclass);
    }

    fn visit_extension_declaration(&mut self, node: Id<ExtensionDeclaration>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_metadata(n.metadata);

        self.builder.nest();
        self.token(n.extension_keyword);

        // Don't put a space after `extension` if the extension is unnamed. That
        // way, generic unnamed extensions format like `extension<T> on ...`.
        self.token_with(n.name, Some(Self::space), None);

        self.visit_opt(n.type_parameters);
        if let Some(on_clause) = n.on_clause {
            let on_clause = &ast[on_clause];
            self.solo_split(Cost::NORMAL);
            self.token(on_clause.on_keyword);
            self.space();
            self.visit(on_clause.extended_type);
        }
        if ast.kind(n.body) != NodeKind::EmptyClassBody {
            self.space();
        }
        self.builder.unnest();

        self.visit(n.body);
    }

    fn visit_extension_type_declaration(&mut self, node: Id<ExtensionTypeDeclaration>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_metadata(n.metadata);

        self.builder.nest();
        self.token(n.extension_keyword);
        self.space();
        self.token(n.type_keyword);

        self.visit_with(Some(n.name_part), Some(Self::space), None);

        let rule = self.builder.arena.add_rule(CombinatorRule::new_rule());
        self.builder.start_rule(Some(rule));
        self.visit_opt(n.implements_clause);
        self.builder.end_rule();

        if ast.kind(n.body) != NodeKind::EmptyClassBody {
            self.space();
        }
        self.builder.unnest();

        self.visit(n.body);
    }

    fn visit_field_declaration(&mut self, node: Id<FieldDeclaration>) {
        let n = &self.ast[node];
        self.visit_metadata(n.metadata);

        self.simple_statement(Some(n.semicolon), |v| {
            v.modifier(n.external_keyword);
            v.modifier(n.static_keyword);
            v.modifier(n.abstract_keyword);
            v.modifier(n.covariant_keyword);
            v.visit(n.fields);
        });
    }

    fn visit_field_formal_parameter(&mut self, node: Id<FieldFormalParameter>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_parameter_metadata(n.metadata, |v| {
            v.begin_formal_parameter(n.required_keyword, n.covariant_keyword);
            v.token_with(n.const_final_or_var_keyword, None, Some(Self::space));
            v.visit_opt(n.type_);
            v.separator_between_type_and_variable(n.type_, false);
            v.token(n.this_keyword);
            v.token(n.period);
            v.token(n.name);
            if let Some(suffix) = n.function_typed_suffix {
                let suffix = &ast[suffix];
                v.visit_opt(suffix.type_parameters);
                v.visit(suffix.formal_parameters);
                v.token_opt(suffix.question);
            }
            v.end_formal_parameter();
            v.visit_opt(n.default_clause);
        });
    }

    fn visit_formal_parameter_default_clause(&mut self, node: Id<FormalParameterDefaultClause>) {
        let ast = self.ast;
        let n = &ast[node];
        self.builder.start_span_normal();
        self.builder.nest();

        // The '=' separator is preceded by a space, ":" is not.
        if ast.tokens.ty(n.separator) == TokenType::EQ {
            self.space();
        }
        self.token(n.separator);

        self.solo_split(assignment_cost(ast, n.value.raw()));
        self.visit(n.value);

        self.builder.unnest();
        self.builder.end_span();
    }

    pub fn visit_formal_parameter_list(&mut self, node: Id<FormalParameterList>, nest_expression: bool) {
        let ast = self.ast;
        let n = &ast[node];
        let parameters = ast.list_raw(n.parameters);

        // Corner case: empty parameter lists.
        if parameters.is_empty() {
            self.token(n.left_parenthesis);

            // If there is a comment, do allow splitting before it.
            if has_preceding_comments(ast, n.right_parenthesis) {
                self.solo_zero_split();
            }

            self.token(n.right_parenthesis);
            return;
        }

        // If the parameter list has a trailing comma, format it like a collection
        // literal where each parameter goes on its own line, they are indented +2,
        // and the ")" ends up on its own line.
        if nodes_have_comma_after(ast, parameters) {
            self.visit_trailing_comma_parameter_list(node);
            return;
        }

        let required_params: Vec<NodeId> = parameters
            .iter()
            .copied()
            .filter(|&param| !is_named_or_optional_positional(ast, param))
            .collect();
        let optional_params: Vec<NodeId> = parameters
            .iter()
            .copied()
            .filter(|&param| is_named_or_optional_positional(ast, param))
            .collect();

        if nest_expression {
            self.builder.nest();
        }
        self.token(n.left_parenthesis);

        let mut rule = None;
        if !required_params.is_empty() {
            let positional = self.builder.arena.add_rule(PositionalRule::new_rule(
                None,
                required_params.len() as i32,
                0,
                0,
            ));
            rule = Some(positional);

            self.builder.start_rule(Some(positional));
            if is_function_expression_body(ast, node.raw()) {
                // Don't allow splitting before the first argument (i.e. right after
                // the bare "(" in a lambda. Instead, just stuff a null chunk in there
                // to avoid confusing the arg rule.
                self.builder.arena.rule_mut(positional).before_argument(None);
            } else {
                // Split before the first argument.
                let chunk = self.zero_split();
                self.builder
                    .arena
                    .rule_mut(positional)
                    .before_argument(Some(chunk));
            }

            // Make sure record and function type parameter lists are indented.
            self.builder.start_block_argument_nesting();
            self.builder.start_span_normal();

            for &param in &required_params {
                self.visit(param);
                self.write_comma_after(param);

                if param != *required_params.last().unwrap() {
                    let chunk = self.split();
                    self.builder
                        .arena
                        .rule_mut(positional)
                        .before_argument(Some(chunk));
                }
            }

            self.builder.end_block_argument_nesting();
            self.builder.end_span();
            self.builder.end_rule();
        }

        if !optional_params.is_empty() {
            let named_rule = self.builder.arena.add_rule(NamedRule::new_rule(None, 0, 0));
            if let Some(rule) = rule {
                self.builder
                    .arena
                    .rule_mut(rule)
                    .add_named_args_constraints(named_rule);
            }

            self.builder.start_rule(Some(named_rule));

            // Make sure multi-line default values, record types, and inner function
            // types are indented.
            self.builder.start_block_argument_nesting();

            let chunk = self.builder.split(true, !required_params.is_empty());
            self.builder
                .arena
                .rule_mut(named_rule)
                .before_argument(Some(chunk));

            // "[" or "{" for optional parameters.
            self.token_opt(n.left_delimiter);

            for &param in &optional_params {
                self.visit(param);
                self.write_comma_after(param);

                if param != *optional_params.last().unwrap() {
                    let chunk = self.split();
                    self.builder
                        .arena
                        .rule_mut(named_rule)
                        .before_argument(Some(chunk));
                }
            }

            self.builder.end_block_argument_nesting();
            self.builder.end_rule();

            // "]" or "}" for optional parameters.
            self.token_opt(n.right_delimiter);
        }

        self.token(n.right_parenthesis);
        if nest_expression {
            self.builder.unnest();
        }
    }

    fn visit_for_element(&mut self, node: Id<ForElement>) {
        let ast = self.ast;
        let n = &ast[node];

        // Treat a spread of a collection literal like a block in a for statement
        // and don't split after the for parts.
        let is_spread_body = is_spread_collection(ast, n.body.raw());

        self.builder.nest();
        self.token_with(n.await_keyword, None, Some(Self::space));
        self.token(n.for_keyword);
        self.space();
        self.token(n.left_parenthesis);

        // Start the body rule so that if the parts split, the body does too.
        self.builder.start_rule(None);

        // The rule for the parts.
        self.builder.start_rule(None);
        self.visit(n.for_loop_parts);
        self.token(n.right_parenthesis);
        self.builder.end_rule();
        self.builder.unnest();

        self.builder.nest_expression(Some(Indent::BLOCK), true);

        if is_spread_body {
            self.space();
        } else {
            self.split();

            // If the body is a non-spread collection or lambda, indent it.
            self.builder.start_block_argument_nesting();
        }

        self.visit(n.body);

        if !is_spread_body {
            self.builder.end_block_argument_nesting();
        }
        self.builder.unnest();

        // If a control flow element is nested inside another, force the outer one
        // to split.
        if is_control_flow_element(ast, n.body.raw()) {
            self.builder.force_rules();
        }

        self.builder.end_rule();
    }

    fn visit_for_statement(&mut self, node: Id<ForStatement>) {
        let n = &self.ast[node];
        self.builder.nest();
        self.token_with(n.await_keyword, None, Some(Self::space));
        self.token(n.for_keyword);
        self.space();
        self.token(n.left_parenthesis);

        self.builder.start_rule(None);

        self.visit(n.for_loop_parts);

        self.token(n.right_parenthesis);
        self.builder.end_rule();
        self.builder.unnest();

        self.visit_loop_body(n.body.raw());
    }

    fn visit_for_each_parts_with_declaration(&mut self, node: Id<ForEachPartsWithDeclaration>) {
        let ast = self.ast;
        let n = &ast[node];

        // TODO(rnystrom): The formatting logic here is slightly different from
        // how parameter metadata is handled and from how variable metadata is
        // handled. I think what it does works better in the context of a for-in
        // loop, but consider trying to unify this with one of the above.
        //
        // Metadata on class and variable declarations is *always* split:
        //
        //     @foo
        //     class Bar {}
        //
        // Metadata on parameters has some complex logic to handle multiple
        // parameters with metadata. It also indents the parameters farther than
        // the metadata when split:
        //
        //     function(
        //         @foo(long arg list...)
        //             parameter1,
        //         @foo
        //             parameter2) {}
        //
        // For for-in variables, we allow it to not split, like parameters, but
        // don't indent the variable when it does split:
        //
        //     for (
        //         @foo
        //         @bar
        //         var blah in stuff) {}
        let loop_variable = n.loop_variable;
        self.visit_nodes(
            ast.list_raw(ast[loop_variable].metadata),
            None,
            Some(Self::split_callback),
            Some(Self::split_callback),
        );
        self.visit(loop_variable);

        self.visit_for_each_parts_from_in(n.in_keyword, n.iterable.raw());
    }

    fn visit_for_each_parts_from_in(&mut self, in_keyword: TokenId, iterable: NodeId) {
        self.solo_split(Cost::NORMAL);
        self.token(in_keyword);
        self.space();
        self.visit(iterable);
    }

    fn visit_for_each_parts_with_identifier(&mut self, node: Id<ForEachPartsWithIdentifier>) {
        let n = &self.ast[node];
        self.visit(n.identifier);
        self.visit_for_each_parts_from_in(n.in_keyword, n.iterable.raw());
    }

    fn visit_for_each_parts_with_pattern(&mut self, node: Id<ForEachPartsWithPattern>) {
        let ast = self.ast;
        let n = &ast[node];
        self.builder.start_block_argument_nesting();
        self.visit_nodes(
            ast.list_raw(n.metadata),
            None,
            Some(Self::split_callback),
            Some(Self::split_callback),
        );
        self.token(n.keyword);
        self.space();
        self.visit(n.pattern);
        self.builder.end_block_argument_nesting();
        self.visit_for_each_parts_from_in(n.in_keyword, n.iterable.raw());
    }

    fn visit_for_parts_with_declarations(&mut self, node: Id<ForPartsWithDeclarations>) {
        let ast = self.ast;
        let n = &ast[node];

        // Nest split variables more so they aren't at the same level
        // as the rest of the loop clauses.
        self.builder.nest();

        // Allow the variables to stay unsplit even if the clauses split.
        self.builder.start_rule(None);

        let declaration = &ast[n.variables];
        self.visit_nodes(
            ast.list_raw(declaration.metadata),
            None,
            Some(Self::split_callback),
            Some(Self::split_callback),
        );
        self.modifier(declaration.keyword);
        self.visit_with(declaration.type_, None, Some(Self::space));

        self.visit_comma_separated_nodes(
            ast.list_raw(declaration.variables),
            Some(Self::split_callback),
        );

        self.builder.end_rule();
        self.builder.unnest();

        self.visit_for_parts_from_left_separator(
            n.left_separator,
            n.condition,
            n.right_separator,
            ast.list_raw(n.updaters),
        );
    }

    fn visit_for_parts_with_expression(&mut self, node: Id<ForPartsWithExpression>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_opt(n.initialization);
        self.visit_for_parts_from_left_separator(
            n.left_separator,
            n.condition,
            n.right_separator,
            ast.list_raw(n.updaters),
        );
    }

    fn visit_for_parts_with_pattern(&mut self, node: Id<ForPartsWithPattern>) {
        let ast = self.ast;
        let n = &ast[node];
        self.builder.start_block_argument_nesting();
        self.builder.nest();

        let declaration = &ast[n.variables];
        self.visit_nodes(
            ast.list_raw(declaration.metadata),
            None,
            Some(Self::split_callback),
            Some(Self::split_callback),
        );
        self.token(declaration.keyword);
        self.space();
        self.visit(declaration.pattern);
        self.visit_assignment(declaration.equals, declaration.expression.raw(), false);

        self.builder.unnest();
        self.builder.end_block_argument_nesting();

        self.visit_for_parts_from_left_separator(
            n.left_separator,
            n.condition,
            n.right_separator,
            ast.list_raw(n.updaters),
        );
    }

    fn visit_for_parts_from_left_separator(
        &mut self,
        left_separator: TokenId,
        condition: Option<Id<Expression>>,
        right_separator: TokenId,
        updaters: &[NodeId],
    ) {
        self.token(left_separator);

        // The condition clause.
        if condition.is_some() {
            self.split();
        }
        self.visit_opt(condition);
        self.token(right_separator);

        // The update clause.
        if !updaters.is_empty() {
            self.split();

            // Allow the updates to stay unsplit even if the clauses split.
            self.builder.start_rule(None);

            self.visit_comma_separated_nodes(updaters, Some(Self::split_callback));

            self.builder.end_rule();
        }
    }

    fn visit_function_declaration(&mut self, node: Id<FunctionDeclaration>) {
        let ast = self.ast;
        let n = &ast[node];
        let function = &ast[n.function_expression];
        self.visit_function_or_method_declaration(
            n.metadata,
            n.external_keyword,
            n.property_keyword,
            None,
            None,
            n.name,
            n.return_type,
            function.type_parameters,
            function.parameters,
            function.body,
        );
    }

    fn visit_function_declaration_statement(&mut self, node: Id<FunctionDeclarationStatement>) {
        self.visit(self.ast[node].function_declaration);
    }

    fn visit_function_expression(&mut self, node: Id<FunctionExpression>) {
        let n = &self.ast[node];
        self.visit_function_body(n.type_parameters, n.parameters, n.body, None);
    }

    fn visit_function_expression_invocation(&mut self, node: Id<FunctionExpressionInvocation>) {
        let n = &self.ast[node];

        // Try to keep the entire invocation one line.
        self.builder.start_span_normal();
        self.builder.nest();

        self.visit(n.function);
        self.visit_opt(n.type_arguments);
        self.visit_argument_list(n.argument_list, false);

        self.builder.unnest();
        self.builder.end_span();
    }

    fn visit_function_reference(&mut self, node: Id<FunctionReference>) {
        let n = &self.ast[node];
        self.visit(n.function);
        self.visit_opt(n.type_arguments);
    }

    fn visit_function_type_alias(&mut self, node: Id<FunctionTypeAlias>) {
        let n = &self.ast[node];
        self.visit_metadata(n.metadata);
        self.simple_statement(Some(n.semicolon), |v| {
            v.token(n.typedef_keyword);
            v.space();
            v.visit_with(n.return_type, None, Some(Self::space));
            v.token(n.name);
            v.visit_opt(n.type_parameters);
            v.visit(n.parameters);
        });
    }

    fn visit_function_typed_formal_parameter_suffix(
        &mut self,
        node: Id<FunctionTypedFormalParameterSuffix>,
    ) {
        let n = &self.ast[node];
        self.visit_parameter_signature(n.type_parameters, Some(n.formal_parameters));
        self.token_opt(n.question);
    }

    fn visit_generic_function_type(&mut self, node: Id<GenericFunctionType>) {
        let n = &self.ast[node];
        self.builder.start_lazy_rule(None);
        self.builder.nest();

        self.visit_with(n.return_type, None, Some(Self::split_callback));
        self.token(n.function_keyword);

        self.builder.unnest();
        self.builder.end_rule();
        self.visit_parameter_signature(n.type_parameters, Some(n.parameters));

        self.token_opt(n.question);
    }

    fn visit_generic_type_alias(&mut self, node: Id<GenericTypeAlias>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_nodes(
            ast.list_raw(n.metadata),
            None,
            Some(Self::newline),
            Some(Self::newline),
        );
        self.simple_statement(Some(n.semicolon), |v| {
            v.token(n.typedef_keyword);
            v.space();

            // If the typedef's type parameters split, split after the "=" too,
            // mainly to ensure the function's type parameters and parameters get
            // end up on successive lines with the same indentation.
            v.builder.start_rule(None);

            v.token(n.name);
            v.visit_opt(n.type_parameters);
            v.split();
            v.token(n.equals);

            v.builder.end_rule();

            v.space();
            v.visit(n.type_);
        });
    }

    fn visit_hide_combinator(&mut self, node: Id<HideCombinator>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_combinator(n.keyword, ast.list_raw(n.hidden_names));
    }

    fn visit_if_element(&mut self, node: Id<IfElement>) {
        let ast = self.ast;

        // Treat a chain of if-else elements as a single unit so that we don't
        // unnecessarily indent each subsequent section of the chain.
        let mut if_elements: Vec<Id<IfElement>> = Vec::new();
        let mut this_node = Some(node.raw());
        while let Some(element) = this_node.and_then(|n| ast.cast::<IfElement>(n)) {
            if_elements.push(element);
            this_node = ast[element].else_element.map(|e| e.raw());
        }

        // If the body of the then or else branch is a spread of a collection
        // literal, then we want to format those collections more like blocks than
        // like standalone objects. In particular, if both the then and else branch
        // are spread collection literals, we want to ensure that they both split
        // if either splits. So this:
        //
        //     [
        //       if (condition) ...[
        //         thenClause
        //       ] else ...[
        //         elseClause
        //       ]
        //     ]
        //
        // And not something like this:
        //
        //     [
        //       if (condition) ...[
        //         thenClause
        //       ] else ...[elseClause]
        //     ]
        //
        // To do that, if we see that either clause is a spread collection, we
        // create a single rule and force both collections to use it.
        let spread_rule = self.builder.arena.new_rule();
        let mut spread_brackets: Vec<NodeId> = Vec::new();
        for &element in &if_elements {
            let spread_bracket = spread_collection_bracket(ast, ast[element].then_element.raw());
            if let Some(spread_bracket) = spread_bracket {
                spread_brackets.push(element.raw());
                self.before_block(spread_bracket, spread_rule, None);
            }
        }

        let last = *if_elements.last().unwrap();
        let else_spread_bracket = ast[last]
            .else_element
            .and_then(|e| spread_collection_bracket(ast, e.raw()));
        if let Some(else_spread_bracket) = else_spread_bracket {
            spread_brackets.push(ast[last].else_element.unwrap().raw());
            self.before_block(else_spread_bracket, spread_rule, None);
        }

        let visit_child = |v: &mut Self, element: NodeId, child: NodeId| {
            v.builder.nest_expression(Some(2), true);

            // Treat a spread of a collection literal like a block in an if statement
            // and don't split after the "else".
            let is_spread = spread_brackets.contains(&element);
            if is_spread {
                v.space();
            } else {
                v.split();

                // If the then clause is a non-spread collection or lambda, make sure
                // the body is indented.
                v.builder.start_block_argument_nesting();
            }

            v.visit(child);

            if !is_spread {
                v.builder.end_block_argument_nesting();
            }
            v.builder.unnest();
        };

        // Wrap the whole thing in a single rule. If a split happens inside the
        // condition or the then clause, we want the then and else clauses to split.
        self.builder.start_lazy_rule(None);

        let mut has_inner_control_flow = false;
        for &element in &if_elements {
            let e = &ast[element];
            self.visit_if_condition(
                e.if_keyword,
                e.left_parenthesis,
                e.expression.raw(),
                e.case_clause,
                e.right_parenthesis,
            );

            visit_child(self, element.raw(), e.then_element.raw());
            if is_control_flow_element(ast, e.then_element.raw()) {
                has_inner_control_flow = true;
            }

            // Handle this element's "else" keyword and prepare to write the element,
            // but don't write it. It will either be the next element in [ifElements]
            // or the final else element handled after the loop.
            if e.else_element.is_some() {
                if spread_brackets.contains(&element.raw()) {
                    self.space();
                } else {
                    self.split();
                }

                self.token_opt(e.else_keyword);

                // If there is another if element in the chain, put a space between
                // it and this "else".
                if element != last {
                    self.space();
                }
            }
        }

        // Handle the final trailing else if there is one.
        if let Some(last_else) = ast[last].else_element {
            visit_child(self, last_else.raw(), last_else.raw());

            if is_control_flow_element(ast, last_else.raw()) {
                has_inner_control_flow = true;
            }
        }

        // If a control flow element is nested inside another, force the outer one
        // to split.
        if has_inner_control_flow {
            self.builder.force_rules();
        }
        self.builder.end_rule();
    }

    fn visit_if_statement(&mut self, node: Id<IfStatement>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_if_condition(
            n.if_keyword,
            n.left_parenthesis,
            n.expression.raw(),
            n.case_clause,
            n.right_parenthesis,
        );

        let has_else = n.else_statement.is_some();
        let visit_clause = |v: &mut Self, clause: NodeId| {
            if matches!(ast.kind(clause), NodeKind::Block | NodeKind::IfStatement) {
                v.space();
                v.visit(clause);
            } else {
                // Allow splitting in a statement-bodied if even though it's against
                // the style guide. Since we can't fix the code itself to follow the
                // style guide, we should at least format it as well as we can.
                v.builder.indent(None);
                v.builder.start_rule(None);

                // If there is an else clause, always split before both the then and
                // else statements.
                if has_else {
                    v.builder.write_newline(false, false, false);
                } else {
                    v.builder.split(false, true);
                }

                v.visit(clause);

                v.builder.end_rule();
                v.builder.unindent();
            }
        };

        visit_clause(self, n.then_statement.raw());

        if let Some(else_statement) = n.else_statement {
            if ast.kind(n.then_statement) == NodeKind::Block {
                self.space();
            } else {
                // Corner case where an else follows a single-statement then clause.
                // This is against the style guide, but we still need to handle it. If
                // it happens, put the else on the next line.
                self.newline();
            }

            self.token_opt(n.else_keyword);
            visit_clause(self, else_statement.raw());
        }
    }

    fn visit_implements_clause(&mut self, node: Id<ImplementsClause>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_combinator(n.implements_keyword, ast.list_raw(n.interfaces));
    }

    fn visit_import_directive(&mut self, node: Id<ImportDirective>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_directive_metadata(node.raw(), n.metadata);
        self.simple_statement(Some(n.semicolon), |v| {
            v.token(n.import_keyword);
            v.space();
            v.visit(n.uri);

            v.visit_configurations(ast.list_raw(n.configurations));

            if n.as_keyword.is_some() {
                v.solo_split(Cost::NORMAL);
                v.token_with(n.deferred_keyword, None, Some(Self::space));
                v.token_opt(n.as_keyword);
                v.space();
                v.visit_opt(n.prefix);
            }

            v.visit_combinators(ast.list_raw(n.combinators));
        });
    }

    fn visit_index_expression(&mut self, node: Id<IndexExpression>) {
        let n = &self.ast[node];
        self.builder.nest();

        if n.period.is_some() {
            self.token_opt(n.period);
        } else {
            self.visit_opt(n.target);
        }

        self.finish_index_expression(node);

        self.builder.unnest();
    }

    /// Visit the index part of [node], excluding the target.
    ///
    /// Called by [CallChainVisitor] to handle index expressions in the middle of
    /// call chains.
    pub fn finish_index_expression(&mut self, node: Id<IndexExpression>) {
        let ast = self.ast;
        let n = &ast[node];
        if n
            .target
            .is_some_and(|target| ast.kind(target) == NodeKind::IndexExpression)
        {
            // Edge case: On a chain of [] accesses, allow splitting between them.
            // Produces nicer output in cases like:
            //
            //     someJson['property']['property']['property']['property']...
            self.solo_zero_split();
        }

        self.builder.start_span(Cost::INDEX);
        self.token_opt(n.question);
        self.token(n.left_bracket);
        self.solo_zero_split();
        self.visit(n.index);
        self.token(n.right_bracket);
        self.builder.end_span();
    }

    fn visit_instance_creation_expression(&mut self, node: Id<InstanceCreationExpression>) {
        let n = &self.ast[node];
        self.builder.start_span_normal();

        self.token_with(n.keyword, None, Some(Self::space));
        self.builder.start_span(Cost::CONSTRUCTOR_NAME);

        // Start the expression nesting for the argument list here, in case this
        // is a generic constructor with type arguments. If it is, we need the type
        // arguments to be nested too so they get indented past the arguments.
        self.builder.nest();
        self.visit(n.constructor_name);

        self.builder.end_span();
        self.visit_argument_list(n.argument_list, false);
        self.builder.end_span();

        self.builder.unnest();
    }

    fn visit_integer_literal(&mut self, node: Id<IntegerLiteral>) {
        self.token(self.ast[node].literal);
    }

    fn visit_interpolation_expression(&mut self, node: Id<InterpolationExpression>) {
        let n = &self.ast[node];
        self.builder.prevent_split();
        self.token(n.left_bracket);
        self.builder.start_span_normal();
        self.visit(n.expression);
        self.builder.end_span();
        self.token_opt(n.right_bracket);
        self.builder.end_prevent_split();
    }

    fn visit_interpolation_string(&mut self, node: Id<InterpolationString>) {
        self.write_string_literal(self.ast[node].contents);
    }

    fn visit_is_expression(&mut self, node: Id<IsExpression>) {
        let n = &self.ast[node];
        self.builder.start_span_normal();
        self.builder.nest();
        self.visit(n.expression);
        self.solo_split(Cost::NORMAL);
        self.token(n.is_operator);
        self.token_opt(n.not_operator);
        self.space();
        self.visit(n.type_);
        self.builder.unnest();
        self.builder.end_span();
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
        self.visit_labels(ast.list_raw(n.labels));
        self.visit(n.statement);
    }

    fn visit_library_directive(&mut self, node: Id<LibraryDirective>) {
        let n = &self.ast[node];
        self.visit_directive_metadata(node.raw(), n.metadata);
        self.simple_statement(Some(n.semicolon), |v| {
            v.token(n.library_keyword);
            if let Some(name) = n.name {
                v.visit_with(Some(name), Some(Self::space), None);
            }
        });
    }

    fn visit_list_literal(&mut self, node: Id<ListLiteral>) {
        let ast = self.ast;
        let n = &ast[node];

        // Corner case: Splitting inside a list looks bad if there's only one
        // element, so make those more costly. (Dart passes the cost, but
        // `_visitCollectionLiteral` doesn't use it.)
        self.visit_collection_literal(
            n.left_bracket,
            ast.list_raw(n.elements),
            n.right_bracket,
            n.const_keyword,
            n.type_arguments,
            true,
            false,
        );
    }

    fn visit_list_pattern(&mut self, node: Id<ListPattern>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_collection_literal(
            n.left_bracket,
            ast.list_raw(n.elements),
            n.right_bracket,
            None,
            n.type_arguments,
            false,
            false,
        );
    }

    fn visit_logical_and_pattern(&mut self, node: Id<LogicalAndPattern>) {
        self.visit_binary(
            node.raw(),
            |ast, e| {
                let pattern = ast.cast::<LogicalAndPattern>(e)?;
                let p = &ast[pattern];
                Some((p.left_operand.raw(), p.operator, p.right_operand.raw()))
            },
            None,
            true,
        );
    }

    fn visit_logical_or_pattern(&mut self, node: Id<LogicalOrPattern>) {
        self.visit_binary(
            node.raw(),
            |ast, e| {
                let pattern = ast.cast::<LogicalOrPattern>(e)?;
                let p = &ast[pattern];
                Some((p.left_operand.raw(), p.operator, p.right_operand.raw()))
            },
            None,
            true,
        );
    }

    fn visit_map_literal_entry(&mut self, node: Id<MapLiteralEntry>) {
        let n = &self.ast[node];
        self.builder.nest();
        self.visit(n.key);
        self.token(n.separator);
        self.solo_split(Cost::NORMAL);
        self.visit(n.value);
        self.builder.unnest();
    }

    fn visit_map_pattern(&mut self, node: Id<MapPattern>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_collection_literal(
            n.left_bracket,
            ast.list_raw(n.elements),
            n.right_bracket,
            None,
            n.type_arguments,
            false,
            false,
        );
    }

    fn visit_map_pattern_entry(&mut self, node: Id<MapPatternEntry>) {
        let n = &self.ast[node];
        self.builder.nest();
        self.visit(n.key);
        self.token(n.separator);
        self.solo_split(Cost::NORMAL);
        self.visit(n.value);
        self.builder.unnest();
    }

    fn visit_method_declaration(&mut self, node: Id<MethodDeclaration>) {
        let n = &self.ast[node];
        self.visit_function_or_method_declaration(
            n.metadata,
            n.external_keyword,
            n.property_keyword,
            n.modifier_keyword,
            n.operator_keyword,
            n.name,
            n.return_type,
            n.type_parameters,
            n.parameters,
            n.body,
        );
    }

    fn visit_method_invocation(&mut self, node: Id<MethodInvocation>) {
        let ast = self.ast;
        let n = &ast[node];

        // If there's no target, this is a "bare" function call like "foo(1, 2)",
        // or a section in a cascade.
        //
        // If it looks like a constructor or static call, we want to keep the
        // target and method together instead of including the method in the
        // subsequent method chain. When this happens, it's important that this
        // code here has the same rules as in [visitInstanceCreationExpression].
        //
        // That ensures that the way some code is formatted is not affected by the
        // presence or absence of `new`/`const`.
        if n.target.is_none() || looks_like_static_call(ast, node.raw()) {
            // Try to keep the entire method invocation one line.
            self.builder.nest();
            self.builder.start_span_normal();

            if n.target.is_some() {
                self.builder.start_span(Cost::CONSTRUCTOR_NAME);
                self.visit_opt(n.target);
                self.solo_zero_split();
            }

            // If target is null, this will be `..` for a cascade.
            self.token_opt(n.operator);
            self.visit(n.method_name);

            if n.target.is_some() {
                self.builder.end_span();
            }

            // TODO(rnystrom): Currently, there are no constraints between a generic
            // method's type arguments and arguments. That can lead to some funny
            // splitting like:
            //
            //     method<VeryLongType,
            //             AnotherTypeArgument>(argument,
            //         argument, argument, argument);
            //
            // The indentation is fine, but splitting in the middle of each argument
            // list looks kind of strange. If this ends up happening in real world
            // code, consider putting a constraint between them.
            self.builder.nest();
            self.visit_opt(n.type_arguments);
            self.visit_argument_list(n.argument_list, false);
            self.builder.unnest();

            self.builder.end_span();
            self.builder.unnest();
            return;
        }

        CallChainVisitor::new(self, node.raw()).visit(self);
    }

    fn visit_mixin_declaration(&mut self, node: Id<MixinDeclaration>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_metadata(n.metadata);

        self.builder.nest();
        self.modifier(n.base_keyword);
        self.token(n.mixin_keyword);
        self.space();
        self.token(n.name);
        self.visit_opt(n.type_parameters);

        // If there is only a single superclass constraint, format it like an
        // "extends" in a class.
        let on_clause = n.on_clause;
        let constraints = on_clause.map(|on| ast.list_raw(ast[on].superclass_constraints));
        if let (Some(on_clause), Some(constraints)) = (on_clause, constraints) {
            if constraints.len() == 1 {
                self.solo_split(Cost::NORMAL);
                self.token(ast[on_clause].on_keyword);
                self.space();
                self.visit(constraints[0]);
            }
        }

        let rule = self.builder.arena.add_rule(CombinatorRule::new_rule());
        self.builder.start_rule(Some(rule));

        // If there are multiple superclass constraints, format them like the
        // "implements" clause.
        if let (Some(on_clause), Some(constraints)) = (on_clause, constraints) {
            if constraints.len() > 1 {
                self.visit(on_clause);
            }
        }

        self.visit_opt(n.implements_clause);
        self.builder.end_rule();

        if ast.kind(n.body) != NodeKind::EmptyClassBody {
            self.space();
        }
        self.builder.unnest();

        self.visit(n.body);
    }

    fn visit_name_with_type_parameters(&mut self, node: Id<NameWithTypeParameters>) {
        let n = &self.ast[node];
        self.token(n.type_name);
        self.visit_opt(n.type_parameters);
    }

    pub fn visit_named_argument(&mut self, node: Id<NamedArgument>, rule: Option<RuleId>) {
        let n = &self.ast[node];
        self.visit_named_node(n.name, n.colon, n.argument_expression.raw(), rule);
    }

    fn visit_named_type(&mut self, node: Id<NamedType>) {
        let ast = self.ast;
        let n = &ast[node];
        if let Some(import_prefix) = n.import_prefix {
            let import_prefix = &ast[import_prefix];
            self.builder.start_span_normal();
            self.token(import_prefix.name);
            self.solo_zero_split();
            self.token(import_prefix.period);
            self.token(n.name);
            self.builder.end_span();
        } else {
            self.token(n.name);
        }

        self.visit_opt(n.type_arguments);
        self.token_opt(n.question);
    }

    fn visit_native_clause(&mut self, node: Id<NativeClause>) {
        let n = &self.ast[node];
        self.token(n.native_keyword);
        self.visit_with(n.name, Some(Self::space), None);
    }

    fn visit_native_function_body(&mut self, node: Id<NativeFunctionBody>) {
        let n = &self.ast[node];
        self.simple_statement(Some(n.semicolon), |v| {
            v.builder.nest_expression(None, true);
            v.solo_split(Cost::NORMAL);
            v.token(n.native_keyword);
            v.visit_with(n.string_literal, Some(Self::space), None);
            v.builder.unnest();
        });
    }

    fn visit_null_assert_pattern(&mut self, node: Id<NullAssertPattern>) {
        let n = &self.ast[node];
        self.visit(n.pattern);
        self.token(n.operator);
    }

    fn visit_null_check_pattern(&mut self, node: Id<NullCheckPattern>) {
        let n = &self.ast[node];
        self.visit(n.pattern);
        self.token(n.operator);
    }

    fn visit_null_literal(&mut self, node: Id<NullLiteral>) {
        self.token(self.ast[node].literal);
    }

    fn visit_object_pattern(&mut self, node: Id<ObjectPattern>) {
        let ast = self.ast;
        let n = &ast[node];

        // Even though object patterns syntactically resemble constructor or
        // function calls, we format them like collections (or like argument lists
        // with trailing commas). In other words, like this:
        //
        //     case Foo(
        //         first: 1,
        //         second: 2,
        //         third: 3
        //       ):
        //       body;
        //
        // Not like:
        //
        //     case Foo(
        //           first: 1,
        //           second: 2,
        //           third: 3):
        //       body;
        //
        // This is less consistent with the corresponding expression form, but is
        // more consistent with all of the other delimited patterns -- list, map,
        // and record -- which have collection-like formatting.
        // TODO(rnystrom): If we move to consistently using collection-like
        // formatting for all argument lists, then this will all be consistent and
        // this comment should be removed.
        self.visit(n.type_);
        self.visit_collection_literal(
            n.left_parenthesis,
            ast.list_raw(n.fields),
            n.right_parenthesis,
            None,
            None,
            false,
            false,
        );
    }

    fn visit_mixin_on_clause(&mut self, node: Id<MixinOnClause>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_combinator(n.on_keyword, ast.list_raw(n.superclass_constraints));
    }

    fn visit_parenthesized_expression(&mut self, node: Id<ParenthesizedExpression>) {
        let n = &self.ast[node];
        self.builder.nest();
        self.token(n.left_parenthesis);
        self.visit(n.expression);
        self.builder.unnest();
        self.token(n.right_parenthesis);
    }

    fn visit_parenthesized_pattern(&mut self, node: Id<ParenthesizedPattern>) {
        let n = &self.ast[node];
        self.builder.nest();
        self.token(n.left_parenthesis);
        self.visit(n.pattern);
        self.builder.unnest();
        self.token(n.right_parenthesis);
    }

    fn visit_part_directive(&mut self, node: Id<PartDirective>) {
        let n = &self.ast[node];
        self.visit_directive_metadata(node.raw(), n.metadata);
        self.simple_statement(Some(n.semicolon), |v| {
            v.token(n.part_keyword);
            v.space();
            v.visit(n.uri);
        });
    }

    fn visit_part_of_directive(&mut self, node: Id<PartOfDirective>) {
        let n = &self.ast[node];
        self.visit_directive_metadata(node.raw(), n.metadata);
        self.simple_statement(Some(n.semicolon), |v| {
            v.token(n.part_keyword);
            v.space();
            v.token(n.of_keyword);
            v.space();

            // Part-of may have either a name or a URI. Only one of these will be
            // non-null. We visit both since visit() ignores null.
            v.visit_opt(n.library_name);
            v.visit_opt(n.uri);
        });
    }

    fn visit_pattern_assignment(&mut self, node: Id<PatternAssignment>) {
        let n = &self.ast[node];
        self.visit(n.pattern);
        self.visit_assignment(n.equals, n.expression.raw(), false);
    }

    fn visit_pattern_field(&mut self, node: Id<PatternField>) {
        let ast = self.ast;
        let n = &ast[node];
        if let Some(field_name) = n.name {
            let field_name = &ast[field_name];
            if let Some(name) = field_name.name {
                self.visit_named_node(name, field_name.colon, n.pattern.raw(), None);
            } else {
                // Named field with inferred name, like:
                //
                //     var (:x) = (x: 1);
                self.token(field_name.colon);
                self.visit(n.pattern);
            }
        } else {
            self.visit(n.pattern);
        }
    }

    fn visit_pattern_variable_declaration(&mut self, node: Id<PatternVariableDeclaration>) {
        let n = &self.ast[node];
        self.visit_metadata(n.metadata);
        self.builder.nest();
        self.token(n.keyword);
        self.space();
        self.visit(n.pattern);
        self.visit_assignment(n.equals, n.expression.raw(), false);
        self.builder.unnest();
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
        self.visit(n.operand);
        self.token(n.operator);
    }

    fn visit_prefixed_identifier(&mut self, node: Id<PrefixedIdentifier>) {
        CallChainVisitor::new(self, node.raw()).visit(self);
    }

    fn visit_prefix_expression(&mut self, node: Id<PrefixExpression>) {
        let ast = self.ast;
        let n = &ast[node];
        self.token(n.operator);

        // Edge case: put a space after "-" if the operand is "-" or "--" so we
        // don't merge the operators.
        if let Some(operand) = ast.cast::<PrefixExpression>(n.operand) {
            let lexeme = ast.tokens.lexeme(ast[operand].operator);
            if lexeme == "-" || lexeme == "--" {
                self.space();
            }
        }

        self.visit(n.operand);
    }

    fn visit_primary_constructor_declaration(&mut self, node: Id<PrimaryConstructorDeclaration>) {
        let n = &self.ast[node];
        self.modifier(n.const_keyword);
        self.token(n.type_name);
        self.visit_opt(n.type_parameters);
        self.visit_opt(n.constructor_name);
        self.visit(n.formal_parameters);
    }

    fn visit_primary_constructor_body(&mut self, node: Id<PrimaryConstructorBody>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_metadata(n.metadata);
        self.token(n.this_keyword);

        // TODO(scheglov): unify with `visitConstructorDeclaration`.
        if !n.initializers.is_empty() {
            self.builder.start_rule(None);
            self.builder.indent(Some(Indent::CONSTRUCTOR_INITIALIZER));
            self.split();
            self.token_opt(n.colon);
            self.space();
            self.visit_comma_separated_nodes(ast.list_raw(n.initializers), None);
            self.builder.unindent();
            self.builder.end_rule();
        }

        self.visit(n.body);
    }

    fn visit_primary_constructor_name(&mut self, node: Id<PrimaryConstructorName>) {
        let n = &self.ast[node];
        self.token(n.period);
        self.token(n.name);
    }

    fn visit_property_access(&mut self, node: Id<PropertyAccess>) {
        let ast = self.ast;
        let n = &ast[node];
        let operator = ast.tokens.ty(n.operator);
        if operator == TokenType::PERIOD_PERIOD || operator == TokenType::QUESTION_PERIOD_PERIOD {
            self.token(n.operator);
            self.visit(n.property_name);
            return;
        }

        CallChainVisitor::new(self, node.raw()).visit(self);
    }

    fn visit_redirecting_constructor_invocation(
        &mut self,
        node: Id<RedirectingConstructorInvocation>,
    ) {
        let n = &self.ast[node];
        self.builder.start_span_normal();

        self.token(n.this_keyword);
        self.token_opt(n.period);
        self.visit_opt(n.constructor_name);
        self.visit(n.argument_list);

        self.builder.end_span();
    }

    fn visit_record_literal(&mut self, node: Id<RecordLiteral>) {
        let ast = self.ast;
        let n = &ast[node];
        self.modifier(n.const_keyword);
        self.visit_collection_literal(
            n.left_parenthesis,
            ast.list_raw(n.fields),
            n.right_parenthesis,
            None,
            None,
            false,
            true,
        );
    }

    fn visit_record_literal_named_field(&mut self, node: Id<RecordLiteralNamedField>) {
        let n = &self.ast[node];
        self.visit_named_node(n.name, n.colon, n.field_expression.raw(), None);
    }

    fn visit_record_pattern(&mut self, node: Id<RecordPattern>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_collection_literal(
            n.left_parenthesis,
            ast.list_raw(n.fields),
            n.right_parenthesis,
            None,
            None,
            false,
            true,
        );
    }

    fn visit_record_type_annotation(&mut self, node: Id<RecordTypeAnnotation>) {
        let ast = self.ast;
        let n = &ast[node];
        let named_fields = n.named_fields.map(|named| &ast[named]);
        let positional_fields = ast.list_raw(n.positional_fields);

        // Handle empty record types specially.
        if positional_fields.is_empty() && named_fields.is_none() {
            self.token(n.left_parenthesis);

            // If there is a comment inside the parens, do allow splitting before it.
            if has_preceding_comments(ast, n.right_parenthesis) {
                self.solo_zero_split();
            }

            self.token(n.right_parenthesis);
            self.token_opt(n.question);
            return;
        }

        self.token(n.left_parenthesis);
        self.builder.start_rule(None);

        // If all parameters are named, put the "{" right after "(".
        if positional_fields.is_empty() {
            self.token(named_fields.unwrap().left_bracket);
        }

        // Process the parameters as a separate set of chunks.
        self.builder.start_block(None, true, false);

        // Write the positional fields.
        for &field in positional_fields {
            self.builder.split(false, field != positional_fields[0]);
            self.visit(field);
            self.write_comma_after(field);
        }

        // Then the named fields.
        let mut first_closing_delimiter = n.right_parenthesis;
        if let Some(named_fields) = named_fields {
            if !positional_fields.is_empty() {
                self.space();
                self.token(named_fields.left_bracket);
            }

            let fields = ast.list_raw(named_fields.fields);
            for &field in fields {
                self.builder.split(false, field != fields[0]);
                self.visit(field);
                self.write_comma_after(field);
            }

            first_closing_delimiter = named_fields.right_bracket;
        }

        // Put comments before the closing ")" or "}" inside the block.
        if has_preceding_comments(ast, first_closing_delimiter) {
            self.newline();
            self.write_preceding_comments_and_newlines(first_closing_delimiter);
        }

        // If there is a trailing comma, then force the record type to split. But
        // don't force if there is only a single positional element because then
        // the trailing comma is actually mandatory.
        let force = match named_fields {
            None => {
                positional_fields.len() > 1
                    && has_comma_after(ast, *positional_fields.last().unwrap())
            }
            Some(named_fields) => {
                let fields = ast.list_raw(named_fields.fields);
                has_comma_after(ast, *fields.last().unwrap())
            }
        };

        self.builder.end_block(force);
        self.builder.end_rule();

        // Now write the delimiter(s) themselves.
        self.write_text(
            ast.tokens.lexeme(first_closing_delimiter),
            first_closing_delimiter,
            None,
            true,
        );
        if named_fields.is_some() {
            self.token(n.right_parenthesis);
        }

        self.token_opt(n.question);
    }

    fn visit_record_type_annotation_named_field(&mut self, node: Id<RecordTypeAnnotationNamedField>) {
        let n = &self.ast[node];
        self.visit_parameter_metadata(n.metadata, |v| {
            v.visit(n.type_);
            v.token_with(Some(n.name), Some(Self::space), None);
        });
    }

    fn visit_record_type_annotation_positional_field(
        &mut self,
        node: Id<RecordTypeAnnotationPositionalField>,
    ) {
        let n = &self.ast[node];
        self.visit_parameter_metadata(n.metadata, |v| {
            v.visit(n.type_);
            v.token_with(n.name, Some(Self::space), None);
        });
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
        self.token(n.operator);
        self.visit_opt(n.pattern);
    }

    fn visit_return_statement(&mut self, node: Id<ReturnStatement>) {
        let n = &self.ast[node];
        self.simple_statement(Some(n.semicolon), |v| {
            v.token(n.return_keyword);
            v.visit_with(n.expression, Some(Self::space), None);
        });
    }

    fn visit_script_tag(&mut self, node: Id<ScriptTag>) {
        let ast = self.ast;
        let script_tag = ast[node].script_tag;

        // The lexeme includes the trailing newline. Strip it off since the
        // formatter ensures it gets a newline after it. Since the script tag must
        // come at the top of the file, we don't have to worry about preceding
        // comments or whitespace.
        self.write_text(dart_trim(ast.tokens.lexeme(script_tag)), script_tag, None, true);
        self.two_newlines();
    }

    fn visit_set_or_map_literal(&mut self, node: Id<SetOrMapLiteral>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_collection_literal(
            n.left_bracket,
            ast.list_raw(n.elements),
            n.right_bracket,
            n.const_keyword,
            n.type_arguments,
            true,
            false,
        );
    }

    fn visit_show_combinator(&mut self, node: Id<ShowCombinator>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_combinator(n.keyword, ast.list_raw(n.shown_names));
    }

    fn visit_regular_formal_parameter(&mut self, node: Id<RegularFormalParameter>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_parameter_metadata(n.metadata, |v| {
            if let Some(function_typed_suffix) = n.function_typed_suffix {
                let suffix = &ast[function_typed_suffix];
                v.modifier(n.required_keyword);
                v.modifier(n.covariant_keyword);
                v.visit_with(n.type_, None, Some(Self::space));
                // Try to keep the function's parameters with its name.
                v.builder.start_span_normal();
                v.token_opt(n.name);
                v.visit_parameter_signature(suffix.type_parameters, Some(suffix.formal_parameters));
                v.token_opt(suffix.question);
                v.builder.end_span();
                v.visit_opt(n.default_clause);
            } else {
                v.begin_formal_parameter(n.required_keyword, n.covariant_keyword);

                v.modifier(n.const_final_or_var_keyword);

                v.visit_opt(n.type_);
                if n.name.is_some() {
                    v.separator_between_type_and_variable(n.type_, false);
                }
                v.token_opt(n.name);

                v.end_formal_parameter();
                v.visit_opt(n.default_clause);
            }
        });
    }

    fn visit_simple_identifier(&mut self, node: Id<SimpleIdentifier>) {
        self.token(self.ast[node].token);
    }

    fn visit_simple_string_literal(&mut self, node: Id<SimpleStringLiteral>) {
        self.write_string_literal(self.ast[node].literal);
    }

    fn visit_spread_element(&mut self, node: Id<SpreadElement>) {
        let n = &self.ast[node];
        self.token(n.spread_operator);
        self.visit(n.expression);
    }

    fn visit_string_interpolation(&mut self, node: Id<StringInterpolation>) {
        let ast = self.ast;
        for &element in ast.list_raw(ast[node].elements) {
            self.visit(element);
        }
    }

    fn visit_super_constructor_invocation(&mut self, node: Id<SuperConstructorInvocation>) {
        let n = &self.ast[node];
        self.builder.start_span_normal();

        self.token(n.super_keyword);
        self.token_opt(n.period);
        self.visit_opt(n.constructor_name);
        self.visit(n.argument_list);

        self.builder.end_span();
    }

    fn visit_super_expression(&mut self, node: Id<SuperExpression>) {
        self.token(self.ast[node].super_keyword);
    }

    fn visit_super_formal_parameter(&mut self, node: Id<SuperFormalParameter>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_parameter_metadata(n.metadata, |v| {
            v.begin_formal_parameter(n.required_keyword, n.covariant_keyword);
            v.token_with(n.const_final_or_var_keyword, None, Some(Self::space));
            v.visit_with(n.type_, None, Some(Self::split_callback));
            v.token(n.super_keyword);
            v.token(n.period);
            v.token(n.name);
            if let Some(suffix) = n.function_typed_suffix {
                let suffix = &ast[suffix];
                v.visit_opt(suffix.type_parameters);
                v.visit(suffix.formal_parameters);
                v.token_opt(suffix.question);
            }
            v.end_formal_parameter();
            v.visit_opt(n.default_clause);
        });
    }

    fn visit_switch_expression(&mut self, node: Id<SwitchExpression>) {
        let ast = self.ast;
        let n = &ast[node];
        let cases = ast.list_raw(n.cases);

        if !can_split(ast, n.cases, n.right_bracket) {
            // Don't allow splitting an empty switch expression.
            self.visit_switch_value(
                n.switch_keyword,
                n.left_parenthesis,
                n.expression.raw(),
                n.right_parenthesis,
            );
            self.token(n.left_bracket);
            self.token(n.right_bracket);
            return;
        }

        // Start the rule for splitting between the cases before the value. That
        // way, if the value expression splits, the cases do too. Avoids:
        //
        //     switch ([
        //        element,
        //     ]) { inline => caseBody };
        self.builder.start_rule(None);

        self.visit_switch_value(
            n.switch_keyword,
            n.left_parenthesis,
            n.expression.raw(),
            n.right_parenthesis,
        );

        self.token(n.left_bracket);
        self.builder.start_block(None, true, !cases.is_empty());

        self.visit_comma_separated_nodes(cases, Some(Self::split_callback));

        let has_trailing_comma =
            !cases.is_empty() && comma_after(ast, *cases.last().unwrap()).is_some();

        // TODO(rnystrom): If there is a line comment at the end of a case, make
        // sure the switch expression splits. Looking for line comments explicitly
        // instead of having them harden the surrounding rules is a hack. But this
        // code will be going away when we move to the new Piece representation, so
        // going with something expedient.
        let force_split = contains_line_comments(ast, cases, Some(n.right_bracket));

        self.end_body(n.right_bracket, has_trailing_comma || force_split);
    }

    fn visit_switch_expression_case(&mut self, node: Id<SwitchExpressionCase>) {
        let ast = self.ast;
        let n = &ast[node];

        // If the pattern is a series of `||` patterns, then flatten them out and
        // format them like empty cases with fallthrough in a switch statement
        // instead of like a single indented binary pattern. Prefer:
        //
        //   e = switch (obj) {
        //     constant1 ||
        //     constant2 ||
        //     constant3 =>
        //       body
        //   };
        //
        // Instead of:
        //
        //   e = switch (obj) {
        //     constant1 ||
        //        constant2 ||
        //        constant3 =>
        //       body
        //   };
        let mut or_branches: Vec<NodeId> = Vec::new();
        let mut or_tokens: Vec<TokenId> = Vec::new();

        fn flatten_or(ast: &Ast, e: NodeId, branches: &mut Vec<NodeId>, tokens: &mut Vec<TokenId>) {
            match ast.cast::<LogicalOrPattern>(e) {
                None => branches.push(e),
                Some(or) => {
                    let or = &ast[or];
                    flatten_or(ast, or.left_operand.raw(), branches, tokens);
                    tokens.push(or.operator);
                    flatten_or(ast, or.right_operand.raw(), branches, tokens);
                }
            }
        }

        let guarded_pattern = &ast[n.guarded_pattern];
        flatten_or(ast, guarded_pattern.pattern.raw(), &mut or_branches, &mut or_tokens);

        // Wrap the rule for splitting after "=>" around the pattern so that a
        // split in the pattern forces the expression to move to the next line too.
        self.builder.start_lazy_rule(None);

        // Write the "||" operands up to the last one.
        for i in 0..or_branches.len() - 1 {
            // Note that orBranches will always have one more element than orTokens.
            self.visit(or_branches[i]);
            self.space();
            self.token(or_tokens[i]);
            self.split();
        }

        // Wrap the expression's nesting around the final pattern so that a split in
        // the pattern is indented farther then the body expression. Used +2 indent
        // because switch expressions are block-like, similar to how we split the
        // bodies of if and for elements in collections.
        self.builder.nest_expression(Some(Indent::BLOCK), false);

        let when_clause = guarded_pattern.when_clause;
        if when_clause.is_some() {
            // Wrap the when clause rule around the pattern so that if the pattern
            // splits then we split before "when" too.
            self.builder.start_lazy_rule(None);
            self.builder.nest_expression(Some(Indent::BLOCK), false);
        }

        // Write the last pattern in the "||" chain. If the case pattern isn't an
        // "||" pattern at all, this writes the one pattern.
        self.visit(*or_branches.last().unwrap());

        if let Some(when_clause) = when_clause {
            self.split();
            self.builder.start_block_argument_nesting();
            self.visit_when_clause(when_clause);
            self.builder.end_block_argument_nesting();
            self.builder.unnest();
            self.builder.end_rule();
        }

        self.space();
        self.token(n.arrow);
        self.split();
        self.builder.end_rule();

        self.builder.start_block_argument_nesting();
        self.visit(n.expression);
        self.builder.end_block_argument_nesting();

        self.builder.unnest();
    }

    fn visit_switch_statement(&mut self, node: Id<SwitchStatement>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_switch_value(
            n.switch_keyword,
            n.left_parenthesis,
            n.expression.raw(),
            n.right_parenthesis,
        );
        self.begin_body(n.left_bracket, false);
        let members = ast.list_raw(n.members);
        for &member in members {
            let (labels, keyword, colon, statements) = match ast.kind(member) {
                NodeKind::SwitchCase => {
                    let m = &ast[Id::<SwitchCase>::from_raw(member)];
                    (m.labels, m.keyword, m.colon, m.statements)
                }
                NodeKind::SwitchPatternCase => {
                    let m = &ast[Id::<SwitchPatternCase>::from_raw(member)];
                    (m.labels, m.keyword, m.colon, m.statements)
                }
                _ => {
                    let m = &ast[Id::<SwitchDefault>::from_raw(member)];
                    (m.labels, m.keyword, m.colon, m.statements)
                }
            };
            self.visit_labels(ast.list_raw(labels));
            self.token(keyword);

            if let Some(case) = ast.cast::<SwitchCase>(member) {
                self.space();
                self.visit(ast[case].expression);
            } else if let Some(case) = ast.cast::<SwitchPatternCase>(member) {
                self.space();
                let guarded_pattern = &ast[ast[case].guarded_pattern];
                match guarded_pattern.when_clause {
                    None => {
                        self.builder.indent(None);
                        self.visit(guarded_pattern.pattern);
                        self.builder.unindent();
                    }
                    Some(when_clause) => {
                        // Wrap the when clause rule around the pattern so that if the pattern
                        // splits then we split before "when" too.
                        self.builder.start_rule(None);
                        self.builder.nest();
                        self.builder.start_block_argument_nesting();
                        self.visit(guarded_pattern.pattern);
                        self.split();
                        self.visit_when_clause(when_clause);
                        self.builder.end_block_argument_nesting();
                        self.builder.unnest();
                        self.builder.end_rule();
                    }
                }
            } else {
                // SwitchDefault: nothing to do.
            }

            self.token(colon);

            if !statements.is_empty() {
                self.builder.indent(None);
                self.newline();
                self.visit_nodes(
                    ast.list_raw(statements),
                    None,
                    Some(Self::one_or_two_newlines),
                    None,
                );
                self.builder.unindent();
                self.one_or_two_newlines();
            } else {
                // Don't preserve blank lines between empty cases.
                self.builder.write_newline(false, false, false);
            }
        }

        if !members.is_empty() {
            self.newline();
        }
        self.end_body(n.right_bracket, !members.is_empty());
    }

    /// Visits the `switch (expr)` part of a switch statement or expression.
    fn visit_switch_value(
        &mut self,
        switch_keyword: TokenId,
        left_parenthesis: TokenId,
        value: NodeId,
        right_parenthesis: TokenId,
    ) {
        self.builder.nest();
        self.token(switch_keyword);
        self.space();
        self.token(left_parenthesis);
        self.solo_zero_split();
        self.visit(value);
        self.token(right_parenthesis);
        self.space();
        self.builder.unnest();
    }

    fn visit_symbol_literal(&mut self, node: Id<SymbolLiteral>) {
        let ast = self.ast;
        let n = &ast[node];
        self.token(n.pound_sign);
        for &component in ast.token_list(n.components) {
            // The '.' separator
            let previous = ast.tokens.previous(component);
            if ast.tokens.lexeme(previous) == "." {
                self.token(previous);
            }
            self.token(component);
        }
    }

    fn visit_this_expression(&mut self, node: Id<ThisExpression>) {
        self.token(self.ast[node].this_keyword);
    }

    fn visit_throw_expression(&mut self, node: Id<ThrowExpression>) {
        let n = &self.ast[node];
        self.token(n.throw_keyword);
        self.space();
        self.visit(n.expression);
    }

    fn visit_top_level_variable_declaration(&mut self, node: Id<TopLevelVariableDeclaration>) {
        let n = &self.ast[node];
        self.visit_metadata(n.metadata);

        self.simple_statement(Some(n.semicolon), |v| {
            v.modifier(n.external_keyword);
            v.visit(n.variables);
        });
    }

    fn visit_try_statement(&mut self, node: Id<TryStatement>) {
        let ast = self.ast;
        let n = &ast[node];
        self.token(n.try_keyword);
        self.space();
        self.visit(n.body);
        self.visit_nodes(
            ast.list_raw(n.catch_clauses),
            Some(Self::space),
            Some(Self::space),
            None,
        );
        self.token_with(n.finally_keyword, Some(Self::space), Some(Self::space));
        self.visit_opt(n.finally_block);
    }

    fn visit_type_argument_list(&mut self, node: Id<TypeArgumentList>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_generic_list(n.left_bracket, n.right_bracket, ast.list_raw(n.arguments));
    }

    fn visit_type_parameter(&mut self, node: Id<TypeParameter>) {
        let n = &self.ast[node];
        self.visit_parameter_metadata(n.metadata, |v| {
            v.token(n.name);
            v.token_with(n.extends_keyword, Some(Self::space), Some(Self::space));
            v.visit_opt(n.bound);
        });
    }

    fn visit_type_parameter_list(&mut self, node: Id<TypeParameterList>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_generic_list(
            n.left_bracket,
            n.right_bracket,
            ast.list_raw(n.type_parameters),
        );
    }

    fn visit_variable_declaration(&mut self, node: Id<VariableDeclaration>) {
        let ast = self.ast;
        let n = &ast[node];
        self.token(n.name);
        let Some(initializer) = n.initializer else {
            return;
        };

        // If there are multiple variables being declared, we want to nest the
        // initializers farther so they don't line up with the variables. Bad:
        //
        //     var a =
        //         aValue,
        //         b =
        //         bValue;
        //
        // Good:
        //
        //     var a =
        //             aValue,
        //         b =
        //             bValue;
        let parent = ast.cast::<VariableDeclarationList>(ast.parent(node).unwrap()).unwrap();
        let has_multiple_variables = ast[parent].variables.len() > 1;

        self.visit_assignment(n.equals.unwrap(), initializer.raw(), has_multiple_variables);
    }

    fn visit_variable_declaration_list(&mut self, node: Id<VariableDeclarationList>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_metadata(n.metadata);

        // Allow but try to avoid splitting between the type and name.
        self.builder.start_span_normal();

        self.modifier(n.late_keyword);
        self.modifier(n.keyword);
        self.visit_opt(n.type_);
        self.separator_between_type_and_variable(n.type_, true);

        self.builder.end_span();

        // Use a single rule for all of the variables. If there are multiple
        // declarations, we will try to keep them all on one line. If that isn't
        // possible, we split after *every* declaration so that each is on its own
        // line.
        self.builder.start_rule(None);

        // If there are multiple declarations split across lines, then we want any
        // blocks in the initializers to indent past the variables.
        let variables = ast.list_raw(n.variables);
        if variables.len() > 1 {
            self.builder.start_block_argument_nesting();
        }

        self.visit_comma_separated_nodes(variables, Some(Self::split_callback));

        if variables.len() > 1 {
            self.builder.end_block_argument_nesting();
        }

        self.builder.end_rule();
    }

    fn visit_variable_declaration_statement(&mut self, node: Id<VariableDeclarationStatement>) {
        let n = &self.ast[node];
        self.simple_statement(Some(n.semicolon), |v| {
            v.visit(n.variables);
        });
    }

    fn visit_while_statement(&mut self, node: Id<WhileStatement>) {
        let n = &self.ast[node];
        self.builder.nest();
        self.token(n.while_keyword);
        self.space();
        self.token(n.left_parenthesis);
        self.solo_zero_split();
        self.visit(n.condition);
        self.token(n.right_parenthesis);
        self.builder.unnest();

        self.visit_loop_body(n.body.raw());
    }

    fn visit_wildcard_pattern(&mut self, node: Id<WildcardPattern>) {
        let n = &self.ast[node];
        self.visit_variable_pattern(n.keyword, n.type_, n.name);
    }

    fn visit_with_clause(&mut self, node: Id<WithClause>) {
        let ast = self.ast;
        let n = &ast[node];
        self.visit_combinator(n.with_keyword, ast.list_raw(n.mixin_types));
    }

    fn visit_yield_statement(&mut self, node: Id<YieldStatement>) {
        let n = &self.ast[node];
        self.simple_statement(Some(n.semicolon), |v| {
            v.token(n.yield_keyword);
            v.token_opt(n.star);
            v.space();
            v.visit(n.expression);
        });
    }

    /// Visit a [node], and if not null, optionally preceded or followed by the
    /// specified functions.
    pub fn visit_with<T: ?Sized>(
        &mut self,
        node: Option<Id<T>>,
        before: Callback<'a>,
        after: Callback<'a>,
    ) {
        let Some(node) = node else {
            return;
        };

        if let Some(before) = before {
            before(self);
        }

        self.visit(node.raw());

        if let Some(after) = after {
            after(self);
        }
    }

    /// Dart `visit(node)` with a nullable node.
    pub fn visit_opt<T: ?Sized>(&mut self, node: Option<Id<T>>) {
        if let Some(node) = node {
            self.visit(node.raw());
        }
    }

    /// Visit metadata annotations on declarations, and members.
    ///
    /// These always force the annotations to be on the previous line.
    fn visit_metadata(&mut self, metadata: NodeList<Annotation>) {
        let ast = self.ast;
        self.visit_nodes(
            ast.list_raw(metadata),
            None,
            Some(Self::newline),
            Some(Self::newline),
        );
    }

    /// Visit metadata annotations for a directive.
    ///
    /// Always force the annotations to be on a previous line.
    fn visit_directive_metadata(&mut self, directive: NodeId, metadata: NodeList<Annotation>) {
        let ast = self.ast;

        // Preserve a blank line before the first directive since users (in
        // particular the test package) sometimes use that for metadata that
        // applies to the entire library and not the following directive itself.
        let unit = ast.cast::<CompilationUnit>(ast.parent(directive).unwrap()).unwrap();
        let is_first = ast.list_raw(ast[unit].directives).first() == Some(&directive);

        self.visit_nodes(
            ast.list_raw(metadata),
            None,
            Some(Self::newline),
            Some(if is_first {
                Self::one_or_two_newlines
            } else {
                Self::newline
            }),
        );
    }

    /// Visits metadata annotations on parameters and type parameters.
    ///
    /// Unlike other annotations, these are allowed to stay on the same line as
    /// the parameter.
    fn visit_parameter_metadata(
        &mut self,
        metadata: NodeList<Annotation>,
        visit_parameter: impl FnOnce(&mut Self),
    ) {
        if metadata.is_empty() {
            visit_parameter(self);
            return;
        }

        // Split before all of the annotations on this parameter or none of them.
        self.builder.start_lazy_rule(None);

        let ast = self.ast;
        self.visit_nodes(
            ast.list_raw(metadata),
            None,
            Some(Self::split_callback),
            Some(Self::split_callback),
        );
        visit_parameter(self);

        // Wrap the rule around the parameter too. If it splits, we want to force
        // the annotations to split as well.
        self.builder.end_rule();
    }

    /// Visits syntax of the form `identifier: <node>`: a named argument or a
    /// named record field.
    ///
    /// This is called directly by [ArgumentListVisitor] so that it can pass in
    /// the surrounding named argument rule. That way, this can ensure that a
    /// split between the name and argument forces the argument list to split
    /// too.
    pub fn visit_named_node(
        &mut self,
        name: TokenId,
        colon: TokenId,
        node: NodeId,
        rule: Option<RuleId>,
    ) {
        self.builder.nest();
        self.builder.start_span_normal();
        self.token(name);
        self.token(colon);

        // Don't allow a split between a name and a collection. Instead, we want
        // the collection itself to split, or to split before the argument.
        if matches!(
            self.ast.kind(node),
            NodeKind::ListLiteral | NodeKind::SetOrMapLiteral | NodeKind::RecordLiteral
        ) {
            self.space();
        } else {
            let split = self.solo_split(Cost::NORMAL);
            if let Some(rule) = rule {
                self.builder.arena.rule_mut(split).constrain_when_split(rule);
            }
        }

        self.visit(node);
        self.builder.end_span();
        self.builder.unnest();
    }

    /// Visits the `=` and the following expression in any place where an `=`
    /// appears:
    ///
    /// * Assignment
    /// * Variable declaration
    /// * Constructor initialization
    ///
    /// If [nest] is true, an extra level of expression nesting is added after
    /// the "=".
    fn visit_assignment(&mut self, equals_operator: TokenId, right_hand_side: NodeId, nest: bool) {
        self.space();
        self.token(equals_operator);

        if nest {
            self.builder.nest_expression(None, true);
        }

        self.solo_split(assignment_cost(self.ast, right_hand_side));
        self.builder.start_span_normal();
        self.visit(right_hand_side);
        self.builder.end_span();

        if nest {
            self.builder.unnest();
        }
    }

    /// Visits an infix operator-like AST node: a binary operator expression, or
    /// binary pattern.
    ///
    /// In a tree of binary AST nodes, all operators at the same precedence are
    /// treated as a single chain of operators that either all split or none do.
    /// Operands within those (which may themselves be chains of higher
    /// precedence binary operators) are then formatted independently.
    ///
    /// [destructure_node] returns the operands and operator of a node of the
    /// kind being visited, or `None` for any other node. We need this since
    /// there's no interface shared by the various binary operator AST nodes.
    ///
    /// If [precedence] is given, then only flattens binary nodes with that same
    /// precedence. If [nest] is `false`, then elides the nesting around the
    /// expression.
    fn visit_binary(
        &mut self,
        node: NodeId,
        destructure_node: fn(&Ast, NodeId) -> Option<(NodeId, TokenId, NodeId)>,
        precedence: Option<u8>,
        nest: bool,
    ) {
        self.builder.start_span_normal();

        if nest {
            self.builder.nest();
        }

        // Start lazily so we don't force the operator to split if a line comment
        // appears before the first operand.
        self.builder.start_lazy_rule(None);

        // Blocks as operands to infix operators should always nest like regular
        // operands. (Granted, this case is exceedingly rare in real code.)
        self.builder.start_block_argument_nesting();

        fn traverse(
            v: &mut SourceVisitor,
            e: NodeId,
            destructure_node: fn(&Ast, NodeId) -> Option<(NodeId, TokenId, NodeId)>,
            precedence: Option<u8>,
        ) {
            let ast = v.ast;
            match destructure_node(ast, e) {
                None => v.visit(e),
                Some((left, operator, right)) => {
                    if precedence.is_some_and(|p| ast.tokens.ty(operator).precedence() != p) {
                        // Binary node, but a different precedence, so don't flatten.
                        v.visit(e);
                    } else {
                        traverse(v, left, destructure_node, precedence);

                        v.space();
                        v.token(operator);

                        v.split();
                        traverse(v, right, destructure_node, precedence);
                    }
                }
            }
        }

        traverse(self, node, destructure_node, precedence);

        self.builder.end_block_argument_nesting();

        if nest {
            self.builder.unnest();
        }
        self.builder.end_span();
        self.builder.end_rule();
    }

    /// Visits the "with" and "implements" clauses in a type declaration.
    fn visit_clauses(
        &mut self,
        with_clause: Option<Id<WithClause>>,
        implements_clause: Option<Id<ImplementsClause>>,
    ) {
        let rule = self.builder.arena.add_rule(CombinatorRule::new_rule());
        self.builder.start_rule(Some(rule));
        self.visit_opt(with_clause);
        self.visit_opt(implements_clause);
        self.builder.end_rule();
    }

    /// Visits a list of combinators in a directive.
    fn visit_combinators(&mut self, combinators: &[NodeId]) {
        let rule = self.builder.arena.add_rule(CombinatorRule::new_rule());
        self.builder.start_rule(Some(rule));
        self.visit_nodes(combinators, None, None, None);
        self.builder.end_rule();
    }

    /// Visits a type parameter or type argument list.
    fn visit_generic_list(&mut self, left_bracket: TokenId, right_bracket: TokenId, nodes: &[NodeId]) {
        let ast = self.ast;
        let rule = self.builder.arena.add_rule(TypeArgumentRule::new_rule());
        self.builder.start_lazy_rule(Some(rule));
        self.builder.start_span_normal();
        self.builder.nest();

        self.token(left_bracket);
        let chunk = self.zero_split();
        self.builder.arena.rule_mut(rule).before_argument(Some(chunk));

        // Set the block nesting in case an argument is a function type with a
        // trailing comma or a record type.
        self.builder.start_block_argument_nesting();

        for (i, &node) in nodes.iter().enumerate() {
            self.visit(node);

            // Write the comma separator.
            if i != nodes.len() - 1 {
                let mut comma = ast.tokens.next(ast.end_token(node));

                // TODO(rnystrom): There is a bug in analyzer where the end token of a
                // nullable record type is the ")" and not the "?". This works around
                // that. Remove that's fixed.
                if ast.tokens.lexeme(comma) == "?" {
                    comma = ast.tokens.next(comma);
                }

                self.token(comma);
                let chunk = self.split();
                self.builder.arena.rule_mut(rule).before_argument(Some(chunk));
            }
        }

        self.token(right_bracket);

        self.builder.end_block_argument_nesting();
        self.builder.unnest();
        self.builder.end_span();
        self.builder.end_rule();
    }

    /// Visits a sequence of labels before a statement or switch case.
    fn visit_labels(&mut self, labels: &[NodeId]) {
        self.visit_nodes(labels, None, Some(Self::newline), Some(Self::newline));
    }

    /// Visits the members in a type declaration or the statements in a block.
    fn visit_body_contents(&mut self, nodes: &[NodeId]) {
        for (i, &node) in nodes.iter().enumerate() {
            self.visit(node);

            // If the node has a non-empty braced body, then require a blank line
            // between it and the next node.
            if i != nodes.len() - 1 {
                if has_non_empty_body(self.ast, node) {
                    self.two_newlines();
                } else {
                    self.one_or_two_newlines();
                }
            }
        }
    }

    /// Visits a variable or wildcard pattern.
    fn visit_variable_pattern(
        &mut self,
        keyword: Option<TokenId>,
        type_: Option<Id<TypeAnnotation>>,
        name: TokenId,
    ) {
        self.modifier(keyword);
        self.visit_with(type_, None, Some(Self::solo_split_callback));
        self.token(name);
    }

    /// Visits a top-level function or method declaration.
    #[allow(clippy::too_many_arguments)]
    fn visit_function_or_method_declaration(
        &mut self,
        metadata: NodeList<Annotation>,
        external_keyword: Option<TokenId>,
        property_keyword: Option<TokenId>,
        modifier_keyword: Option<TokenId>,
        operator_keyword: Option<TokenId>,
        name: TokenId,
        return_type: Option<Id<TypeAnnotation>>,
        type_parameters: Option<Id<TypeParameterList>>,
        formal_parameters: Option<Id<FormalParameterList>>,
        body: Id<FunctionBody>,
    ) {
        self.visit_metadata(metadata);

        // Nest the signature in case we have to split between the return type and
        // name.
        self.builder.nest();
        self.builder.start_span_normal();
        self.modifier(external_keyword);
        self.modifier(modifier_keyword);
        self.visit_with(return_type, None, Some(Self::solo_split_callback));
        self.modifier(property_keyword);
        self.modifier(operator_keyword);
        self.token(name);
        self.builder.end_span();

        let is_expression_body = self.ast.kind(body) == NodeKind::ExpressionFunctionBody;
        self.visit_function_body(
            type_parameters,
            formal_parameters,
            body,
            Some(&mut |v: &mut Self| {
                // If the body is a block, we need to exit nesting before we hit the body
                // indentation, but we do want to wrap it around the parameters.
                if !is_expression_body {
                    v.builder.unnest();
                }
            }),
        );

        // If it's an expression, we want to wrap the nesting around that so that
        // the body gets nested farther.
        if is_expression_body {
            self.builder.unnest();
        }
    }

    /// Visit the given function [parameters] followed by its [body], printing a
    /// space before it if it's not empty.
    ///
    /// If [before_body] is provided, it is invoked before the body is visited.
    fn visit_function_body(
        &mut self,
        type_parameters: Option<Id<TypeParameterList>>,
        parameters: Option<Id<FormalParameterList>>,
        body: Id<FunctionBody>,
        before_body: Option<&mut dyn FnMut(&mut Self)>,
    ) {
        // If the body is "=>", add an extra level of indentation around the
        // parameters and a rule that spans the parameters and the "=>". This
        // ensures that if the parameters wrap, they wrap more deeply than the "=>"
        // does, as in:
        //
        //     someFunction(parameter,
        //             parameter, parameter) =>
        //         "the body";
        //
        // Also, it ensures that if the parameters wrap, we split at the "=>" too
        // to avoid:
        //
        //     someFunction(parameter,
        //         parameter) => function(
        //         argument);
        //
        // This is confusing because it looks like those two lines are at the same
        // level when they are actually unrelated. Splitting at "=>" forces:
        //
        //     someFunction(parameter,
        //             parameter) =>
        //         function(
        //             argument);
        let is_expression_body = self.ast.kind(body) == NodeKind::ExpressionFunctionBody;
        if is_expression_body {
            self.builder.nest();

            // This rule is ended by visitExpressionFunctionBody().
            let rule = self.builder.arena.new_rule_with_cost(Cost::ARROW);
            self.builder.start_lazy_rule(Some(rule));
        }

        self.visit_parameter_signature(type_parameters, parameters);

        if let Some(before_body) = before_body {
            before_body(self);
        }
        self.visit(body);

        if is_expression_body {
            self.builder.unnest();
        }
    }

    /// Visits the type parameters (if any) and formal parameters of a method
    /// declaration, function declaration, or generic function type.
    fn visit_parameter_signature(
        &mut self,
        type_parameters: Option<Id<TypeParameterList>>,
        parameters: Option<Id<FormalParameterList>>,
    ) {
        // Start the nesting for the parameters here, so they indent past the
        // type parameters too, if any.
        self.builder.nest();

        self.visit_opt(type_parameters);
        if let Some(parameters) = parameters {
            self.visit_formal_parameter_list(parameters, false);
        }

        self.builder.unnest();
    }

    /// Visits the body statement of a `for`, `for in`, or `while` loop.
    fn visit_loop_body(&mut self, body: NodeId) {
        match self.ast.kind(body) {
            NodeKind::EmptyStatement => {
                // No space before the ";".
                self.visit(body);
            }
            NodeKind::Block => {
                self.space();
                self.visit(body);
            }
            _ => {
                // Allow splitting in a statement-bodied loop even though it's against
                // the style guide. Since we can't fix the code itself to follow the
                // style guide, we should at least format it as well as we can.
                self.builder.indent(None);
                self.builder.start_rule(None);

                self.builder.split(false, true);
                self.visit(body);

                self.builder.end_rule();
                self.builder.unindent();
            }
        }
    }

    /// Visit a list of [nodes] if not null, optionally separated and/or preceded
    /// and followed by the given functions.
    pub fn visit_nodes(
        &mut self,
        nodes: &[NodeId],
        before: Callback<'a>,
        between: Callback<'a>,
        after: Callback<'a>,
    ) {
        if nodes.is_empty() {
            return;
        }

        if let Some(before) = before {
            before(self);
        }

        self.visit(nodes[0]);
        for &node in &nodes[1..] {
            if let Some(between) = between {
                between(self);
            }
            self.visit(node);
        }

        if let Some(after) = after {
            after(self);
        }
    }

    /// Visit a comma-separated list of [nodes] if not null.
    pub fn visit_comma_separated_nodes(&mut self, nodes: &[NodeId], between: Callback<'a>) {
        if nodes.is_empty() {
            return;
        }

        let between = between.unwrap_or(Self::space);
        let ast = self.ast;

        let mut first = true;
        for &node in nodes {
            if !first {
                between(self);
            }
            first = false;

            self.visit(node);

            // The comma after the node.
            let next = ast.tokens.next(ast.end_token(node));
            if ast.tokens.lexeme(next) == "," {
                self.token(next);
            }
        }
    }

    /// Visits the construct whose body starts with [left_bracket],
    /// ends with [right_bracket] and contains [elements].
    ///
    /// This is used for collection literals, collection patterns, and argument
    /// lists with a trailing comma which are considered "collection-like".
    ///
    /// If [split_outer_collection] is `true` then this collection forces any
    /// surrounding collections to split even if this one doesn't. We do this for
    /// collection literals, but not other collection-like constructs.
    #[allow(clippy::too_many_arguments)]
    fn visit_collection_literal(
        &mut self,
        left_bracket: TokenId,
        elements: &[NodeId],
        right_bracket: TokenId,
        const_keyword: Option<TokenId>,
        type_arguments: Option<Id<TypeArgumentList>>,
        split_outer_collection: bool,
        is_record: bool,
    ) {
        let ast = self.ast;
        self.modifier(const_keyword);

        // Don't use the normal type argument list formatting code because we don't
        // want to allow splitting before the "<" since there is no preceding
        // identifier and it looks weird to have a "<" hanging by itself. Prevents:
        //
        //   var list = <
        //       LongTypeName<
        //           TypeArgument,
        //           TypeArgument>>[];
        if let Some(type_arguments) = type_arguments {
            let type_arguments = &ast[type_arguments];
            self.builder.start_span_normal();
            self.builder.nest();
            self.token(type_arguments.left_bracket);
            let rule = self.builder.arena.new_rule_with_cost(Cost::TYPE_ARGUMENT);
            self.builder.start_rule(Some(rule));

            let arguments = ast.list_raw(type_arguments.arguments);
            for (i, &type_argument) in arguments.iter().enumerate() {
                self.visit(type_argument);

                // Write the comma separator.
                if i != arguments.len() - 1 {
                    let mut comma = ast.tokens.next(ast.end_token(type_argument));

                    // TODO(rnystrom): There is a bug in analyzer where the end token of a
                    // nullable record type is the ")" and not the "?". This works around
                    // that. Remove once that's fixed.
                    if ast.tokens.lexeme(comma) == "?" {
                        comma = ast.tokens.next(comma);
                    }

                    self.token(comma);
                    self.split();
                }
            }

            self.token(type_arguments.right_bracket);
            self.builder.end_rule();
            self.builder.unnest();
            self.builder.end_span();
        }

        // Handle empty collections, with or without comments.
        if elements.is_empty() {
            self.visit_body(left_bracket, elements, right_bracket);
            return;
        }

        // Unlike other collections, records don't force outer ones to split.
        if split_outer_collection {
            // Force all of the surrounding collections to split.
            for split in &mut self.collection_splits {
                *split = true;
            }

            // Add this collection to the stack.
            self.collection_splits.push(false);
        }

        self.begin_body(left_bracket, false);

        // If a collection contains a line comment, we assume it's a big complex
        // blob of data with some documented structure. In that case, the user
        // probably broke the elements into lines deliberately, so preserve those.
        if contains_line_comments(ast, elements, Some(right_bracket)) {
            // Newlines are significant, so we'll explicitly write those. Elements
            // on the same line all share an argument-list-like rule that allows
            // splitting between zero, one, or all of them. This is faster in long
            // lists than using individual splits after each element.
            let mut line_rule = self.builder.arena.add_rule(TypeArgumentRule::new_rule());
            self.builder.start_lazy_rule(Some(line_rule));

            for &element in elements {
                // See if the next element is on the next line.
                let begin = ast.begin_token(element);
                if self.end_line(ast.tokens.previous(begin)) != self.start_line(begin) {
                    self.one_or_two_newlines();

                    // Start a new rule for the new line.
                    self.builder.end_rule();
                    line_rule = self.builder.arena.add_rule(TypeArgumentRule::new_rule());
                    self.builder.start_lazy_rule(Some(line_rule));
                } else {
                    let chunk = self.split();
                    self.builder
                        .arena
                        .rule_mut(line_rule)
                        .before_argument(Some(chunk));
                }

                self.visit(element);
                self.write_comma_after(element);
            }

            self.builder.end_rule();
        } else {
            for &element in elements {
                self.builder.split(false, element != elements[0]);
                self.visit(element);
                self.write_comma_after(element);
            }
        }

        // If there is a collection inside this one, it forces this one to split.
        let mut force = false;
        if split_outer_collection {
            force = self.collection_splits.pop().unwrap();
        }

        // If the collection has a trailing comma, the user must want it to split.
        // (Unless it's a single-element record literal, in which case the trailing
        // comma is required for disambiguation.)
        let is_single_element_record = is_record && elements.len() == 1;
        if nodes_have_comma_after(ast, elements) && !is_single_element_record {
            force = true;
        }

        self.end_body(right_bracket, force);
    }

    /// Writes [parameters], which is assumed to have a trailing comma after the
    /// last parameter.
    ///
    /// Parameter lists with trailing commas are formatted differently from
    /// regular parameter lists. They are treated more like collection literals.
    ///
    /// We don't reuse [visit_collection_literal] here because there are enough
    /// weird differences around optional parameters that it's easiest just to
    /// give them their own method.
    fn visit_trailing_comma_parameter_list(&mut self, node: Id<FormalParameterList>) {
        let ast = self.ast;
        let n = &ast[node];
        let parameters = ast.list_raw(n.parameters);

        // Always split the parameters.
        let rule = self.builder.arena.new_hard_rule();
        self.builder.start_rule(Some(rule));

        self.token(n.left_parenthesis);

        // Find the parameter immediately preceding the optional parameters (if
        // there are any).
        let mut last_required = None;
        for i in 0..parameters.len() {
            if is_named_or_optional_positional(ast, parameters[i]) {
                if i > 0 {
                    last_required = Some(parameters[i - 1]);
                }
                break;
            }
        }

        // If all parameters are optional, put the "[" or "{" right after "(".
        if is_named_or_optional_positional(ast, parameters[0]) {
            self.token_opt(n.left_delimiter);
        }

        // Process the parameters as a separate set of chunks.
        self.builder.start_block(None, true, false);

        for &parameter in parameters {
            self.builder.write_newline(false, false, false);
            self.visit(parameter);
            self.write_comma_after(parameter);

            // If the optional parameters start after this one, put the delimiter
            // at the end of its line.
            if Some(parameter) == last_required {
                self.space();
                self.token_opt(n.left_delimiter);
                last_required = None;
            }
        }

        // Put comments before the closing ")", "]", or "}" inside the block.
        let first_delimiter = n.right_delimiter.unwrap_or(n.right_parenthesis);
        if has_preceding_comments(ast, first_delimiter) {
            self.builder.write_newline(false, false, false);
            self.write_preceding_comments_and_newlines(first_delimiter);
        }

        self.builder.end_block(true);
        self.builder.end_rule();

        // Now write the delimiter itself.
        self.write_text(ast.tokens.lexeme(first_delimiter), first_delimiter, None, true);
        if first_delimiter != n.right_parenthesis {
            self.token(n.right_parenthesis);
        }
    }

    /// Begins writing a formal parameter of any kind.
    fn begin_formal_parameter(
        &mut self,
        required_keyword: Option<TokenId>,
        covariant_keyword: Option<TokenId>,
    ) {
        let rule = self.builder.arena.new_rule_with_cost(Cost::PARAMETER_TYPE);
        self.builder.start_lazy_rule(Some(rule));
        self.builder.nest();
        self.modifier(required_keyword);
        self.modifier(covariant_keyword);
    }

    /// Ends writing a formal parameter of any kind.
    fn end_formal_parameter(&mut self) {
        self.builder.unnest();
        self.builder.end_rule();
    }

    /// Visits the `if (<expr> [case <pattern> [when <expr>]])` header of an if
    /// statement or element.
    fn visit_if_condition(
        &mut self,
        if_keyword: TokenId,
        left_parenthesis: TokenId,
        condition: NodeId,
        case_clause: Option<Id<CaseClause>>,
        right_parenthesis: TokenId,
    ) {
        let ast = self.ast;
        self.builder.nest();
        self.token(if_keyword);
        self.space();
        self.token(left_parenthesis);

        match case_clause {
            None => {
                // Simple if with no "case".
                self.visit(condition);
            }
            Some(case_clause) => {
                // If-case.
                let case_clause = &ast[case_clause];
                let guarded_pattern = &ast[case_clause.guarded_pattern];

                // Wrap the rule for splitting before "case" around the value expression
                // so that if the value splits, we split before "case" too.
                self.builder.start_rule(None);

                self.visit(condition);

                // "case" and pattern.
                self.split();
                self.token(case_clause.case_keyword);
                self.space();
                self.builder.start_block_argument_nesting();
                self.builder.nest_expression(None, true);
                self.visit(guarded_pattern.pattern);
                self.builder.unnest();
                self.builder.end_block_argument_nesting();

                self.builder.end_rule(); // Case rule.

                if let Some(when_clause) = guarded_pattern.when_clause {
                    // Wrap the rule for "when" around the guard so that a split in the
                    // guard splits at "when" too.
                    self.builder.start_rule(None);
                    self.split();
                    self.builder.start_block_argument_nesting();
                    self.builder.nest();
                    self.visit_when_clause(when_clause);
                    self.builder.unnest();
                    self.builder.end_block_argument_nesting();
                    self.builder.end_rule(); // Guard rule.
                }
            }
        }

        self.token(right_parenthesis);
        self.builder.unnest();
    }

    fn visit_when_clause(&mut self, when_clause: Id<WhenClause>) {
        let n = &self.ast[when_clause];
        self.token(n.when_keyword);
        self.space();
        self.visit(n.expression);
    }

    /// Writes the separator between a type annotation and a variable or
    /// parameter. If the preceding type annotation ends in a delimited list of
    /// elements that have block formatting, then we don't split between the
    /// type annotation and parameter name, as in:
    ///
    ///     Function(
    ///       int,
    ///     ) variable;
    ///
    /// Otherwise, we can.
    fn separator_between_type_and_variable(&mut self, type_: Option<Id<TypeAnnotation>>, is_solo: bool) {
        let Some(type_) = type_ else {
            return;
        };
        let ast = self.ast;

        let mut is_block_type = false;
        if let Some(function_type) = ast.cast::<GenericFunctionType>(type_) {
            // Function types get block-like formatting if they have a trailing comma.
            let parameters = ast.list_raw(ast[ast[function_type].parameters].parameters);
            is_block_type = !parameters.is_empty() && has_comma_after(ast, *parameters.last().unwrap());
        } else if ast.kind(type_) == NodeKind::RecordTypeAnnotation {
            // Record types always have block-like formatting.
            is_block_type = true;
        }

        if is_block_type {
            self.space();
        } else if is_solo {
            self.solo_split(Cost::NORMAL);
        } else {
            self.split();
        }
    }

    /// Whether [node] should be forced to split even if completely empty.
    ///
    /// Most empty blocks format as `{}` but in a couple of cases where there is
    /// a subsequent block, we split the previous one.
    fn split_empty_block(&self, node: Id<Block>) -> bool {
        let ast = self.ast;
        let Some(parent) = ast.parent(node) else {
            return false;
        };

        // Force a split when used as the then body of an if with an else:
        //
        //     if (condition) {
        //     } else ...
        if let Some(if_statement) = ast.cast::<IfStatement>(parent) {
            let if_statement = &ast[if_statement];
            return if_statement.else_statement.is_some()
                && if_statement.then_statement.raw() == node.raw();
        }

        // Force a split in an empty catch if there is a finally or other catch
        // after it:
        if ast.kind(parent) == NodeKind::CatchClause {
            if let Some(try_statement) = ast.parent(parent).and_then(|p| ast.cast::<TryStatement>(p)) {
                let try_statement = &ast[try_statement];

                // Split the catch if there is something after it, a finally or another
                // catch.
                let last_catch = *ast.list(try_statement.catch_clauses).last().unwrap();
                return try_statement.finally_block.is_some() || node != ast[last_catch].body;
            }
        }

        false
    }

    /// Begins writing a bracket-delimited body whose contents are a nested
    /// block chunk.
    ///
    /// If [space] is `true`, writes a space after [left_bracket] when not split.
    ///
    /// Writes the delimiter (with a space after it when unsplit if [space] is
    /// `true`).
    fn begin_body(&mut self, left_bracket: TokenId, space: bool) {
        self.token(left_bracket);

        // Create a rule for whether or not to split the block contents. If this
        // literal is associated with an argument list or if element that wants to
        // handle splitting and indenting it, use its rule. Otherwise, use a
        // default rule.
        let rule = self.block_rules.get(&left_bracket).copied();
        self.builder.start_rule(rule);

        // Process the contents as a separate set of chunks.
        let argument_chunk = self.block_previous_chunks.get(&left_bracket).copied();
        self.builder.start_block(argument_chunk, true, space);
    }

    /// Ends the body started by a call to [begin_body()].
    ///
    /// If [force_split] is `true`, forces the body to split.
    fn end_body(&mut self, right_bracket: TokenId, force_split: bool) {
        // Put comments before the closing delimiter inside the block.
        let has_leading_newline = self.write_preceding_comments_and_newlines(right_bracket);

        self.builder.end_block(has_leading_newline || force_split);

        self.builder.end_rule();

        // Now write the delimiter itself.
        let lexeme = self.ast.tokens.lexeme(right_bracket);
        self.write_text(lexeme, right_bracket, None, true);
    }

    /// Visits a list of configurations in an import or export directive.
    fn visit_configurations(&mut self, configurations: &[NodeId]) {
        if configurations.is_empty() {
            return;
        }

        self.builder.start_rule(None);

        for &configuration in configurations {
            self.split();
            self.visit(configuration);
        }

        self.builder.end_rule();
    }

    /// Visits a "combinator".
    ///
    /// This is a [keyword] followed by a list of [nodes], with specific line
    /// splitting rules. As the name implies, this is used for [HideCombinator]
    /// and [ShowCombinator], but it also used for "with" and "implements"
    /// clauses in class declarations, which are formatted the same way.
    ///
    /// This assumes the current rule is a [CombinatorRule].
    fn visit_combinator(&mut self, keyword: TokenId, nodes: &[NodeId]) {
        // Allow splitting before the keyword.
        let rule = self.builder.rule();
        let chunk = self.split();
        self.builder.arena.rule_mut(rule).add_combinator(chunk);

        self.builder.nest();
        self.token(keyword);

        let chunk = self.split();
        self.builder.arena.rule_mut(rule).add_name(chunk);

        // Dart `visitCommaSeparatedNodes(nodes, between: () => rule.addName(split()))`.
        let ast = self.ast;
        let mut first = true;
        for &node in nodes {
            if !first {
                let chunk = self.split();
                self.builder.arena.rule_mut(rule).add_name(chunk);
            }
            first = false;

            self.visit(node);

            // The comma after the node.
            let next = ast.tokens.next(ast.end_token(node));
            if ast.tokens.lexeme(next) == "," {
                self.token(next);
            }
        }

        self.builder.unnest();
    }

    /// Writes the simple statement or semicolon-delimited top-level declaration.
    ///
    /// Handles nesting if a line break occurs in the statement and writes the
    /// terminating semicolon. Invokes [body] which should write statement itself.
    fn simple_statement(&mut self, semicolon: Option<TokenId>, body: impl FnOnce(&mut Self)) {
        self.builder.nest();
        body(self);

        self.token_opt(semicolon);
        self.builder.unnest();
    }

    /// Marks the block that starts with [token] as being controlled by
    /// [rule] and following [previous_chunk].
    ///
    /// When the block is visited, these will determine the indentation and
    /// splitting rule for the block. These are used for handling block-like
    /// expressions inside argument lists and spread collections inside if
    /// elements.
    pub fn before_block(&mut self, token: TokenId, rule: RuleId, previous_chunk: Option<ChunkId>) {
        self.block_rules.insert(token, rule);
        if let Some(previous_chunk) = previous_chunk {
            self.block_previous_chunks.insert(token, previous_chunk);
        }
    }

    /// Writes the brace-delimited body containing [nodes].
    fn visit_body(&mut self, left_bracket: TokenId, nodes: &[NodeId], right_bracket: TokenId) {
        let ast = self.ast;

        // Don't allow splitting in an empty body.
        if nodes.is_empty() && !has_preceding_comments(ast, right_bracket) {
            self.token(left_bracket);
            self.token(right_bracket);
            return;
        }

        self.begin_body(left_bracket, false);
        self.visit_body_contents(nodes);
        self.end_body(right_bracket, !nodes.is_empty());
    }

    /// Writes the string literal [string] to the output.
    ///
    /// Splits multiline strings into separate chunks so that the line splitter
    /// can handle them correctly.
    fn write_string_literal(&mut self, string: TokenId) {
        let ast = self.ast;

        // Since we output the string literal manually, ensure any preceding
        // comments are written first.
        self.write_preceding_comments_and_newlines(string);

        // Dart `string.lexeme.split(RegExp(r'\r\n?|\n'))`.
        let lexeme = ast.tokens.lexeme(string);
        let mut lines: Vec<&str> = Vec::new();
        let bytes = lexeme.as_bytes();
        let mut start = 0;
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'\r' => {
                    lines.push(&lexeme[start..i]);
                    if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                        i += 1;
                    }
                    start = i + 1;
                }
                b'\n' => {
                    lines.push(&lexeme[start..i]);
                    start = i + 1;
                }
                _ => {}
            }
            i += 1;
        }
        lines.push(&lexeme[start..]);

        let mut offset = ast.tokens.offset(string) as i64;
        let first_line = lines[0];
        if lines.len() > 1 {
            // Special case for multiline string which contains
            // at least one newline.
            self.write_string_first_line(first_line, string, offset);
        } else {
            self.write_text(first_line, string, Some(offset), true);
        }
        offset += utf16_len(first_line) as i64;

        for line in &lines[1..] {
            self.builder.write_newline(false, true, true);
            offset += 1;
            self.write_text(line, string, Some(offset), false);
            offset += utf16_len(line) as i64;
        }
    }

    /// Writes the first line of a multi-line string.
    ///
    /// If the string is a multiline string, and it has only whitespace
    /// and escaped whitespace before a first line break,
    /// omit the non-escaped trailing whitespace.
    /// Normalize escaped non-final whitspace to spaces.
    ///
    /// More specifically:
    /// If a multiline string literal contains at least one line-break
    /// (a CR, LF or CR+LF) as part of the source character content
    /// (characters inside interpolation expressions do not count),
    /// and the source characters from the starting quote to the first
    /// line-break contains only the characters space, tab and backslash,
    /// with no two adjacent backslashes, then that part of the string source,
    /// including the following line break, is excluded from particiapting
    /// code points to the string value.
    ///
    /// This function normalizes such excluded character sequences
    /// to just the back-slashes, separated by space characters.
    fn write_string_first_line(&mut self, line: &str, string: TokenId, offset: i64) {
        // Detect leading whitespace on the first line of multiline strings.
        let bytes = line.as_bytes();
        let quote_start = if line.starts_with('r') { 1 } else { 0 };
        let quote_end = quote_start + 3;
        let mut backslash_count = 0;
        if bytes.len() > quote_end
            && (line[quote_start..].starts_with("'''") || line[quote_start..].starts_with("\"\"\""))
        {
            // Start of a multiline string literal.
            // Check if rest of the line is whitespace, possibly preceded by
            // backslash, or has a single trailing backslash preceding the newline.
            // Count the backslashes.
            let mut cursor = quote_end;
            const BACKSLASH: u8 = 0x5c;
            const SPACE: u8 = 0x20;
            const TAB: u8 = 0x09;

            loop {
                let mut char = bytes[cursor];
                if char == BACKSLASH {
                    cursor += 1;
                    backslash_count += 1;
                    if cursor >= bytes.len() {
                        break;
                    }
                    char = bytes[cursor];
                }
                if char != SPACE && char != TAB {
                    break;
                }
                cursor += 1;
                if cursor >= bytes.len() {
                    break;
                }
            }
            if cursor == bytes.len() {
                // No invalid character sequence found before end of line.
                // Normalize the ignored "escaped" whitespace which has no
                // effect on string content.
                let mut first_line_text = line[..quote_end].to_string();
                if backslash_count > 0 {
                    first_line_text.push('\\');
                    backslash_count -= 1;
                    while backslash_count > 0 {
                        first_line_text.push_str(" \\");
                        backslash_count -= 1;
                    }
                }
                self.write_text(&first_line_text, string, Some(offset), true);
                return;
            }
        }
        self.write_text(line, string, Some(offset), true);
    }

    /// Write the comma token following [node], if there is one.
    fn write_comma_after(&mut self, node: NodeId) {
        let comma = comma_after(self.ast, node);
        self.token_opt(comma);
    }

    /// Emit the given [modifier] if it's non null, followed by non-breaking
    /// whitespace.
    fn modifier(&mut self, modifier: Option<TokenId>) {
        self.token_with(modifier, None, Some(Self::space));
    }

    /// Emit a non-breaking space.
    pub fn space(&mut self) {
        self.builder.write_space();
    }

    /// Emit a single mandatory newline.
    pub fn newline(&mut self) {
        self.builder.write_newline(false, false, false);
    }

    /// Emit a two mandatory newlines.
    fn two_newlines(&mut self) {
        self.builder.write_newline(true, false, false);
    }

    /// Allow either a single split or newline to be emitted before the next
    /// non-whitespace token based on whether a newline exists in the source
    /// between the last token and the next one.
    fn split_or_newline(&mut self) {
        if self.lines_before_next_token() > 0 {
            self.builder.write_newline(false, false, true);
        } else {
            self.split();
        }
    }

    /// Allow either a single split or newline to be emitted before the next
    /// non-whitespace token based on whether any blank lines exist in the source
    /// between the last token and the next one.
    fn split_or_two_newlines(&mut self) {
        if self.lines_before_next_token() > 1 {
            self.two_newlines();
        } else {
            self.split();
        }
    }

    /// Allow either one or two newlines to be emitted before the next
    /// non-whitespace token based on whether any blank lines exist in the source
    /// between the last token and the next one.
    fn one_or_two_newlines(&mut self) {
        let is_double = self.lines_before_next_token() > 1;
        self.builder.write_newline(is_double, false, false);
    }

    /// The number of newlines between the last written token and the next one to
    /// be written, including comments.
    ///
    /// Zero means "on the same line", one means "on subsequent lines", etc.
    fn lines_before_next_token(&self) -> i32 {
        let tokens = &self.ast.tokens;
        let previous = self.last_token;
        let mut next = tokens.next(previous);
        if let Some(comment) = tokens.get(next).preceding_comments.get() {
            next = comment;
        }

        self.start_line(next) - self.end_line(previous)
    }

    /// Writes a single space split owned by the current rule.
    ///
    /// Returns the chunk the split was applied to.
    pub fn split(&mut self) -> ChunkId {
        self.builder.split(true, true)
    }

    fn split_callback(&mut self) {
        self.split();
    }

    /// Writes a zero-space split owned by the current rule.
    ///
    /// Returns the chunk the split was applied to.
    pub fn zero_split(&mut self) -> ChunkId {
        self.builder.split(true, false)
    }

    fn zero_split_callback(&mut self) {
        self.zero_split();
    }

    /// Writes a single space split with its own rule.
    pub fn solo_split(&mut self, cost: i32) -> RuleId {
        let rule = self.builder.arena.new_rule_with_cost(cost);
        self.builder.start_rule(Some(rule));
        self.split();
        self.builder.end_rule();
        rule
    }

    fn solo_split_callback(&mut self) {
        self.solo_split(Cost::NORMAL);
    }

    /// Writes a zero-space split with its own rule.
    pub fn solo_zero_split(&mut self) {
        self.builder.start_rule(None);
        self.builder.split(true, false);
        self.builder.end_rule();
    }

    /// Emit [token], along with any comments and formatted whitespace that comes
    /// before it.
    pub fn token(&mut self, token: TokenId) {
        self.write_preceding_comments_and_newlines(token);

        let lexeme = self.ast.tokens.lexeme(token);
        self.write_text(lexeme, token, None, true);
    }

    /// Dart `token(token)` with a nullable token: does nothing if [token] is
    /// `None`.
    pub fn token_opt(&mut self, token: Option<TokenId>) {
        if let Some(token) = token {
            self.token(token);
        }
    }

    /// Emit [token], along with any comments and formatted whitespace that comes
    /// before it.
    ///
    /// Does nothing if [token] is `None`. If [before] is given, it will be
    /// executed before the token is outout. Likewise, [after] will be called
    /// after the token is output.
    pub fn token_with(&mut self, token: Option<TokenId>, before: Callback<'a>, after: Callback<'a>) {
        let Some(token) = token else {
            return;
        };

        self.write_preceding_comments_and_newlines(token);

        if let Some(before) = before {
            before(self);
        }

        let lexeme = self.ast.tokens.lexeme(token);
        self.write_text(lexeme, token, None, true);

        if let Some(after) = after {
            after(self);
        }
    }

    /// Writes all formatted whitespace and comments that appear before [token].
    pub fn write_preceding_comments_and_newlines(&mut self, token: TokenId) -> bool {
        let ast = self.ast;
        let tokens = &ast.tokens;
        let first_comment = tokens.get(token).preceding_comments;

        // For performance, avoid calculating newlines between tokens unless
        // actually needed.
        if first_comment.is_none() {
            return false;
        }

        // If the token's comments are already handled, do not write them here.
        if self.suppress_preceding_comments_and_new_lines.contains(&token) {
            return false;
        }

        let previous = tokens.previous(token);
        let mut previous_line = self.end_line(previous);
        let token_line = self.start_line(token);

        // Edge case: The analyzer includes the "\n" in the script tag's lexeme,
        // which confuses some of these calculations. We don't want to allow a
        // blank line between the script tag and a following comment anyway, so
        // just override the script tag's line.
        if tokens.ty(previous) == TokenType::SCRIPT_TAG {
            previous_line = token_line;
        }

        let mut comments = Vec::new();
        for comment in tokens.comments(token) {
            let comment_line = self.start_line(comment);

            // Don't preserve newlines at the top of the file.
            if comment == first_comment && tokens.ty(previous) == TokenType::EOF {
                previous_line = comment_line;
            }

            let text = dart_trim(tokens.lexeme(comment));
            let mut lines_before = comment_line - previous_line;
            let mut flush_left = self.start_column(comment) == 1;

            let is_line_doc = text.starts_with("///") && !text.starts_with("////");
            if is_line_doc {
                // Line doc comments are always indented even if they were flush left.
                flush_left = false;

                // Always add a blank line (if possible) before a doc comment block.
                if comment == first_comment {
                    lines_before = 2;
                }
            }

            let type_ = if is_line_doc || (text.starts_with("/**") && text != "/**/") {
                CommentType::Doc
            } else if tokens.ty(comment) == TokenType::SINGLE_LINE_COMMENT {
                CommentType::Line
            } else if comment_line == previous_line || comment_line == token_line {
                CommentType::InlineBlock
            } else {
                CommentType::Block
            };

            let mut source_comment = SourceComment::new(text.to_string(), type_, lines_before, flush_left);

            // If this comment contains either of the selection endpoints, mark them
            // in the comment.
            let t = tokens.get(comment);
            let start = self.get_selection_start_within(t.offset as i64, t.length as i64);
            if let Some(start) = start {
                source_comment.selection_start = Some(start as i32);
            }

            let end = self.get_selection_end_within(t.offset as i64, t.length as i64);
            if let Some(end) = end {
                source_comment.selection_end = Some(end as i32);
            }

            comments.push(source_comment);

            previous_line = self.end_line(comment);
        }

        let first_lines_before = self.builder.write_comments(
            comments,
            token_line - previous_line,
            tokens.lexeme(token),
        );

        // TODO(rnystrom): This is wrong. Consider:
        //
        // [/* inline comment */
        //     // line comment
        //     element];
        first_lines_before > 0
    }

    /// Write [text] to the current chunk, derived from [token].
    ///
    /// Also outputs the selection endpoints if needed.
    ///
    /// Usually, [text] is simply [token]'s lexeme, but for multiline strings, or
    /// a couple of other cases, it will be different.
    ///
    /// If [offset] is given, uses that for calculating selection location.
    /// Otherwise, uses the offset of [token].
    fn write_text(&mut self, text: &str, token: TokenId, offset: Option<i64>, merge_empty_splits: bool) {
        let offset = offset.unwrap_or_else(|| self.ast.tokens.offset(token) as i64);

        self.builder.write(text, merge_empty_splits);

        // If this text contains either of the selection endpoints, mark them in
        // the chunk.
        if self.source.selection_start.is_some() {
            let length = utf16_len(text) as i64;
            if let Some(start) = self.get_selection_start_within(offset, length) {
                self.builder.start_selection_from_end((length - start) as i32);
            }

            if let Some(end) = self.get_selection_end_within(offset, length) {
                self.builder.end_selection_from_end((length - end) as i32);
            }
        }

        self.last_token = token;
    }

    /// Returns the number of characters past [offset] in the source where the
    /// selection start appears if it appears before `offset + length`.
    ///
    /// Returns `None` if the selection start has already been processed or is
    /// not within that range.
    fn get_selection_start_within(&mut self, offset: i64, length: i64) -> Option<i64> {
        // If there is no selection, do nothing.
        let selection_start = self.source.selection_start? as i64;

        // If we've already passed it, don't consider it again.
        if self.passed_selection_start {
            return None;
        }

        let mut start = selection_start - offset;

        // If it started in whitespace before this text, push it forward to the
        // beginning of the non-whitespace text.
        if start < 0 {
            start = 0;
        }

        // If we haven't reached it yet, don't consider it.
        if start >= length {
            return None;
        }

        // We found it.
        self.passed_selection_start = true;

        Some(start)
    }

    /// Returns the number of characters past [offset] in the source where the
    /// selection endpoint appears if it appears before `offset + length`.
    ///
    /// Returns `None` if the selection endpoint has already been processed or is
    /// not within that range.
    fn get_selection_end_within(&mut self, offset: i64, length: i64) -> Option<i64> {
        // If there is no selection, do nothing.
        self.source.selection_length?;

        // If we've already passed it, don't consider it again.
        if self.passed_selection_end {
            return None;
        }

        let mut end = self.find_selection_end() - offset;

        // If it started in whitespace before this text, push it forward to the
        // beginning of the non-whitespace text.
        if end < 0 {
            end = 0;
        }

        // If we haven't reached it yet, don't consider it.
        if end > length {
            return None;
        }

        if end == length && Some(self.find_selection_end()) == self.source.selection_start.map(|s| s as i64) {
            return None;
        }

        // We found it.
        self.passed_selection_end = true;

        Some(end)
    }

    /// Calculates the character offset in the source text of the end of the
    /// selection.
    ///
    /// Removes any trailing whitespace from the selection.
    fn find_selection_end(&mut self) -> i64 {
        if let Some(selection_end) = self.selection_end {
            return selection_end;
        }

        let selection_start = self.source.selection_start.unwrap() as i64;
        let mut end = selection_start + self.source.selection_length.unwrap() as i64;

        let units = self
            .source_units
            .get_or_insert_with(|| self.source.text.encode_utf16().collect());

        // If the selection bumps to the end of the source, pin it there.
        if end == units.len() as i64 {
            self.selection_end = Some(end);
            return end;
        }

        // Trim off any trailing whitespace. We want the selection to "rubberband"
        // around the selected non-whitespace tokens since the whitespace will
        // be munged by the formatter itself.
        while end > selection_start {
            // Stop if we hit anything other than space, tab, newline or carriage
            // return.
            let char = units[(end - 1) as usize];
            if char != 0x20 && char != 0x09 && char != 0x0a && char != 0x0d {
                break;
            }

            end -= 1;
        }

        self.selection_end = Some(end);
        end
    }

    /// Dart `LineInfo.getLocation(offset)` for a possibly negative offset
    /// (the synthetic token before the first token has offset -1).
    fn location(&self, offset: u32) -> dartr_syntax::line_info::CharacterLocation {
        if (offset as i32) < 0 {
            return dartr_syntax::line_info::CharacterLocation {
                line_number: 1,
                column_number: 0,
            };
        }
        self.line_info.get_location(offset)
    }

    /// Gets the 1-based line number that the beginning of [token] lies on.
    fn start_line(&self, token: TokenId) -> i32 {
        self.location(self.ast.tokens.offset(token)).line_number as i32
    }

    /// Gets the 1-based line number that the end of [token] lies on.
    fn end_line(&self, token: TokenId) -> i32 {
        self.location(self.ast.tokens.get(token).end()).line_number as i32
    }

    /// Gets the 1-based column number that the beginning of [token] lies on.
    fn start_column(&self, token: TokenId) -> i32 {
        self.location(self.ast.tokens.offset(token)).column_number as i32
    }
}

/// Gets the cost to split at an assignment (or `:` in the case of a named
/// default value) with the given [right_hand_side].
///
/// "Block-like" expressions (collections and cascades) bind a bit tighter
/// because it looks better to have code like:
///
///     var list = [
///       element,
///       element,
///       element
///     ];
///
///     var builder = new SomeBuilderClass()
///       ..method()
///       ..method();
///
/// over:
///
///     var list =
///         [element, element, element];
///
///     var builder =
///         new SomeBuilderClass()..method()..method();
fn assignment_cost(ast: &Ast, right_hand_side: NodeId) -> i32 {
    match ast.kind(right_hand_side) {
        NodeKind::ListLiteral | NodeKind::SetOrMapLiteral | NodeKind::CascadeExpression => {
            Cost::ASSIGN_BLOCK
        }
        _ => Cost::ASSIGN,
    }
}

/// Dart `(node as InvocationExpression).argumentList` if [node] is an
/// [InvocationExpression].
fn invocation_argument_list(ast: &Ast, node: NodeId) -> Option<Id<ArgumentList>> {
    match ast.kind(node) {
        NodeKind::MethodInvocation => Some(ast[Id::<MethodInvocation>::from_raw(node)].argument_list),
        NodeKind::FunctionExpressionInvocation => {
            Some(ast[Id::<FunctionExpressionInvocation>::from_raw(node)].argument_list)
        }
        NodeKind::DotShorthandInvocation => {
            Some(ast[Id::<DotShorthandInvocation>::from_raw(node)].argument_list)
        }
        NodeKind::DotShorthandConstructorInvocation => {
            Some(ast[Id::<DotShorthandConstructorInvocation>::from_raw(node)].argument_list)
        }
        _ => None,
    }
}

/// Dart `String.trim()`: removes leading and trailing whitespace.
fn dart_trim(s: &str) -> &str {
    s.trim_matches(|c: char| {
        matches!(
            c,
            '\u{9}'..='\u{D}'
                | ' '
                | '\u{85}'
                | '\u{A0}'
                | '\u{1680}'
                | '\u{2000}'..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
        )
    })
}
