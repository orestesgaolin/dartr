// Dart source: pkg/analysis_server/lib/src/computer/computer_outline.dart

//! The outline of a compilation unit (Dart `DartUnitOutlineComputer`). The
//! Dart computer runs on the resolved unit; the parts that need resolution
//! use [`super::heuristics`]. The Flutter widget outline nodes
//! (`withBasicFlutter`) need static types and are not produced.

use dartr_ast::to_source::to_source;
use dartr_ast::*;
use dartr_syntax::TokenId;

use super::heuristics;
use crate::mapping::ElementKind;

/// Dart `Element` (protocol): the parts that the LSP mapping reads.
#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    pub kind: ElementKind,
    pub name: String,
    pub is_deprecated: bool,
    /// `(offset, length)`.
    pub location: Option<(u32, u32)>,
    pub parameters: Option<String>,
    pub return_type: Option<String>,
    pub type_parameters: Option<String>,
    pub extended_type: Option<String>,
}

impl Element {
    fn new(kind: ElementKind, name: impl Into<String>) -> Element {
        Element {
            kind,
            name: name.into(),
            is_deprecated: false,
            location: None,
            parameters: None,
            return_type: None,
            type_parameters: None,
            extended_type: None,
        }
    }
}

/// Dart `Outline` (protocol).
#[derive(Clone, Debug, PartialEq)]
pub struct Outline {
    pub element: Element,
    pub offset: u32,
    pub length: u32,
    pub code_offset: u32,
    pub code_length: u32,
    pub children: Option<Vec<Outline>>,
}

/// Dart `DartUnitOutlineComputer(result).compute()`.
pub fn compute_outline(ast: &Ast, unit: Id<CompilationUnit>) -> Outline {
    let c = Computer {
        ast,
        has_test_import: heuristics::imports_test_framework(ast, unit),
    };
    c.compute(unit)
}

struct Computer<'a> {
    ast: &'a Ast,
    has_test_import: bool,
}

fn safe_to_source(ast: &Ast, node: Option<NodeId>) -> String {
    match node {
        Some(n) => to_source(ast, n),
        None => String::new(),
    }
}

impl<'a> Computer<'a> {
    fn compute(&self, unit: Id<CompilationUnit>) -> Outline {
        let ast = self.ast;
        let mut contents = Vec::new();
        for &member in ast.list(ast[unit].declarations) {
            let member = member.raw();
            match ast.kind(member) {
                NodeKind::ClassDeclaration => {
                    let node = Id::<ClassDeclaration>::from_raw(member);
                    let mut children = self.outlines_for_primary_constructor(ast[node].name_part);
                    children.extend(self.outlines_for_members(self.class_members(ast[node].body)));
                    contents.push(self.new_class_outline(node, children));
                }
                NodeKind::MixinDeclaration => {
                    let node = Id::<MixinDeclaration>::from_raw(member);
                    let children = self.outlines_for_members(self.class_members(ast[node].body));
                    contents.push(self.new_mixin_outline(node, children));
                }
                NodeKind::EnumDeclaration => {
                    let node = Id::<EnumDeclaration>::from_raw(member);
                    let mut children = Vec::new();
                    if let Some(body) = ast.cast::<BlockEnumBody>(ast[node].body) {
                        for &constant in ast.list(ast[body].constants) {
                            children.push(self.new_enum_constant(constant));
                        }
                        children.extend(self.outlines_for_members(ast[body].members));
                    }
                    contents.push(self.new_enum_outline(node, children));
                }
                NodeKind::ExtensionDeclaration => {
                    let node = Id::<ExtensionDeclaration>::from_raw(member);
                    let children = self.outlines_for_members(self.class_members(ast[node].body));
                    contents.push(self.new_extension_outline(node, children));
                }
                NodeKind::ExtensionTypeDeclaration => {
                    let node = Id::<ExtensionTypeDeclaration>::from_raw(member);
                    let children = self.outlines_for_members(self.class_members(ast[node].body));
                    contents.push(self.new_extension_type_outline(node, children));
                }
                NodeKind::TopLevelVariableDeclaration => {
                    let node = Id::<TopLevelVariableDeclaration>::from_raw(member);
                    let fields = &ast[ast[node].variables];
                    let type_name = safe_to_source(ast, fields.type_.map(|t| t.raw()));
                    for &field in ast.list(fields.variables) {
                        contents.push(self.new_variable_outline(
                            &type_name,
                            ElementKind::TopLevelVariable,
                            field,
                            false,
                        ));
                    }
                }
                NodeKind::FunctionDeclaration => {
                    let node = Id::<FunctionDeclaration>::from_raw(member);
                    contents.push(self.new_function_outline(node, true));
                }
                NodeKind::ClassTypeAlias => {
                    contents.push(self.new_class_type_alias(Id::from_raw(member)));
                }
                NodeKind::FunctionTypeAlias => {
                    contents.push(self.new_function_type_alias_outline(Id::from_raw(member)));
                }
                NodeKind::GenericTypeAlias => {
                    contents.push(self.new_generic_type_alias_outline(Id::from_raw(member)));
                }
                _ => {}
            }
        }
        // Dart `_newUnitOutline`.
        let mut element = Element::new(ElementKind::CompilationUnit, "<unit>");
        element.location = Some((ast.offset(unit), ast.length(unit)));
        self.node_outline(unit.raw(), element, contents)
    }

