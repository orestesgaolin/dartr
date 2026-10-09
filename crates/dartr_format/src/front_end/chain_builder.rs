// Dart source: dart_style lib/src/front_end/chain_builder.dart

use dartr_ast::*;
use dartr_syntax::TokenType;

use crate::ast_extensions::{can_block_split, can_split, cascade_allow_inline, has_single_element, looks_like_static_call};
use crate::back_end::code_writer::Indent;
use crate::piece::{CallType, ChainCall, ChainPiece, PieceId, PieceKind, State};

use super::ast_node_visitor::AstNodeVisitor;
use super::piece_factory::NodeContext;

/// Creates [Chain] pieces from method calls and property accesses, along
/// with postfix operations (`!`, index operators, and function invocation
/// expressions) that follow them.
///
/// In the AST for method calls, selectors are nested bottom up such that
/// this expression:
///
///     obj.a(1)[2].c(3)
///
/// Is structured like:
///
///           .c()
///           /  \
///          []   3
///         /  \
///       .a()  2
///       /  \
///     obj   1
///
/// This means visiting the AST from top down visits the selectors from right
/// to left. It's easier to format if we organize them as a linear series of
/// selectors from left to right. Further, we want to organize it into a
/// two-tier hierarchy. We have an outer list of method calls and property
/// accesses. Then each of those may have one or more postfix selectors
/// attached: indexers, null-assertions, or invocations. This mirrors how
/// they are formatted.
///
/// This lets us create a single [ChainPiece] for the entire series of dotted
/// operations, so that we can control splitting them or not as a unit.
pub struct ChainBuilder {
    /// The outermost expression being converted to a chain.
    ///
    /// If it's a [CascadeExpression], then the chain is the cascade sections.
    /// Otherwise, it's some kind of method call or property access and the
    /// chain is the nested series of selector subexpressions.
    root: NodeId,

    /// The left-most target of the chain.
    target: Option<PieceId>,

    /// Whether the target expression may contain newlines when the chain is
    /// not fully split. (It may always contain newlines when the chain
    /// splits.)
    ///
    /// This is true for most expressions but false for delimited ones to
    /// avoid ugly formatting like:
    ///
    ///     function(
    ///       argument,
    ///     )
    ///         .method();
    allow_split_in_target: bool,

    /// Whether the target of the chain is a call or collection with a single
    /// argument or element (and we are in a style that treats that
    /// specially).
    has_single_element_target: bool,

    /// The dotted property accesses and method calls following the target.
    calls: Vec<ChainCall>,
}

impl ChainBuilder {
    pub fn new(v: &mut AstNodeVisitor<'_>, root: NodeId) -> ChainBuilder {
        let ast = v.ast;
        let mut builder = ChainBuilder {
            root,
            target: None,
            allow_split_in_target: false,
            has_single_element_target: false,
            calls: Vec::new(),
        };

        if let Some(cascade) = ast.cast::<CascadeExpression>(root) {
            let cascade = &ast[cascade];
            builder.visit_target(v, cascade.target.raw(), true);

            // When [root] is a cascade, the chain is the series of cascade
            // sections.
            for &section in ast.list_raw(cascade.cascade_sections) {
                let piece = v.node_piece(section);

                let call_type = if matches!(v.arena.kind(piece), PieceKind::LeadingComment(_)) {
                    // Force the cascade to split if there are leading comments
                    // before the cascade section to avoid:
                    //
                    //     target// comment
                    //     ..method(
                    //       argument,
                    //     );
                    CallType::UnsplittableCall
                } else if let Some(invocation) = ast.cast::<MethodInvocation>(section) {
                    let invocation = &ast[invocation];
                    if invocation.target.is_some() {
                        // If the section is itself a method chain, then force
                        // the cascade to split if the method does, as in:
                        //
                        //     cascadeTarget
                        //       ..methodTarget.method(
                        //         argument,
                        //       );
                        CallType::UnsplittableCall
                    } else {
                        // Otherwise, allow a direct method call in the cascade
                        // to not split the cascade if the arguments can split,
                        // as in:
                        //
                        //     cascadeTarget..method(
                        //       argument,
                        //     );
                        let arguments = &ast[invocation.argument_list];
                        if can_split(ast, arguments.arguments, arguments.right_parenthesis) {
                            CallType::SplittableCall
                        } else {
                            CallType::UnsplittableCall
                        }
                    }
                } else {
                    CallType::UnsplittableCall
                };

                builder.calls.push(ChainCall::new(piece, call_type));
            }
        } else {
            builder.unwrap_call(v, root);
        }

        builder
    }

