// Dart source: pkg/analyzer/lib/src/dart/ast/ast.dart
// Dart source: pkg/analyzer/lib/src/generated/utilities_dart.dart (ParameterKind)

//! Hand-written node behavior: the members that the Dart generator does not
//! generate (`@DoNotGenerate`, and the classes without `@GenerateNodeImpl`:
//! `CommentImpl`, `CompilationUnitImpl`).

use dartr_syntax::TokenId;

use crate::arena::{Ast, Entity, NodeId};
use crate::generated::nodes::*;

/// Dart `ParameterKind`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Default)]
pub enum ParameterKind {
    /// A positional required parameter.
    #[default]
    Required,
    /// A positional optional parameter.
    Positional,
    /// A named required parameter.
    NamedRequired,
    /// A named optional parameter.
    Named,
}

impl ParameterKind {
    pub fn name(self) -> &'static str {
        match self {
            ParameterKind::Required => "REQUIRED",
            ParameterKind::Positional => "POSITIONAL",
            ParameterKind::NamedRequired => "NAMED_REQUIRED",
            ParameterKind::Named => "NAMED",
        }
    }

    pub fn is_positional(self) -> bool {
        matches!(self, ParameterKind::Required | ParameterKind::Positional)
    }

    pub fn is_required_positional(self) -> bool {
        self == ParameterKind::Required
    }

    pub fn is_optional_positional(self) -> bool {
        self == ParameterKind::Positional
    }

    pub fn is_named(self) -> bool {
        matches!(self, ParameterKind::NamedRequired | ParameterKind::Named)
    }

    pub fn is_required_named(self) -> bool {
        self == ParameterKind::NamedRequired
    }

    pub fn is_optional_named(self) -> bool {
        self == ParameterKind::Named
    }

    pub fn is_required(self) -> bool {
        matches!(self, ParameterKind::Required | ParameterKind::NamedRequired)
    }

    pub fn is_optional(self) -> bool {
        matches!(self, ParameterKind::Positional | ParameterKind::Named)
    }
}

impl Comment {
    /// Dart `CommentImpl.beginToken`: `tokens[0]`.
    pub fn begin_token(&self, ast: &Ast) -> TokenId {
        ast.token_list(self.tokens)[0]
    }

    /// Dart `CommentImpl.endToken`: `tokens[tokens.length - 1]`.
    pub fn end_token(&self, ast: &Ast) -> TokenId {
        *ast.token_list(self.tokens).last().unwrap()
    }

    /// Dart `CommentImpl._childEntities`: references, then tokens.
    pub(crate) fn child_entities_unsorted(&self, ast: &Ast, out: &mut Vec<Entity>) {
        out.extend(ast.list_raw(self.references).iter().map(|&n| Entity::Node(n)));
        out.extend(ast.token_list(self.tokens).iter().map(|&t| Entity::Token(t)));
    }
}

impl CompilationUnit {
    /// Dart `CompilationUnitImpl.beginToken` (a field).
    pub fn begin_token(&self, _ast: &Ast) -> TokenId {
        self.begin_token
    }

    /// Dart `CompilationUnitImpl.endToken` (a field).
    pub fn end_token(&self, _ast: &Ast) -> TokenId {
        self.end_token
    }

    /// Dart `CompilationUnitImpl._childEntities`.
    pub(crate) fn child_entities_unsorted(&self, ast: &Ast, out: &mut Vec<Entity>) {
        if let Some(s) = self.script_tag {
            out.push(Entity::Node(s.raw()));
        }
        out.extend(ast.list_raw(self.directives).iter().map(|&n| Entity::Node(n)));
        out.extend(ast.list_raw(self.declarations).iter().map(|&n| Entity::Node(n)));
    }
}

impl FormalParameterList {
    /// Dart `FormalParameterListImpl._childEntities` ("special logic for
    /// delimiters"): the left delimiter is added before the first parameter
    /// after it, and is missing when there is no such parameter.
    pub(crate) fn child_entities_unsorted(&self, ast: &Ast, out: &mut Vec<Entity>) {
        out.push(Entity::Token(self.left_parenthesis));
        let mut left_delimiter_needed = self.left_delimiter.is_some();
        for &parameter in ast.list_raw(self.parameters) {
            if left_delimiter_needed {
                let d = self.left_delimiter.unwrap();
                if ast.tokens.offset(d) < ast.offset(parameter) {
                    out.push(Entity::Token(d));
                    left_delimiter_needed = false;
                }
            }
            out.push(Entity::Node(parameter));
        }
        if let Some(d) = self.right_delimiter {
            out.push(Entity::Token(d));
        }
        out.push(Entity::Token(self.right_parenthesis));
    }
}

impl PostfixExpression {
    /// Dart `PostfixExpressionImpl.isInValueExpressionSlot` ("role depends
    /// on operator").
    pub fn is_in_value_expression_slot(&self, ast: &Ast, _child: NodeId) -> bool {
        !is_increment_operator(ast, self.operator)
    }
}

impl PrefixExpression {
    /// Dart `PrefixExpressionImpl.isInValueExpressionSlot`.
    pub fn is_in_value_expression_slot(&self, ast: &Ast, _child: NodeId) -> bool {
        !is_increment_operator(ast, self.operator)
    }
}

/// Dart `TokenType.isIncrementOperator`.
fn is_increment_operator(ast: &Ast, t: TokenId) -> bool {
    matches!(ast.tokens.ty(t).lexeme(), "++" | "--")
}
