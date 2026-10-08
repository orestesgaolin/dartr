// Dart source: pkg/analyzer/lib/src/dart/ast/ast.dart (the hand-written
// getters of the *Impl node classes that resolution uses:
// SimpleIdentifierImpl.inGetterContext / inSetterContext,
// IndexExpressionImpl.inGetterContext / inSetterContext,
// ExpressionImpl.unParenthesized, SwitchStatementImpl.memberGroups, ...)

//! Dart AST getters that are computed from the syntax (not resolution
//! results). Add a function here when a ported file needs another one; keep
//! the Dart name.

use dartr_ast::{
    Ast, AssignmentExpression, ConstructorFieldInitializer, Expression, ForEachPartsWithIdentifier,
    Id, IndexExpression, Label, NodeId, NodeList, ParenthesizedExpression, PostfixExpression,
    PrefixExpression, PrefixedIdentifier, PropertyAccess, SimpleIdentifier, Statement, SwitchCase,
    SwitchDefault, SwitchMember, SwitchPatternCase, SwitchStatement,
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

/// Dart `SwitchMember.labels`.
pub fn switch_member_labels(ast: &Ast, member: Id<SwitchMember>) -> NodeList<Label> {
    let node: NodeId = member.raw();
    if let Some(m) = ast.cast::<SwitchCase>(node) {
        ast[m].labels
    } else if let Some(m) = ast.cast::<SwitchDefault>(node) {
        ast[m].labels
    } else if let Some(m) = ast.cast::<SwitchPatternCase>(node) {
        ast[m].labels
    } else {
        unreachable!("unknown SwitchMember {:?}", ast.kind(node))
    }
}

/// Dart `SwitchMember.statements`.
pub fn switch_member_statements(ast: &Ast, member: Id<SwitchMember>) -> NodeList<Statement> {
    let node: NodeId = member.raw();
    if let Some(m) = ast.cast::<SwitchCase>(node) {
        ast[m].statements
    } else if let Some(m) = ast.cast::<SwitchDefault>(node) {
        ast[m].statements
    } else if let Some(m) = ast.cast::<SwitchPatternCase>(node) {
        ast[m].statements
    } else {
        unreachable!("unknown SwitchMember {:?}", ast.kind(node))
    }
}

/// Dart `SwitchStatementCaseGroup` (without the `variables`, which are
/// resolution data).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwitchStatementCaseGroup {
    pub members: Vec<Id<SwitchMember>>,
    pub has_labels: bool,
}

impl SwitchStatementCaseGroup {
    /// Dart `SwitchStatementCaseGroup.statements`: the statements of the
    /// last member.
    pub fn statements(&self, ast: &Ast) -> NodeList<Statement> {
        let last = *self.members.last().expect("a group has members");
        switch_member_statements(ast, last)
    }
}

/// Dart `SwitchStatementImpl.memberGroups`: the members, grouped so that
/// the members of one group share the statements of the last one.
pub fn switch_statement_member_groups(
    ast: &Ast,
    node: Id<SwitchStatement>,
) -> Vec<SwitchStatementCaseGroup> {
    let mut groups = Vec::new();
    let mut group_members = Vec::new();
    let mut group_has_labels = false;
    for &member in ast.list(ast[node].members) {
        group_members.push(member);
        group_has_labels |= !switch_member_labels(ast, member).is_empty();
        if !switch_member_statements(ast, member).is_empty() {
            groups.push(SwitchStatementCaseGroup {
                members: std::mem::take(&mut group_members),
                has_labels: group_has_labels,
            });
            group_has_labels = false;
        }
    }
    if !group_members.is_empty() {
        groups.push(SwitchStatementCaseGroup {
            members: group_members,
            has_labels: group_has_labels,
        });
    }
    groups
}
