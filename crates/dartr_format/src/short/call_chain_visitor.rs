// Dart source: dart_style lib/src/short/call_chain_visitor.dart

use dartr_ast::*;
use dartr_syntax::TokenType;

use crate::ast_extensions::{has_comma_after, is_collection_literal, looks_like_static_call};

use super::arena::RuleId;
use super::argument_list_visitor::ArgumentListVisitor;
use super::rule::argument::PositionalRule;
use super::source_visitor::{SourceVisitor, argument_expression};

/// Helper class for [SourceVisitor] that handles visiting and writing a
/// chained series of "selectors": method invocations, property accesses,
/// prefixed identifiers, index expressions, and null-assertion operators.
///
/// In the AST, selectors are nested bottom up such that this expression:
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
/// to left. It's easier to format that if we organize them as a linear series
/// of selectors from left to right. Further, we want to organize it into a
/// two-tier hierarchy. We have an outer list of method calls and property
/// accesses. Then each of those may have one or more postfix selectors
/// attached: indexers, null-assertions, or invocations. This mirrors how they
/// are formatted.
pub struct CallChainVisitor {
    /// The initial target of the call chain.
    ///
    /// This may be any expression except [MethodInvocation], [PropertyAccess] or
    /// [PrefixedIdentifier].
    target: NodeId,

    /// The list of dotted names ([PropertyAccess] and [PrefixedIdentifier]) at
    /// the start of the call chain.
    ///
    /// This will be empty if the [target] is not a [SimpleIdentifier].
    properties: Vec<Selector>,

    /// The mixed method calls and property accesses in the call chain in the
    /// order that they appear in the source reading from left to right.
    calls: Vec<Selector>,

    /// The method calls containing block function literals that break the method
    /// chain and escape its indentation.
    ///
    ///     receiver.a().b().c(() {
    ///       ;
    ///     }).d(() {
    ///       ;
    ///     }).e();
    ///
    /// Here, it will contain `c` and `d`.
    ///
    /// The block calls must be contiguous and must be a suffix of the list of
    /// calls (except for the one allowed hanging call). Otherwise, none of them
    /// are treated as block calls:
    ///
    ///     receiver
    ///         .a()
    ///         .b(() {
    ///           ;
    ///         })
    ///         .c(() {
    ///           ;
    ///         })
    ///         .d()
    ///         .e();
    block_calls: Option<Vec<Selector>>,

    /// If there is one or more block calls and a single chained expression after
    /// that, this will be that expression.
    ///
    ///     receiver.a().b().c(() {
    ///       ;
    ///     }).d(() {
    ///       ;
    ///     }).e();
    ///
    /// We allow a single hanging call after the blocks because it will never
    /// need to split before its `.` and this accommodates the common pattern of
    /// a trailing `toList()` or `toSet()` after a series of higher-order methods
    /// on an iterable.
    hanging_call: Option<Selector>,

    /// Whether or not a [Rule] is currently active for the call chain.
    rule_enabled: bool,

    /// Whether or not the span wrapping the call chain is currently active.
    span_ended: bool,

    /// After the properties are visited (if there are any), this will be the
    /// rule used to split between them.
    property_rule: Option<RuleId>,
}