    fn class_members(&self, body: Id<ClassBody>) -> NodeList<ClassMember> {
        match self.ast.cast::<BlockClassBody>(body) {
            Some(b) => self.ast[b].members,
            None => NodeList::EMPTY,
        }
    }

    fn token_location(&self, t: TokenId) -> Option<(u32, u32)> {
        let tok = self.ast.tokens.get(t);
        Some((tok.offset, tok.length))
    }

    fn lexeme(&self, t: TokenId) -> String {
        self.ast.tokens.lexeme(t).to_string()
    }

    fn type_parameters_str(&self, p: Option<Id<TypeParameterList>>) -> Option<String> {
        p.map(|p| to_source(self.ast, p))
    }

    fn class_name_part(&self, part: Id<ClassNamePart>) -> (TokenId, Option<Id<TypeParameterList>>) {
        let ast = self.ast;
        if let Some(n) = ast.cast::<NameWithTypeParameters>(part) {
            (ast[n].type_name, ast[n].type_parameters)
        } else if let Some(p) = ast.cast::<PrimaryConstructorDeclaration>(part) {
            (ast[p].type_name, ast[p].type_parameters)
        } else {
            unreachable!("ClassNamePart {:?}", ast.kind(part))
        }
    }

    fn new_class_outline(&self, node: Id<ClassDeclaration>, children: Vec<Outline>) -> Outline {
        let n = &self.ast[node];
        let (name_token, type_parameters) = self.class_name_part(n.name_part);
        let mut e = Element::new(ElementKind::Class, self.lexeme(name_token));
        e.is_deprecated = heuristics::has_deprecated(self.ast, n.metadata);
        e.location = self.token_location(name_token);
        e.type_parameters = self.type_parameters_str(type_parameters);
        self.node_outline(node.raw(), e, children)
    }

    fn new_class_type_alias(&self, node: Id<ClassTypeAlias>) -> Outline {
        let n = &self.ast[node];
        let mut e = Element::new(ElementKind::ClassTypeAlias, self.lexeme(n.name));
        e.is_deprecated = heuristics::has_deprecated(self.ast, n.metadata);
        e.location = self.token_location(n.name);
        e.type_parameters = self.type_parameters_str(n.type_parameters);
        self.node_outline(node.raw(), e, Vec::new())
    }

    /// Dart `_newConstructorBodyOutline`.
    fn new_constructor_body_outline(&self, body: Id<PrimaryConstructorBody>) -> Outline {
        let ast = self.ast;
        let n = &ast[body];
        let mut e = Element::new(ElementKind::Constructor, "this");
        e.location = self.token_location(n.this_keyword);
        let contents = self.function_body_outlines(n.body);
        self.node_outline(body.raw(), e, contents)
    }

