// Dart source: pkg/analyzer/lib/src/dart/ast/ast.dart (StringLiteral.stringValue,
// FunctionBody.isAsynchronous/isGenerator/isSynchronous, isComplete,
// SimpleIdentifier/IndexExpression inGetterContext/inSetterContext),
// pkg/analyzer/lib/src/dart/ast/invokes_super_self.dart,
// pkg/analyzer/lib/src/dart/ast/mixin_super_invoked_names.dart

//! AST getters that the Dart AST classes compute (they are not fields of
//! the node structs), used by the driver and the linker.

use dartr_ast::*;
use dartr_syntax::{TokenId, TokenType};
use indexmap::IndexSet;

/// Dart `StringLiteral.stringValue`: the value of a string literal without
/// interpolation, else `None`.
pub fn string_value(ast: &Ast, node: Id<StringLiteral>) -> Option<String> {
    let node = node.raw();
    if let Some(s) = ast.cast::<SimpleStringLiteral>(node) {
        return Some(ast.get(s).value.to_string());
    }
    if let Some(a) = ast.cast::<AdjacentStrings>(node) {
        let mut out = String::new();
        for &part in ast.list(ast.get(a).strings) {
            out.push_str(&string_value(ast, part)?);
        }
        return Some(out);
    }
    None
}

/// The lexemes of a `DottedName` joined (Dart
/// `name.tokens.map((e) => e.lexeme).join()`).
pub fn dotted_name(ast: &Ast, node: Id<DottedName>) -> String {
    let mut out = String::new();
    for &t in ast.token_list(ast.get(node).tokens) {
        out.push_str(ast.tokens.lexeme(t));
    }
    out
}

/// Dart `Token.lexeme`, `None` for a synthetic token (Dart
/// `_getFragmentName`).
pub fn fragment_name(ast: &Ast, token: Option<TokenId>) -> Option<&str> {
    let token = token?;
    if ast.tokens.get(token).is_synthetic() {
        return None;
    }
    Some(ast.tokens.lexeme(token))
}

/// Dart `Token.offsetIfNotEmpty`: the offset, `None` if the lexeme is
/// empty.
pub fn offset_if_not_empty(ast: &Ast, token: Option<TokenId>) -> Option<u32> {
    let token = token?;
    if ast.tokens.lexeme(token).is_empty() {
        None
    } else {
        Some(ast.tokens.offset(token))
    }
}

/// Dart `FunctionBody.keyword` lexeme.
fn body_keyword(ast: &Ast, body: Id<FunctionBody>) -> (Option<&str>, bool) {
    let body = body.raw();
    if let Some(b) = ast.cast::<BlockFunctionBody>(body) {
        let b = ast.get(b);
        (b.keyword.map(|k| ast.tokens.lexeme(k)), b.star.is_some())
    } else if let Some(b) = ast.cast::<ExpressionFunctionBody>(body) {
        let b = ast.get(b);
        (b.keyword.map(|k| ast.tokens.lexeme(k)), b.star.is_some())
    } else {
        (None, false)
    }
}

/// Dart `FunctionBody.isAsynchronous`.
pub fn is_asynchronous(ast: &Ast, body: Id<FunctionBody>) -> bool {
    body_keyword(ast, body).0 == Some("async")
}

/// Dart `FunctionBody.isGenerator`.
pub fn is_generator(ast: &Ast, body: Id<FunctionBody>) -> bool {
    body_keyword(ast, body).1
}

/// Dart `FunctionBody.isSynchronous`.
pub fn is_synchronous(ast: &Ast, body: Id<FunctionBody>) -> bool {
    body_keyword(ast, body).0 != Some("async")
}

/// Whether [body] is an `EmptyFunctionBody`.
pub fn is_empty_body(ast: &Ast, body: Id<FunctionBody>) -> bool {
    ast.is::<EmptyFunctionBody>(body.raw())
}

/// Dart `FunctionDeclarationImpl.isComplete`.
pub fn function_is_complete(ast: &Ast, node: Id<FunctionDeclaration>) -> bool {
    let n = ast.get(node);
    n.external_keyword.is_some() || !is_empty_body(ast, ast.get(n.function_expression).body)
}

/// Dart `MethodDeclarationImpl.isComplete`.
pub fn method_is_complete(ast: &Ast, node: Id<MethodDeclaration>) -> bool {
    let n = ast.get(node);
    n.external_keyword.is_some() || !is_empty_body(ast, n.body)
}

/// Dart `ConstructorDeclarationImpl.isComplete`.
pub fn constructor_is_complete(ast: &Ast, node: Id<ConstructorDeclaration>) -> bool {
    let n = ast.get(node);
    if n.external_keyword.is_some() {
        return true;
    }
    if !is_empty_body(ast, n.body) {
        return true;
    }
    if n.redirected_constructor.is_some() || !n.initializers.is_empty() {
        return true;
    }
    ast.list(ast.get(n.parameters).parameters).iter().any(|&p| {
        ast.is::<FieldFormalParameter>(p.raw()) || ast.is::<SuperFormalParameter>(p.raw())
    })
}

