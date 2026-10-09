// Dart source: dart_style lib/src/front_end/piece_factory.dart

//! Dart `PieceFactory` is a mixin of [AstNodeVisitor]; here its methods are
//! methods of [AstNodeVisitor].

use dartr_ast::*;
use dartr_syntax::{TokenId, TokenType};

use crate::ast_extensions::*;
use crate::back_end::code_writer::Indent;
use crate::piece::{
    AdjacentPiece, AssignPiece, AssignPiece3Dot7, ClausePiece, Commas, ControlFlowPiece,
    ForInPiece, ForPiece, GroupingPiece, IfCasePiece, InfixPiece, ListStyle, PieceId, PieceKind,
    PrefixPiece, State, TypeTestPiece, VariablePiece,
};

use super::ast_node_visitor::AstNodeVisitor;
use super::chain_builder::ChainBuilder;
use super::delimited_list_builder::DelimitedListBuilder;
use super::piece_writer::OptNode;
use super::sequence_builder::SequenceBuilder;

/// Record type for a destructured binary operator-like syntactic construct.
pub type BinaryOperation = (NodeId, TokenId, NodeId);

/// The kind of syntax surrounding a node when being converted to a [Piece],
/// if that surrounding syntax may affect how the child node is formatted.
///
/// For example, binary operators indent their subsequent operands in most
/// places:
///
///     function(
///       operand +
///           operand,
///     );
///
/// But not when they appear on the right-hand side of an assignment or
/// assignment-like structure:
///
///     variable =
///         operand +
///         operand;
///
/// To handle this, when the code for a node recursively visits a child, it
/// can pass in a context describing itself, which the child can then access
/// to decide how it should be formatted.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NodeContext {
    /// No specified context.
    None,

    /// The child is the right-hand side of an assignment-like form.
    ///
    /// This includes assignments, variable declarations, named arguments,
    /// map entries, and `=>` function bodies.
    Assignment,

    /// The child is the target of a cascade expression.
    CascadeTarget,

    /// The child is the then or else operand of a conditional expression.
    ConditionalBranch,

    /// The child is a variable declaration in a for loop.
    ForLoopVariable,

    /// The child is an expression in a named argument or named record field.
    NamedExpression,

    /// The child is a string interpolation inside a multiline string.
    MultilineStringInterpolation,

    /// The child is the outermost pattern in a switch expression case.
    SwitchExpressionCase,
}

/// The parts of a Dart `FormalParameter` that the formatter uses.
pub struct FormalParameterParts {
    pub metadata: NodeList<Annotation>,
    pub kind: ParameterKind,
    pub required_keyword: Option<TokenId>,
    pub covariant_keyword: Option<TokenId>,
    pub const_final_or_var_keyword: Option<TokenId>,
    pub default_clause: Option<Id<FormalParameterDefaultClause>>,
}

/// The parts of a Dart `FormalParameter`, or `None` if [node] is not a
/// formal parameter.
pub fn formal_parameter_parts(ast: &Ast, node: NodeId) -> Option<FormalParameterParts> {
    macro_rules! parts {
        ($n:expr) => {{
            let n = $n;
            Some(FormalParameterParts {
                metadata: n.metadata,
                kind: n.kind,
                required_keyword: n.required_keyword,
                covariant_keyword: n.covariant_keyword,
                const_final_or_var_keyword: n.const_final_or_var_keyword,
                default_clause: n.default_clause,
            })
        }};
    }
    if let Some(p) = ast.cast::<RegularFormalParameter>(node) {
        return parts!(&ast[p]);
    }
    if let Some(p) = ast.cast::<FieldFormalParameter>(node) {
        return parts!(&ast[p]);
    }
    if let Some(p) = ast.cast::<SuperFormalParameter>(node) {
        return parts!(&ast[p]);
    }
    None
}

/// Dart `Expression.canBlockSplit` or `DartPattern.canBlockSplit` for
/// [node], and `false` for other nodes.
pub fn node_can_block_split(ast: &Ast, node: NodeId) -> bool {
    if let Some(expression) = ast.cast::<Expression>(node) {
        can_block_split(ast, expression)
    } else if ast.is::<DartPattern>(node) {
        pattern_can_block_split(ast, node)
    } else {
        false
    }
}

impl<'a> AstNodeVisitor<'a> {
    /// Writes a [ListPiece] for an argument list.
    pub fn write_argument_list(&mut self, argument_list: Id<ArgumentList>) {
        let ast = self.ast;
        let list = &ast[argument_list];
        self.write_arguments(
            list.left_parenthesis,
            ast.list_raw(list.arguments),
            list.right_parenthesis,
        );
    }

    /// Writes a [ListPiece] for an argument list.
    pub fn write_arguments(
        &mut self,
        left_bracket: TokenId,
        arguments: &[NodeId],
        right_bracket: TokenId,
    ) {
        let ast = self.ast;

        // In 3.7, we don't support preserving trailing commas or eager
        // splitting.
        if self.style.is_3_dot_7() {
            self.write_list(
                arguments,
                left_bracket,
                right_bracket,
                ListStyle::default(),
                false,
                true,
                true,
            );
            return;
        }

        // If the argument list is completely empty, write the brackets inline
        // so we create fewer pieces.
        if !nodes_can_split(ast, arguments, right_bracket) {
            self.token(left_bracket);
            self.token(right_bracket);
            return;
        }

        self.contents.begin_call(ast, arguments);

        let mut builder = DelimitedListBuilder::new(ListStyle::default());
        builder.left_bracket(self, left_bracket);
        builder.visit_all(self, arguments, true);
        builder.right_bracket(self, right_bracket, None, None);
        let force_split = self
            .style
            .preserve_trailing_comma_before(ast, right_bracket);
        let arguments_piece = builder.build_with(self, force_split, true);

        // If the call is complex enough, force it to split even if it would
        // fit.
        if self.contents.end_call() {
            // Don't force an argument list to fully split if it could block
            // split.
            // TODO(rnystrom): Ideally, if the argument list has a block
            // argument, we would force it to either block split or fully
            // split, but disallow it from being all on one line.
            // Unfortunately, the solver can't currently represent a
            // constraint like that.
            //
            // We definitely don't want to eagerly force the argument list to
            // fully split. Most of the time, that argument list will block
            // format, and that will look much better than being fully split.
            //
            // Since we can't prevent a list piece with block arguments from
            // being on one line, we don't try to eagerly split it at all. In
            // practice, almost all argument lists that can block split
            // contain either function expressions or large collections which
            // will force the call to (block) split, so the goal of not
            // packing too much on one line is still met even without this
            // eager splitting heuristic.
            if !self.is_list_with_block_element(arguments_piece) {
                self.arena.pin(arguments_piece, State::SPLIT);
            }
        }

        self.add(arguments_piece);
    }

    /// Dart `piece is ListPiece && piece.hasBlockElement`.
    pub fn is_list_with_block_element(&self, piece: PieceId) -> bool {
        match self.arena.kind(piece) {
            PieceKind::List(list) => list.has_block_element(&self.arena),
            _ => false,
        }
    }

    /// Writes a bracket-delimited block or declaration body.
    ///
    /// If [force_split] is `true`, then the block will split even if empty.
    /// This is used, for example, with empty blocks in `if` statements
    /// followed by `else` clauses:
    ///
    ///     if (condition) {
    ///     } else {}
    pub fn write_body(
        &mut self,
        left_bracket: TokenId,
        contents: &[NodeId],
        right_bracket: TokenId,
        force_split: bool,
    ) {
        let ast = self.ast;

        // If the body is completely empty, write the brackets directly inline
        // so that we create fewer pieces.
        if !force_split && !nodes_can_split(ast, contents, right_bracket) {
            self.token(left_bracket);
            self.token(right_bracket);
            return;
        }

        let mut sequence = SequenceBuilder::new();

        // Treat the `{` as soft so that if a function expression follows a
        // long string literal (like the lambda after a description in a
        // `test()` call), then the function header doesn't interfere with
        // the soft overflow of the string.
        sequence.left_bracket(self, left_bracket, true);

        let mut needs_blank = false;
        for &node in contents {
            sequence.visit_with(self, node, Indent::None, needs_blank);

            // If the node has a non-empty braced body, then require a blank
            // line between it and the next node.
            needs_blank = has_non_empty_body(ast, node);
        }

        sequence.right_bracket(self, right_bracket);
        let piece = sequence.build_with(self, force_split);
        self.add(piece);
    }