impl CallChainVisitor {
    /// Creates a new call chain visitor for [visitor] for the method chain
    /// contained in [node].
    ///
    /// The [node] is the outermost expression containing the chained "."
    /// operators and must be a [MethodInvocation], [PropertyAccess] or
    /// [PrefixedIdentifier].
    pub fn new(visitor: &SourceVisitor, node: NodeId) -> CallChainVisitor {
        let ast = visitor.ast;

        // Flatten the call chain tree to a list of selectors with postfix
        // expressions.
        let mut calls: Vec<Selector> = Vec::new();
        let target = unwrap_target(ast, node, &mut calls);

        // An expression that starts with a series of dotted names gets treated a
        // little specially. We don't force leading properties to split with the
        // rest of the chain. Allows code like:
        //
        //     address.street.number
        //       .toString()
        //       .length;
        let mut properties_length = 0;
        if ast.kind(unwrap_null_assertion(ast, target)) == NodeKind::SimpleIdentifier {
            properties_length = calls.iter().take_while(|call| call.is_property()).count();
        }

        let properties: Vec<Selector> = calls.drain(..properties_length).collect();

        // Separate out the block calls, if there are any.
        let mut block_calls: Option<Vec<usize>> = None;
        let mut hanging_call: Option<usize> = None;

        let mut in_block_calls = false;
        for (i, call) in calls.iter().enumerate() {
            if call.is_block_call(ast) {
                in_block_calls = true;
                block_calls.get_or_insert_with(Vec::new).push(i);
            } else if in_block_calls {
                // We found a non-block call after a block call.
                if i == calls.len() - 1 {
                    // It's the one allowed hanging one, so it's OK.
                    hanging_call = Some(i);
                    break;
                }

                // Don't allow any of the calls to be block formatted.
                block_calls = None;
                break;
            }
        }

        let mut remaining = Vec::new();
        let mut blocks = Vec::new();
        let mut hanging = None;
        for (i, call) in calls.into_iter().enumerate() {
            if block_calls.as_ref().is_some_and(|b| b.contains(&i)) {
                blocks.push(call);
            } else if hanging_call == Some(i) {
                hanging = Some(call);
            } else {
                remaining.push(call);
            }
        }

        CallChainVisitor {
            target,
            properties,
            calls: remaining,
            block_calls: block_calls.map(|_| blocks),
            hanging_call: hanging,
            rule_enabled: false,
            span_ended: false,
            property_rule: None,
        }
    }

    /// Builds chunks for the call chain.
    pub fn visit(mut self, visitor: &mut SourceVisitor) {
        let ast = visitor.ast;
        visitor.builder.nest();

        // Try to keep the entire method invocation one line.
        visitor.builder.start_span_normal();

        // If a split in the target expression forces the first `.` to split, then
        // start the rule now so that it surrounds the target.
        let split_on_target = forces_split(ast, self.target);

        if split_on_target {
            if self.properties.len() > 1 {
                let rule = visitor.builder.arena.add_rule(PositionalRule::new_rule(
                    None,
                    self.properties.len() as i32,
                    0,
                    0,
                ));
                self.property_rule = Some(rule);
                visitor.builder.start_lazy_rule(Some(rule));
            } else {
                self.enable_rule(visitor, true);
            }
        }

        visitor.visit(self.target);

        // Leading properties split like positional arguments: either not at all,
        // before one ".", or before all of them.
        if self.properties.len() == 1 {
            visitor.solo_zero_split();
            let property = self.properties[0].clone();
            property.write(&mut self, visitor);
        } else if self.properties.len() > 1 {
            if !split_on_target {
                let rule = visitor.builder.arena.add_rule(PositionalRule::new_rule(
                    None,
                    self.properties.len() as i32,
                    0,
                    0,
                ));
                self.property_rule = Some(rule);
                visitor.builder.start_rule(Some(rule));
            }

            for property in self.properties.clone() {
                let chunk = visitor.zero_split();
                visitor
                    .builder
                    .arena
                    .rule_mut(self.property_rule.unwrap())
                    .before_argument(Some(chunk));
                property.write(&mut self, visitor);
            }

            visitor.builder.end_rule();
        }

        // Indent any block arguments in the chain that don't get special formatting
        // below. Only do this if there is more than one argument to avoid spurious
        // indentation in cases like:
        //
        //     object.method(wrapper(() {
        //       body;
        //     });
        // TODO(rnystrom): Come up with a less arbitrary way to express this?
        if self.calls.len() > 1 {
            visitor.builder.start_block_argument_nesting();
        }

        // The chain of calls splits atomically (either all or none). Any block
        // arguments inside them get indented to line up with the `.`.
        for call in self.calls.clone() {
            self.enable_rule(visitor, false);
            visitor.zero_split();
            call.write(&mut self, visitor);
        }

        if self.calls.len() > 1 {
            visitor.builder.end_block_argument_nesting();
        }

        // If there are block calls, end the chain and write those without any
        // extra indentation.
        if let Some(block_calls) = self.block_calls.clone() {
            self.enable_rule(visitor, false);
            visitor.zero_split();
            self.disable_rule(visitor);

            for block_call in block_calls {
                block_call.write(&mut self, visitor);
            }

            // If there is a hanging call after the last block, write it without any
            // split before the ".".
            if let Some(hanging_call) = self.hanging_call.clone() {
                hanging_call.write(&mut self, visitor);
            }
        }

        self.disable_rule(visitor);
        self.end_span(visitor);
        visitor.builder.unnest();
    }

