// Dart source: dart_style lib/src/short/argument_list_visitor.dart

use dartr_ast::*;
use dartr_syntax::TokenId;

use crate::ast_extensions::{has_comma_after, has_preceding_comments, string_is_multiline};
use crate::constants::Cost;

use super::arena::{ChunkId, RuleId};
use super::rule::argument::{NamedRule, PositionalRule};
use super::source_visitor::{SourceVisitor, argument_expression};

/// Helper class for [SourceVisitor] that handles visiting and writing an
/// [ArgumentList], including all of the special code needed to handle
/// block-formatted arguments.
pub struct ArgumentListVisitor {
    /// The "(" before the argument list.
    left_parenthesis: TokenId,

    /// The ")" after the argument list.
    right_parenthesis: TokenId,

    /// All of the arguments, positional, named, and functions, in the argument
    /// list.
    all_arguments: Vec<NodeId>,

    /// The normal arguments preceding any block function arguments.
    arguments: ArgumentSublist,

    /// The contiguous list of block function arguments, if any.
    ///
    /// Otherwise, this is `None`.
    functions: Option<Vec<NodeId>>,

    /// If there are block function arguments, this is the arguments after them.
    ///
    /// Otherwise, this is `None`.
    arguments_after_functions: Option<ArgumentSublist>,
}

impl ArgumentListVisitor {
    pub fn new(visitor: &SourceVisitor, node: Id<ArgumentList>) -> ArgumentListVisitor {
        Self::for_node(visitor.ast, node)
    }

    /// Dart `ArgumentListVisitor(visitor, node)` with only the AST.
    pub fn for_node(ast: &Ast, node: Id<ArgumentList>) -> ArgumentListVisitor {
        let n = &ast[node];
        Self::for_arguments_in(
            ast,
            n.left_parenthesis,
            n.right_parenthesis,
            ast.list_raw(n.arguments).to_vec(),
        )
    }

    pub fn for_arguments(
        visitor: &SourceVisitor,
        left_parenthesis: TokenId,
        right_parenthesis: TokenId,
        arguments: Vec<NodeId>,
    ) -> ArgumentListVisitor {
        Self::for_arguments_in(visitor.ast, left_parenthesis, right_parenthesis, arguments)
    }

    fn for_arguments_in(
        ast: &Ast,
        left_parenthesis: TokenId,
        right_parenthesis: TokenId,
        arguments: Vec<NodeId>,
    ) -> ArgumentListVisitor {
        let Some((functions_start, functions_end)) = contiguous_functions(ast, &arguments) else {
            // No functions, so there is just a single argument list.
            let sublist = ArgumentSublist::new(ast, &arguments, arguments.clone());
            return ArgumentListVisitor {
                left_parenthesis,
                right_parenthesis,
                all_arguments: arguments,
                arguments: sublist,
                functions: None,
                arguments_after_functions: None,
            };
        };

        // Split the arguments into two independent argument lists with the
        // functions in the middle.
        let arguments_before = arguments[..functions_start].to_vec();
        let functions = arguments[functions_start..functions_end].to_vec();
        let arguments_after = arguments[functions_end..].to_vec();

        ArgumentListVisitor {
            left_parenthesis,
            right_parenthesis,
            arguments: ArgumentSublist::new(ast, &arguments, arguments_before),
            functions: Some(functions),
            arguments_after_functions: Some(ArgumentSublist::new(ast, &arguments, arguments_after)),
            all_arguments: arguments,
        }
    }

    /// Returns `true` if there is only a single positional argument.
    fn is_single(&self, ast: &Ast) -> bool {
        self.all_arguments.len() == 1 && ast.kind(self.all_arguments[0]) != NodeKind::NamedArgument
    }

    /// Whether this argument list has any arguments that should be formatted as
    /// blocks.
    // TODO(rnystrom): Returning true based on collections is non-optimal. It
    // forces a method chain to break into two but the result collection may not
    // actually split which can lead to a method chain that's allowed to break
    // where it shouldn't.
    pub fn has_block_arguments(&self) -> bool {
        !self.arguments.blocks.is_empty() || self.functions.is_some()
    }