    /// Writes a [SequencePiece] for a given [Block].
    ///
    /// If [force_split] is `true`, then the block will split even if empty.
    pub fn write_block(&mut self, block: Id<Block>, force_split: bool) {
        let ast = self.ast;
        let block = &ast[block];
        self.write_body(
            block.left_bracket,
            ast.list_raw(block.statements),
            block.right_bracket,
            force_split,
        );
    }

    /// Writes a piece for a `break` or `continue` statement.
    pub fn write_break(
        &mut self,
        keyword: TokenId,
        label: Option<Id<LabelReference>>,
        semicolon: TokenId,
    ) {
        self.token(keyword);
        self.visit_with(label, true, false, NodeContext::None);
        self.token(semicolon);
    }

    pub fn write_chain(&mut self, node: NodeId) {
        let is_cascade_target = self.parent_context == NodeContext::CascadeTarget;
        let builder = ChainBuilder::new(self, node);
        let piece = builder.build(self, is_cascade_target);
        self.add(piece);
    }

    /// Writes a [ListPiece] for a collection literal or pattern.
    ///
    /// If [split_eagerly] is `true`, then this collection is forced to split
    /// if its contents are sufficiently complex enough to be hard to read on
    /// one line even if it would otherwise fit. For example:
    ///
    ///     // Prefer:
    ///     data = {
    ///       'a': [1, 2, 3],
    ///       'b': [
    ///         4,
    ///         [5],
    ///         6,
    ///       ]
    ///       'c': [7, 8],
    ///     };
    ///
    ///     // Over:
    ///     data = {'a': [1, 2, 3], 'b': [4, [5], 6] 'c': [7, 8]};
    ///
    /// We don't do this for record expressions because those are not
    /// unbounded in size and generally represent small aggregations of data
    /// where the fields are more "closely" bundled together.
    ///
    /// We don't do this for patterns because it's better to fit a pattern on
    /// a single line when possible for parallel cases in switches.
    ///
    /// If [preserve_newlines] is `true`, then any newlines or lack of
    /// newlines between pairs of elements in the input are preserved in the
    /// output. This is used for collection literals that contain line
    /// comments to preserve the author's deliberate structuring, as in:
    ///
    ///     matrix = [
    ///       // X, Y, Z:
    ///       1, 2, 3,
    ///       4, 5, 6,
    ///       7, 8, 9,
    ///     ];
    #[allow(clippy::too_many_arguments)]
    pub fn write_collection(
        &mut self,
        left_bracket: TokenId,
        elements: &[NodeId],
        right_bracket: TokenId,
        const_keyword: Option<TokenId>,
        type_arguments: Option<Id<TypeArgumentList>>,
        style: ListStyle,
        split_eagerly: bool,
        preserve_newlines: bool,
    ) {
        let ast = self.ast;
        self.modifier(const_keyword);
        self.visit(type_arguments);

        // If the collection is completely empty, write the brackets inline so
        // we create fewer pieces. The early return here also means that an
        // empty collection won't affect nesting and force outer collections
        // to split.
        if !nodes_can_split(ast, elements, right_bracket) {
            self.token(left_bracket);
            self.token(right_bracket);
            return;
        }

        // Add this collection to the stack.
        if split_eagerly {
            let is_named = self.parent_context == NodeContext::NamedExpression;
            self.contents.begin_collection(is_named);
        }

        let collection = self.build(|v| {
            v.write_list(
                elements,
                left_bracket,
                right_bracket,
                style,
                preserve_newlines,
                false,
                true,
            );
        });

        if split_eagerly && self.contents.end_collection(elements.len()) {
            self.arena.pin(collection, State::SPLIT);
        }

        self.add(collection);
    }

    /// Creates a comma-separated [ListPiece] for [nodes].
    pub fn create_comma_separated(&mut self, nodes: &[NodeId]) -> PieceId {
        let mut builder = DelimitedListBuilder::new(ListStyle::with_commas(Commas::NonTrailing));
        for &node in nodes {
            builder.visit(self, node);
        }
        builder.build(self)
    }

    /// Writes the leading keyword and parenthesized expression at the
    /// beginning of an `if`, `while`, or `switch` expression or statement.
    pub fn write_control_flow_start(
        &mut self,
        keyword: TokenId,
        left_parenthesis: TokenId,
        value: NodeId,
        right_parenthesis: TokenId,
    ) {
        self.token(keyword);
        self.space();
        self.token(left_parenthesis);
        self.visit(value);
        self.token(right_parenthesis);
    }

    /// Creates a [Piece] for an enum constant.
    ///
    /// If [comma_after] is `true`, then a comma after the constant is
    /// written, if present. If the constant is the last constant in an enum
    /// declaration that also declares members (and preserve trailing commas
    /// is off), then [semicolon] is the `;` token before the members and
    /// will be written after the constant.
    pub fn create_enum_constant(
        &mut self,
        node: Id<EnumConstantDeclaration>,
        comma_after: bool,
        semicolon: Option<TokenId>,
    ) -> PieceId {
        let ast = self.ast;
        let constant = &ast[node];
        self.build_with_metadata(ast.list_raw(constant.metadata), false, |v| {
            v.token(constant.name);
            if let Some(arguments) = constant.arguments {
                let arguments = &ast[arguments];
                v.visit(arguments.type_arguments);
                v.visit(arguments.constructor_selector);
                v.visit(arguments.argument_list);
            }

            if comma_after {
                v.token(comma_after_token(ast, node.raw()));
            } else if let Some(semicolon) = semicolon {
                // Discard the trailing comma if there is one since there is a
                // semicolon to use as the separator, but preserve any
                // comments before the discarded comma.
                let piece =
                    v.token_piece_with(semicolon, comma_after_token(ast, node.raw()), false, false);
                v.add(piece);
            }
        })
    }