    /// Called when a [MethodSelector] has written its name and is about to
    /// write the argument list.
    fn before_method_arguments(&mut self, visitor: &mut SourceVisitor, selector: &Selector) {
        // If we don't have any block calls, stop the rule after the last method
        // call name, but before its arguments. This allows unsplit chains where
        // the last argument list wraps, like:
        //
        //     foo().bar().baz(
        //         argument, list);
        if self.block_calls.is_none()
            && self
                .calls
                .last()
                .is_some_and(|last| last.node == selector.node)
        {
            self.disable_rule(visitor);
        }

        // For a single method call on an identifier, stop the span before the
        // arguments to make it easier to keep the call name with the target. In
        // other words, prefer:
        //
        //     target.method(
        //         argument, list);
        //
        // Over:
        //
        //     target
        //         .method(argument, list);
        //
        // Alternatively, the way to think of this is try to avoid splitting on the
        // "." when calling a single method on a single name. This is especially
        // important because the identifier is often a library prefix, and splitting
        // there looks really odd.
        if self.properties.is_empty()
            && self.calls.len() == 1
            && self.block_calls.is_none()
            && visitor.ast.kind(self.target) == NodeKind::SimpleIdentifier
        {
            self.end_span(visitor);
        }
    }

    /// If a [Rule] for the method chain is currently active, ends it.
    fn disable_rule(&mut self, visitor: &mut SourceVisitor) {
        if !self.rule_enabled {
            return;
        }

        visitor.builder.end_rule();
        self.rule_enabled = false;
    }

    /// Creates a new method chain [Rule] if one is not already active.
    fn enable_rule(&mut self, visitor: &mut SourceVisitor, lazy: bool) {
        if self.rule_enabled {
            return;
        }

        // If the properties split, force the calls to split too.
        let rule = visitor.builder.arena.new_rule();
        if let Some(property_rule) = self.property_rule {
            visitor
                .builder
                .arena
                .rule_mut(property_rule)
                .add_named_args_constraints(rule);
        }

        if lazy {
            visitor.builder.start_lazy_rule(Some(rule));
        } else {
            visitor.builder.start_rule(Some(rule));
        }

        self.rule_enabled = true;
    }

    /// Ends the span wrapping the call chain if it hasn't ended already.
    fn end_span(&mut self, visitor: &mut SourceVisitor) {
        if self.span_ended {
            return;
        }

        visitor.builder.end_span();
        self.span_ended = true;
    }
}

/// Returns `true` if the method chain should split if a split occurs inside
/// [expression].
///
/// In most cases, splitting in a method chain's target forces the chain to
/// split too:
///
///      receiver(very, long, argument,
///              list)                    // <-- Split here...
///          .method();                   //     ...forces split here.
///
/// However, if the target is a collection or function literal (or an
/// argument list ending in one of those), we don't want to split:
///
///      receiver(inner(() {
///        ;
///      }).method();                     // <-- Unsplit.
fn forces_split(ast: &Ast, mut expression: NodeId) -> bool {
    // TODO(rnystrom): Other cases we may want to consider handling and
    // recursing into:
    // * The right operand in an infix operator call.
    // * The body of a `=>` function.

    // Unwrap parentheses.
    while let Some(parenthesized) = ast.cast::<ParenthesizedExpression>(expression) {
        expression = ast[parenthesized].expression.raw();
    }

    // Don't split right after a collection literal.
    if is_collection_literal(ast, expression) {
        return false;
    }

    // Don't split right after a non-empty curly-bodied function.
    if let Some(function) = ast.cast::<FunctionExpression>(expression) {
        let Some(body) = ast.cast::<BlockFunctionBody>(ast[function].body) else {
            return false;
        };

        return ast[ast[body].block].statements.is_empty();
    }

    // If the expression ends in an argument list, base the splitting on the
    // last argument.
    let argument_list = match ast.kind(expression) {
        NodeKind::MethodInvocation => {
            Some(ast[Id::<MethodInvocation>::from_raw(expression)].argument_list)
        }
        NodeKind::InstanceCreationExpression => {
            Some(ast[Id::<InstanceCreationExpression>::from_raw(expression)].argument_list)
        }
        NodeKind::FunctionExpressionInvocation => {
            Some(ast[Id::<FunctionExpressionInvocation>::from_raw(expression)].argument_list)
        }
        _ => None,
    };

    // Any other kind of expression always splits.
    let Some(argument_list) = argument_list else {
        return true;
    };
    let arguments = ast.list_raw(ast[argument_list].arguments);
    let Some(&argument) = arguments.last() else {
        return true;
    };

    // If the argument list has a trailing comma, treat it like a collection.
    if has_comma_after(ast, argument) {
        return false;
    }

    // TODO(rnystrom): This logic is similar (but not identical) to
    // ArgumentListVisitor.hasBlockArguments. They overlap conceptually and
    // both have their own peculiar heuristics. It would be good to unify and
    // rationalize them.

    forces_split(ast, argument_expression(ast, argument))
}