    /// The name of the class-like declaration that contains [node] (Dart
    /// `declaredFragment.element.enclosingElement.name`).
    fn enclosing_type_name(&self, node: NodeId) -> Option<String> {
        let ast = self.ast;
        let mut current = ast.parent(node);
        while let Some(p) = current {
            match ast.kind(p) {
                NodeKind::ClassDeclaration => {
                    let n = &ast[Id::<ClassDeclaration>::from_raw(p)];
                    return Some(self.lexeme(self.class_name_part(n.name_part).0));
                }
                NodeKind::EnumDeclaration => {
                    let n = &ast[Id::<EnumDeclaration>::from_raw(p)];
                    return Some(self.lexeme(self.class_name_part(n.name_part).0));
                }
                NodeKind::ExtensionTypeDeclaration => {
                    let n = &ast[Id::<ExtensionTypeDeclaration>::from_raw(p)];
                    return Some(self.lexeme(self.class_name_part(n.name_part).0));
                }
                NodeKind::MixinDeclaration => {
                    return Some(self.lexeme(ast[Id::<MixinDeclaration>::from_raw(p)].name));
                }
                NodeKind::ExtensionDeclaration => {
                    return ast[Id::<ExtensionDeclaration>::from_raw(p)]
                        .name
                        .map(|t| self.lexeme(t));
                }
                _ => current = ast.parent(p),
            }
        }
        None
    }

    fn new_constructor_outline(&self, node: Id<ConstructorDeclaration>) -> Outline {
        let ast = self.ast;
        let n = &ast[node];
        let mut name;
        let mut offset;
        let mut length;
        let keyword = n.new_keyword.or(n.factory_keyword);
        if let Some(type_name) = n.type_name {
            name = self.lexeme(ast[type_name].token);
            offset = ast.offset(type_name);
            length = ast.length(type_name);
        } else {
            name = self
                .enclosing_type_name(node.raw())
                .unwrap_or_else(|| "<unknown>".to_string());
            match keyword {
                Some(k) => {
                    let t = ast.tokens.get(k);
                    offset = t.offset;
                    length = t.length;
                }
                None => {
                    offset = ast.offset(node);
                    length = ast.length(node);
                }
            }
        }
        let mut is_private = false;
        if let Some(constructor_name) = n.name {
            let text = self.lexeme(constructor_name);
            is_private = text.starts_with('_');
            if text != "new" {
                name.push('.');
                name.push_str(&text);
            }
            let t = ast.tokens.get(constructor_name);
            offset = t.offset;
            length = t.length;
        }
        let _ = is_private;
        let mut e = Element::new(ElementKind::Constructor, name);
        e.is_deprecated = heuristics::has_deprecated(ast, n.metadata);
        e.location = Some((offset, length));
        e.parameters = Some(to_source(ast, n.parameters));
        let contents = self.function_body_outlines(n.body);
        self.node_outline(node.raw(), e, contents)
    }

    /// Dart `_newDeclaredFieldOutline`.
    fn new_declared_field_outline(&self, parameter: NodeId) -> Option<Outline> {
        let ast = self.ast;
        let (metadata, type_, suffix, name) =
            if let Some(p) = ast.cast::<RegularFormalParameter>(parameter) {
                let n = &ast[p];
                (n.metadata, n.type_, n.function_typed_suffix, n.name?)
            } else if let Some(p) = ast.cast::<FieldFormalParameter>(parameter) {
                let n = &ast[p];
                (n.metadata, n.type_, n.function_typed_suffix, n.name)
            } else {
                let n = &ast[ast.cast::<SuperFormalParameter>(parameter)?];
                (n.metadata, n.type_, n.function_typed_suffix, n.name)
            };
        let type_name = match suffix {
            None => safe_to_source(ast, type_.map(|t| t.raw())),
            Some(s) => {
                let s = &ast[s];
                format!(
                    "{} Function{}{}{}",
                    safe_to_source(ast, type_.map(|t| t.raw())),
                    safe_to_source(ast, s.type_parameters.map(|t| t.raw())),
                    to_source(ast, s.formal_parameters),
                    s.question.map(|q| self.lexeme(q)).unwrap_or_default()
                )
            }
        };
        let mut e = Element::new(ElementKind::Field, self.lexeme(name));
        e.is_deprecated = heuristics::has_deprecated(ast, metadata);
        e.location = self.token_location(name);
        e.return_type = Some(type_name);
        Some(self.node_outline(parameter, e, Vec::new()))
    }