    /// Writes a piece for a for statement or element.
    #[allow(clippy::too_many_arguments)]
    pub fn write_for(
        &mut self,
        await_keyword: Option<TokenId>,
        for_keyword: TokenId,
        left_parenthesis: TokenId,
        for_loop_parts: Id<ForLoopParts>,
        right_parenthesis: TokenId,
        body: NodeId,
        has_block_body: bool,
        force_split_body: bool,
    ) {
        let ast = self.ast;
        let for_keyword_piece = self.build(|v| {
            v.modifier(await_keyword);
            v.token(for_keyword);
        });

        let parts_node = for_loop_parts.raw();
        let for_parts_piece = match ast.kind(parts_node) {
            NodeKind::ForPartsWithExpression
            | NodeKind::ForPartsWithDeclarations
            | NodeKind::ForPartsWithPattern => {
                let (initializer, left_separator, condition, right_separator, updaters) =
                    for_parts(ast, parts_node);

                // Edge case: A totally empty for loop is formatted just as
                // `(;;)` with no splits or spaces anywhere.
                if ast.kind(parts_node) == NodeKind::ForPartsWithExpression
                    && initializer.is_none()
                    && !has_preceding_comments(ast, left_separator)
                    && condition.is_none()
                    && !has_preceding_comments(ast, right_separator)
                    && updaters.is_empty()
                    && !has_preceding_comments(ast, right_parenthesis)
                {
                    self.build(|v| {
                        v.token(left_parenthesis);
                        v.token(left_separator);
                        v.token(right_separator);
                        v.token(right_parenthesis);
                    })
                } else {
                    // In a C-style for loop, treat the for loop parts like an
                    // argument list where each clause is a separate argument.
                    // This means that when they split, they split like:
                    //
                    //     for (
                    //       initializerClause;
                    //       conditionClause;
                    //       incrementClause
                    //     ) {
                    //       body;
                    //     }
                    let mut parts_list =
                        DelimitedListBuilder::new(ListStyle::with_commas(Commas::None));
                    parts_list.left_bracket(self, left_parenthesis);

                    // The initializer clause.
                    if let Some(initializer) = initializer {
                        parts_list.add_comments_before(self, ast.begin_token(initializer));
                        let piece = self.build(|v| {
                            v.visit_in(initializer, NodeContext::ForLoopVariable);
                            v.token(left_separator);
                        });
                        parts_list.add(self, piece);
                    } else {
                        // No initializer, so look at the comments before `;`.
                        parts_list.add_comments_before(self, left_separator);
                        let piece = self.token_piece(left_separator);
                        parts_list.add(self, piece);
                    }

                    // The condition clause.
                    if let Some(condition_expression) = condition {
                        parts_list.add_comments_before(self, ast.begin_token(condition_expression));
                        let piece = self.build(|v| {
                            v.visit(condition_expression);
                            v.token(right_separator);
                        });
                        parts_list.add(self, piece);
                    } else {
                        parts_list.add_comments_before(self, right_separator);
                        let piece = self.token_piece(right_separator);
                        parts_list.add(self, piece);
                    }

                    // The update clauses.
                    if !updaters.is_empty() {
                        parts_list.add_comments_before(self, ast.begin_token(updaters[0]));

                        // Unlike most places in the language, if the updaters
                        // split, we don't want to add a trailing comma. But if
                        // the user has preserve trailing commas on, we should
                        // preserve the comma if there is one but not add one
                        // if there isn't and it splits.
                        let mut list_style = ListStyle::with_commas(Commas::NonTrailing);
                        if self.style.preserve_trailing_comma_after_for_updaters()
                            && has_comma_before(ast, right_parenthesis)
                        {
                            list_style = ListStyle::with_commas(Commas::Trailing);
                        }

                        // Create a nested list builder for the updaters so
                        // that they can remain unsplit even while the clauses
                        // split.
                        let mut updater_builder = DelimitedListBuilder::new(list_style);

                        for &updater in updaters {
                            updater_builder.visit(self, updater);
                        }

                        // Add the updater builder to the clause builder so
                        // that any comments around a trailing comma after the
                        // updaters don't get dropped.
                        let force_split = self
                            .style
                            .preserve_trailing_comma_before(ast, right_parenthesis);
                        parts_list.add_inner_builder(self, updater_builder, force_split);
                    }

                    parts_list.right_bracket(self, right_parenthesis, None, None);
                    parts_list.build(self)
                }
            }

            NodeKind::ForEachPartsWithDeclaration => {
                let parts = &ast[ast.cast::<ForEachPartsWithDeclaration>(parts_node).unwrap()];
                self.build(|v| {
                    v.token(left_parenthesis);
                    v.write_declared_for_in(parts.loop_variable, parts.in_keyword, parts.iterable);
                    v.token(right_parenthesis);
                })
            }

            NodeKind::ForEachPartsWithIdentifier => {
                let parts = &ast[ast.cast::<ForEachPartsWithIdentifier>(parts_node).unwrap()];
                // If a for-in loop, treat the for parts like an assignment, so
                // they split like:
                //
                //     for (var variable in [
                //       initializer,
                //     ]) {
                //       body;
                //     }
                self.build(|v| {
                    v.token(left_parenthesis);
                    v.write_for_in(parts.identifier.raw(), parts.in_keyword, parts.iterable);
                    v.token(right_parenthesis);
                })
            }

            NodeKind::ForEachPartsWithPattern => {
                let parts = &ast[ast.cast::<ForEachPartsWithPattern>(parts_node).unwrap()];
                self.build(|v| {
                    v.token(left_parenthesis);

                    // Hoist any leading comments so they don't force the
                    // for-in clauses to split.
                    let metadata = ast.list_raw(parts.metadata);
                    let first = match metadata.first() {
                        Some(&annotation) => ast.begin_token(annotation),
                        None => parts.keyword,
                    };
                    v.hoist_leading_comments(first, |v| {
                        // Use a nested piece so that the metadata precedes
                        // the keyword and not the `(`.
                        v.build_with_metadata(metadata, true, |v| {
                            v.token(parts.keyword);
                            v.space();
                            v.write_for_in(parts.pattern.raw(), parts.in_keyword, parts.iterable);
                        })
                    });

                    v.token(right_parenthesis);
                })
            }

            _ => unreachable!("unexpected for loop parts"),
        };

        let body_piece = self.node_piece(body);

        // If there is metadata before the for loop variable or pattern, then
        // make sure that the entire contents of the for loop parts are
        // indented so that the annotations are indented.
        let indent_header = match ast.kind(parts_node) {
            NodeKind::ForEachPartsWithDeclaration => {
                let parts = &ast[ast.cast::<ForEachPartsWithDeclaration>(parts_node).unwrap()];
                !ast[parts.loop_variable].metadata.is_empty()
            }
            NodeKind::ForEachPartsWithPattern => {
                let parts = &ast[ast.cast::<ForEachPartsWithPattern>(parts_node).unwrap()];
                !parts.metadata.is_empty()
            }
            _ => false,
        };

        if has_block_body {
            let piece = self.arena.add(ForPiece::new(
                for_keyword_piece,
                for_parts_piece,
                indent_header,
            ));
            self.add(piece);
            self.space();
            self.add(body_piece);
        } else {
            let header = self.arena.add(ForPiece::new(
                for_keyword_piece,
                for_parts_piece,
                indent_header,
            ));
            let mut for_piece = ControlFlowPiece::new(true);
            for_piece.add(header, body_piece, false);
            let for_piece = self.arena.add(for_piece);

            if force_split_body {
                self.arena.pin(for_piece, State::SPLIT);
            }
            self.add(for_piece);
        }
    }

    /// Writes a normal (not function-typed) formal parameter with a name
    /// and/or type annotation.
    ///
    /// If [mutable_keyword] is given, it should be the `var` or `final`
    /// keyword. If [field_keyword] and [period] are given, the former should
    /// be the `this` or `super` keyword for an initializing formal or super
    /// parameter.
    pub fn write_formal_parameter(
        &mut self,
        node: NodeId,
        ty: Option<Id<TypeAnnotation>>,
        name: Option<TokenId>,
        mutable_keyword: Option<TokenId>,
        field_keyword: Option<TokenId>,
        period: Option<TokenId>,
    ) {
        let ast = self.ast;
        let parts = formal_parameter_parts(ast, node).unwrap();
        let default_value_record = parts.default_clause.map(|default_clause| {
            let clause = &ast[default_clause];
            (clause.separator, clause.value.raw())
        });

        self.write_parameter(
            ty,
            name,
            ast.list_raw(parts.metadata),
            &[
                parts.required_keyword,
                parts.covariant_keyword,
                mutable_keyword,
            ],
            field_keyword,
            period,
            default_value_record,
        );
    }

