// Dart source: pkg/analyzer/lib/src/dart/ast/ast.dart (the hand-written
// getters of the *Impl node classes that resolution uses:
// SimpleIdentifierImpl.inGetterContext / inSetterContext,
// IndexExpressionImpl.inGetterContext / inSetterContext,
// ExpressionImpl.unParenthesized, ...)

//! Dart AST getters that are computed from the syntax (not resolution
//! results). Add a function here when a ported file needs another one; keep
//! the Dart name.

use dartr_ast::{
    Annotation, AssignmentExpression, Ast, BlockFunctionBody, CascadeExpression, Comment,
    ConstantPattern, ConstructorFieldInitializer, DotShorthandConstructorInvocation, Expression,
    ExpressionFunctionBody, FieldFormalParameter, ForEachPartsWithIdentifier,
    FormalParameterDefaultClause, FunctionTypedFormalParameterSuffix, Id, IndexExpression,
    InstanceCreationExpression, Label, LabeledStatement, ListLiteral, MethodInvocation, NodeId,
    NodeKind, NodeList, NodeType, ParameterKind, ParenthesizedExpression, PostfixExpression,
    PrefixExpression, PrefixedIdentifier, PropertyAccess, RecordLiteral, RegularFormalParameter,
    SetOrMapLiteral, SimpleIdentifier, SuperFormalParameter, TypeAnnotation,
    VariableDeclarationList,
};
use dartr_syntax::{TokenId, TokenType};

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

// ------------------------------------------------------------ C1 helpers

/// Dart `Token.nameIfNotEmpty` (element_binding_visitor.dart): the lexeme,
/// or `None` when it is empty (a synthetic identifier).
pub fn name_if_not_empty(ast: &Ast, token: TokenId) -> Option<&str> {
    let lexeme = ast.tokens.lexeme(token);
    if lexeme.is_empty() { None } else { Some(lexeme) }
}

/// Dart `Token.offsetIfNotEmpty` (element_binding_visitor.dart).
pub fn offset_if_not_empty(ast: &Ast, token: TokenId) -> Option<u32> {
    if ast.tokens.lexeme(token).is_empty() {
        None
    } else {
        Some(ast.tokens.offset(token))
    }
}

/// Dart `Token.end`.
pub fn token_end(ast: &Ast, token: TokenId) -> u32 {
    ast.tokens.get(token).end()
}

/// Dart `Token.isSynthetic`.
pub fn token_is_synthetic(ast: &Ast, token: TokenId) -> bool {
    ast.tokens.get(token).is_synthetic()
}

/// Dart `token?.keyword == Keyword.X`, by the keyword lexeme.
pub fn is_keyword(ast: &Ast, token: Option<TokenId>, keyword: &str) -> bool {
    token.is_some_and(|t| ast.tokens.lexeme(t) == keyword)
}

/// Dart `FunctionBody.isAsynchronous`.
pub fn function_body_is_asynchronous(ast: &Ast, body: NodeId) -> bool {
    let keyword = if let Some(b) = ast.cast::<BlockFunctionBody>(body) {
        ast[b].keyword
    } else if let Some(b) = ast.cast::<ExpressionFunctionBody>(body) {
        ast[b].keyword
    } else {
        None
    };
    is_keyword(ast, keyword, "async")
}

/// Dart `FunctionBody.isGenerator`.
pub fn function_body_is_generator(ast: &Ast, body: NodeId) -> bool {
    if let Some(b) = ast.cast::<BlockFunctionBody>(body) {
        ast[b].star.is_some()
    } else if let Some(b) = ast.cast::<ExpressionFunctionBody>(body) {
        ast[b].star.is_some()
    } else {
        false
    }
}

/// The fields of `FormalParameterImpl` that the three formal parameter node
/// kinds (`RegularFormalParameter`, `FieldFormalParameter`,
/// `SuperFormalParameter`) share.
#[derive(Clone, Copy, Debug)]
pub struct FormalParameterParts {
    pub documentation_comment: Option<Id<Comment>>,
    pub metadata: NodeList<Annotation>,
    pub kind: ParameterKind,
    pub covariant_keyword: Option<TokenId>,
    pub const_final_or_var_keyword: Option<TokenId>,
    pub type_: Option<Id<TypeAnnotation>>,
    pub name: Option<TokenId>,
    pub function_typed_suffix: Option<Id<FunctionTypedFormalParameterSuffix>>,
    pub default_clause: Option<Id<FormalParameterDefaultClause>>,
}

impl FormalParameterParts {
    /// Dart `FormalParameterImpl.isConst`.
    pub fn is_const(&self, ast: &Ast) -> bool {
        is_keyword(ast, self.const_final_or_var_keyword, "const")
    }

    /// Dart `FormalParameterImpl.isFinal`.
    pub fn is_final(&self, ast: &Ast) -> bool {
        is_keyword(ast, self.const_final_or_var_keyword, "final")
    }
}

