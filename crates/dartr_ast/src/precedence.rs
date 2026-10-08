// Dart source: pkg/analyzer/lib/dart/ast/precedence.dart
// Dart source: pkg/analyzer/lib/src/dart/ast/ast.dart (`Expression.precedence`)

//! Operator precedence of expressions.

use crate::arena::{Ast, Id, NodeId};
use crate::generated::nodes::*;

/// Dart `Precedence`: the precedence of an expression (the index is the
/// `TokenType.precedence` of the scanner).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct Precedence(pub u8);

impl Precedence {
    pub const NONE: Precedence = Precedence(0);
    pub const ASSIGNMENT: Precedence = Precedence(1);
    pub const CASCADE: Precedence = Precedence(2);
    pub const CONDITIONAL: Precedence = Precedence(3);
    pub const IF_NULL: Precedence = Precedence(4);
    pub const LOGICAL_OR: Precedence = Precedence(5);
    pub const LOGICAL_AND: Precedence = Precedence(6);
    pub const EQUALITY: Precedence = Precedence(7);
    pub const RELATIONAL: Precedence = Precedence(8);
    pub const BITWISE_OR: Precedence = Precedence(9);
    pub const BITWISE_XOR: Precedence = Precedence(10);
    pub const BITWISE_AND: Precedence = Precedence(11);
    pub const SHIFT: Precedence = Precedence(12);
    pub const ADDITIVE: Precedence = Precedence(13);
    pub const MULTIPLICATIVE: Precedence = Precedence(14);
    pub const PREFIX: Precedence = Precedence(15);
    pub const POSTFIX: Precedence = Precedence(16);
    pub const PRIMARY: Precedence = Precedence(17);
}

/// Dart `PatternPrecedence`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct PatternPrecedence(pub u8);

impl PatternPrecedence {
    pub const LOGICAL_OR: PatternPrecedence = PatternPrecedence(1);
    pub const LOGICAL_AND: PatternPrecedence = PatternPrecedence(2);
    pub const RELATIONAL: PatternPrecedence = PatternPrecedence(3);
    pub const POSTFIX: PatternPrecedence = PatternPrecedence(4);
    pub const PRIMARY: PatternPrecedence = PatternPrecedence(5);
}

impl Ast {
    /// Dart `Expression.precedence`. [id] must be an expression.
    pub fn precedence(&self, id: impl Into<NodeId>) -> Precedence {
        let id: NodeId = id.into();
        use NodeKind as K;
        match self.kind(id) {
            K::AnonymousMethodInvocation
            | K::ConstructorReference
            | K::DotShorthandConstructorInvocation
            | K::DotShorthandInvocation
            | K::DotShorthandPropertyAccess
            | K::ExtensionOverride
            | K::FunctionExpressionInvocation
            | K::IndexExpression
            | K::MethodInvocation
            | K::PostfixExpression
            | K::PrefixedIdentifier
            | K::PropertyAccess => Precedence::POSTFIX,
            K::AsExpression | K::IsExpression => Precedence::RELATIONAL,
            K::AssignmentExpression
            | K::PatternAssignment
            | K::RethrowExpression
            | K::ThrowExpression => Precedence::ASSIGNMENT,
            K::AwaitExpression | K::PrefixExpression => Precedence::PREFIX,
            K::BinaryExpression => {
                let n = &self[Id::<BinaryExpression>::from_raw(id)];
                Precedence(self.tokens.ty(n.operator).precedence())
            }
            K::CascadeExpression => Precedence::CASCADE,
            K::ConditionalExpression => Precedence::CONDITIONAL,
            K::FunctionReference => {
                let n = &self[Id::<FunctionReference>::from_raw(id)];
                if n.type_arguments.is_none() {
                    self.precedence(n.function)
                } else {
                    Precedence::POSTFIX
                }
            }
            K::ImplicitCallReference => {
                let n = &self[Id::<ImplicitCallReference>::from_raw(id)];
                if n.type_arguments.is_none() {
                    self.precedence(n.expression)
                } else {
                    Precedence::POSTFIX
                }
            }
            K::TypeLiteral => {
                let n = &self[Id::<TypeLiteral>::from_raw(id)];
                let t = &self[n.type_];
                if t.type_arguments.is_some() || t.import_prefix.is_some() {
                    Precedence::POSTFIX
                } else {
                    Precedence::PRIMARY
                }
            }
            _ => Precedence::PRIMARY,
        }
    }
}