    /// Writes a function, method, getter, or setter declaration.
    ///
    /// If [modifiers] are given, they should be the `external`, `static` or
    /// `abstract` modifiers. If [operator_keyword] is given, it should be the
    /// `operator` keyword on an operator declaration. If [property_keyword]
    /// is given, it should be the `get` or `set` keyword on a getter or
    /// setter declaration.
    #[allow(clippy::too_many_arguments)]
    pub fn write_function(
        &mut self,
        metadata: &[NodeId],
        modifiers: &[Option<TokenId>],
        return_type: Option<Id<TypeAnnotation>>,
        operator_keyword: Option<TokenId>,
        property_keyword: Option<TokenId>,
        name: Option<TokenId>,
        type_parameters: Option<Id<TypeParameterList>>,
        parameters: Option<Id<FormalParameterList>>,
        body: Id<FunctionBody>,
    ) {
        // Create a piece to attach metadata to the function.
        self.with_metadata(metadata, false, |v| {
            v.write_function_and_return_type(modifiers, return_type, |v| {
                // If there's no return type, attach modifiers to the
                // signature.
                if return_type.is_none() {
                    for &keyword in modifiers {
                        v.modifier(keyword);
                    }
                }

                v.modifier(operator_keyword);
                v.modifier(property_keyword);
                v.token(name);
                v.visit(type_parameters);
                v.visit(parameters);
                v.visit(body);
            });
        });
    }

    /// Writes a return type followed by either a function signature (when
    /// writing a function type annotation or function-typed formal) or a
    /// signature and a body (when writing a function declaration).
    ///
    /// The [write_function] callback should write the function's signature
    /// and body if there is one.
    ///
    /// If there is no return type, invokes [write_function] directly and
    /// returns. Otherwise, writes the return type and function and wraps
    /// them in a piece to allow splitting after the return type.
    pub fn write_function_and_return_type(
        &mut self,
        modifiers: &[Option<TokenId>],
        return_type: Option<Id<TypeAnnotation>>,
        write_function: impl FnOnce(&mut Self),
    ) {
        let Some(return_type) = return_type else {
            write_function(self);
            return;
        };

        // Hoist any comments before the function so they don't force a split
        // between the return type and function. In most cases, this doesn't
        // matter because the [SequenceBuilder] for the surrounding code will
        // separate out the leading comment. But if there is a metadata
        // annotation followed by a comment, then the function, then the
        // comment doesn't get captured by the [SequenceBuilder], as in:
        //
        //     @meta
        //     // Weird place for comment.
        //     int f() {}
        let ast = self.ast;
        let first_token = modifiers
            .iter()
            .flatten()
            .next()
            .copied()
            .unwrap_or_else(|| first_non_comment_token(ast, return_type.raw()));
        self.hoist_leading_comments(first_token, |v| {
            let return_type_piece = v.build(|v| {
                for &keyword in modifiers {
                    v.modifier(keyword);
                }

                v.visit(return_type);
            });

            let signature = v.build(|v| {
                write_function(v);
            });

            let is_3_dot_7 = v.style.is_3_dot_7();
            v.arena.add(VariablePiece::new(
                return_type_piece,
                vec![signature],
                true,
                is_3_dot_7,
            ))
        });
    }

    /// If [parameter] has a [default_value] then writes a piece for the
    /// parameter followed by that default value.
    ///
    /// Otherwise, just writes [parameter].
    pub fn write_default_value(
        &mut self,
        parameter: PieceId,
        default_value: Option<(TokenId, NodeId)>,
    ) {
        let Some((separator, value)) = default_value else {
            self.add(parameter);
            return;
        };

        let ast = self.ast;
        let is_3_dot_7 = self.style.is_3_dot_7();
        let operator_piece = self.build(|v| {
            if !is_3_dot_7 {
                v.add(parameter);
            }
            if ast.tokens.ty(separator) == TokenType::EQ {
                v.space();
            }
            v.token(separator);
            if ast.tokens.ty(separator) != TokenType::EQ {
                v.space();
            }
        });

        let value_piece = self.node_piece_with(value, false, NodeContext::Assignment);

        if is_3_dot_7 {
            let can_block_split_right = node_can_block_split(ast, value);
            let piece = self.arena.add(AssignPiece3Dot7::new(
                operator_piece,
                value_piece,
                Some(parameter),
                false,
                can_block_split_right,
                false,
            ));
            self.add(piece);
        } else {
            let piece = self
                .arena
                .add(AssignPiece::new(operator_piece, value_piece));
            self.add(piece);
        }
    }

    /// Writes a function type or function-typed formal.
    ///
    /// If creating a piece for a function-typed formal, then [parameter] is
    /// the formal parameter. If there is a default value, then
    /// [default_value] is the `=` or `:` separator followed by the constant
    /// expression.
    ///
    /// If this is a function-typed initializing formal (`this.foo()`), then
    /// [field_keyword] is `this` and [period] is the `.`. Likewise, for a
    /// function-typed super parameter, [field_keyword] is `super`.
    #[allow(clippy::too_many_arguments)]
    pub fn write_function_type(
        &mut self,
        return_type: Option<Id<TypeAnnotation>>,
        function_keyword_or_name: TokenId,
        type_parameters: Option<Id<TypeParameterList>>,
        parameters: Id<FormalParameterList>,
        question: Option<TokenId>,
        parameter: Option<NodeId>,
        field_keyword: Option<TokenId>,
        period: Option<TokenId>,
    ) {
        let ast = self.ast;
        let parts = parameter.and_then(|p| formal_parameter_parts(ast, p));
        let metadata: &[NodeId] = match &parts {
            Some(parts) => ast.list_raw(parts.metadata),
            None => &[],
        };
        self.with_metadata(metadata, true, |v| {
            let modifiers = [
                parts.as_ref().and_then(|p| p.required_keyword),
                parts.as_ref().and_then(|p| p.covariant_keyword),
                parts.as_ref().and_then(|p| p.const_final_or_var_keyword),
            ];

            let write = |v: &mut Self| {
                // If there's no return type, attach the parameter modifiers
                // to the signature.
                if return_type.is_none() {
                    for &modifier in &modifiers {
                        v.modifier(modifier);
                    }
                }

                v.token(field_keyword);
                v.token(period);
                v.token(function_keyword_or_name);
                v.visit(type_parameters);
                v.visit(parameters);
                v.token(question);
            };

            if let Some(default_clause) = parts.as_ref().and_then(|p| p.default_clause) {
                let function = v.build(|v| {
                    v.write_function_and_return_type(&modifiers, return_type, write);
                });

                let clause = &ast[default_clause];
                v.write_default_value(function, Some((clause.separator, clause.value.raw())));
            } else {
                v.write_function_and_return_type(&modifiers, return_type, write);
            }
        });
    }

    /// Writes a parenthesized expression or pattern.
    pub fn write_parenthesized(
        &mut self,
        left_bracket: TokenId,
        content: NodeId,
        right_bracket: TokenId,
    ) {
        self.token(left_bracket);
        self.visit(content);
        self.token(right_bracket);
    }

    /// Writes a piece for the header -- everything from the `if` keyword to
    /// the closing `)` -- of an if statement, if element, if-case statement,
    /// or if-case element.
    pub fn write_if_condition(
        &mut self,
        if_keyword: TokenId,
        left_parenthesis: TokenId,
        expression: Id<Expression>,
        case_clause: Option<Id<CaseClause>>,
        right_parenthesis: TokenId,
    ) {
        let ast = self.ast;
        self.token(if_keyword);
        self.space();
        self.token(left_parenthesis);

        if let Some(case_clause) = case_clause {
            let expression_piece = self.node_piece(expression);

            let case_clause = &ast[case_clause];
            let guarded_pattern = &ast[case_clause.guarded_pattern];
            let case_piece = self.build(|v| {
                v.token(case_clause.case_keyword);
                v.space();
                v.visit(guarded_pattern.pattern);
            });

            let guard_piece = self.optional_node_piece(guarded_pattern.when_clause);

            let piece = self.arena.add(IfCasePiece::new(
                expression_piece,
                case_piece,
                guard_piece,
                pattern_can_block_split(ast, guarded_pattern.pattern.raw()),
                self.style.block_format_if_case_with_guard(),
            ));
            self.add(piece);
        } else {
            self.visit(expression);
        }

        self.token(right_parenthesis);
    }