/// The shared fields of a formal parameter node.
pub fn formal_parameter_parts(ast: &Ast, node: NodeId) -> FormalParameterParts {
    if let Some(p) = ast.cast::<RegularFormalParameter>(node) {
        let p = &ast[p];
        FormalParameterParts {
            documentation_comment: p.documentation_comment,
            metadata: p.metadata,
            kind: p.kind,
            covariant_keyword: p.covariant_keyword,
            const_final_or_var_keyword: p.const_final_or_var_keyword,
            type_: p.type_,
            name: p.name,
            function_typed_suffix: p.function_typed_suffix,
            default_clause: p.default_clause,
        }
    } else if let Some(p) = ast.cast::<FieldFormalParameter>(node) {
        let p = &ast[p];
        FormalParameterParts {
            documentation_comment: p.documentation_comment,
            metadata: p.metadata,
            kind: p.kind,
            covariant_keyword: p.covariant_keyword,
            const_final_or_var_keyword: p.const_final_or_var_keyword,
            type_: p.type_,
            name: Some(p.name),
            function_typed_suffix: p.function_typed_suffix,
            default_clause: p.default_clause,
        }
    } else if let Some(p) = ast.cast::<SuperFormalParameter>(node) {
        let p = &ast[p];
        FormalParameterParts {
            documentation_comment: p.documentation_comment,
            metadata: p.metadata,
            kind: p.kind,
            covariant_keyword: p.covariant_keyword,
            const_final_or_var_keyword: p.const_final_or_var_keyword,
            type_: p.type_,
            name: Some(p.name),
            function_typed_suffix: p.function_typed_suffix,
            default_clause: p.default_clause,
        }
    } else {
        panic!("not a formal parameter: {:?}", ast.kind(node))
    }
}

/// Dart `Statement.unlabeled`.
pub fn statement_unlabeled(ast: &Ast, mut statement: NodeId) -> NodeId {
    while let Some(l) = ast.cast::<LabeledStatement>(statement) {
        statement = ast[l].statement.raw();
    }
    statement
}

/// Whether [operator] is `..` or `?..`.
fn is_cascade_operator(ast: &Ast, operator: Option<TokenId>) -> bool {
    operator.is_some_and(|o| matches!(ast.tokens.lexeme(o), ".." | "?.."))
}

/// Dart `MethodInvocationImpl.isCascaded`.
pub fn method_invocation_is_cascaded(ast: &Ast, node: Id<MethodInvocation>) -> bool {
    is_cascade_operator(ast, ast[node].operator)
}

/// Dart `PropertyAccessImpl.isCascaded`.
pub fn property_access_is_cascaded(ast: &Ast, node: Id<PropertyAccess>) -> bool {
    is_cascade_operator(ast, Some(ast[node].operator))
}

/// Dart `_ancestorCascade.target` of a cascaded node: the target of the
/// nearest enclosing `CascadeExpression`.
fn ancestor_cascade_target(ast: &Ast, node: NodeId) -> Option<Id<Expression>> {
    let mut current = ast.parent(node);
    while let Some(p) = current {
        if let Some(c) = ast.cast::<CascadeExpression>(p) {
            return Some(ast[c].target);
        }
        current = ast.parent(p);
    }
    None
}

/// Dart `MethodInvocationImpl.realTarget`.
pub fn method_invocation_real_target(ast: &Ast, node: Id<MethodInvocation>) -> Option<Id<Expression>> {
    if method_invocation_is_cascaded(ast, node) {
        return ancestor_cascade_target(ast, node.raw());
    }
    ast[node].target
}

/// Dart `InstanceCreationExpressionImpl.isConst`.
pub fn instance_creation_is_const(ast: &Ast, node: Id<InstanceCreationExpression>) -> bool {
    match ast[node].keyword {
        Some(k) => ast.tokens.lexeme(k) == "const",
        None => in_constant_context(ast, node.raw()),
    }
}

/// Dart `ExpressionImpl.inConstantContext`
/// (`constantContext(includeSelf: false) != null`).
pub fn in_constant_context(ast: &Ast, node: NodeId) -> bool {
    let mut current = ast.parent(node);
    while let Some(p) = current {
        match ast.kind(p) {
            NodeKind::Annotation | NodeKind::EnumConstantArguments | NodeKind::SwitchCase => {
                return true;
            }
            NodeKind::ConstantPattern => {
                return ast[Id::<ConstantPattern>::from_raw(p)].const_keyword.is_some();
            }
            NodeKind::DotShorthandConstructorInvocation => {
                if ast[Id::<DotShorthandConstructorInvocation>::from_raw(p)]
                    .const_keyword
                    .is_some()
                {
                    return true;
                }
            }
            NodeKind::InstanceCreationExpression => {
                let n = &ast[Id::<InstanceCreationExpression>::from_raw(p)];
                if is_keyword(ast, n.keyword, "const") {
                    return true;
                }
            }
            NodeKind::RecordLiteral => {
                if ast[Id::<RecordLiteral>::from_raw(p)].const_keyword.is_some() {
                    return true;
                }
            }
            NodeKind::ListLiteral => {
                if ast[Id::<ListLiteral>::from_raw(p)].const_keyword.is_some() {
                    return true;
                }
            }
            NodeKind::SetOrMapLiteral => {
                if ast[Id::<SetOrMapLiteral>::from_raw(p)].const_keyword.is_some() {
                    return true;
                }
            }
            NodeKind::VariableDeclarationList => {
                let keyword = ast[Id::<VariableDeclarationList>::from_raw(p)].keyword;
                return is_keyword(ast, keyword, "const");
            }
            NodeKind::ArgumentList
            | NodeKind::ForElement
            | NodeKind::IfElement
            | NodeKind::InterpolationExpression
            | NodeKind::MapLiteralEntry
            | NodeKind::NamedArgument
            | NodeKind::RecordLiteralNamedField
            | NodeKind::NullAwareElement
            | NodeKind::SpreadElement
            | NodeKind::VariableDeclaration => {}
            k if Expression::test(k) => {}
            _ => return false,
        }
        current = ast.parent(p);
    }
    false
}