/// Dart `MethodDeclaration.isGetter`.
pub fn method_is_getter(ast: &Ast, node: Id<MethodDeclaration>) -> bool {
    ast.get(node)
        .property_keyword
        .is_some_and(|k| ast.tokens.lexeme(k) == "get")
}

/// Dart `MethodDeclaration.isSetter`.
pub fn method_is_setter(ast: &Ast, node: Id<MethodDeclaration>) -> bool {
    ast.get(node)
        .property_keyword
        .is_some_and(|k| ast.tokens.lexeme(k) == "set")
}

/// Dart `MethodDeclaration.isStatic`.
pub fn method_is_static(ast: &Ast, node: Id<MethodDeclaration>) -> bool {
    ast.get(node)
        .modifier_keyword
        .is_some_and(|k| ast.tokens.lexeme(k) == "static")
}

/// Dart `FunctionDeclaration.isGetter`.
pub fn function_is_getter(ast: &Ast, node: Id<FunctionDeclaration>) -> bool {
    ast.get(node)
        .property_keyword
        .is_some_and(|k| ast.tokens.lexeme(k) == "get")
}

/// Dart `FunctionDeclaration.isSetter`.
pub fn function_is_setter(ast: &Ast, node: Id<FunctionDeclaration>) -> bool {
    ast.get(node)
        .property_keyword
        .is_some_and(|k| ast.tokens.lexeme(k) == "set")
}

/// Dart `VariableDeclarationList.isConst` / `isFinal` / `isLate`.
pub fn variable_list_keyword(ast: &Ast, node: Id<VariableDeclarationList>) -> Option<&str> {
    ast.get(node).keyword.map(|k| ast.tokens.lexeme(k))
}

pub fn variable_list_is_const(ast: &Ast, node: Id<VariableDeclarationList>) -> bool {
    variable_list_keyword(ast, node) == Some("const")
}

pub fn variable_list_is_final(ast: &Ast, node: Id<VariableDeclarationList>) -> bool {
    variable_list_keyword(ast, node) == Some("final")
}

pub fn variable_list_is_late(ast: &Ast, node: Id<VariableDeclarationList>) -> bool {
    ast.get(node).late_keyword.is_some()
}

/// Dart `FieldDeclaration.isStatic`.
pub fn field_is_static(ast: &Ast, node: Id<FieldDeclaration>) -> bool {
    ast.get(node).static_keyword.is_some()
}

fn is_increment_operator(ty: TokenType) -> bool {
    matches!(ty, TokenType::PLUS_PLUS | TokenType::MINUS_MINUS)
}

/// Dart `SimpleIdentifierImpl.inGetterContext`.
pub fn identifier_in_getter_context(ast: &Ast, node: Id<SimpleIdentifier>) -> bool {
    let initial_parent = ast.parent(node).expect("parent");
    let mut parent = initial_parent;
    let mut target = node.raw();
    if let Some(p) = ast.cast::<PrefixedIdentifier>(initial_parent) {
        if ast.get(p).prefix.raw() == node.raw() {
            return true;
        }
        parent = ast.parent(initial_parent).expect("parent");
        target = initial_parent;
    } else if let Some(p) = ast.cast::<PropertyAccess>(initial_parent) {
        if ast.get(p).target.map(|t| t.raw()) == Some(node.raw()) {
            return true;
        }
        parent = ast.parent(initial_parent).expect("parent");
        target = initial_parent;
    }
    if ast.is::<Label>(parent) {
        return false;
    }
    if let Some(a) = ast.cast::<AssignmentExpression>(parent) {
        let a = ast.get(a);
        if a.left_hand_side.raw() == target && ast.tokens.ty(a.operator) == TokenType::EQ {
            return false;
        }
    }
    if let Some(c) = ast.cast::<ConstructorFieldInitializer>(parent)
        && ast.get(c).field_name.raw() == target
    {
        return false;
    }
    if let Some(f) = ast.cast::<ForEachPartsWithIdentifier>(parent)
        && ast.get(f).identifier.raw() == target
    {
        return false;
    }
    true
}