    /// Writes a [TryPiece] for try statement.
    pub fn write_try(&mut self, try_statement: Id<TryStatement>) {
        let ast = self.ast;
        let try_statement = &ast[try_statement];
        self.token(try_statement.try_keyword);
        self.space();
        self.write_block(try_statement.body, false);

        let catch_clauses = ast.list(try_statement.catch_clauses);
        for (i, &catch_clause) in catch_clauses.iter().enumerate() {
            let catch_clause = &ast[catch_clause];

            self.space();
            if let Some(on_keyword) = catch_clause.on_keyword {
                self.token_with(on_keyword, false, true, false);
                self.visit(catch_clause.exception_type);
            }

            if catch_clause.on_keyword.is_some() && catch_clause.catch_keyword.is_some() {
                self.space();
            }

            if let Some(catch_keyword) = catch_clause.catch_keyword {
                self.token(catch_keyword);
                self.space();

                let mut parameters =
                    DelimitedListBuilder::new(ListStyle::with_commas(Commas::NonTrailing));
                parameters.left_bracket(self, catch_clause.left_parenthesis.unwrap());
                if let Some(exception_parameter) = catch_clause.exception_parameter {
                    parameters.visit(self, exception_parameter.raw());
                }
                if let Some(stack_trace_parameter) = catch_clause.stack_trace_parameter {
                    parameters.visit(self, stack_trace_parameter.raw());
                }
                parameters.right_bracket(self, catch_clause.right_parenthesis.unwrap(), None, None);
                let piece = parameters.build(self);
                self.add(piece);
            }

            self.space();

            // Edge case: When there's another catch/on/finally after this
            // one, we want to force the block to split even if it's empty.
            //
            //     try {
            //       ..
            //     } on Foo {
            //     } finally Bar {
            //       body;
            //     }
            let force_split = i < catch_clauses.len() - 1 || try_statement.finally_block.is_some();
            self.write_block(catch_clause.body, force_split);
        }

        if let Some(finally_block) = try_statement.finally_block {
            self.space();
            self.token(try_statement.finally_keyword.unwrap());
            self.space();
            self.write_block(finally_block, false);
        }
    }

    /// Writes an [ImportPiece] for an import or export directive.
    #[allow(clippy::too_many_arguments)]
    pub fn write_import(
        &mut self,
        metadata: NodeList<Annotation>,
        uri: Id<StringLiteral>,
        configurations: NodeList<Configuration>,
        combinators: NodeList<Combinator>,
        semicolon: TokenId,
        keyword: TokenId,
        deferred_keyword: Option<TokenId>,
        as_keyword: Option<TokenId>,
        prefix: Option<Id<SimpleIdentifier>>,
    ) {
        let ast = self.ast;
        self.with_metadata(ast.list_raw(metadata), false, |v| {
            // Build a piece for the directive itself.
            let directive_piece = v.build(|v| {
                v.token(keyword);
                v.space();
                v.visit(uri);
            });

            // Add all of the clauses and combinators.
            let mut clauses = Vec::new();

            // Include any `if` clauses.
            for &configuration in ast.list_raw(configurations) {
                clauses.push(v.node_piece(configuration));
            }

            // Include the `as` clause.
            if let Some(as_keyword) = as_keyword {
                let piece = v.build(|v| {
                    v.token_with(deferred_keyword, false, true, false);
                    v.token(as_keyword);
                    v.space();
                    v.visit(prefix.unwrap());
                });
                clauses.push(piece);
            }

            // Include the `show` and `hide` clauses.
            for &combinator_node in ast.list_raw(combinators) {
                let (keyword, names) =
                    if let Some(hide) = ast.cast::<HideCombinator>(combinator_node) {
                        let hide = &ast[hide];
                        (hide.keyword, ast.list(hide.hidden_names))
                    } else {
                        let show = &ast[ast.cast::<ShowCombinator>(combinator_node).unwrap()];
                        (show.keyword, ast.list(show.shown_names))
                    };
                let mut operands = vec![v.token_piece(keyword)];
                for &name in names {
                    operands.push(v.token_piece_with(ast[name].token, None, true, false));
                }
                let is_3_dot_7 = v.style.is_3_dot_7();
                clauses.push(v.arena.add(InfixPiece::new(
                    operands,
                    is_3_dot_7,
                    Indent::Expression,
                )));
            }

            // If there are clauses, include them.
            if !clauses.is_empty() {
                let piece = v
                    .arena
                    .add(ClausePiece::new(directive_piece, clauses, false));
                v.add(piece);
            } else {
                v.add(directive_piece);
            }

            v.token(semicolon);
        });
    }

    /// Writes a [Piece] for an index expression.
    pub fn write_index_expression(&mut self, index: Id<IndexExpression>) {
        let ast = self.ast;
        let index = &ast[index];
        // TODO(rnystrom): Consider whether we should allow splitting between
        // successive index expressions, like:
        //
        //     jsonData['some long key']
        //         ['another long key'];
        //
        // The current formatter allows it, but it's very rarely used (0.021%
        // of index expressions in a corpus of pub packages).
        self.token(index.question);
        self.token(index.period);

        if self.style.is_3_dot_7() {
            self.token(index.left_bracket);
            self.visit(index.index);
            self.token(index.right_bracket);
        } else {
            // Wrap the index expression in a [GroupingPiece] so that a split
            // inside the index doesn't cause the surrounding piece to have a
            // certain shape.
            let content = self.build(|v| {
                v.token(index.left_bracket);
                v.visit(index.index);
                v.token(index.right_bracket);
            });
            let piece = self.arena.add(GroupingPiece::new(content));
            self.add(piece);
        }
    }

    /// Writes a single infix operation.
    ///
    /// If [hanging] is `true` then the operator goes at the end of the first
    /// line, like `+`. Otherwise, it goes at the beginning of the second,
    /// like `as`.
    ///
    /// The [operator2] parameter may be passed if the "operator" is actually
    /// two separate tokens, as in `foo is! Bar`.
    pub fn write_infix(
        &mut self,
        left: NodeId,
        operator: TokenId,
        right: NodeId,
        hanging: bool,
        operator2: Option<TokenId>,
        indent: Indent,
    ) {
        let left_piece = self.build(|v| {
            v.visit(left);
            if hanging {
                v.space();
                v.token(operator);
                v.token(operator2);
            }
        });

        let right_piece = self.build(|v| {
            if !hanging {
                v.token(operator);
                v.token(operator2);
                v.space();
            }

            v.visit(right);
        });

        let is_3_dot_7 = self.style.is_3_dot_7();
        let piece = self.arena.add(InfixPiece::new(
            vec![left_piece, right_piece],
            is_3_dot_7,
            indent,
        ));
        self.add(piece);
    }

    /// Writes a chained infix operation: a binary operator expression, or
    /// binary pattern.
    ///
    /// In a tree of binary AST nodes, all operators at the same precedence
    /// are treated as a single chain of operators that either all split or
    /// none do. Operands within those (which may themselves be chains of
    /// higher precedence binary operators) are then formatted independently.
    ///
    /// [destructure] takes a node and yields the operands and operator if
    /// the node is of the type being visited (Dart `T`), or `None`. We need
    /// this since there's no interface shared by the various binary operator
    /// AST nodes.
    ///
    /// If [precedence] is given, then this only flattens binary nodes with
    /// that same precedence.
    pub fn write_infix_chain(
        &mut self,
        node: NodeId,
        destructure: fn(&Ast, NodeId) -> Option<BinaryOperation>,
        precedence: Option<u8>,
        indent: bool,
    ) {
        let ast = self.ast;
        let mut operands = Vec::new();

        fn traverse(
            v: &mut AstNodeVisitor<'_>,
            e: NodeId,
            destructure: fn(&Ast, NodeId) -> Option<BinaryOperation>,
            precedence: Option<u8>,
            operands: &mut Vec<PieceId>,
        ) {
            let ast = v.ast;
            // If the node is one if our infix operators, then recurse into the
            // operands.
            if let Some((left, operator, right)) = destructure(ast, e) {
                if precedence.is_none_or(|p| ast.tokens.ty(operator).precedence() == p) {
                    let operand = v.build(|v| {
                        traverse(v, left, destructure, precedence, operands);
                        v.space();
                        v.token(operator);
                    });
                    operands.push(operand);

                    traverse(v, right, destructure, precedence, operands);
                    return;
                }
            }

            // Otherwise, just write the node itself.
            v.visit(e);
        }

        let _ = ast;
        let last = self.build(|v| {
            traverse(v, node, destructure, precedence, &mut operands);
        });
        operands.push(last);

        let is_3_dot_7 = self.style.is_3_dot_7();
        let piece = self.arena.add(InfixPiece::new(
            operands,
            is_3_dot_7,
            if indent { Indent::Infix } else { Indent::None },
        ));
        self.add(piece);
    }