/// The kind of a [Selector] (the Dart subclasses of `_Selector`).
#[derive(Clone, Copy)]
enum SelectorKind {
    /// Dart `_MethodSelector`.
    Method(Id<MethodInvocation>),
    /// Dart `_PrefixedSelector`.
    Prefixed(Id<PrefixedIdentifier>),
    /// Dart `_PropertySelector`.
    Property(Id<PropertyAccess>),
}

/// One "selector" in a method call chain.
///
/// Each selector is a method call or property access. It may be followed by
/// one or more postfix expressions, which can be index expressions or
/// null-assertion operators. These are not treated like their own selectors
/// because the formatter attaches them to the previous method call or property
/// access:
///
///     receiver
///         .method(arg)[index]
///         .another()!
///         .third();
#[derive(Clone)]
struct Selector {
    kind: SelectorKind,

    /// The node of the selector (its identity).
    node: NodeId,

    /// The series of index and/or null-assertion postfix selectors that follow
    /// and are attached to this one.
    ///
    /// Elements in this list will either be [IndexExpression] or
    /// [PostfixExpression].
    postfixes: Vec<NodeId>,
}

impl Selector {
    fn new(kind: SelectorKind) -> Selector {
        let node = match kind {
            SelectorKind::Method(node) => node.raw(),
            SelectorKind::Prefixed(node) => node.raw(),
            SelectorKind::Property(node) => node.raw(),
        };
        Selector {
            kind,
            node,
            postfixes: Vec::new(),
        }
    }

    /// Whether this selector is a property access as opposed to a method call.
    fn is_property(&self) -> bool {
        !matches!(self.kind, SelectorKind::Method(_))
    }

    /// Whether this selector is a method call whose arguments are block
    /// formatted.
    fn is_block_call(&self, ast: &Ast) -> bool {
        match self.kind {
            SelectorKind::Method(node) => {
                ArgumentListVisitor::for_node(ast, ast[node].argument_list).has_block_arguments()
            }
            _ => false,
        }
    }

    /// Write the selector portion of the expression wrapped by this [Selector]
    /// using [visitor], followed by any postfix selectors.
    fn write(&self, chain: &mut CallChainVisitor, visitor: &mut SourceVisitor) {
        let ast = visitor.ast;
        self.write_selector(chain, visitor);

        // Write any trailing index and null-assertion operators.
        visitor.builder.nest();
        for &postfix in &self.postfixes {
            if let Some(invocation) = ast.cast::<FunctionExpressionInvocation>(postfix) {
                let invocation = &ast[invocation];

                // Allow splitting between the invocations if needed.
                visitor.solo_zero_split();

                visitor.visit_opt(invocation.type_arguments);
                visitor.visit_argument_list(invocation.argument_list, true);
            } else if let Some(index) = ast.cast::<IndexExpression>(postfix) {
                visitor.finish_index_expression(index);
            } else if let Some(postfix) = ast.cast::<PostfixExpression>(postfix) {
                debug_assert!(ast.tokens.ty(ast[postfix].operator) == TokenType::BANG);
                visitor.token(ast[postfix].operator);
            } else {
                // Unexpected type.
                debug_assert!(false);
            }
        }
        visitor.builder.unnest();
    }