/// Dart `SimpleIdentifierImpl.inSetterContext`.
pub fn identifier_in_setter_context(ast: &Ast, node: Id<SimpleIdentifier>) -> bool {
    let initial_parent = ast.parent(node).expect("parent");
    let mut parent = initial_parent;
    let mut target = node.raw();
    if let Some(p) = ast.cast::<PrefixedIdentifier>(initial_parent) {
        if ast.get(p).prefix.raw() == node.raw() {
            return false;
        }
        parent = ast.parent(initial_parent).expect("parent");
        target = initial_parent;
    } else if let Some(p) = ast.cast::<PropertyAccess>(initial_parent) {
        if ast.get(p).target.map(|t| t.raw()) == Some(node.raw()) {
            return false;
        }
        parent = ast.parent(initial_parent).expect("parent");
        target = initial_parent;
    }
    if let Some(p) = ast.cast::<PrefixExpression>(parent) {
        is_increment_operator(ast.tokens.ty(ast.get(p).operator))
    } else if let Some(p) = ast.cast::<PostfixExpression>(parent) {
        is_increment_operator(ast.tokens.ty(ast.get(p).operator))
    } else if let Some(a) = ast.cast::<AssignmentExpression>(parent) {
        ast.get(a).left_hand_side.raw() == target
    } else if let Some(f) = ast.cast::<ForEachPartsWithIdentifier>(parent) {
        ast.get(f).identifier.raw() == target
    } else {
        false
    }
}

/// Dart `IndexExpressionImpl.inGetterContext`.
pub fn index_in_getter_context(ast: &Ast, node: Id<IndexExpression>) -> bool {
    let parent = ast.parent(node).expect("parent");
    if let Some(a) = ast.cast::<AssignmentExpression>(parent) {
        let a = ast.get(a);
        if a.left_hand_side.raw() == node.raw() && ast.tokens.ty(a.operator) == TokenType::EQ {
            return false;
        }
    }
    true
}

/// Dart `IndexExpressionImpl.inSetterContext`.
pub fn index_in_setter_context(ast: &Ast, node: Id<IndexExpression>) -> bool {
    let parent = ast.parent(node).expect("parent");
    if let Some(p) = ast.cast::<PrefixExpression>(parent) {
        is_increment_operator(ast.tokens.ty(ast.get(p).operator))
    } else if let Some(p) = ast.cast::<PostfixExpression>(parent) {
        is_increment_operator(ast.tokens.ty(ast.get(p).operator))
    } else if let Some(a) = ast.cast::<AssignmentExpression>(parent) {
        ast.get(a).left_hand_side.raw() == node.raw()
    } else {
        false
    }
}

fn is_super(ast: &Ast, node: Option<Id<Expression>>) -> bool {
    node.is_some_and(|n| ast.is::<SuperExpression>(n.raw()))
}

/// Dart `_SuperVisitor` (invokes_super_self.dart).
struct SuperVisitor<'a> {
    name: &'a str,
    writing: bool,
    has_super_invocation: bool,
}

