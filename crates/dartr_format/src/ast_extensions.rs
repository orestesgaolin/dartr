// Dart source: dart_style lib/src/ast_extensions.dart
// Dart source: dart_style lib/src/piece/list.dart (BlockFormat)

//! Formatter-specific queries on AST nodes and tokens.
//!
//! The Dart extensions (`AstNodeExtensions`, `ExpressionExtensions`, ...) are
//! free functions that take the [`Ast`] first. The section "Rust helpers" at
//! the end has the generic analyzer API that the dartr AST does not have as a
//! single method (`AnnotatedNode.metadata`, `SimpleStringLiteral.isMultiline`,
//! `Token.precedingComments` as an iterator).

use dartr_ast::*;
use dartr_syntax::{TokenId, TokenType};

/// Dart `BlockFormat` (`piece/list.dart`): the kind of block formatting an
/// element in an argument list or other delimited list can have.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlockFormat {
    /// The element is a function expression, which takes priority over other
    /// kinds of block formatted elements.
    Function,

    /// The element is a collection literal or multiline string literal.
    Collection,

    /// A function or method invocation.
    Invocation,

    /// The element is an adjacent strings expression that's in an list that
    /// requires its subsequent lines to be indented (because there are other
    /// string literal in the list).
    IndentedAdjacentStrings,

    /// The element is an adjacent strings expression that's in an list that
    /// doesn't require its subsequent lines to be indented.
    UnindentedAdjacentStrings,

    /// The element can't be block formatted.
    None,
}

// ---------------------------------------------------------------------------
// AstNodeExtensions

/// Dart `AstNode.blockFormatType`: when this node is in an argument list,
/// what kind of block formatting category it belongs to.
pub fn node_block_format_type(ast: &Ast, node: NodeId) -> BlockFormat {
    if let Some(strings) = ast.cast::<AdjacentStrings>(node) {
        return if indent_strings(ast, strings) {
            BlockFormat::IndentedAdjacentStrings
        } else {
            BlockFormat::UnindentedAdjacentStrings
        };
    }
    if let Some(named) = ast.cast::<NamedArgument>(node) {
        return node_block_format_type(ast, ast[named].argument_expression.raw());
    }
    if let Some(expression) = ast.cast::<Expression>(node) {
        return block_format_type(ast, expression);
    }
    BlockFormat::None
}

/// Dart `AstNode.firstNonCommentToken`: the first token at the beginning of
/// this AST node, not including any tokens for leading doc comments.
///
/// If [node] is an [AnnotatedNode], then [beginToken] includes the
/// leading doc comment, which we want to handle separately. So, in that
/// case, explicitly skip past the doc comment to the subsequent metadata
/// (if there is any), or the beginning of the code.
pub fn first_non_comment_token(ast: &Ast, node: NodeId) -> TokenId {
    if let Some(metadata) = annotated_metadata(ast, node) {
        // If the node is annotated, skip past the doc comments, but not the
        // metadata.
        if let Some(&annotation) = ast.list(metadata).first() {
            return ast.begin_token(annotation);
        }
        return first_token_after_comment_and_metadata(ast, node).unwrap();
    }
    // The inner [PatternVariableDeclaration] is an [AnnotatedNode].
    if let Some(statement) = ast.cast::<PatternVariableDeclarationStatement>(node) {
        return first_non_comment_token(ast, ast[statement].declaration.raw());
    }
    // The inner [VariableDeclarationList] is an [AnnotatedNode].
    if let Some(statement) = ast.cast::<VariableDeclarationStatement>(node) {
        return first_non_comment_token(ast, ast[statement].variables.raw());
    }
    // Otherwise, we don't have to worry about doc comments.
    ast.begin_token(node)
}

/// Dart `AstNode.commaAfter`: the comma token immediately following this if
/// there is one, or `None`.
pub fn comma_after(ast: &Ast, node: NodeId) -> Option<TokenId> {
    let tokens = &ast.tokens;
    let next = tokens.next(ast.end_token(node));
    if tokens.ty(next) == TokenType::COMMA {
        return Some(next);
    }

    // TODO(sdk#38990): endToken doesn't include the "?" on a nullable
    // function-typed formal, so check for that case and handle it.
    if tokens.ty(next) == TokenType::QUESTION && tokens.ty(tokens.next(next)) == TokenType::COMMA {
        return Some(tokens.next(next));
    }

    None
}