    /// Builds a [ChainPiece] for a series of cascade sections.
    pub fn build_cascade(self, v: &mut AstNodeVisitor<'_>) -> PieceId {
        let ast = v.ast;
        // If there is only a single section and it can block split, allow
        // it:
        //
        //     target..cascade(
        //       argument,
        //     );
        let block_call_index = if self.calls.len() == 1 && self.calls[0].can_split() {
            0
        } else {
            -1
        };

        let is_3_dot_7 = v.style.is_3_dot_7();
        let chain = v.arena.add(ChainPiece::new(
            self.target.unwrap(),
            self.calls,
            true,
            0,
            block_call_index,
            Indent::Cascade,
            self.allow_split_in_target,
            self.has_single_element_target,
            is_3_dot_7,
        ));

        let cascade = ast.cast::<CascadeExpression>(self.root).unwrap();
        if !cascade_allow_inline(ast, cascade) {
            v.arena.pin(chain, State::SPLIT);
        }

        chain
    }

    /// Builds a [ChainPiece] for a series of method calls and property
    /// accesses.
    ///
    /// If [is_cascade_target] is `true`, then this call chain occurs as the
    /// target of a cascade expression, as in:
    ///
    ///     call.chain()..cascade();
    pub fn build(self, v: &mut AstNodeVisitor<'_>, is_cascade_target: bool) -> PieceId {
        // If there are no calls, there's no chain.
        if self.calls.is_empty() {
            return self.target.unwrap();
        }

        let calls = &self.calls;

        // Count the number of contiguous properties at the beginning of the
        // chain.
        let mut leading_properties = 0;
        while leading_properties < calls.len() && calls[leading_properties].ty == CallType::Property
        {
            leading_properties += 1;
        }

        // Count the number of leading properties and unsplittable calls.
        let mut leading_unsplittable = leading_properties;
        while leading_unsplittable < calls.len() && !calls[leading_unsplittable].can_split() {
            leading_unsplittable += 1;
        }

        // See if we can block format the chain on one of its calls. We allow
        // the last call in a chain to block format:
        //
        //     target.property.method().last(
        //       argument,
        //     );
        //
        // But we only allow it to do so if either the preceding calls can't
        // split (as in the preceding example) or the last call is actually a
        // block formatted argument list (like a collection or function
        // literal) and not just a split argument list. So this is OK:
        //
        //     target.method(1, 2).last([
        //       element,
        //     ]);
        //
        // Even though `method()` takes arguments and can split, we still
        // allow the chain to block format on the last call because that call
        // is itself a block formatted argument list with a collection
        // literal, and not just a split argument list.
        //
        // Further, we allow the second-to-last call in the chain to be the
        // block formatted call if the last call is a property or unsplittable
        // call and the preceding call can block format. This allows for
        // common hanging operations like `toList()` as in:
        //
        //     things.map((element) {
        //       return doStuffTo(element);
        //     }).toList();
        let mut last_call_index = calls.len() - 1;
        if !calls[last_call_index].can_split()
            && calls.len() > 1
            && calls[last_call_index - 1].ty == CallType::BlockFormatCall
        {
            last_call_index = calls.len() - 2;
        }

        let mut block_call_index = -1;
        if leading_unsplittable == last_call_index && calls[last_call_index].can_split() {
            block_call_index = last_call_index as i32;
        } else if calls[last_call_index].ty == CallType::BlockFormatCall {
            block_call_index = last_call_index as i32;
        }

        // If a method chain appears as the target of a cascade, then we only
        // indent the method chain +2. That way, with the cascade's own +2,
        // the result is a total of +4. This looks more natural than indenting
        // the method chain +4 relative to the cascade's +2:
        //
        //     // Bad:
        //     object
        //           .method()
        //           .method()
        //       ..x = 1
        //       ..y = 2;
        //
        //     // Better:
        //     object
        //         .method()
        //         .method()
        //       ..x = 1
        //       ..y = 2;
        let is_3_dot_7 = v.style.is_3_dot_7();
        v.arena.add(ChainPiece::new(
            self.target.unwrap(),
            self.calls,
            false,
            leading_properties,
            block_call_index,
            if is_cascade_target {
                Indent::Cascade
            } else {
                Indent::Expression
            },
            self.allow_split_in_target,
            self.has_single_element_target,
            is_3_dot_7,
        ))
    }