    /// Writes a [ListPiece] for the given bracket-delimited set of elements.
    ///
    /// If [preserve_newlines] is `true`, then any newlines or lack of
    /// newlines between pairs of elements in the input are preserved in the
    /// output. This is used for collection literals that contain line
    /// comments to preserve the author's deliberate structuring, as in:
    ///
    ///     matrix = [
    ///       1, 2, 3, //
    ///       4, 5, 6,
    ///       7, 8, 9,
    ///     ];
    #[allow(clippy::too_many_arguments)]
    pub fn write_list(
        &mut self,
        elements: &[NodeId],
        left_bracket: TokenId,
        right_bracket: TokenId,
        style: ListStyle,
        preserve_newlines: bool,
        allow_block_argument: bool,
        block_shaped: bool,
    ) {
        let ast = self.ast;

        // If the list is completely empty, write the brackets directly inline
        // so that we create fewer pieces.
        if !nodes_can_split(ast, elements, right_bracket) {
            self.token(left_bracket);
            self.token(right_bracket);
            return;
        }

        let mut builder = DelimitedListBuilder::new(style);

        builder.left_bracket(self, left_bracket);

        if preserve_newlines && contains_line_comments(ast, elements, Some(right_bracket)) {
            self.preserve_newlines_in_collection(elements, &mut builder);
        } else {
            builder.visit_all(self, elements, allow_block_argument);
        }

        builder.right_bracket(self, right_bracket, None, None);
        // If we are always writing a trailing comma (because it's a
        // single-element record), then the comma shouldn't force a split.
        let force_split = style.commas != Commas::AlwaysTrailing
            && self
                .style
                .preserve_trailing_comma_before(ast, right_bracket);
        let piece = builder.build_with(self, force_split, block_shaped);
        self.add(piece);
    }

    /// Writes [elements] into [builder], preserving the original newlines (or
    /// lack thereof) between elements.
    ///
    /// This is used for formatting collection literals that contain at least
    /// one line comment between elements. In that case, we use the line
    /// comment as a single to prefer the author's chosen newlines between
    /// elements. For example, if the user writes:
    ///
    ///     list = [
    ///       1,2,   3, 4,
    ///       // comment
    ///       5,6,    7
    ///     ];
    ///
    /// The formatter produces:
    ///
    ///     list = [
    ///       1, 2, 3, 4,
    ///       // comment
    ///       5, 6, 7
    ///     ];
    fn preserve_newlines_in_collection(
        &mut self,
        elements: &[NodeId],
        builder: &mut DelimitedListBuilder<'a>,
    ) {
        let ast = self.ast;
        // Builder for all of the elements on a single line. We use a
        // ListPiece for this too because even though we prefer to keep all
        // elements that are on a single line in the input also on a single
        // line in the output, we will split them if they don't fit.
        let line_style = ListStyle::with_commas(Commas::NonTrailing);
        let mut line_builder = DelimitedListBuilder::new(line_style);
        let mut at_line_start = true;

        for (i, &element) in elements.iter().enumerate() {
            if !at_line_start
                && self
                    .comments
                    .has_newline_between(ast.end_token(elements[i - 1]), ast.begin_token(element))
            {
                // This element begins a new line. Add the elements on the
                // previous line to the list builder and start a new line.
                let finished =
                    std::mem::replace(&mut line_builder, DelimitedListBuilder::new(line_style));
                builder.add_inner_builder(self, finished, false);
                at_line_start = true;
            }

            // Let the main list builder handle comments that occur between
            // elements that aren't on the same line.
            if at_line_start {
                builder.add_comments_before(self, ast.begin_token(element));
            }

            line_builder.visit(self, element);

            // There is an element on this line now.
            at_line_start = false;
        }

        // Finish the last line if there is anything on it.
        if !at_line_start {
            builder.add_inner_builder(self, line_builder, false);
        }
    }

    /// Writes a [VariablePiece] for a named or wildcard variable pattern.
    pub fn write_pattern_variable(
        &mut self,
        keyword: Option<TokenId>,
        ty: Option<Id<TypeAnnotation>>,
        name: TokenId,
    ) {
        // If it's a wildcard with no declaration keyword or type, there is
        // just a name token.
        if keyword.is_none() && ty.is_none() {
            self.token(name);
            return;
        }

        let header = self.build(|v| {
            v.modifier(keyword);
            v.visit(ty);
        });

        let name_piece = self.token_piece(name);
        let is_3_dot_7 = self.style.is_3_dot_7();
        let piece = self.arena.add(VariablePiece::new(
            header,
            vec![name_piece],
            ty.is_some(),
            is_3_dot_7,
        ));
        self.add(piece);
    }

    /// Writes a [Piece] for an AST node followed by an unsplittable token.
    pub fn write_postfix(&mut self, node: NodeId, operator: TokenId) {
        self.visit(node);
        self.token(operator);
    }

    /// Writes a [Piece] for an AST node preceded by an unsplittable token.
    ///
    /// If [space] is `true` and there is an operator, writes a space between
    /// the operator and operand.
    pub fn write_prefix(&mut self, operator: Option<TokenId>, operand: impl OptNode, space: bool) {
        let operand = operand.opt_node();
        if self.style.is_3_dot_7() {
            self.token_with(operator, false, space, false);
            self.visit(operand);
            return;
        }

        // Wrap in grouping so that an infix split inside the prefix operator
        // doesn't get collapsed in the surrounding context, like:
        //
        //     // Wrong:
        //     var x =
        //         throw 'Some long string '
        //         'wrapped.';
        //
        //     // Right:
        //     var x =
        //         throw 'Some long string '
        //             'wrapped.';
        let content = self.build(|v| {
            v.token_with(operator, false, space, false);
            v.visit(operand);
        });
        let piece = self.arena.add(PrefixPiece::new(content));
        self.add(piece);
    }

    /// Writes an [AdjacentPiece] for a given record type field.
    pub fn write_record_type_field(
        &mut self,
        metadata: NodeList<Annotation>,
        ty: Id<TypeAnnotation>,
        name: Option<TokenId>,
    ) {
        let ast = self.ast;
        self.write_parameter(
            Some(ty),
            name,
            ast.list_raw(metadata),
            &[],
            None,
            None,
            None,
        );
    }

    /// Writes a [ListPiece] for a record literal or pattern.
    pub fn write_record(
        &mut self,
        left_parenthesis: TokenId,
        fields: &[NodeId],
        right_parenthesis: TokenId,
        const_keyword: Option<TokenId>,
        preserve_newlines: bool,
    ) {
        let ast = self.ast;
        let style = match fields {
            // Record types or patterns with a single named field don't add a
            // trailing comma unless it's split, like:
            //
            //     ({int n}) x;
            //
            // Or:
            //
            //     if (obj case (name: value)) {
            //       ;
            //     }
            [field]
                if ast
                    .cast::<PatternField>(*field)
                    .is_some_and(|f| ast[f].name.is_some()) =>
            {
                ListStyle::with_commas(Commas::Trailing)
            }
            [field] if ast.kind(*field) == NodeKind::RecordLiteralNamedField => {
                ListStyle::with_commas(Commas::Trailing)
            }

            // Record types or patterns with a single positional field always
            // have a trailing comma to disambiguate from parenthesized
            // expressions or patterns, like:
            //
            //     (int,) x;
            //
            // Or:
            //
            //     if (obj case (pattern,)) {
            //       ;
            //     }
            [_] => ListStyle::with_commas(Commas::AlwaysTrailing),

            // Record types or patterns with multiple fields have regular
            // trailing commas when split.
            _ => ListStyle::with_commas(Commas::Trailing),
        };

        self.write_collection(
            left_parenthesis,
            fields,
            right_parenthesis,
            const_keyword,
            None,
            style,
            false,
            preserve_newlines,
        );
    }