    /// Subclasses implement this to write their selector.
    fn write_selector(&self, chain: &mut CallChainVisitor, visitor: &mut SourceVisitor) {
        let ast = visitor.ast;
        match self.kind {
            SelectorKind::Method(node) => {
                let n = &ast[node];
                visitor.token_opt(n.operator);
                visitor.token(ast[n.method_name].token);

                chain.before_method_arguments(visitor, self);

                visitor.builder.nest();
                visitor.visit_opt(n.type_arguments);
                visitor.visit_argument_list(n.argument_list, false);
                visitor.builder.unnest();
            }
            SelectorKind::Prefixed(node) => {
                let n = &ast[node];
                visitor.token(n.period);
                visitor.visit(n.identifier);
            }
            SelectorKind::Property(node) => {
                let n = &ast[node];
                visitor.token(n.operator);
                visitor.visit(n.property_name);
            }
        }
    }
}

/// If [expression] is a null-assertion operator, returns its operand.
fn unwrap_null_assertion(ast: &Ast, expression: NodeId) -> NodeId {
    if let Some(postfix) = ast.cast::<PostfixExpression>(expression) {
        let postfix = &ast[postfix];
        if ast.tokens.ty(postfix.operator) == TokenType::BANG {
            return postfix.operand.raw();
        }
    }

    expression
}

/// Given [node], which is the outermost expression for some call chain,
/// recursively traverses the selectors to fill in the list of [calls].
///
/// Returns the remaining target expression that precedes the method chain.
/// For example, given:
///
///     foo.bar()!.baz[0][1].bang()
///
/// This returns `foo` and fills calls with:
///
///     selector  postfixes
///     --------  ---------
///     .bar()    !
///     .baz      [0], [1]
///     .bang()
fn unwrap_target(ast: &Ast, node: NodeId, calls: &mut Vec<Selector>) -> NodeId {
    // Don't include things that look like static method or constructor
    // calls in the call chain because that tends to split up named
    // constructors from their class.
    if looks_like_static_call(ast, node) {
        return node;
    }

    // Selectors.
    if let Some(invocation) = ast.cast::<MethodInvocation>(node) {
        if let Some(target) = ast[invocation].target {
            return unwrap_selector(
                ast,
                target.raw(),
                Selector::new(SelectorKind::Method(invocation)),
                calls,
            );
        }
    }

    if let Some(access) = ast.cast::<PropertyAccess>(node) {
        if let Some(target) = ast[access].target {
            return unwrap_selector(
                ast,
                target.raw(),
                Selector::new(SelectorKind::Property(access)),
                calls,
            );
        }
    }

    if let Some(prefixed) = ast.cast::<PrefixedIdentifier>(node) {
        return unwrap_selector(
            ast,
            ast[prefixed].prefix.raw(),
            Selector::new(SelectorKind::Prefixed(prefixed)),
            calls,
        );
    }

    // Postfix expressions.
    if let Some(index) = ast.cast::<IndexExpression>(node) {
        if let Some(target) = ast[index].target {
            return unwrap_postfix(ast, node, target.raw(), calls);
        }
    }

    if let Some(invocation) = ast.cast::<FunctionExpressionInvocation>(node) {
        return unwrap_postfix(ast, node, ast[invocation].function.raw(), calls);
    }

    if let Some(postfix) = ast.cast::<PostfixExpression>(node) {
        let postfix = &ast[postfix];
        if ast.tokens.ty(postfix.operator) == TokenType::BANG {
            return unwrap_postfix(ast, node, postfix.operand.raw(), calls);
        }
    }

    // Otherwise, it isn't a selector so we're done.
    node
}

fn unwrap_postfix(ast: &Ast, node: NodeId, target: NodeId, calls: &mut Vec<Selector>) -> NodeId {
    let target = unwrap_target(ast, target, calls);

    // If we don't have a preceding selector to hang the postfix expression off
    // of, don't unwrap it and leave it attached to the target expression. For
    // example:
    //
    //     (list + another)[index]
    let Some(last) = calls.last_mut() else {
        return node;
    };

    last.postfixes.push(node);
    target
}

fn unwrap_selector(
    ast: &Ast,
    target: NodeId,
    selector: Selector,
    calls: &mut Vec<Selector>,
) -> NodeId {
    let target = unwrap_target(ast, target, calls);
    calls.push(selector);
    target
}