    /// Given [expression], which is the expression for some call chain,
    /// traverses the selectors to fill in the list of [calls] and initialize
    /// [target]. For example, given:
    ///
    ///     foo.bar()!.baz[0][1].bang()
    ///
    /// This initializes [target] to `foo` and fills [calls] with:
    ///
    ///     .bar()!
    ///     .baz[0][1]
    ///     .bang()
    fn unwrap_call(&mut self, v: &mut AstNodeVisitor<'_>, expression: NodeId) {
        let ast = v.ast;

        if looks_like_static_call(ast, expression) {
            // Don't include things that look like static method or
            // constructor calls in the call chain because that tends to split
            // up named constructors from their class.
            self.visit_target(v, expression, false);
            return;
        }

        match ast.kind(expression) {
            // Selectors.
            NodeKind::MethodInvocation
                if ast[ast.cast::<MethodInvocation>(expression).unwrap()]
                    .target
                    .is_some() =>
            {
                let invocation = &ast[ast.cast::<MethodInvocation>(expression).unwrap()];
                self.unwrap_call(v, invocation.target.unwrap().raw());

                let mut call_type = CallType::UnsplittableCall;

                let argument_list = &ast[invocation.argument_list];
                if can_split(ast, argument_list.arguments, argument_list.right_parenthesis) {
                    call_type = CallType::SplittableCall;
                }

                let call_piece = v.build(|v| {
                    v.token(invocation.operator);
                    v.visit(invocation.method_name);
                    v.visit(invocation.type_arguments);

                    // Create the argument piece manually so that we can see if
                    // it has a block argument or not.
                    let arguments = v.build(|v| {
                        v.write_argument_list(invocation.argument_list);
                    });

                    if v.is_list_with_block_element(arguments) {
                        call_type = CallType::BlockFormatCall;
                    }

                    v.add(arguments);
                });

                self.calls.push(ChainCall::new(call_piece, call_type));
            }

            NodeKind::PropertyAccess
                if ast[ast.cast::<PropertyAccess>(expression).unwrap()]
                    .target
                    .is_some() =>
            {
                let access = &ast[ast.cast::<PropertyAccess>(expression).unwrap()];
                self.unwrap_call(v, access.target.unwrap().raw());

                let piece = v.build(|v| {
                    v.token(access.operator);
                    v.visit(access.property_name);
                });

                self.calls.push(ChainCall::new(piece, CallType::Property));
            }

            NodeKind::PrefixedIdentifier => {
                let prefixed = &ast[ast.cast::<PrefixedIdentifier>(expression).unwrap()];
                self.unwrap_call(v, prefixed.prefix.raw());

                let piece = v.build(|v| {
                    v.token(prefixed.period);
                    v.visit(prefixed.identifier);
                });

                self.calls.push(ChainCall::new(piece, CallType::Property));
            }

            // Postfix expressions.
            NodeKind::FunctionExpressionInvocation => {
                let invocation =
                    &ast[ast.cast::<FunctionExpressionInvocation>(expression).unwrap()];
                self.unwrap_postfix(v, expression, invocation.function.raw(), |v, target| {
                    v.build(|v| {
                        v.add(target);
                        v.visit(invocation.type_arguments);
                        v.visit(invocation.argument_list);
                    })
                });
            }

            NodeKind::IndexExpression
                if ast[ast.cast::<IndexExpression>(expression).unwrap()]
                    .target
                    .is_some() =>
            {
                // We check for a non-null target because the target may be
                // `null` if the chain we are building is itself in a cascade
                // section that begins with an index expression like:
                //
                //     object..[index].chain();
                let index = ast.cast::<IndexExpression>(expression).unwrap();
                let target = ast[index].target.unwrap();
                self.unwrap_postfix(v, expression, target.raw(), |v, target| {
                    v.build(|v| {
                        v.add(target);
                        v.write_index_expression(index);
                    })
                });
            }

            NodeKind::PostfixExpression
                if ast.tokens.ty(ast[ast.cast::<PostfixExpression>(expression).unwrap()].operator)
                    == TokenType::BANG =>
            {
                let postfix = &ast[ast.cast::<PostfixExpression>(expression).unwrap()];
                self.unwrap_postfix(v, expression, postfix.operand.raw(), |v, target| {
                    v.build(|v| {
                        v.add(target);
                        v.token(postfix.operator);
                    })
                });
            }

            _ => {
                // Otherwise, it isn't a selector so we've reached the target.
                self.visit_target(v, expression, false);
            }
        }
    }