    /// Builds chunks for the argument list.
    pub fn visit(mut self, visitor: &mut SourceVisitor) {
        let ast = visitor.ast;
        let is_single = self.is_single(ast);

        // If there is just one positional argument, it tends to look weird to
        // split before it, so try not to.
        if is_single {
            visitor.builder.start_span_normal();
        }

        visitor.builder.start_span_normal();
        visitor.token(self.left_parenthesis);

        self.arguments.visit(visitor);

        visitor.builder.end_span();

        if let Some(functions) = &self.functions {
            // TODO(rnystrom): It might look better to treat the parameter list of the
            // first function as if it were an argument in the preceding argument list
            // instead of just having this little solo split here. That would try to
            // keep the parameter list with other arguments when possible, and, I
            // think, generally look nicer.
            if functions[0] == self.all_arguments[0] {
                visitor.solo_zero_split();
            } else {
                visitor.solo_split(Cost::NORMAL);
            }

            for &argument in functions {
                if argument != functions[0] {
                    visitor.space();
                }

                visitor.visit(argument);

                // Write the following comma.
                if has_comma_after(ast, argument) {
                    visitor.token(ast.tokens.next(ast.end_token(argument)));
                }
            }

            visitor.builder.start_span_normal();
            self.arguments_after_functions
                .as_mut()
                .unwrap()
                .visit(visitor);
            visitor.builder.end_span();
        }

        visitor.token(self.right_parenthesis);

        if is_single {
            visitor.builder.end_span();
        }
    }
}

/// Look for a single contiguous range of block function [arguments] that
/// should receive special formatting.
///
/// Returns a (start, end] range of indexes if found, otherwise returns `None`.
fn contiguous_functions(ast: &Ast, arguments: &[NodeId]) -> Option<(usize, usize)> {
    let mut functions_start = None;
    let mut functions_end = None;

    // Find the range of block function arguments, if any.
    for (i, &argument) in arguments.iter().enumerate() {
        if is_block_function(ast, argument) {
            functions_start.get_or_insert(i);

            // The functions must be one contiguous section.
            if let Some(end) = functions_end {
                if end != i {
                    return None;
                }
            }

            functions_end = Some(i + 1);
        }
    }

    let functions_start = functions_start?;
    let functions_end = functions_end.unwrap();

    // Edge case: If all of the arguments are named, but they aren't all
    // functions, then don't handle the functions specially. A function with a
    // bunch of named arguments tends to look best when they are all lined up,
    // even the function ones (unless they are all functions).
    //
    // Prefers:
    //
    //     function(
    //         named: () {
    //           something();
    //         },
    //         another: argument);
    //
    // Over:
    //
    //     function(named: () {
    //       something();
    //     },
    //         another: argument);
    if is_all_named(ast, arguments) && (functions_start > 0 || functions_end < arguments.len()) {
        return None;
    }

    // Edge case: If all of the function arguments are named and there are
    // other named arguments that are "=>" functions, then don't treat the
    // block-bodied functions specially. In a mixture of the two function
    // styles, it looks cleaner to treat them all like normal expressions so
    // that the named arguments line up.
    if is_all_named(ast, &arguments[functions_start..functions_end]) {
        let is_named_arrow = |argument: NodeId| {
            let Some(named) = ast.cast::<NamedArgument>(argument) else {
                return false;
            };
            let expression = ast[named].argument_expression;

            match ast.cast::<FunctionExpression>(expression) {
                Some(function) => ast.kind(ast[function].body) == NodeKind::ExpressionFunctionBody,
                None => false,
            }
        };

        for &argument in &arguments[..functions_start] {
            if is_named_arrow(argument) {
                return None;
            }
        }

        for &argument in &arguments[functions_end..] {
            if is_named_arrow(argument) {
                return None;
            }
        }
    }

    Some((functions_start, functions_end))
}

/// Returns `true` if every expression in [arguments] is named.
fn is_all_named(ast: &Ast, arguments: &[NodeId]) -> bool {
    arguments
        .iter()
        .all(|&argument| ast.kind(argument) == NodeKind::NamedArgument)
}