    fn new_enum_constant(&self, node: Id<EnumConstantDeclaration>) -> Outline {
        let n = &self.ast[node];
        let mut e = Element::new(ElementKind::EnumConstant, self.lexeme(n.name));
        e.is_deprecated = heuristics::has_deprecated(self.ast, n.metadata);
        e.location = self.token_location(n.name);
        self.node_outline(node.raw(), e, Vec::new())
    }

    fn new_enum_outline(&self, node: Id<EnumDeclaration>, children: Vec<Outline>) -> Outline {
        let n = &self.ast[node];
        let (name_token, _) = self.class_name_part(n.name_part);
        let mut e = Element::new(ElementKind::Enum, self.lexeme(name_token));
        e.is_deprecated = heuristics::has_deprecated(self.ast, n.metadata);
        e.location = self.token_location(name_token);
        self.node_outline(node.raw(), e, children)
    }

    fn new_extension_outline(
        &self,
        node: Id<ExtensionDeclaration>,
        children: Vec<Outline>,
    ) -> Outline {
        let ast = self.ast;
        let n = &ast[node];
        let name = n.name.map(|t| self.lexeme(t)).unwrap_or_default();
        let extended_type = n.on_clause.map(|c| ast[c].extended_type);
        let location = match (n.name, extended_type) {
            (Some(t), _) => self.token_location(t),
            (None, Some(t)) => Some((ast.offset(t), ast.length(t))),
            (None, None) => None,
        };
        let mut e = Element::new(ElementKind::Extension, name);
        e.is_deprecated = heuristics::has_deprecated(ast, n.metadata);
        e.location = location;
        e.type_parameters = self.type_parameters_str(n.type_parameters);
        e.extended_type = extended_type.map(|t| to_source(ast, t));
        self.node_outline(node.raw(), e, children)
    }

    fn new_extension_type_outline(
        &self,
        node: Id<ExtensionTypeDeclaration>,
        children: Vec<Outline>,
    ) -> Outline {
        let n = &self.ast[node];
        let (name_token, type_parameters) = self.class_name_part(n.name_part);
        let mut e = Element::new(ElementKind::ExtensionType, self.lexeme(name_token));
        e.is_deprecated = heuristics::has_deprecated(self.ast, n.metadata);
        e.location = self.token_location(name_token);
        e.type_parameters = self.type_parameters_str(type_parameters);
        self.node_outline(node.raw(), e, children)
    }

    fn new_function_outline(&self, node: Id<FunctionDeclaration>, _is_static: bool) -> Outline {
        let ast = self.ast;
        let n = &ast[node];
        let function_expression = &ast[n.function_expression];
        let kind = match n.property_keyword.map(|k| ast.tokens.lexeme(k)) {
            Some("get") => ElementKind::Getter,
            Some("set") => ElementKind::Setter,
            _ => ElementKind::Function,
        };
        let mut e = Element::new(kind, self.lexeme(n.name));
        e.is_deprecated = heuristics::has_deprecated(ast, n.metadata);
        e.location = self.token_location(n.name);
        e.parameters = Some(safe_to_source(
            ast,
            function_expression.parameters.map(|p| p.raw()),
        ));
        e.return_type = Some(safe_to_source(ast, n.return_type.map(|t| t.raw())));
        e.type_parameters = self.type_parameters_str(function_expression.type_parameters);
        let contents = self.function_body_outlines(function_expression.body);
        self.node_outline(node.raw(), e, contents)
    }

    fn new_function_type_alias_outline(&self, node: Id<FunctionTypeAlias>) -> Outline {
        let ast = self.ast;
        let n = &ast[node];
        let mut e = Element::new(ElementKind::FunctionTypeAlias, self.lexeme(n.name));
        e.is_deprecated = heuristics::has_deprecated(ast, n.metadata);
        e.location = self.token_location(n.name);
        e.parameters = Some(to_source(ast, n.parameters));
        e.return_type = Some(safe_to_source(ast, n.return_type.map(|t| t.raw())));
        e.type_parameters = self.type_parameters_str(n.type_parameters);
        self.node_outline(node.raw(), e, Vec::new())
    }