    /// Creates and stores the resulting Piece for [target] as well as whether
    /// it allows being split.
    ///
    /// If [cascade_target] is `true`, then this is the target of a cascade
    /// expression. Otherwise, it's the target of a call chain.
    fn visit_target(&mut self, v: &mut AstNodeVisitor<'_>, target: NodeId, cascade_target: bool) {
        self.initialize_target_properties(v, target);
        self.target = Some(v.node_piece_with(
            target,
            false,
            if cascade_target {
                NodeContext::CascadeTarget
            } else {
                NodeContext::None
            },
        ));
    }

    /// When [expression] is some kind of postfix expression containing
    /// [postfix_part], attempts to apply the postfix expression to the
    /// preceding call in the call chain contained by [expression].
    ///
    /// If [expression] doesn't have any more call chain selectors preceding
    /// [postfix_part], then [expression] and [postfix_part] all become the
    /// [target] of the call chain. Otherwise, [postfix_part] is added as a
    /// postfix operation to the preceding call in the call chain and we
    /// recurse in to find the inner target.
    fn unwrap_postfix<'a>(
        &mut self,
        v: &mut AstNodeVisitor<'a>,
        expression: NodeId,
        postfix_part: NodeId,
        create_postfix: impl FnOnce(&mut AstNodeVisitor<'a>, PieceId) -> PieceId,
    ) {
        self.unwrap_call(v, postfix_part);

        // If we don't have a preceding call to hang the postfix expression
        // off of, make it part of the target expression. For example:
        //
        //     (list + another)!
        if self.calls.is_empty() {
            // Prior to 3.13, ChainBuilder didn't recalculate
            // [allow_split_in_target] when unwrapping a postfix expression
            // that ends up being part of the target. Because of this, a
            // function expression invocation would have
            // [allow_split_in_target] set to `false` (because the inner
            // function expression itself can't block split) even when the
            // argument list ends up part of the target. For compatibility,
            // this if statement preserves that behavior on older versions.
            if v.style.avoid_splitting_single_element_call_chain_targets() {
                self.initialize_target_properties(v, expression);
            }

            self.target = Some(create_postfix(v, self.target.unwrap()));
        } else {
            let last = self.calls.last_mut().unwrap();
            last.call = create_postfix(v, last.call);
        }
    }

    fn initialize_target_properties(&mut self, v: &AstNodeVisitor<'_>, target: NodeId) {
        let ast = v.ast;
        let expression = ast.cast::<Expression>(target).unwrap();
        self.allow_split_in_target = can_block_split(ast, expression);

        self.has_single_element_target = v
            .style
            .avoid_splitting_single_element_call_chain_targets()
            && has_single_element(ast, expression);
    }
}