    /// Writes a [ListPiece] for a type argument or type parameter list.
    pub fn write_type_list(
        &mut self,
        left_bracket: TokenId,
        elements: &[NodeId],
        right_bracket: TokenId,
    ) {
        self.write_list(
            elements,
            left_bracket,
            right_bracket,
            ListStyle {
                commas: Commas::NonTrailing,
                split_cost: 3,
                ..ListStyle::default()
            },
            false,
            false,
            false,
        );
    }

    /// Writes an `as`, `is`, or `is!` expression.
    pub fn write_type_test(
        &mut self,
        expression: Id<Expression>,
        keyword: TokenId,
        ty: Id<TypeAnnotation>,
        bang: Option<TokenId>,
    ) {
        let ast = self.ast;
        if self.style.block_format_type_test() {
            let expression_piece = self.node_piece(expression);

            let operator_piece = self.build(|v| {
                v.token(keyword);
                v.token(bang);
            });

            let type_piece = self.node_piece(ty);
            let piece = self.arena.add(TypeTestPiece::new(
                expression_piece,
                operator_piece,
                type_piece,
                can_block_split(ast, expression),
            ));
            self.add(piece);
        } else {
            let is_3_dot_7 = self.style.is_3_dot_7();
            self.write_infix(
                expression.raw(),
                keyword,
                ty.raw(),
                false,
                bang,
                // Don't use Indent.infix after 3.7 because it will flatten the
                // indentation if the expression occurs in an assignment.
                if is_3_dot_7 {
                    Indent::Infix
                } else {
                    Indent::Expression
                },
            );
        }
    }

    /// Handles the `async`, `sync*`, or `async*` modifiers on a function
    /// body.
    pub fn write_function_body_modifiers(
        &mut self,
        keyword: Option<TokenId>,
        star: Option<TokenId>,
    ) {
        // The `async` or `sync` keyword.
        self.token_soft(keyword);
        self.token_soft(star);

        // We treat these keywords as soft so that overly long test
        // descriptions in `test()` calls don't force the test call to split
        // and make it easier for the user to see they need to split the
        // string.

        if keyword.is_some() {
            self.space();
        }
    }

    /// Writes a [Piece] with "assignment-like" splitting.
    ///
    /// This is used, obviously, for assignments and variable declarations to
    /// handle splitting after the `=`, but is also used in any context where
    /// an expression follows something that it "defines" or "initializes":
    ///
    /// * Assignment
    /// * Variable declaration
    /// * Constructor initializer
    /// * Expression (`=>`) function body
    /// * Named argument or named record field (`:`)
    /// * Map entry (`:`)
    #[allow(clippy::too_many_arguments)]
    pub fn write_assignment(
        &mut self,
        left_hand_side: NodeId,
        operator: TokenId,
        right_hand_side: NodeId,
        include_comma: bool,
        left_hand_side_context: NodeContext,
        right_hand_side_context: NodeContext,
    ) {
        if self.style.is_3_dot_7() {
            self.write_assignment_3_dot_7(
                left_hand_side,
                operator,
                right_hand_side,
                include_comma,
                left_hand_side_context,
            );
            return;
        }

        let ast = self.ast;
        let left_piece = self.build(|v| {
            v.visit_in(left_hand_side, left_hand_side_context);
            if ast.tokens.ty(operator) != TokenType::COLON {
                v.space();
            }
            v.token(operator);
        });

        let right_piece =
            self.node_piece_with(right_hand_side, include_comma, right_hand_side_context);

        let piece = self.arena.add(AssignPiece::new(left_piece, right_piece));
        self.add(piece);
    }

    /// Writes an assignment-like construct with a token on the left-hand
    /// side.
    ///
    /// This is used instead of [write_assignment] when the left-hand side is
    /// a [Token] (like the name of a named argument or record field) rather
    /// than a full [AstNode].
    pub fn write_token_assignment(
        &mut self,
        left_hand_side: TokenId,
        operator: TokenId,
        right_hand_side: NodeId,
        include_comma: bool,
        right_hand_side_context: NodeContext,
    ) {
        let ast = self.ast;
        if self.style.is_3_dot_7() {
            let can_block_split_right = node_can_block_split(ast, right_hand_side);

            let operator_piece = self.build(|v| {
                if ast.tokens.ty(operator) != TokenType::COLON {
                    v.space();
                }
                v.token(operator);
            });

            let right_piece =
                self.node_piece_with(right_hand_side, include_comma, NodeContext::Assignment);

            let left = self.token_piece(left_hand_side);
            let piece = self.arena.add(AssignPiece3Dot7::new(
                operator_piece,
                right_piece,
                Some(left),
                false,
                can_block_split_right,
                false,
            ));
            self.add(piece);
            return;
        }

        let left_piece = self.build(|v| {
            v.token(left_hand_side);
            if ast.tokens.ty(operator) != TokenType::COLON {
                v.space();
            }
            v.token(operator);
        });

        let right_piece =
            self.node_piece_with(right_hand_side, include_comma, right_hand_side_context);

        let piece = self.arena.add(AssignPiece::new(left_piece, right_piece));
        self.add(piece);
    }

    /// Writes the `<variable> in <expression>` part of an identifier or
    /// pattern for-in loop.
    fn write_for_in(
        &mut self,
        left_hand_side: NodeId,
        in_keyword: TokenId,
        sequence: Id<Expression>,
    ) {
        let ast = self.ast;
        // Hoist any leading comments so they don't force the for-in clauses
        // to split.
        self.hoist_leading_comments(first_non_comment_token(ast, left_hand_side), |v| {
            let left_piece = v.node_piece_with(left_hand_side, false, NodeContext::ForLoopVariable);
            let sequence_piece = v.create_for_in_sequence(in_keyword, sequence);
            let is_3_dot_7 = v.style.is_3_dot_7();
            v.arena.add(ForInPiece::new(
                left_piece,
                sequence_piece,
                can_block_split(ast, sequence),
                is_3_dot_7,
            ))
        });
    }

    /// Writes the `<variable> in <expression>` part of a for-in loop when the
    /// part before `in` is a variable declaration.
    ///
    /// A for-in loop with a variable declaration can have metadata before
    /// it, which requires some special handling so that we don't push the
    /// metadata and any comments after it into the left child piece of
    /// [ForInPiece].
    fn write_declared_for_in(
        &mut self,
        identifier: Id<DeclaredIdentifier>,
        in_keyword: TokenId,
        sequence: Id<Expression>,
    ) {
        let ast = self.ast;
        let declared = &ast[identifier];
        // Hoist any leading comments so they don't force the for-in clauses
        // to split.
        self.hoist_leading_comments(ast.begin_token(identifier), |v| {
            // Use a nested piece so that the metadata precedes the keyword and
            // not the `(`.
            v.build_with_metadata(ast.list_raw(declared.metadata), true, |v| {
                let left_piece = v.build(|v| {
                    v.write_parameter(
                        declared.type_,
                        Some(declared.name),
                        &[],
                        &[declared.keyword],
                        None,
                        None,
                        None,
                    );
                });

                let sequence_piece = v.create_for_in_sequence(in_keyword, sequence);

                let is_3_dot_7 = v.style.is_3_dot_7();
                let piece = v.arena.add(ForInPiece::new(
                    left_piece,
                    sequence_piece,
                    can_block_split(ast, sequence),
                    is_3_dot_7,
                ));
                v.add(piece);
            })
        });
    }