/// Dart `AstNode.hasCommaAfter`.
pub fn has_comma_after(ast: &Ast, node: NodeId) -> bool {
    comma_after(ast, node).is_some()
}

/// Dart `AstNode.hasNonEmptyBody`: whether this node is a statement or
/// member with a braced body that isn't empty.
///
/// Used to determine if a blank line should be inserted after the node.
pub fn has_non_empty_body(ast: &Ast, node: NodeId) -> bool {
    let body: Option<Id<FunctionBody>> = if let Some(method) = ast.cast::<MethodDeclaration>(node) {
        Some(ast[method].body)
    } else if let Some(statement) = ast.cast::<FunctionDeclarationStatement>(node) {
        let function = ast[statement].function_declaration;
        Some(ast[ast[function].function_expression].body)
    } else if let Some(function) = ast.cast::<FunctionDeclaration>(node) {
        Some(ast[ast[function].function_expression].body)
    } else {
        None
    };

    match body.and_then(|body| ast.cast::<BlockFunctionBody>(body)) {
        Some(body) => !ast[ast[body].block].statements.is_empty(),
        None => false,
    }
}

/// Dart `AstNode.isCollectionLiteral`: whether this node is a
/// bracket-delimited collection literal.
pub fn is_collection_literal(ast: &Ast, node: NodeId) -> bool {
    matches!(
        ast.kind(node),
        NodeKind::ListLiteral | NodeKind::RecordLiteral | NodeKind::SetOrMapLiteral
    )
}

/// Dart `AstNode.isControlFlowElement`.
pub fn is_control_flow_element(ast: &Ast, node: NodeId) -> bool {
    matches!(ast.kind(node), NodeKind::IfElement | NodeKind::ForElement)
}

/// Dart `AstNode.isFunctionExpressionBody`: whether this is immediately
/// contained within an anonymous [FunctionExpression].
pub fn is_function_expression_body(ast: &Ast, node: NodeId) -> bool {
    match ast.parent(node) {
        Some(parent) if ast.kind(parent) == NodeKind::FunctionExpression => {
            !matches!(ast.parent(parent), Some(p) if ast.kind(p) == NodeKind::FunctionDeclaration)
        }
        _ => false,
    }
}

/// Dart `AstNode.isSpreadCollection`: whether [node] is a spread of a
/// non-empty collection literal.
pub fn is_spread_collection(ast: &Ast, node: NodeId) -> bool {
    spread_collection_bracket(ast, node).is_some()
}

/// Dart `AstNode.spreadCollectionBracket`: if this is a spread of a
/// non-empty collection literal, then returns the token for the opening
/// bracket of the collection, as in:
///
///     [ ...[a, list] ]
///     //   ^
///
/// Otherwise, returns `None`.
pub fn spread_collection_bracket(ast: &Ast, node: NodeId) -> Option<TokenId> {
    let spread = ast.cast::<SpreadElement>(node)?;
    let expression = ast[spread].expression;
    if let Some(list) = ast.cast::<ListLiteral>(expression) {
        let list = &ast[list];
        if can_split(ast, list.elements, list.right_bracket) {
            return Some(list.left_bracket);
        }
    } else if let Some(set_or_map) = ast.cast::<SetOrMapLiteral>(expression) {
        let set_or_map = &ast[set_or_map];
        if can_split(ast, set_or_map.elements, set_or_map.right_bracket) {
            return Some(set_or_map.left_bracket);
        }
    }
    None
}

