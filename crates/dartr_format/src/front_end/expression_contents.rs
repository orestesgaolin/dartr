// Dart source: dart_style lib/src/front_end/expression_contents.dart

use dartr_ast::{Ast, NamedArgument, NodeId, NodeKind, PrefixExpression};
use dartr_syntax::TokenType;

/// Tracks the contents of a nested tree of argument lists and collection
/// literals.
///
/// In general, the formatter tries to pack as much as it can on a single
/// line until it hits the page width. However, with deeply nested call trees
/// (which are pervasive in Flutter UI code), the expression nesting can get
/// deep even in a short piece of code.
///
/// It can be much easier to track the nesting structure and identify
/// siblings in the expression tree if it's forced to split more eagerly.
/// Compare:
///
///     Apple(banana: [Cherry(date: Eggplant(1, 2))], fig: Grape(4))
///
///     Apple(
///       banana: [
///         Cherry(date: Eggplant(1, 2)),
///       ],
///       fig: Grape(4),
///     )
///
/// This class records the necessary state to determine if a given
/// collection literal or argument list is complex enough that it should be
/// eagerly split.
///
/// It considers an operation A to contain another B if B occurs anywhere
/// transitively inside the elements or argument list of A, regardless of any
/// other AST nodes that may intercede. If we only looked at the immediate
/// expressions in the collection or argument list to count nested calls and
/// collections, then wrapping one of those expressions in, say, parentheses,
/// could cause a nested operation to *not* be counted.
///
/// That would violate a reasonable principle that *adding* code to a call or
/// collection should never cause it to go from splitting to not splitting.
/// If a collection or call is complex enough to warrant splitting it
/// eagerly, then adding more code in there should always lead to it still
/// splitting. Tracking the contents transitively ensures that.
///
/// The heuristics for which collections and argument lists split are fairly
/// simple and conservative and are documented below.
pub struct ExpressionContents {
    /// The stack of calls and collections whose contents we are tracking and
    /// that haven't completed yet.
    stack: Vec<Contents>,
}

impl Default for ExpressionContents {
    fn default() -> Self {
        ExpressionContents {
            stack: vec![Contents::new(ContentsType::OtherCall, 0)],
        }
    }
}

impl ExpressionContents {
    /// Begins tracking an argument list.
    pub fn begin_call(&mut self, ast: &Ast, arguments: &[NodeId]) {
        let mut ty = ContentsType::OtherCall;

        // Count the non-trivial named arguments in this call.
        let mut named_arguments = 0;
        for &argument in arguments {
            if let Some(named) = ast.cast::<NamedArgument>(argument) {
                ty = ContentsType::CallWithNamedArgument;
                if !is_trivial(ast, ast[named].argument_expression.raw()) {
                    named_arguments += 1;
                }
            }
        }

        self.stack.push(Contents::new(ty, named_arguments));
    }

    /// Ends the most recently begun call and returns `true` if its argument
    /// list should eagerly split.
    pub fn end_call(&mut self) -> bool {
        let contents = self.end();

        // If there are "too many" named arguments in this call and the calls
        // it contains, then split it.
        //
        // The basic idea is that when scanning a line of code, it's hard to
        // tell which calls own which named arguments if there are named
        // arguments at multiple levels in the call tree. Splitting makes that
        // clearer. At the same time, it's annoying it the formatter is too
        // aggressive about splitting an expression that feels simple enough
        // to the reader to fit on one line. (Especially because if the
        // formatter does eagerly split it, there's nothing they can do to
        // *prevent* that.)
        //
        // The heuristic here tries to strike a "Goldilocks" balance between
        // not splitting too aggressively or too conservatively. The rule is
        // that the entire call tree must contain at least three named
        // arguments, at least one must be in the outermost call being split,
        // and at least one must *not* be in the outermost call.
        //
        // It would be simpler to split any call that has named arguments at
        // different nesting levels, but that's a little too aggressive and
        // forces common code like this to split:
        //
        //       Text('Item 1', style: TextStyle(color: Colors.white));
        contents.total_named_arguments() > 2
            && contents.named_arguments > 0
            && contents.nested_named_arguments > 0
    }