/// Returns `true` if [argument] is a [FunctionExpression] with a non-empty
/// block body.
fn is_block_function(ast: &Ast, argument: NodeId) -> bool {
    let mut expression = argument_expression(ast, argument);

    // Allow functions wrapped in dotted method calls like "a.b.c(() { ... })".
    if let Some(invocation) = ast.cast::<MethodInvocation>(expression) {
        let invocation = &ast[invocation];
        if !is_valid_wrapping_target(ast, invocation.target.map(|t| t.raw())) {
            return false;
        }
        let arguments = ast.list_raw(ast[invocation.argument_list].arguments);
        if arguments.len() != 1 {
            return false;
        }

        return is_block_function(ast, arguments[0]);
    }

    if let Some(creation) = ast.cast::<InstanceCreationExpression>(expression) {
        let arguments = ast.list_raw(ast[ast[creation].argument_list].arguments);
        if arguments.len() != 1 {
            return false;
        }

        return is_block_function(ast, arguments[0]);
    }

    // Allow immediately-invoked functions like "() { ... }()".
    if let Some(invocation) = ast.cast::<FunctionExpressionInvocation>(expression) {
        let invocation = &ast[invocation];
        if !ast[invocation.argument_list].arguments.is_empty() {
            return false;
        }

        expression = invocation.function.raw();
    }

    // Unwrap parenthesized expressions.
    while let Some(parenthesized) = ast.cast::<ParenthesizedExpression>(expression) {
        expression = ast[parenthesized].expression.raw();
    }

    // Must be a function.
    let Some(function) = ast.cast::<FunctionExpression>(expression) else {
        return false;
    };

    // With a curly body.
    let Some(body) = ast.cast::<BlockFunctionBody>(ast[function].body) else {
        return false;
    };

    // That isn't empty.
    let block = &ast[ast[body].block];
    !block.statements.is_empty() || has_preceding_comments(ast, block.right_bracket)
}

/// Returns `true` if [expression] is a valid method invocation target for
/// an invocation that wraps a function literal argument.
fn is_valid_wrapping_target(ast: &Ast, expression: Option<NodeId>) -> bool {
    // Allow bare function calls.
    let Some(mut expression) = expression else {
        return true;
    };

    // Allow property accesses.
    while let Some(access) = ast.cast::<PropertyAccess>(expression) {
        match ast[access].target {
            Some(target) => expression = target.raw(),
            None => return false,
        }
    }

    matches!(
        ast.kind(expression),
        NodeKind::PrefixedIdentifier | NodeKind::SimpleIdentifier
    )
}

/// A range of arguments from a complete argument list.
///
/// One of these typically covers all of the arguments in an invocation. But,
/// when an argument list has block functions in the middle, the arguments
/// before and after the functions are treated as separate independent lists.
/// In that case, there will be two of these.
pub struct ArgumentSublist {
    /// The first argument of the full argument list from the AST (Dart
    /// `_allArguments.first`), and whether there is more than one argument.
    all_arguments_first: Option<NodeId>,
    all_arguments_length: usize,

    /// If all positional arguments occur before all named arguments, then this
    /// contains the positional arguments, in order. Otherwise (there are no
    /// positional arguments or they are interleaved with named ones), this is
    /// empty.
    positional: Vec<NodeId>,

    /// The named arguments, in order. If there are any named arguments that occur
    /// before positional arguments, then all arguments are treated as named and
    /// end up in this list.
    named: Vec<NodeId>,

    /// Maps each block argument, excluding functions, to the first token for that
    /// argument.
    blocks: Vec<(NodeId, TokenId)>,

    /// The number of leading block arguments, excluding functions.
    ///
    /// If all arguments are blocks, this counts them.
    leading_blocks: i32,

    /// The number of trailing blocks arguments.
    ///
    /// If all arguments are blocks, this is zero.
    trailing_blocks: i32,

    /// The rule used to split the bodies of all block arguments.
    block_rule: Option<RuleId>,

    /// The most recent chunk that split before an argument.
    previous_split: Option<ChunkId>,
}