/// Dart `AstNode.spreadCollection`: if this is a spread of a non-empty
/// collection literal, then returns `this` as a [SpreadElement].
///
/// Otherwise, returns `None`.
pub fn spread_collection(ast: &Ast, node: NodeId) -> Option<Id<SpreadElement>> {
    let spread = ast.cast::<SpreadElement>(node)?;
    let expression = ast[spread].expression;
    if let Some(list) = ast.cast::<ListLiteral>(expression) {
        let list = &ast[list];
        if can_split(ast, list.elements, list.right_bracket) {
            return Some(spread);
        }
    } else if let Some(set_or_map) = ast.cast::<SetOrMapLiteral>(expression) {
        let set_or_map = &ast[set_or_map];
        if can_split(ast, set_or_map.elements, set_or_map.right_bracket) {
            return Some(spread);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// AstIterableExtensions

/// Dart `Iterable<AstNode>.hasCommaAfter`: whether there is a comma token
/// immediately following the last node.
pub fn list_has_comma_after<T: ?Sized>(ast: &Ast, list: NodeList<T>) -> bool {
    match ast.list_raw(list).last() {
        Some(&last) => has_comma_after(ast, last),
        None => false,
    }
}

/// Dart `Iterable<AstNode>.hasCommaAfter` for a slice of nodes.
pub fn nodes_have_comma_after(ast: &Ast, nodes: &[NodeId]) -> bool {
    match nodes.last() {
        Some(&last) => has_comma_after(ast, last),
        None => false,
    }
}

/// Dart `Iterable<AstNode>.canSplit`: whether the delimited construct
/// containing these nodes and terminated by [right_bracket] can have a split
/// inside it.
///
/// We disallow splitting for entirely empty delimited constructs like `[]`,
/// but allow a split if there are elements or comments inside.
pub fn can_split<T: ?Sized>(ast: &Ast, list: NodeList<T>, right_bracket: TokenId) -> bool {
    !list.is_empty() || has_preceding_comments(ast, right_bracket)
}

/// Dart `Iterable<AstNode>.containsLineComments`: returns `true` if the
/// collection containing these elements and terminated by [right_bracket]
/// contains any line comments before, between, or after any elements.
///
/// Comments within an element are ignored.
pub fn contains_line_comments(
    ast: &Ast,
    elements: &[NodeId],
    right_bracket: Option<TokenId>,
) -> bool {
    elements
        .iter()
        .any(|&element| has_line_comment_before(ast, ast.begin_token(element)))
        || right_bracket.is_some_and(|bracket| has_line_comment_before(ast, bracket))
}

// ---------------------------------------------------------------------------
// ExpressionExtensions

/// Dart `Expression.isHomogeneousCollectionBody`: whether this expression is
/// a list, set, or map literal whose elements all have a homogeneous type.
///
/// In that case, the elements are relatively loosely related to each other.
/// The collection has an unbounded number of them and the contents tend to
/// change frequently.
///
/// This isn't true for record literal fields and function call arguments. In
/// those cases, each argument position is meaningful and it's easiest to
/// read them all together.
///
/// Thus it makes sense for the formatter to be looser about splitting list,
/// map, and set literals, while trying to avoid splitting argument lists and
/// records.
pub fn is_homogeneous_collection_body(ast: &Ast, expression: NodeId) -> bool {
    matches!(
        ast.kind(expression),
        NodeKind::ListLiteral | NodeKind::SetOrMapLiteral
    )
}

/// Dart `Expression.canBlockSplit`: whether this expression is a non-empty
/// delimited container for inner expressions that allows "block-like"
/// formatting in some contexts. For example, in an assignment, a split in
/// the assigned value is usually indented:
///
///     var variableName =
///         longValue;
///
/// But if the initializer is block-like, we don't split at the `=`:
///
///     var variableName = [
///       element,
///     ];
///
/// Likewise, in an argument list, block-like expressions can avoid splitting
/// the surrounding argument list:
///
///     function([
///       element,
///     ]);
///
/// Completely empty delimited constructs like `[]` and `foo()` don't allow
/// splitting inside them, so are not considered block-like.
pub fn can_block_split(ast: &Ast, expression: Id<Expression>) -> bool {
    block_format_type(ast, expression) != BlockFormat::None
}

/// Dart `Expression.blockFormatType`: when this expression is in an argument
/// list, what kind of block formatting category it belongs to.
pub fn block_format_type(ast: &Ast, expression: Id<Expression>) -> BlockFormat {
    let node = expression.raw();
    match ast.kind(node) {
        // Allow the target of a single-section cascade to be block formatted.
        NodeKind::CascadeExpression => {
            let cascade = &ast[Id::<CascadeExpression>::from_raw(node)];
            if cascade.cascade_sections.len() == 1 && can_block_split(ast, cascade.target) {
                return BlockFormat::Invocation;
            }
        }

        // A function expression with a non-empty block body can block format.
        NodeKind::FunctionExpression => {
            let function = &ast[Id::<FunctionExpression>::from_raw(node)];
            if let Some(body) = ast.cast::<BlockFunctionBody>(function.body) {
                let block = &ast[ast[body].block];
                if can_split(ast, block.statements, block.right_bracket) {
                    return BlockFormat::Function;
                }
            }
        }

        NodeKind::FunctionExpressionInvocation => {
            let invocation = &ast[Id::<FunctionExpressionInvocation>::from_raw(node)];
            // An immediately invoked function expression is formatted like a
            // function expression.
            if ast.kind(invocation.function) == NodeKind::FunctionExpression
                && block_format_type(ast, invocation.function) == BlockFormat::Function
            {
                return BlockFormat::Function;
            }
            // Function calls can block split if their argument lists can.
            let arguments = &ast[invocation.argument_list];
            if can_split(ast, arguments.arguments, arguments.right_parenthesis) {
                return BlockFormat::Invocation;
            }
        }

        // Non-empty collection literals can block split.
        NodeKind::ListLiteral => {
            let list = &ast[Id::<ListLiteral>::from_raw(node)];
            if can_split(ast, list.elements, list.right_bracket) {
                return BlockFormat::Collection;
            }
        }
        NodeKind::SetOrMapLiteral => {
            let set_or_map = &ast[Id::<SetOrMapLiteral>::from_raw(node)];
            if can_split(ast, set_or_map.elements, set_or_map.right_bracket) {
                return BlockFormat::Collection;
            }
        }
        NodeKind::RecordLiteral => {
            let record = &ast[Id::<RecordLiteral>::from_raw(node)];
            if can_split(ast, record.fields, record.right_parenthesis) {
                return BlockFormat::Collection;
            }
        }
        NodeKind::SwitchExpression => {
            let switch = &ast[Id::<SwitchExpression>::from_raw(node)];
            if can_split(ast, switch.cases, switch.right_bracket) {
                return BlockFormat::Collection;
            }
        }

        // Function calls can block split if their argument lists can.
        NodeKind::InstanceCreationExpression => {
            let creation = &ast[Id::<InstanceCreationExpression>::from_raw(node)];
            let arguments = &ast[creation.argument_list];
            if can_split(ast, arguments.arguments, arguments.right_parenthesis) {
                return BlockFormat::Invocation;
            }
        }
        NodeKind::MethodInvocation => {
            let invocation = &ast[Id::<MethodInvocation>::from_raw(node)];
            let arguments = &ast[invocation.argument_list];
            if can_split(ast, arguments.arguments, arguments.right_parenthesis) {
                return BlockFormat::Invocation;
            }
        }

        // Multi-line strings can.
        NodeKind::StringInterpolation | NodeKind::SimpleStringLiteral => {
            if string_is_multiline(ast, node) {
                return BlockFormat::Collection;
            }
        }

        // Parenthesized expressions unwrap the inner expression.
        NodeKind::ParenthesizedExpression => {
            let inner = ast[Id::<ParenthesizedExpression>::from_raw(node)].expression;
            return block_format_type(ast, inner);
        }

        // Await expressions unwrap the inner expression.
        NodeKind::AwaitExpression => {
            let inner = ast[Id::<AwaitExpression>::from_raw(node)].expression;
            return block_format_type(ast, inner);
        }
        _ => {}
    }
    BlockFormat::None
}

/// Dart `Expression.hasSingleElement`: whether this expression is a call or
/// collection literal with a single argument or element.
pub fn has_single_element(ast: &Ast, expression: Id<Expression>) -> bool {
    let mut expression = expression;
    loop {
        if let Some(e) = ast.cast::<ParenthesizedExpression>(expression) {
            expression = ast[e].expression;
        } else if let Some(e) = ast.cast::<AwaitExpression>(expression) {
            expression = ast[e].expression;
        } else if let Some(e) = ast.cast::<PostfixExpression>(expression) {
            expression = ast[e].operand;
        } else {
            break;
        }
    }

    let node = expression.raw();
    match ast.kind(node) {
        NodeKind::MethodInvocation => {
            let arguments = ast[Id::<MethodInvocation>::from_raw(node)].argument_list;
            ast[arguments].arguments.len() == 1
        }
        NodeKind::InstanceCreationExpression => {
            let arguments = ast[Id::<InstanceCreationExpression>::from_raw(node)].argument_list;
            ast[arguments].arguments.len() == 1
        }
        NodeKind::FunctionExpressionInvocation => {
            let arguments = ast[Id::<FunctionExpressionInvocation>::from_raw(node)].argument_list;
            ast[arguments].arguments.len() == 1
        }
        NodeKind::ListLiteral => ast[Id::<ListLiteral>::from_raw(node)].elements.len() == 1,
        NodeKind::SetOrMapLiteral => ast[Id::<SetOrMapLiteral>::from_raw(node)].elements.len() == 1,
        NodeKind::RecordLiteral => ast[Id::<RecordLiteral>::from_raw(node)].fields.len() == 1,
        _ => false,
    }
}

/// Dart `Expression.isTrailingCommaArgument`: whether this is an argument in
/// an argument list with a trailing comma.
pub fn is_trailing_comma_argument(ast: &Ast, expression: NodeId) -> bool {
    let mut parent = ast.parent(expression);
    if let Some(p) = parent {
        if ast.kind(p) == NodeKind::NamedArgument {
            parent = ast.parent(p);
        }
    }
    match parent.and_then(|p| ast.cast::<ArgumentList>(p)) {
        Some(list) => list_has_comma_after(ast, ast[list].arguments),
        None => false,
    }
}

/// Dart `Expression.looksLikeStaticCall`: whether this is a method
/// invocation that looks like it might be a static method or constructor
/// call without a `new` keyword.
///
/// With optional `new`, we can no longer reliably identify constructor calls
/// statically, but we still don't want to mix named constructor calls into
/// a call chain like:
///
///     Iterable
///         .generate(...)
///         .toList();
///
/// And instead prefer:
///
///     Iterable.generate(...)
///         .toList();
///
/// So we try to identify these calls syntactically. The heuristic we use is
/// that a target that's a capitalized name (possibly prefixed by "_") is
/// assumed to be a class.
///
/// This has the effect of also keeping static method calls with the class,
/// but that tends to look pretty good too, and is certainly better than
/// splitting up named constructors.
pub fn looks_like_static_call(ast: &Ast, node: NodeId) -> bool {
    let Some(invocation) = ast.cast::<MethodInvocation>(node) else {
        return false;
    };
    let invocation = &ast[invocation];
    let Some(target) = invocation.target else {
        return false;
    };

    // A prefixed unnamed constructor call:
    //
    //     prefix.Foo();
    if ast.kind(target) == NodeKind::SimpleIdentifier
        && looks_like_class_name(simple_identifier_name(ast, invocation.method_name))
    {
        return true;
    }

    // A prefixed or unprefixed named constructor call:
    //
    //     Foo.named();
    //     prefix.Foo.named();
    let mut target = target.raw();
    if let Some(prefixed) = ast.cast::<PrefixedIdentifier>(target) {
        target = ast[prefixed].identifier.raw();
    }

    match ast.cast::<SimpleIdentifier>(target) {
        Some(identifier) => looks_like_class_name(simple_identifier_name(ast, identifier)),
        None => false,
    }
}

/// Dart `ExpressionExtensions._looksLikeClassName`: whether [name] appears
/// to be a type name.
///
/// Type names begin with a capital letter and contain at least one lowercase
/// letter (so that we can distinguish them from SCREAMING_CAPS constants).
pub fn looks_like_class_name(name: &str) -> bool {
    // Handle the weird lowercase corelib names.
    if matches!(name, "bool" | "double" | "int" | "num") {
        return true;
    }

    let bytes = name.as_bytes();
    let mut start = 0;
    let Some(&first) = bytes.first() else {
        return false;
    };
    let mut first_char = first;
    start += 1;

    // It can be private.
    if first_char == b'_' {
        if name.len() == 1 {
            return false;
        }
        first_char = bytes[start];
        start += 1;
    }

    // It must start with a capital letter.
    if !first_char.is_ascii_uppercase() {
        return false;
    }

    // And have at least one lowercase letter in it. Otherwise it could be a
    // SCREAMING_CAPS constant.
    bytes[start..].iter().any(|c| c.is_ascii_lowercase())
}

// ---------------------------------------------------------------------------
// CascadeExpressionExtensions

/// Dart `CascadeExpression.allowInline`: whether a cascade should be allowed
/// to be inline with the target as opposed to moving the sections to the
/// next line.
pub fn cascade_allow_inline(ast: &Ast, cascade: Id<CascadeExpression>) -> bool {
    let cascade = &ast[cascade];
    // Cascades with multiple sections always split.
    if cascade.cascade_sections.len() > 1 {
        return false;
    }
    // If the receiver is an expression that makes the cascade's very low
    // precedence confusing, force it to split. For example:
    //
    //     a ? b : c..d();
    //
    // Here, the cascade is applied to the result of the conditional, not
    // just "c".
    !matches!(
        ast.kind(cascade.target),
        NodeKind::ConditionalExpression
            | NodeKind::BinaryExpression
            | NodeKind::PrefixExpression
            | NodeKind::AwaitExpression
    )
}

// ---------------------------------------------------------------------------
// AdjacentStringsExtensions

/// Dart `AdjacentStrings.indentStrings`: whether subsequent strings should
/// be indented relative to the first string.
///
/// We generally prefer to align the strings because it makes them easier to
/// read as a single paragraph of text (which they often are):
///
///     function(
///       'This is a long string message '
///       'split across multiple lines.',
///     )
///
/// But this is hard to read if there are other string arguments:
///
///     function(
///       'This is a long string message '
///       'split across multiple lines.',
///       'This is a separate argument.',
///     )
///
/// Here, unless you carefully notice the commas, it's hard to tell how many
/// arguments there are.
///
/// To balance these, we omit the indentation in argument lists only if there
/// are no other string arguments.
pub fn indent_strings(ast: &Ast, strings: Id<AdjacentStrings>) -> bool {
    let Some(parent) = ast.parent(strings) else {
        return true;
    };
    if let Some(list) = ast.cast::<ArgumentList>(parent) {
        return has_other_string_argument(ast, strings, ast.list_raw(ast[list].arguments));
    }
    // Treat asserts like argument lists.
    if let Some((condition, message)) = assertion_parts(ast, parent) {
        let mut arguments = vec![condition];
        arguments.extend(message);
        return has_other_string_argument(ast, strings, &arguments);
    }
    true
}

/// Dart `AdjacentStrings.indentStrings3Dot7`: whether subsequent strings
/// should be indented relative to the first string (in 3.7 style).
///
/// We generally want to indent adjacent strings because it can be confusing
/// otherwise when they appear in a list of expressions, like:
///
///     [
///       "one",
///       "two"
///       "three",
///       "four"
///     ]
///
/// Especially when these strings are longer, it can be hard to tell that
/// "three" is a continuation of the previous element.
///
/// However, the indentation is distracting in places that don't suffer from
/// this ambiguity:
///
///     var description =
///         "A very long description..."
///             "this extra indentation is unnecessary.");
///
/// To balance these, we omit the indentation when an adjacent string
/// expression is in a context where it's unlikely to be confusing.
pub fn indent_strings_3_dot_7(ast: &Ast, strings: Id<AdjacentStrings>) -> bool {
    let Some(parent) = ast.parent(strings) else {
        return true;
    };
    if let Some(list) = ast.cast::<ArgumentList>(parent) {
        return has_other_string_argument(ast, strings, ast.list_raw(ast[list].arguments));
    }
    // Treat asserts like argument lists.
    if let Some((condition, message)) = assertion_parts(ast, parent) {
        let mut arguments = vec![condition];
        arguments.extend(message);
        return has_other_string_argument(ast, strings, &arguments);
    }
    match ast.kind(parent) {
        // Don't add extra indentation in a variable initializer or assignment:
        //
        //     var variable =
        //         "no extra"
        //         "indent";
        NodeKind::VariableDeclaration => false,
        NodeKind::AssignmentExpression
            if ast[Id::<AssignmentExpression>::from_raw(parent)]
                .right_hand_side
                .raw()
                == strings.raw() =>
        {
            false
        }

        // Don't indent when following `:`.
        NodeKind::MapLiteralEntry
            if ast[Id::<MapLiteralEntry>::from_raw(parent)].value.raw() == strings.raw() =>
        {
            false
        }
        NodeKind::NamedArgument => false,
        NodeKind::RecordLiteralNamedField => false,

        // Don't indent when the body of a `=>` function.
        NodeKind::ExpressionFunctionBody => false,
        _ => true,
    }
}

fn has_other_string_argument(
    ast: &Ast,
    strings: Id<AdjacentStrings>,
    arguments: &[NodeId],
) -> bool {
    arguments
        .iter()
        .any(|&argument| argument != strings.raw() && ast.is::<StringLiteral>(argument))
}

/// The condition and message of an `Assertion` (`AssertStatement` or
/// `AssertInitializer`).
fn assertion_parts(ast: &Ast, node: NodeId) -> Option<(NodeId, Option<NodeId>)> {
    if let Some(statement) = ast.cast::<AssertStatement>(node) {
        let statement = &ast[statement];
        return Some((statement.condition.raw(), statement.message.map(Id::raw)));
    }
    if let Some(initializer) = ast.cast::<AssertInitializer>(node) {
        let initializer = &ast[initializer];
        return Some((
            initializer.condition.raw(),
            initializer.message.map(Id::raw),
        ));
    }
    None
}

// ---------------------------------------------------------------------------
// PatternExtensions

/// Dart `DartPattern.canBlockSplit`: whether this pattern is a non-empty
/// delimited container for inner expressions that allows "block-like"
/// formatting in some contexts.
pub fn pattern_can_block_split(ast: &Ast, pattern: NodeId) -> bool {
    match ast.kind(pattern) {
        NodeKind::ConstantPattern => can_block_split(
            ast,
            ast[Id::<ConstantPattern>::from_raw(pattern)].expression,
        ),
        NodeKind::ListPattern => {
            let p = &ast[Id::<ListPattern>::from_raw(pattern)];
            can_split(ast, p.elements, p.right_bracket)
        }
        NodeKind::MapPattern => {
            let p = &ast[Id::<MapPattern>::from_raw(pattern)];
            can_split(ast, p.elements, p.right_bracket)
        }
        NodeKind::ObjectPattern => {
            let p = &ast[Id::<ObjectPattern>::from_raw(pattern)];
            can_split(ast, p.fields, p.right_parenthesis)
        }
        NodeKind::RecordPattern => {
            let p = &ast[Id::<RecordPattern>::from_raw(pattern)];
            can_split(ast, p.fields, p.right_parenthesis)
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// TokenExtensions

/// Dart `Token.hasCommaBefore`: whether the token before this one is a
/// comma.
pub fn has_comma_before(ast: &Ast, token: TokenId) -> bool {
    let previous = ast.tokens.previous(token);
    previous.is_some() && ast.tokens.ty(previous) == TokenType::COMMA
}

/// Dart `Token.hasLineCommentBefore`: whether this token has a preceding
/// comment that is a line comment.
pub fn has_line_comment_before(ast: &Ast, token: TokenId) -> bool {
    ast.tokens
        .comments(token)
        .any(|comment| ast.tokens.ty(comment) == TokenType::SINGLE_LINE_COMMENT)
}

// ---------------------------------------------------------------------------
// Rust helpers: generic analyzer API.

/// Dart `token.precedingComments != null`.
pub fn has_preceding_comments(ast: &Ast, token: TokenId) -> bool {
    ast.tokens.get(token).preceding_comments.is_some()
}

/// Dart `SimpleIdentifier.name`.
pub fn simple_identifier_name(ast: &Ast, identifier: Id<SimpleIdentifier>) -> &str {
    ast.tokens.lexeme(ast[identifier].token)
}

/// Dart `SimpleStringLiteral.isMultiline` / `StringInterpolation.isMultiline`
/// (`StringLexemeHelper.isMultiline`): whether the string literal starts
/// with `'''` or `"""` (after an optional `r`).
pub fn string_is_multiline(ast: &Ast, node: NodeId) -> bool {
    lexeme_is_multiline(ast.tokens.lexeme(ast.begin_token(node)))
}

/// Dart `StringLexemeHelper.isMultiline` for a string lexeme.
pub fn lexeme_is_multiline(lexeme: &str) -> bool {
    let lexeme = lexeme.strip_prefix('r').unwrap_or(lexeme);
    lexeme.starts_with("'''") || lexeme.starts_with("\"\"\"")
}

/// Dart `StringLexemeHelper.isRaw`: whether the string literal starts with
/// `r`.
pub fn lexeme_is_raw(lexeme: &str) -> bool {
    lexeme.starts_with('r')
}

macro_rules! annotated_kinds {
    ($ast:ident, $node:ident, $name:ident => $body:expr, _ => $default:expr) => {
        match $ast.kind($node) {
            NodeKind::ClassDeclaration => {
                let $name = &$ast[Id::<ClassDeclaration>::from_raw($node)];
                $body
            }
            NodeKind::ClassTypeAlias => {
                let $name = &$ast[Id::<ClassTypeAlias>::from_raw($node)];
                $body
            }
            NodeKind::ConstructorDeclaration => {
                let $name = &$ast[Id::<ConstructorDeclaration>::from_raw($node)];
                $body
            }
            NodeKind::DeclaredIdentifier => {
                let $name = &$ast[Id::<DeclaredIdentifier>::from_raw($node)];
                $body
            }
            NodeKind::EnumConstantDeclaration => {
                let $name = &$ast[Id::<EnumConstantDeclaration>::from_raw($node)];
                $body
            }
            NodeKind::EnumDeclaration => {
                let $name = &$ast[Id::<EnumDeclaration>::from_raw($node)];
                $body
            }
            NodeKind::ExportDirective => {
                let $name = &$ast[Id::<ExportDirective>::from_raw($node)];
                $body
            }
            NodeKind::ExtensionDeclaration => {
                let $name = &$ast[Id::<ExtensionDeclaration>::from_raw($node)];
                $body
            }
            NodeKind::ExtensionTypeDeclaration => {
                let $name = &$ast[Id::<ExtensionTypeDeclaration>::from_raw($node)];
                $body
            }
            NodeKind::FieldDeclaration => {
                let $name = &$ast[Id::<FieldDeclaration>::from_raw($node)];
                $body
            }
            NodeKind::FieldFormalParameter => {
                let $name = &$ast[Id::<FieldFormalParameter>::from_raw($node)];
                $body
            }
            NodeKind::FunctionDeclaration => {
                let $name = &$ast[Id::<FunctionDeclaration>::from_raw($node)];
                $body
            }
            NodeKind::FunctionTypeAlias => {
                let $name = &$ast[Id::<FunctionTypeAlias>::from_raw($node)];
                $body
            }
            NodeKind::GenericTypeAlias => {
                let $name = &$ast[Id::<GenericTypeAlias>::from_raw($node)];
                $body
            }
            NodeKind::ImportDirective => {
                let $name = &$ast[Id::<ImportDirective>::from_raw($node)];
                $body
            }
            NodeKind::LibraryDirective => {
                let $name = &$ast[Id::<LibraryDirective>::from_raw($node)];
                $body
            }
            NodeKind::MethodDeclaration => {
                let $name = &$ast[Id::<MethodDeclaration>::from_raw($node)];
                $body
            }
            NodeKind::MixinDeclaration => {
                let $name = &$ast[Id::<MixinDeclaration>::from_raw($node)];
                $body
            }
            NodeKind::PartDirective => {
                let $name = &$ast[Id::<PartDirective>::from_raw($node)];
                $body
            }
            NodeKind::PartOfDirective => {
                let $name = &$ast[Id::<PartOfDirective>::from_raw($node)];
                $body
            }
            NodeKind::PatternVariableDeclaration => {
                let $name = &$ast[Id::<PatternVariableDeclaration>::from_raw($node)];
                $body
            }
            NodeKind::PrimaryConstructorBody => {
                let $name = &$ast[Id::<PrimaryConstructorBody>::from_raw($node)];
                $body
            }
            NodeKind::RegularFormalParameter => {
                let $name = &$ast[Id::<RegularFormalParameter>::from_raw($node)];
                $body
            }
            NodeKind::SuperFormalParameter => {
                let $name = &$ast[Id::<SuperFormalParameter>::from_raw($node)];
                $body
            }
            NodeKind::TopLevelVariableDeclaration => {
                let $name = &$ast[Id::<TopLevelVariableDeclaration>::from_raw($node)];
                $body
            }
            NodeKind::TypeParameter => {
                let $name = &$ast[Id::<TypeParameter>::from_raw($node)];
                $body
            }
            NodeKind::VariableDeclaration => {
                let $name = &$ast[Id::<VariableDeclaration>::from_raw($node)];
                $body
            }
            NodeKind::VariableDeclarationList => {
                let $name = &$ast[Id::<VariableDeclarationList>::from_raw($node)];
                $body
            }
            _ => $default,
        }
    };
}

/// Dart `node is AnnotatedNode`.
pub fn is_annotated_node(ast: &Ast, node: NodeId) -> bool {
    annotated_metadata(ast, node).is_some()
}

/// Dart `(node as AnnotatedNode).metadata`, or `None` if [node] is not an
/// `AnnotatedNode`.
pub fn annotated_metadata(ast: &Ast, node: NodeId) -> Option<NodeList<Annotation>> {
    annotated_kinds!(ast, node, n => Some(n.metadata), _ => None)
}

/// Dart `(node as AnnotatedNode).documentationComment`.
pub fn annotated_documentation_comment(ast: &Ast, node: NodeId) -> Option<Id<Comment>> {
    annotated_kinds!(ast, node, n => n.documentation_comment, _ => None)
}

/// Dart `(node as AnnotatedNode).firstTokenAfterCommentAndMetadata`, or
/// `None` if [node] is not an `AnnotatedNode`.
pub fn first_token_after_comment_and_metadata(ast: &Ast, node: NodeId) -> Option<TokenId> {
    annotated_kinds!(ast, node, n => Some(n.first_token_after_comment_and_metadata(ast)), _ => None)
}