    /// Creates a piece for the `in <sequence>` part of a for-in loop.
    fn create_for_in_sequence(&mut self, in_keyword: TokenId, sequence: Id<Expression>) -> PieceId {
        self.build(|v| {
            // Put the `in` at the beginning of the sequence.
            v.token(in_keyword);
            v.space();
            v.visit(sequence);
        })
    }

    fn write_assignment_3_dot_7(
        &mut self,
        left_hand_side: NodeId,
        operator: TokenId,
        right_hand_side: NodeId,
        include_comma: bool,
        left_hand_side_context: NodeContext,
    ) {
        let ast = self.ast;
        // If an operand can have block formatting, then a newline in it
        // doesn't force the operator to split, as in:
        //
        //    var [
        //      element,
        //    ] = list;
        //
        // Or:
        //
        //    var list = [
        //      element,
        //    ];
        let can_block_split_left = match ast.kind(left_hand_side) {
            // Treat method chains and cascades on the LHS as if they were
            // blocks. They don't really fit the "block" term, but it looks
            // much better to force a method chain to split on the left than to
            // try to avoid splitting it and split at the assignment instead:
            //
            //    // Worse:
            //    target.method(
            //          argument,
            //        ).setter =
            //        value;
            //
            //    // Better:
            //    target.method(argument)
            //        .setter = value;
            //
            NodeKind::MethodInvocation
            | NodeKind::PropertyAccess
            | NodeKind::PrefixedIdentifier => true,

            // Otherwise, it must be an actual block construct.
            _ => node_can_block_split(ast, left_hand_side),
        };

        let can_block_split_right = node_can_block_split(ast, right_hand_side);

        let left_piece = self.node_piece_with(left_hand_side, false, left_hand_side_context);

        let operator_piece = self.build(|v| {
            if ast.tokens.ty(operator) != TokenType::COLON {
                v.space();
            }
            v.token(operator);
        });

        let right_piece =
            self.node_piece_with(right_hand_side, include_comma, NodeContext::Assignment);

        let piece = self.arena.add(AssignPiece3Dot7::new(
            operator_piece,
            right_piece,
            Some(left_piece),
            can_block_split_left,
            can_block_split_right,
            false,
        ));
        self.add(piece);
    }

    /// Writes a piece for a parameter-like constructor: Either a simple
    /// formal parameter or a record type field, which is syntactically
    /// similar to a parameter.
    ///
    /// If the parameter has a default value, then [default_value] contains
    /// the `:` or `=` separator and the constant value expression.
    #[allow(clippy::too_many_arguments)]
    pub fn write_parameter(
        &mut self,
        ty: Option<Id<TypeAnnotation>>,
        name: Option<TokenId>,
        metadata: &[NodeId],
        modifiers: &[Option<TokenId>],
        field_keyword: Option<TokenId>,
        period: Option<TokenId>,
        default_value: Option<(TokenId, NodeId)>,
    ) {
        // Begin a piece to attach metadata to the parameter.
        self.with_metadata(metadata, true, |v| {
            let mut type_piece = None;
            if let Some(ty) = ty {
                type_piece = Some(v.build(|v| {
                    for &keyword in modifiers {
                        v.modifier(keyword);
                    }

                    v.visit(ty);
                }));
            }

            let mut name_piece = None;
            if let Some(name) = name {
                name_piece = Some(v.build(|v| {
                    // If there is a type annotation, the modifiers will be
                    // before the type. Otherwise, they go before the name.
                    if type_piece.is_none() {
                        for &keyword in modifiers {
                            v.modifier(keyword);
                        }
                    }

                    v.token(field_keyword);
                    v.token(period);
                    v.token(name);
                }));
            }

            let parameter_piece = match (type_piece, name_piece) {
                (Some(type_piece), Some(name_piece)) => {
                    // We have both a type and name, allow splitting between
                    // them.
                    let is_3_dot_7 = v.style.is_3_dot_7();
                    v.arena.add(VariablePiece::new(
                        type_piece,
                        vec![name_piece],
                        true,
                        is_3_dot_7,
                    ))
                }
                // Will have at least a type or name.
                (type_piece, name_piece) => type_piece.or(name_piece).unwrap(),
            };

            // If there's a default value, include it. We do that inside here
            // so that any metadata surrounds the entire assignment instead of
            // being part of the assignment's left-hand side where a split in
            // the metadata would force a split at the default value
            // separator.
            v.write_default_value(parameter_piece, default_value);
        });
    }

    /// Visits [node] and creates a piece from it.
    #[inline]
    pub fn node_piece(&mut self, node: impl OptNode) -> PieceId {
        self.node_piece_with(node.opt_node().unwrap(), false, NodeContext::None)
    }

    /// Visits [node] and creates a piece from it.
    ///
    /// If [comma_after] is `true`, looks for a comma token after [node] and
    /// writes it to the piece as well.
    pub fn node_piece_with(
        &mut self,
        node: impl OptNode,
        comma_after: bool,
        context: NodeContext,
    ) -> PieceId {
        let node = node.opt_node().unwrap();
        let mut result = self.build(|v| {
            v.visit_node(node, context);
        });

        if comma_after {
            let ast = self.ast;
            let next_token = ast.tokens.next(ast.end_token(node));
            if ast.tokens.lexeme(next_token) == "," {
                let comma = self.token_piece(next_token);
                result = self.arena.add(AdjacentPiece::new(vec![result, comma]));
            }
        }

        result
    }

    /// Visits [node] and creates a piece from it if not `None`.
    ///
    /// Otherwise returns `None`.
    pub fn optional_node_piece(&mut self, node: impl OptNode) -> Option<PieceId> {
        let node = node.opt_node()?;
        Some(self.node_piece(node))
    }

    /// Creates a piece for only [token].
    #[inline]
    pub fn token_piece(&mut self, token: TokenId) -> PieceId {
        self.token_piece_with(token, None, false, false)
    }
}

/// Dart `Iterable<AstNode>.canSplit` for a slice of nodes.
pub fn nodes_can_split(ast: &Ast, nodes: &[NodeId], right_bracket: TokenId) -> bool {
    !nodes.is_empty() || has_preceding_comments(ast, right_bracket)
}

/// Dart `AstNode.commaAfter` as an `Option<TokenId>`.
pub fn comma_after_token(ast: &Ast, node: NodeId) -> Option<TokenId> {
    comma_after(ast, node)
}

/// The fields of Dart `ForParts`: initializer (variables, initialization or
/// pattern variables), left separator, condition, right separator and
/// updaters.
fn for_parts(
    ast: &Ast,
    node: NodeId,
) -> (Option<NodeId>, TokenId, Option<NodeId>, TokenId, &[NodeId]) {
    if let Some(parts) = ast.cast::<ForPartsWithExpression>(node) {
        let parts = &ast[parts];
        return (
            parts.initialization.map(Id::raw),
            parts.left_separator,
            parts.condition.map(Id::raw),
            parts.right_separator,
            ast.list_raw(parts.updaters),
        );
    }
    if let Some(parts) = ast.cast::<ForPartsWithDeclarations>(node) {
        let parts = &ast[parts];
        return (
            Some(parts.variables.raw()),
            parts.left_separator,
            parts.condition.map(Id::raw),
            parts.right_separator,
            ast.list_raw(parts.updaters),
        );
    }
    let parts = &ast[ast.cast::<ForPartsWithPattern>(node).unwrap()];
    (
        Some(parts.variables.raw()),
        parts.left_separator,
        parts.condition.map(Id::raw),
        parts.right_separator,
        ast.list_raw(parts.updaters),
    )
}
