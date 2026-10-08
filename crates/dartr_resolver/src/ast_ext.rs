// Dart source: pkg/analyzer/lib/src/dart/ast/ast.dart (the hand-written
// getters of the *Impl node classes that resolution uses:
// SimpleIdentifierImpl.inGetterContext / inSetterContext,
// IndexExpressionImpl.inGetterContext / inSetterContext,
// ExpressionImpl.unParenthesized, ...)

//! Dart AST getters that are computed from the syntax (not resolution
//! results). Add a function here when a ported file needs another one; keep
//! the Dart name.

use dartr_ast::{
    Ast, AssignmentExpression, ConstructorFieldInitializer, Expression, ForEachPartsWithIdentifier,
    Id, IndexExpression, Label, NodeId, ParenthesizedExpression, PostfixExpression,
    PrefixExpression, PrefixedIdentifier, PropertyAccess, SimpleIdentifier,
};
use dartr_syntax::TokenType;

/// Dart `TokenType.isIncrementOperator`.
pub fn is_increment_operator(ty: TokenType) -> bool {
    ty == TokenType::PLUS_PLUS || ty == TokenType::MINUS_MINUS
}

/// Dart `SimpleIdentifierImpl.name`.
pub fn identifier_name(ast: &Ast, node: Id<SimpleIdentifier>) -> &str {
    ast.tokens.lexeme(ast[node].token)
}

/// Dart `SimpleIdentifierImpl.inGetterContext()`.
pub fn simple_identifier_in_getter_context(ast: &Ast, node: Id<SimpleIdentifier>) -> bool {
    let this: NodeId = node.raw();
    let initial_parent = ast.parent(this).expect("parent");
    let mut parent = initial_parent;
    let mut target = this;
    // skip prefix
    if let Some(p) = ast.cast::<PrefixedIdentifier>(initial_parent) {
        if ast[p].prefix.raw() == this {
            return true;
        }
        parent = ast.parent(initial_parent).expect("parent");
        target = initial_parent;
    } else if let Some(p) = ast.cast::<PropertyAccess>(initial_parent) {
        if ast[p].target.map(|t| t.raw()) == Some(this) {
            return true;
        }
        parent = ast.parent(initial_parent).expect("parent");
        target = initial_parent;
    }
    // skip label
    if ast.is::<Label>(parent) {
        return false;
    }
    // analyze usage
    if let Some(a) = ast.cast::<AssignmentExpression>(parent) {
        if ast[a].left_hand_side.raw() == target && ast.tokens.ty(ast[a].operator) == TokenType::EQ
        {
            return false;
        }
    }
    if let Some(c) = ast.cast::<ConstructorFieldInitializer>(parent) {
        if ast[c].field_name.raw() == target {
            return false;
        }
    }
    if let Some(f) = ast.cast::<ForEachPartsWithIdentifier>(parent) {
        if ast[f].identifier.raw() == target {
            return false;
        }
    }
    true
}

/// Dart `SimpleIdentifierImpl.inSetterContext()`.
pub fn simple_identifier_in_setter_context(ast: &Ast, node: Id<SimpleIdentifier>) -> bool {
    let this: NodeId = node.raw();
    let initial_parent = ast.parent(this).expect("parent");
    let mut parent = initial_parent;
    let mut target = this;
    // skip prefix
    if let Some(p) = ast.cast::<PrefixedIdentifier>(initial_parent) {
        // if this is the prefix, then return false
        if ast[p].prefix.raw() == this {
            return false;
        }
        parent = ast.parent(initial_parent).expect("parent");
        target = initial_parent;
    } else if let Some(p) = ast.cast::<PropertyAccess>(initial_parent) {
        if ast[p].target.map(|t| t.raw()) == Some(this) {
            return false;
        }
        parent = ast.parent(initial_parent).expect("parent");
        target = initial_parent;
    }
    // analyze usage
    if let Some(p) = ast.cast::<PrefixExpression>(parent) {
        is_increment_operator(ast.tokens.ty(ast[p].operator))
    } else if let Some(p) = ast.cast::<PostfixExpression>(parent) {
        is_increment_operator(ast.tokens.ty(ast[p].operator))
    } else if let Some(a) = ast.cast::<AssignmentExpression>(parent) {
        ast[a].left_hand_side.raw() == target
    } else if let Some(f) = ast.cast::<ForEachPartsWithIdentifier>(parent) {
        ast[f].identifier.raw() == target
    } else {
        false
    }
}

/// Dart `IndexExpressionImpl.inGetterContext()`.
pub fn index_expression_in_getter_context(ast: &Ast, node: Id<IndexExpression>) -> bool {
    let parent = ast.parent(node).expect("parent");
    if let Some(a) = ast.cast::<AssignmentExpression>(parent) {
        if ast[a].left_hand_side.raw() == node.raw()
            && ast.tokens.ty(ast[a].operator) == TokenType::EQ
        {
            return false;
        }
    }
    true
}

/// Dart `IndexExpressionImpl.inSetterContext()`.
pub fn index_expression_in_setter_context(ast: &Ast, node: Id<IndexExpression>) -> bool {
    let parent = ast.parent(node).expect("parent");
    if let Some(p) = ast.cast::<PrefixExpression>(parent) {
        is_increment_operator(ast.tokens.ty(ast[p].operator))
    } else if let Some(p) = ast.cast::<PostfixExpression>(parent) {
        is_increment_operator(ast.tokens.ty(ast[p].operator))
    } else if let Some(a) = ast.cast::<AssignmentExpression>(parent) {
        ast[a].left_hand_side.raw() == node.raw()
    } else {
        false
    }
}

/// Dart `ExpressionImpl.unParenthesized`.
pub fn un_parenthesized(ast: &Ast, mut node: Id<Expression>) -> Id<Expression> {
    while let Some(p) = ast.cast::<ParenthesizedExpression>(node) {
        node = ast[p].expression;
    }
    node
}