    fn new_generic_type_alias_outline(&self, node: Id<GenericTypeAlias>) -> Outline {
        let ast = self.ast;
        let n = &ast[node];
        let function_type = ast.cast::<GenericFunctionType>(n.type_);
        let kind = if function_type.is_some() {
            ElementKind::FunctionTypeAlias
        } else {
            ElementKind::TypeAlias
        };
        let mut e = Element::new(kind, self.lexeme(n.name));
        e.is_deprecated = heuristics::has_deprecated(ast, n.metadata);
        e.location = self.token_location(n.name);
        if let Some(f) = function_type {
            e.parameters = Some(to_source(ast, ast[f].parameters));
            e.return_type = Some(safe_to_source(ast, ast[f].return_type.map(|t| t.raw())));
        }
        e.type_parameters = self.type_parameters_str(n.type_parameters);
        self.node_outline(node.raw(), e, Vec::new())
    }

    fn new_method_outline(&self, node: Id<MethodDeclaration>) -> Outline {
        let ast = self.ast;
        let n = &ast[node];
        let kind = match n.property_keyword.map(|k| ast.tokens.lexeme(k)) {
            Some("get") => ElementKind::Getter,
            Some("set") => ElementKind::Setter,
            _ => ElementKind::Method,
        };
        let mut e = Element::new(kind, self.lexeme(n.name));
        e.is_deprecated = heuristics::has_deprecated(ast, n.metadata);
        e.location = self.token_location(n.name);
        e.parameters = n.parameters.map(|p| to_source(ast, p));
        e.return_type = Some(safe_to_source(ast, n.return_type.map(|t| t.raw())));
        e.type_parameters = self.type_parameters_str(n.type_parameters);
        let contents = self.function_body_outlines(n.body);
        self.node_outline(node.raw(), e, contents)
    }

    fn new_mixin_outline(&self, node: Id<MixinDeclaration>, children: Vec<Outline>) -> Outline {
        let n = &self.ast[node];
        let mut e = Element::new(ElementKind::Mixin, self.lexeme(n.name));
        e.is_deprecated = heuristics::has_deprecated(self.ast, n.metadata);
        e.location = self.token_location(n.name);
        e.type_parameters = self.type_parameters_str(n.type_parameters);
        self.node_outline(node.raw(), e, children)
    }

    /// Dart `_newPrimaryConstructorOutline`.
    fn new_primary_constructor_outline(
        &self,
        node: Id<PrimaryConstructorDeclaration>,
        body_metadata: NodeList<Annotation>,
    ) -> Outline {
        let ast = self.ast;
        let n = &ast[node];
        let mut name = self.lexeme(n.type_name);
        let mut location = self.token_location(n.type_name);
        if let Some(c) = n.constructor_name {
            name.push('.');
            name.push_str(&self.lexeme(ast[c].name));
            location = Some((ast.offset(c), ast.length(c)));
        }
        let mut e = Element::new(ElementKind::Constructor, name);
        e.is_deprecated = heuristics::has_deprecated(ast, body_metadata);
        e.location = location;
        e.parameters = Some(to_source(ast, n.formal_parameters));
        self.node_outline(node.raw(), e, Vec::new())
    }

    fn new_variable_outline(
        &self,
        type_name: &str,
        kind: ElementKind,
        variable: Id<VariableDeclaration>,
        _is_static: bool,
    ) -> Outline {
        let n = &self.ast[variable];
        let mut e = Element::new(kind, self.lexeme(n.name));
        e.is_deprecated = heuristics::has_deprecated(self.ast, n.metadata);
        e.location = self.token_location(n.name);
        e.return_type = Some(type_name.to_string());
        self.node_outline(variable.raw(), e, Vec::new())
    }