impl ArgumentSublist {
    fn new(ast: &Ast, all_arguments: &[NodeId], arguments: Vec<NodeId>) -> ArgumentSublist {
        let (positional, named) = split_argument_lists(ast, &arguments);

        let mut blocks: Vec<(NodeId, TokenId)> = Vec::new();
        for &argument in &arguments {
            if let Some(bracket) = block_token(ast, argument) {
                blocks.push((argument, bracket));
            }
        }
        let contains = |blocks: &[(NodeId, TokenId)], argument: NodeId| {
            blocks.iter().any(|&(block, _)| block == argument)
        };

        // Count the leading arguments that are blocks.
        let mut leading_blocks = 0;
        for &argument in &arguments {
            if !contains(&blocks, argument) {
                break;
            }
            leading_blocks += 1;
        }

        // Count the trailing arguments that are blocks.
        let mut trailing_blocks = 0;
        if leading_blocks != arguments.len() {
            for &argument in arguments.iter().rev() {
                if !contains(&blocks, argument) {
                    break;
                }
                trailing_blocks += 1;
            }
        }

        // Blocks must all be a prefix or suffix of the argument list (and not
        // both).
        if leading_blocks != blocks.len() {
            leading_blocks = 0;
        }
        if trailing_blocks != blocks.len() {
            trailing_blocks = 0;
        }

        // Ignore any blocks in the middle of the argument list.
        if leading_blocks == 0 && trailing_blocks == 0 {
            blocks.clear();
        }

        ArgumentSublist {
            all_arguments_first: all_arguments.first().copied(),
            all_arguments_length: all_arguments.len(),
            positional,
            named,
            blocks,
            leading_blocks: leading_blocks as i32,
            trailing_blocks: trailing_blocks as i32,
            block_rule: None,
            previous_split: None,
        }
    }

    fn visit(&mut self, visitor: &mut SourceVisitor) {
        if !self.blocks.is_empty() {
            self.block_rule = Some(visitor.builder.arena.new_rule_with_cost(Cost::SPLIT_BLOCKS));
        }

        let rule = self.visit_positional(visitor);
        self.visit_named(visitor, rule);
    }

    /// Writes the positional arguments, if any.
    fn visit_positional(&mut self, visitor: &mut SourceVisitor) -> Option<RuleId> {
        if self.positional.is_empty() {
            return None;
        }

        // Allow splitting after "(".
        // Only count the blocks in the positional rule.
        let leading_blocks = self.leading_blocks.min(self.positional.len() as i32);
        let trailing_blocks = (self.trailing_blocks - self.named.len() as i32).max(0);
        let rule = visitor.builder.arena.add_rule(PositionalRule::new_rule(
            self.block_rule,
            self.positional.len() as i32,
            leading_blocks,
            trailing_blocks,
        ));
        let positional = std::mem::take(&mut self.positional);
        self.visit_arguments(visitor, &positional, true, rule);
        self.positional = positional;

        Some(rule)
    }

    /// Writes the named arguments, if any.
    fn visit_named(&mut self, visitor: &mut SourceVisitor, positional_rule: Option<RuleId>) {
        if self.named.is_empty() {
            return;
        }

        // Only count the blocks in the named rule.
        let leading_blocks = (self.leading_blocks - self.positional.len() as i32).max(0);
        let trailing_blocks = self.trailing_blocks.min(self.named.len() as i32);
        let named_rule = visitor.builder.arena.add_rule(NamedRule::new_rule(
            self.block_rule,
            leading_blocks,
            trailing_blocks,
        ));

        // Let the positional args force the named ones to split.
        if let Some(positional_rule) = positional_rule {
            visitor
                .builder
                .arena
                .rule_mut(positional_rule)
                .add_named_args_constraints(named_rule);
        }

        let named = std::mem::take(&mut self.named);
        self.visit_arguments(visitor, &named, false, named_rule);
        self.named = named;
    }

    fn visit_arguments(
        &mut self,
        visitor: &mut SourceVisitor,
        arguments: &[NodeId],
        is_positional: bool,
        rule: RuleId,
    ) {
        visitor.builder.start_rule(Some(rule));

        // Split before the first argument.
        let split = visitor
            .builder
            .split(true, Some(arguments[0]) != self.all_arguments_first);
        self.previous_split = Some(split);
        visitor
            .builder
            .arena
            .rule_mut(rule)
            .before_argument(Some(split));

        // Try to not split the positional arguments.
        if is_positional {
            visitor.builder.start_span(Cost::POSITIONAL_ARGUMENTS);
        }

        for (i, &argument) in arguments.iter().enumerate() {
            self.visit_argument(visitor, rule, argument);

            // Write the split.
            if i != arguments.len() - 1 {
                let split = visitor.split();
                self.previous_split = Some(split);
                visitor
                    .builder
                    .arena
                    .rule_mut(rule)
                    .before_argument(Some(split));
            }
        }

        if is_positional {
            visitor.builder.end_span();
        }

        visitor.builder.end_rule();
    }