    /// Begin tracking a collection literal and its contents.
    pub fn begin_collection(&mut self, is_named: bool) {
        self.stack.last_mut().unwrap().collections += 1;
        self.stack.push(Contents::new(
            if is_named {
                ContentsType::NamedCollection
            } else {
                ContentsType::Collection
            },
            0,
        ));
    }

    /// Ends the most recently begun collection literal and returns whether
    /// it should eagerly split.
    pub fn end_collection(&mut self, element_count: usize) -> bool {
        let contents = self.end();

        // Split any collection that contains another non-empty collection.
        if contents.collections > 0 {
            return true;
        }

        // If the collection is itself a named argument in a surrounding call
        // that may be be forced to eagerly split, then split the collection
        // too. In that case, the collection is sort of like a vararg argument
        // to the call. Prefers:
        //
        //     TabBar(
        //       tabs: <Widget>[
        //         Tab(text: 'Tab 1'),
        //         Tab(text: 'Tab 2'),
        //       ],
        //     );
        //
        // Over:
        //
        //     TabBar(
        //       tabs: <Widget>[Tab(text: 'Tab 1'), Tab(text: 'Tab 2')],
        //     );
        //
        // Splitting a collection is also helpful, because it shows each
        // element in parallel with each on its own line. But that's only true
        // when there are multiple elements, so we don't eagerly split
        // collections with just a single element.
        element_count > 1
            && contents.ty == ContentsType::NamedCollection
            && contents.total_named_arguments() > 0
    }

    /// Ends the most recently begun operation and returns its contents.
    fn end(&mut self) -> Contents {
        let contents = self.stack.pop().unwrap();

        // Transitively include this operation's contents in the surrounding
        // one.
        let parent = self.stack.last_mut().unwrap();
        parent.collections += contents.collections;
        parent.nested_named_arguments +=
            contents.named_arguments + contents.nested_named_arguments;

        contents
    }
}

/// Whether [expression] is "trivial".
///
/// When deciding whether an argument list should be eagerly split, or
/// should force surrounding argument lists to eagerly split, we ignore any
/// named arguments whose expression is "trivial". This allows a little more
/// code to be packed onto a single line when the inner call is creating a
/// simple data structure with literal values, like:
///
///     MediaQueryData(padding: EdgeInsets.only(left: 40));
///
/// Here, if we didn't treat `40` as a trivial expression and ignore it, then
/// the call to `MediaQueryData(...)` would be forced to split.
fn is_trivial(ast: &Ast, expression: NodeId) -> bool {
    match ast.kind(expression) {
        NodeKind::NullLiteral
        | NodeKind::BooleanLiteral
        | NodeKind::IntegerLiteral
        | NodeKind::DoubleLiteral => true,
        NodeKind::PrefixExpression => {
            let prefix = &ast[ast.cast::<PrefixExpression>(expression).unwrap()];
            ast.tokens.ty(prefix.operator) == TokenType::MINUS
                && is_trivial(ast, prefix.operand.raw())
        }
        _ => false,
    }
}

/// The number of function calls and collection literals occurring
/// transitively inside some other operation.
struct Contents {
    ty: ContentsType,

    /// The number of non-empty list, set, and map literals transitively
    /// inside this operation.
    collections: i32,

    /// The number of non-trivial named arguments in this call's own argument
    /// list.
    named_arguments: i32,

    /// The number of non-trivial named arguments transitively inside this
    /// operation, but not including the call's own named arguments.
    nested_named_arguments: i32,
}

impl Contents {
    fn new(ty: ContentsType, named_arguments: i32) -> Contents {
        Contents {
            ty,
            collections: 0,
            named_arguments,
            nested_named_arguments: 0,
        }
    }

    /// The total number of non-trivial named arguments in this operation's
    /// own argument list and all of transitive contents.
    fn total_named_arguments(&self) -> i32 {
        self.named_arguments + self.nested_named_arguments
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ContentsType {
    /// A non-empty list, map, or set literal.
    Collection,

    /// A non-empty list, map, or set literal that is the immediate expression
    /// in a named argument in a surrounding argument list.
    NamedCollection,

    /// An argument list with at least one named argument and which may be
    /// subject to eager splitting.
    CallWithNamedArgument,

    /// An argument list with no named arguments that isn't subject to eager
    /// splitting.
    OtherCall,
}