    /// Dart `_nodeOutline`.
    fn node_outline(&self, node: NodeId, element: Element, children: Vec<Outline>) -> Outline {
        let ast = self.ast;
        let mut offset = ast.offset(node);
        let mut end = ast.end(node);
        if ast.kind(node) == NodeKind::VariableDeclaration {
            let parent = ast.parent(node);
            let grand_parent = parent.and_then(|p| ast.parent(p));
            if let (Some(parent), Some(grand_parent)) = (parent, grand_parent) {
                if let Some(list) = ast.cast::<VariableDeclarationList>(parent) {
                    let variables = ast.list(ast[list].variables);
                    if let (Some(first), Some(last)) = (variables.first(), variables.last()) {
                        if first.raw() == node {
                            offset = ast.offset(grand_parent);
                        }
                        if last.raw() == node {
                            end = ast.end(grand_parent);
                        }
                    }
                }
            }
        }
        let code_offset = first_token_after_comment_and_metadata(ast, node)
            .map(|t| ast.tokens.offset(t))
            .unwrap_or_else(|| ast.offset(node));
        Outline {
            element,
            offset,
            length: end - offset,
            code_offset,
            code_length: ast.end(node) - code_offset,
            children: if children.is_empty() {
                None
            } else {
                Some(children)
            },
        }
    }

    fn outlines_for_members(&self, members: NodeList<ClassMember>) -> Vec<Outline> {
        let ast = self.ast;
        let mut outlines = Vec::new();
        for &member in ast.list(members) {
            let member = member.raw();
            match ast.kind(member) {
                NodeKind::ConstructorDeclaration => {
                    outlines.push(self.new_constructor_outline(Id::from_raw(member)));
                }
                NodeKind::FieldDeclaration => {
                    let node = Id::<FieldDeclaration>::from_raw(member);
                    let field_declaration = &ast[node];
                    let fields = &ast[field_declaration.fields];
                    let type_name = safe_to_source(ast, fields.type_.map(|t| t.raw()));
                    let is_static = field_declaration.static_keyword.is_some();
                    for &field in ast.list(fields.variables) {
                        outlines.push(self.new_variable_outline(
                            &type_name,
                            ElementKind::Field,
                            field,
                            is_static,
                        ));
                    }
                }
                NodeKind::MethodDeclaration => {
                    outlines.push(self.new_method_outline(Id::from_raw(member)));
                }
                NodeKind::PrimaryConstructorBody => {
                    outlines.push(self.new_constructor_body_outline(Id::from_raw(member)));
                }
                _ => {}
            }
        }
        outlines
    }

    /// Dart `_outlinesForPrimaryConstructor`. Dart checks that the
    /// constructor has a declared fragment (always true for valid code) and
    /// that a parameter element is a declaring field formal parameter
    /// (`var`/`final` parameters of the primary constructor).
    fn outlines_for_primary_constructor(&self, part: Id<ClassNamePart>) -> Vec<Outline> {
        let ast = self.ast;
        let Some(node) = ast.cast::<PrimaryConstructorDeclaration>(part) else {
            return Vec::new();
        };
        let body_metadata = self
            .primary_constructor_body(node)
            .map(|b| ast[b].metadata)
            .unwrap_or(NodeList::EMPTY);
        let mut outlines = vec![self.new_primary_constructor_outline(node, body_metadata)];
        let parameters = ast[ast[node].formal_parameters].parameters;
        for &parameter in ast.list(parameters) {
            let keyword = if let Some(p) = ast.cast::<RegularFormalParameter>(parameter) {
                ast[p].const_final_or_var_keyword
            } else {
                None
            };
            let declaring = keyword
                .map(|k| matches!(ast.tokens.lexeme(k), "var" | "final"))
                .unwrap_or(false);
            if declaring {
                if let Some(o) = self.new_declared_field_outline(parameter.raw()) {
                    outlines.push(o);
                }
            }
        }
        outlines
    }

    fn primary_constructor_body(
        &self,
        node: Id<PrimaryConstructorDeclaration>,
    ) -> Option<Id<PrimaryConstructorBody>> {
        let ast = self.ast;
        let class = ast.parent(node)?;
        let class = ast.cast::<ClassDeclaration>(class)?;
        let body = ast.cast::<BlockClassBody>(ast[class].body)?;
        ast.list(ast[body].members)
            .iter()
            .find_map(|&m| ast.cast::<PrimaryConstructorBody>(m))
    }