    fn visit_argument(&mut self, visitor: &mut SourceVisitor, rule: RuleId, argument: NodeId) {
        let ast = visitor.ast;
        let is_named = ast.kind(argument) == NodeKind::NamedArgument;
        let nest_block_argument = self.all_arguments_length > 1
            || ast.kind(self.all_arguments_first.unwrap()) == NodeKind::RecordLiteral;

        // If we're about to write a block argument, handle it specially.
        let argument_block = self
            .blocks
            .iter()
            .find(|&&(block, _)| block == argument)
            .map(|&(_, token)| token);
        if let Some(argument_block) = argument_block {
            visitor
                .builder
                .arena
                .rule_mut(rule)
                .disable_split_on_inner_rules();

            // Tell it to use the rule we've already created.
            visitor.before_block(
                argument_block,
                self.block_rule.unwrap(),
                self.previous_split,
            );
        } else if nest_block_argument {
            // Edge case: Only bump the nesting if there are multiple arguments. This
            // lets us avoid spurious indentation in cases like:
            //
            //     function(function(() {
            //       body;
            //     }));
            //
            // Do bump the nesting if the single argument is a record because records
            // are formatted like regular values when they appear in argument lists
            // even though they internally get block-like formatting.
            visitor.builder.start_block_argument_nesting();
        } else if !is_named {
            // Edge case: Likewise, don't force the argument to split if there is
            // only a single positional one, like:
            //
            //     outer(inner(
            //         longArgument));
            visitor
                .builder
                .arena
                .rule_mut(rule)
                .disable_split_on_inner_rules();
        }

        if let Some(named) = ast.cast::<NamedArgument>(argument) {
            visitor.visit_named_argument(named, Some(rule));
        } else {
            visitor.visit(argument);
        }

        if argument_block.is_some() {
            visitor
                .builder
                .arena
                .rule_mut(rule)
                .enable_split_on_inner_rules();
        } else if nest_block_argument {
            visitor.builder.end_block_argument_nesting();
        } else if !is_named {
            visitor
                .builder
                .arena
                .rule_mut(rule)
                .enable_split_on_inner_rules();
        }

        // Write the following comma.
        if has_comma_after(ast, argument) {
            visitor.token(ast.tokens.next(ast.end_token(argument)));
        }
    }
}

/// Splits [arguments] into two lists: the list of leading positional
/// arguments and the list of trailing named arguments.
///
/// If positional arguments are interleaved with the named arguments then
/// all arguments are treat as named since that provides simpler, consistent
/// output.
///
/// Returns a list of two lists: the positional arguments then the named ones.
fn split_argument_lists(ast: &Ast, arguments: &[NodeId]) -> (Vec<NodeId>, Vec<NodeId>) {
    let mut positional = Vec::new();
    let mut named = Vec::new();
    let mut in_named = false;
    for &argument in arguments {
        if ast.kind(argument) == NodeKind::NamedArgument {
            in_named = true;
        } else if in_named {
            // Got a positional argument after a named one.
            return (Vec::new(), arguments.to_vec());
        }

        if in_named {
            named.push(argument);
        } else {
            positional.push(argument);
        }
    }

    (positional, named)
}

/// If [argument] can be formatted as a block, returns the token that opens
/// the block, such as a collection's bracket.
///
/// Block-formatted arguments can get special indentation to make them look
/// more statement-like.
fn block_token(ast: &Ast, argument: NodeId) -> Option<TokenId> {
    let expression = argument_expression(ast, argument);

    // TODO(rnystrom): Should we step into parenthesized expressions?

    match ast.kind(expression) {
        NodeKind::ListLiteral => Some(ast[Id::<ListLiteral>::from_raw(expression)].left_bracket),
        NodeKind::RecordLiteral => {
            Some(ast[Id::<RecordLiteral>::from_raw(expression)].left_parenthesis)
        }
        NodeKind::SetOrMapLiteral => {
            Some(ast[Id::<SetOrMapLiteral>::from_raw(expression)].left_bracket)
        }
        NodeKind::SimpleStringLiteral | NodeKind::StringInterpolation
            if string_is_multiline(ast, expression) =>
        {
            Some(ast.begin_token(expression))
        }
        // Not a collection literal.
        _ => None,
    }
}