impl AstVisitor for SuperVisitor<'_> {
    fn visit_assignment_expression(&mut self, ast: &Ast, node: Id<AssignmentExpression>) {
        if self.writing {
            let left = ast.get(node).left_hand_side.raw();
            if let Some(left) = ast.cast::<PropertyAccess>(left) {
                let left = ast.get(left);
                if is_super(ast, left.target)
                    && ast.tokens.lexeme(ast.get(left.property_name).token) == self.name
                {
                    self.has_super_invocation = true;
                    return;
                }
            }
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_binary_expression(&mut self, ast: &Ast, node: Id<BinaryExpression>) {
        if !self.writing {
            let n = ast.get(node);
            if ast.is::<SuperExpression>(n.left_operand.raw())
                && ast.tokens.lexeme(n.operator) == self.name
            {
                self.has_super_invocation = true;
                return;
            }
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_method_invocation(&mut self, ast: &Ast, node: Id<MethodInvocation>) {
        if !self.writing {
            let n = ast.get(node);
            if is_super(ast, n.target)
                && ast.tokens.lexeme(ast.get(n.method_name).token) == self.name
            {
                self.has_super_invocation = true;
                return;
            }
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_property_access(&mut self, ast: &Ast, node: Id<PropertyAccess>) {
        if !self.writing {
            let parent = ast.parent(node);
            let is_assignment_target = parent
                .and_then(|p| ast.cast::<AssignmentExpression>(p))
                .is_some_and(|a| ast.get(a).left_hand_side.raw() == node.raw());
            if !is_assignment_target {
                let n = ast.get(node);
                if is_super(ast, n.target)
                    && ast.tokens.lexeme(ast.get(n.property_name).token) == self.name
                {
                    self.has_super_invocation = true;
                    return;
                }
            }
        }
        self.visit_node(ast, node.raw());
    }
}

/// Dart `MethodDeclarationExtension.invokesSuperSelf`.
pub fn invokes_super_self(ast: &Ast, node: Id<MethodDeclaration>) -> bool {
    let n = ast.get(node);
    let mut visitor = SuperVisitor {
        name: ast.tokens.lexeme(n.name),
        writing: method_is_setter(ast, node),
        has_super_invocation: false,
    };
    ast.accept(n.body, &mut visitor);
    visitor.has_super_invocation
}

/// Dart `MixinSuperInvokedNamesCollector`: collects into an ordered set (a
/// Dart `Set` literal keeps insertion order).
pub struct MixinSuperInvokedNamesCollector<'n> {
    pub names: &'n mut IndexSet<String>,
}

impl AstVisitor for MixinSuperInvokedNamesCollector<'_> {
    fn visit_binary_expression(&mut self, ast: &Ast, node: Id<BinaryExpression>) {
        let n = ast.get(node);
        if ast.is::<SuperExpression>(n.left_operand.raw()) {
            self.names.insert(ast.tokens.lexeme(n.operator).to_string());
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_index_expression(&mut self, ast: &Ast, node: Id<IndexExpression>) {
        if is_super(ast, ast.get(node).target) {
            if index_in_getter_context(ast, node) {
                self.names.insert("[]".to_string());
            }
            if index_in_setter_context(ast, node) {
                self.names.insert("[]=".to_string());
            }
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_method_invocation(&mut self, ast: &Ast, node: Id<MethodInvocation>) {
        let n = ast.get(node);
        if is_super(ast, n.target) {
            self.names
                .insert(ast.tokens.lexeme(ast.get(n.method_name).token).to_string());
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_prefix_expression(&mut self, ast: &Ast, node: Id<PrefixExpression>) {
        let n = ast.get(node);
        if ast.is::<SuperExpression>(n.operand.raw()) {
            match ast.tokens.ty(n.operator) {
                TokenType::MINUS => {
                    self.names.insert("unary-".to_string());
                }
                TokenType::TILDE => {
                    self.names.insert("~".to_string());
                }
                _ => {}
            }
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_property_access(&mut self, ast: &Ast, node: Id<PropertyAccess>) {
        let n = ast.get(node);
        if is_super(ast, n.target) {
            let name = ast.tokens.lexeme(ast.get(n.property_name).token);
            if identifier_in_getter_context(ast, n.property_name) {
                self.names.insert(name.to_string());
            }
            if identifier_in_setter_context(ast, n.property_name) {
                self.names.insert(format!("{name}="));
            }
        }
        self.visit_node(ast, node.raw());
    }
}

/// Dart `MixinDeclaration.superInvokedNames` (unlinked_api_signature.dart):
/// the names that the whole declaration invokes on `super`.
pub fn mixin_super_invoked_names(ast: &Ast, node: Id<MixinDeclaration>) -> Vec<String> {
    let mut names = IndexSet::new();
    let mut collector = MixinSuperInvokedNamesCollector { names: &mut names };
    ast.accept(node, &mut collector);
    names.into_iter().collect()
}

/// Dart `ClassNamePart.typeName`.
pub fn class_name_part_name(ast: &Ast, node: Id<ClassNamePart>) -> TokenId {
    if let Some(n) = ast.cast::<NameWithTypeParameters>(node.raw()) {
        ast.get(n).type_name
    } else {
        let p = ast
            .cast::<PrimaryConstructorDeclaration>(node.raw())
            .expect("ClassNamePart");
        ast.get(p).type_name
    }
}

/// Dart `ClassNamePart.typeParameters`.
pub fn class_name_part_type_parameters(
    ast: &Ast,
    node: Id<ClassNamePart>,
) -> Option<Id<TypeParameterList>> {
    if let Some(n) = ast.cast::<NameWithTypeParameters>(node.raw()) {
        ast.get(n).type_parameters
    } else {
        let p = ast
            .cast::<PrimaryConstructorDeclaration>(node.raw())
            .expect("ClassNamePart");
        ast.get(p).type_parameters
    }
}

/// The members of a class body (`BlockClassBody.members`, none for an
/// `EmptyClassBody`).
pub fn class_body_members(ast: &Ast, body: Id<ClassBody>) -> Vec<Id<ClassMember>> {
    match ast.cast::<BlockClassBody>(body.raw()) {
        Some(b) => ast.list(ast.get(b).members).to_vec(),
        None => Vec::new(),
    }
}

/// The members of an enum body.
pub fn enum_body_members(ast: &Ast, body: Id<EnumBody>) -> Vec<Id<ClassMember>> {
    match ast.cast::<BlockEnumBody>(body.raw()) {
        Some(b) => ast.list(ast.get(b).members).to_vec(),
        None => Vec::new(),
    }
}

/// The constants of an enum body.
pub fn enum_body_constants(ast: &Ast, body: Id<EnumBody>) -> Vec<Id<EnumConstantDeclaration>> {
    match ast.cast::<BlockEnumBody>(body.raw()) {
        Some(b) => ast.list(ast.get(b).constants).to_vec(),
        None => Vec::new(),
    }
}