    /// Dart `_addFunctionBodyOutlines`.
    fn function_body_outlines(&self, body: Id<FunctionBody>) -> Vec<Outline> {
        let mut contents = Vec::new();
        self.ast.accept(
            body,
            &mut FunctionBodyVisitor {
                computer: self,
                contents: &mut contents,
            },
        );
        contents
    }
}

/// Dart `AnnotatedNode.firstTokenAfterCommentAndMetadata`: the first child
/// token or node that is not the documentation comment or an annotation.
pub(crate) fn first_token_after_comment_and_metadata(ast: &Ast, node: NodeId) -> Option<TokenId> {
    if !ast.is::<AnnotatedNode>(node) {
        return None;
    }
    for entity in ast.child_entities(node) {
        match entity {
            Entity::Node(n) => {
                if matches!(ast.kind(n), NodeKind::Comment | NodeKind::Annotation) {
                    continue;
                }
                return Some(ast.begin_token(n));
            }
            Entity::Token(t) => return Some(t),
        }
    }
    None
}

/// Dart `_FunctionBodyOutlinesVisitor`.
struct FunctionBodyVisitor<'c, 'a> {
    computer: &'c Computer<'a>,
    contents: &'c mut Vec<Outline>,
}

impl AstVisitor for FunctionBodyVisitor<'_, '_> {
    fn visit_function_declaration(&mut self, _ast: &Ast, node: Id<FunctionDeclaration>) {
        self.contents
            .push(self.computer.new_function_outline(node, false));
    }

    fn visit_method_invocation(&mut self, ast: &Ast, node: Id<MethodInvocation>) {
        let n = &ast[node];
        let test_call = heuristics::test_call(ast, node, self.computer.has_test_import);
        let Some(kind) = test_call else {
            // Dart returns without visiting the children when the method
            // name does not resolve to an executable element; every call is
            // treated as resolved here.
            self.visit_node(ast, node.raw());
            return;
        };
        let extract_string = || -> String {
            let arguments = ast.list(ast[n.argument_list].arguments);
            let Some(&first) = arguments.first() else {
                return "unnamed".to_string();
            };
            let value = match ast.cast::<NamedArgument>(first) {
                Some(named) => ast[named].argument_expression.raw(),
                None => first.raw(),
            };
            if let Some(s) = string_value(ast, value) {
                return s;
            }
            to_source(ast, value)
        };
        let method_name = n.method_name;
        let name = format!(
            "{}(\"{}\")",
            ast.tokens.lexeme(ast[method_name].token),
            extract_string()
        );
        let (element_kind, children) = match kind {
            heuristics::TestCall::Group => {
                let mut group_contents = Vec::new();
                ast.accept(
                    n.argument_list,
                    &mut FunctionBodyVisitor {
                        computer: self.computer,
                        contents: &mut group_contents,
                    },
                );
                (ElementKind::UnitTestGroup, group_contents)
            }
            heuristics::TestCall::Test => (ElementKind::UnitTestTest, Vec::new()),
        };
        let mut e = Element::new(element_kind, name);
        e.location = Some((ast.offset(method_name), ast.length(method_name)));
        self.contents.push(Outline {
            element: e,
            offset: ast.offset(node),
            length: ast.length(node),
            code_offset: ast.offset(node),
            code_length: ast.length(node),
            children: if children.is_empty() {
                None
            } else {
                Some(children)
            },
        });
    }
}

/// Dart `StringLiteral.stringValue` for simple strings and adjacent
/// simple strings.
fn string_value(ast: &Ast, node: NodeId) -> Option<String> {
    if let Some(s) = ast.cast::<SimpleStringLiteral>(node) {
        return Some(ast[s].value.to_string());
    }
    if let Some(a) = ast.cast::<AdjacentStrings>(node) {
        let mut out = String::new();
        for &s in ast.list(ast[a].strings) {
            out.push_str(&string_value(ast, s.raw())?);
        }
        return Some(out);
    }
    None
}
