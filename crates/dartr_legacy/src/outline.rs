use dartr_ast::to_source::to_source;
use dartr_ast::*;
use dartr_element::{Ctx, ElementId, ResolutionTables, Tag};
use dartr_resolver::element_metadata::{
    UnitAst, element_has, flags as meta_flags, is_deprecated_with_kind,
};
use dartr_syntax::{LineInfo, TokenId};

use crate::convert::{location_from_line_info, make_element_flags};
use crate::protocol::{AnalysisOutlineParams, Element, ElementKind, FileKind, Location, Outline};

pub fn compute_analysis_outline(
    file: &str,
    ctx: &Ctx<'_>,
    ast: &Ast,
    tables: &ResolutionTables,
    line_info: &LineInfo,
    unit: Id<CompilationUnit>,
) -> AnalysisOutlineParams {
    let directives = ast.list_raw(ast[unit].directives);
    let file_kind = if directives.iter().any(|&d| ast.is::<PartOfDirective>(d)) {
        FileKind::PART
    } else {
        FileKind::LIBRARY
    };
    let library_name = compute_library_name(ast, directives);
    let computer = OutlineComputer {
        file,
        ctx,
        ast,
        tables,
        line_info,
    };
    let outline = computer.compute(unit);
    AnalysisOutlineParams {
        file: file.to_string(),
        kind: file_kind,
        library_name,
        outline,
    }
}

fn compute_library_name(ast: &Ast, directives: &[NodeId]) -> Option<String> {
    for &d in directives {
        if let Some(lib_dir) = ast.cast::<LibraryDirective>(d) {
            return ast[lib_dir].name.map(|n| to_source(ast, n.raw()));
        }
    }
    for &d in directives {
        if let Some(part_of) = ast.cast::<PartOfDirective>(d) {
            return ast[part_of].library_name.map(|n| to_source(ast, n.raw()));
        }
    }
    None
}

fn safe_to_source(ast: &Ast, node: Option<NodeId>) -> String {
    match node {
        Some(n) => to_source(ast, n),
        None => String::new(),
    }
}

fn is_private_name(name: &str) -> bool {
    name.starts_with('_')
}

fn class_type_parameters(ast: &Ast, part: Id<ClassNamePart>) -> Option<Id<TypeParameterList>> {
    match ast.kind(part) {
        NodeKind::NameWithTypeParameters => {
            ast[Id::<NameWithTypeParameters>::from_raw(part.raw())].type_parameters
        }
        NodeKind::PrimaryConstructorDeclaration => {
            ast[Id::<PrimaryConstructorDeclaration>::from_raw(part.raw())].type_parameters
        }
        _ => None,
    }
}

struct OutlineComputer<'a, 'c> {
    file: &'a str,
    ctx: &'a Ctx<'c>,
    ast: &'a Ast,
    tables: &'a ResolutionTables,
    line_info: &'a LineInfo,
}

impl<'a, 'c> OutlineComputer<'a, 'c> {
    fn compute(&self, unit: Id<CompilationUnit>) -> Outline {
        let ast = self.ast;
        let mut unit_contents = Vec::new();
        for &member in ast.list(ast[unit].declarations) {
            let member = member.raw();
            match ast.kind(member) {
                NodeKind::ClassDeclaration => {
                    let node = Id::<ClassDeclaration>::from_raw(member);
                    let mut children = self.outlines_for_primary_constructor(ast[node].name_part);
                    children.extend(self.outlines_for_members(self.class_members(ast[node].body)));
                    unit_contents.push(self.new_class_outline(node, children));
                }
                NodeKind::MixinDeclaration => {
                    let node = Id::<MixinDeclaration>::from_raw(member);
                    let children = self.outlines_for_members(self.class_members(ast[node].body));
                    unit_contents.push(self.new_mixin_outline(node, children));
                }
                NodeKind::EnumDeclaration => {
                    let node = Id::<EnumDeclaration>::from_raw(member);
                    let mut children = Vec::new();
                    if let Some(body) = ast.cast::<BlockEnumBody>(ast[node].body) {
                        for &constant in ast.list(ast[body].constants) {
                            children.push(self.new_enum_constant(constant));
                        }
                        children.extend(self.outlines_for_members(ast.list(ast[body].members)));
                    }
                    unit_contents.push(self.new_enum_outline(node, children));
                }
                NodeKind::ExtensionDeclaration => {
                    let node = Id::<ExtensionDeclaration>::from_raw(member);
                    let children = self.outlines_for_members(self.class_members(ast[node].body));
                    unit_contents.push(self.new_extension_outline(node, children));
                }
                NodeKind::ExtensionTypeDeclaration => {
                    let node = Id::<ExtensionTypeDeclaration>::from_raw(member);
                    let children = self.outlines_for_members(self.class_members(ast[node].body));
                    unit_contents.push(self.new_extension_type_outline(node, children));
                }
                NodeKind::TopLevelVariableDeclaration => {
                    let node = Id::<TopLevelVariableDeclaration>::from_raw(member);
                    let fields = &ast[ast[node].variables];
                    let field_type_name = safe_to_source(ast, fields.type_.map(|t| t.raw()));
                    for &field in ast.list(fields.variables) {
                        unit_contents.push(self.new_variable_outline(
                            &field_type_name,
                            ElementKind::TopLevelVariable,
                            field,
                            false,
                        ));
                    }
                }
                NodeKind::FunctionDeclaration => {
                    let node = Id::<FunctionDeclaration>::from_raw(member);
                    unit_contents.push(self.new_function_outline(node, true));
                }
                NodeKind::ClassTypeAlias => {
                    let node = Id::<ClassTypeAlias>::from_raw(member);
                    unit_contents.push(self.new_class_type_alias(node));
                }
                NodeKind::FunctionTypeAlias => {
                    let node = Id::<FunctionTypeAlias>::from_raw(member);
                    unit_contents.push(self.new_function_type_alias_outline(node));
                }
                NodeKind::GenericTypeAlias => {
                    let node = Id::<GenericTypeAlias>::from_raw(member);
                    unit_contents.push(self.new_generic_type_alias_outline(node));
                }
                _ => {}
            }
        }
        self.new_unit_outline(unit, unit_contents)
    }

    fn class_members(&self, body: Id<ClassBody>) -> &[Id<ClassMember>] {
        match self.ast.cast::<BlockClassBody>(body) {
            Some(b) => self.ast.list(self.ast[b].members),
            None => &[],
        }
    }

    fn declared_element(&self, node: impl Into<NodeId>) -> Option<ElementId> {
        let frag = self.tables.declared_fragment.get(node.into()).copied()?;
        self.ctx.fragment_data(frag)?.element.try_get().copied()
    }

    fn unit_ast(&self) -> Option<UnitAst<'a>> {
        Some(UnitAst {
            ast: self.ast,
            tables: self.tables,
        })
    }

    fn is_node_deprecated(&self, node: impl Into<NodeId>) -> bool {
        let n = node.into();
        if let Some(el) = self.declared_element(n) {
            return is_deprecated_with_kind(self.ctx, el, "use", self.unit_ast());
        }
        false
    }

    fn get_location_offset_length(&self, offset: u32, length: u32) -> Location {
        location_from_line_info(self.file, self.line_info, offset, length)
    }

    fn get_location_token(&self, token: TokenId) -> Location {
        let t = self.ast.tokens.get(token);
        self.get_location_offset_length(t.offset, t.end() - t.offset)
    }

    fn get_location_node(&self, node: impl Into<NodeId>) -> Location {
        let n = node.into();
        self.get_location_offset_length(self.ast.offset(n), self.ast.length(n))
    }

    fn get_type_parameters_str(&self, params: Option<Id<TypeParameterList>>) -> Option<String> {
        params.map(|p| to_source(self.ast, p.raw()))
    }

    fn node_outline(
        &self,
        node: impl Into<NodeId>,
        element: Element,
        children: Vec<Outline>,
    ) -> Outline {
        let ast = self.ast;
        let node = node.into();
        let mut offset = ast.offset(node);
        let mut end = ast.end(node);
        if ast.is::<VariableDeclaration>(node)
            && let Some(parent) = ast.parent(node)
            && let Some(list) = ast.cast::<VariableDeclarationList>(parent)
            && let Some(grand_parent) = ast.parent(parent)
        {
            let vars = ast.list(ast[list].variables);
            if !vars.is_empty() {
                if vars.first().map(|v| v.raw()) == Some(node) {
                    offset = ast.offset(grand_parent);
                }
                if vars.last().map(|v| v.raw()) == Some(node) {
                    end = ast.end(grand_parent);
                }
            }
        }
        let code_offset = self
            .first_token_after_comment_and_metadata(node)
            .map(|t| ast.tokens.get(t).offset)
            .unwrap_or_else(|| ast.offset(node));
        let length = end.saturating_sub(offset);
        let code_length = ast.end(node).saturating_sub(code_offset);
        Outline {
            element,
            offset: offset as i64,
            length: length as i64,
            code_offset: code_offset as i64,
            code_length: code_length as i64,
            children: if children.is_empty() {
                None
            } else {
                Some(children)
            },
        }
    }

    fn first_token_after_comment_and_metadata(&self, node: NodeId) -> Option<TokenId> {
        let ast = self.ast;
        match ast.kind(node) {
            NodeKind::ClassDeclaration => Some(
                ast[Id::<ClassDeclaration>::from_raw(node)]
                    .first_token_after_comment_and_metadata(ast),
            ),
            NodeKind::ClassTypeAlias => Some(
                ast[Id::<ClassTypeAlias>::from_raw(node)]
                    .first_token_after_comment_and_metadata(ast),
            ),
            NodeKind::ConstructorDeclaration => Some(
                ast[Id::<ConstructorDeclaration>::from_raw(node)]
                    .first_token_after_comment_and_metadata(ast),
            ),
            NodeKind::EnumConstantDeclaration => Some(
                ast[Id::<EnumConstantDeclaration>::from_raw(node)]
                    .first_token_after_comment_and_metadata(ast),
            ),
            NodeKind::EnumDeclaration => Some(
                ast[Id::<EnumDeclaration>::from_raw(node)]
                    .first_token_after_comment_and_metadata(ast),
            ),
            NodeKind::ExtensionDeclaration => Some(
                ast[Id::<ExtensionDeclaration>::from_raw(node)]
                    .first_token_after_comment_and_metadata(ast),
            ),
            NodeKind::ExtensionTypeDeclaration => Some(
                ast[Id::<ExtensionTypeDeclaration>::from_raw(node)]
                    .first_token_after_comment_and_metadata(ast),
            ),
            NodeKind::FieldDeclaration => Some(
                ast[Id::<FieldDeclaration>::from_raw(node)]
                    .first_token_after_comment_and_metadata(ast),
            ),
            NodeKind::FunctionDeclaration => Some(
                ast[Id::<FunctionDeclaration>::from_raw(node)]
                    .first_token_after_comment_and_metadata(ast),
            ),
            NodeKind::FunctionTypeAlias => Some(
                ast[Id::<FunctionTypeAlias>::from_raw(node)]
                    .first_token_after_comment_and_metadata(ast),
            ),
            NodeKind::GenericTypeAlias => Some(
                ast[Id::<GenericTypeAlias>::from_raw(node)]
                    .first_token_after_comment_and_metadata(ast),
            ),
            NodeKind::MethodDeclaration => Some(
                ast[Id::<MethodDeclaration>::from_raw(node)]
                    .first_token_after_comment_and_metadata(ast),
            ),
            NodeKind::MixinDeclaration => Some(
                ast[Id::<MixinDeclaration>::from_raw(node)]
                    .first_token_after_comment_and_metadata(ast),
            ),
            NodeKind::PrimaryConstructorBody => Some(
                ast[Id::<PrimaryConstructorBody>::from_raw(node)]
                    .first_token_after_comment_and_metadata(ast),
            ),
            NodeKind::TopLevelVariableDeclaration => Some(
                ast[Id::<TopLevelVariableDeclaration>::from_raw(node)]
                    .first_token_after_comment_and_metadata(ast),
            ),
            NodeKind::RegularFormalParameter => Some(
                ast[Id::<RegularFormalParameter>::from_raw(node)]
                    .first_token_after_comment_and_metadata(ast),
            ),
            NodeKind::FieldFormalParameter => Some(
                ast[Id::<FieldFormalParameter>::from_raw(node)]
                    .first_token_after_comment_and_metadata(ast),
            ),
            NodeKind::SuperFormalParameter => Some(
                ast[Id::<SuperFormalParameter>::from_raw(node)]
                    .first_token_after_comment_and_metadata(ast),
            ),
            _ => None,
        }
    }

    fn new_unit_outline(&self, unit: Id<CompilationUnit>, children: Vec<Outline>) -> Outline {
        let element = Element {
            kind: ElementKind::CompilationUnit,
            name: "<unit>".to_string(),
            location: Some(self.get_location_node(unit)),
            flags: 0,
            parameters: None,
            return_type: None,
            type_parameters: None,
            aliased_type: None,
            extended_type: None,
        };
        self.node_outline(unit, element, children)
    }

    fn new_class_outline(&self, node: Id<ClassDeclaration>, children: Vec<Outline>) -> Outline {
        let ast = self.ast;
        let name_token = dartr_resolver::error::support::class_name_token(ast, ast[node].name_part);
        let name = ast.tokens.lexeme(name_token).to_string();
        let type_params = class_type_parameters(ast, ast[node].name_part);
        let element = Element {
            kind: ElementKind::CLASS,
            name: name.clone(),
            location: Some(self.get_location_token(name_token)),
            flags: make_element_flags(
                ast[node].abstract_keyword.is_some(),
                false,
                false,
                false,
                self.is_node_deprecated(node),
                is_private_name(&name),
            ),
            parameters: None,
            return_type: None,
            type_parameters: self.get_type_parameters_str(type_params),
            aliased_type: None,
            extended_type: None,
        };
        self.node_outline(node, element, children)
    }

    fn new_class_type_alias(&self, node: Id<ClassTypeAlias>) -> Outline {
        let ast = self.ast;
        let name_token = ast[node].name;
        let name = ast.tokens.lexeme(name_token).to_string();
        let element = Element {
            kind: ElementKind::ClassTypeAlias,
            name: name.clone(),
            location: Some(self.get_location_token(name_token)),
            flags: make_element_flags(
                ast[node].abstract_keyword.is_some(),
                false,
                false,
                false,
                self.is_node_deprecated(node),
                is_private_name(&name),
            ),
            parameters: None,
            return_type: None,
            type_parameters: self.get_type_parameters_str(ast[node].type_parameters),
            aliased_type: None,
            extended_type: None,
        };
        self.node_outline(node, element, Vec::new())
    }

    fn new_mixin_outline(&self, node: Id<MixinDeclaration>, children: Vec<Outline>) -> Outline {
        let ast = self.ast;
        let name_token = ast[node].name;
        let name = ast.tokens.lexeme(name_token).to_string();
        let element = Element {
            kind: ElementKind::MIXIN,
            name: name.clone(),
            location: Some(self.get_location_token(name_token)),
            flags: make_element_flags(
                false,
                false,
                false,
                false,
                self.is_node_deprecated(node),
                is_private_name(&name),
            ),
            parameters: None,
            return_type: None,
            type_parameters: self.get_type_parameters_str(ast[node].type_parameters),
            aliased_type: None,
            extended_type: None,
        };
        self.node_outline(node, element, children)
    }

    fn new_enum_outline(&self, node: Id<EnumDeclaration>, children: Vec<Outline>) -> Outline {
        let ast = self.ast;
        let name_token = dartr_resolver::error::support::class_name_token(ast, ast[node].name_part);
        let name = ast.tokens.lexeme(name_token).to_string();
        let element = Element {
            kind: ElementKind::ENUM,
            name: name.clone(),
            location: Some(self.get_location_token(name_token)),
            flags: make_element_flags(
                false,
                false,
                false,
                false,
                self.is_node_deprecated(node),
                is_private_name(&name),
            ),
            parameters: None,
            return_type: None,
            type_parameters: None,
            aliased_type: None,
            extended_type: None,
        };
        self.node_outline(node, element, children)
    }

    fn new_enum_constant(&self, node: Id<EnumConstantDeclaration>) -> Outline {
        let ast = self.ast;
        let name_token = ast[node].name;
        let name = ast.tokens.lexeme(name_token).to_string();
        let element = Element {
            kind: ElementKind::EnumConstant,
            name: name.clone(),
            location: Some(self.get_location_token(name_token)),
            flags: make_element_flags(
                false,
                false,
                false,
                false,
                self.is_node_deprecated(node),
                is_private_name(&name),
            ),
            parameters: None,
            return_type: None,
            type_parameters: None,
            aliased_type: None,
            extended_type: None,
        };
        self.node_outline(node, element, Vec::new())
    }

    fn new_extension_outline(
        &self,
        node: Id<ExtensionDeclaration>,
        children: Vec<Outline>,
    ) -> Outline {
        let ast = self.ast;
        let name_token = ast[node].name;
        let name = name_token
            .map(|t| ast.tokens.lexeme(t).to_string())
            .unwrap_or_default();
        let location = if let Some(t) = name_token {
            Some(self.get_location_token(t))
        } else {
            ast[node]
                .on_clause
                .map(|on| self.get_location_node(ast[on].extended_type))
        };
        let extended_type = ast[node]
            .on_clause
            .map(|on| safe_to_source(ast, Some(ast[on].extended_type.raw())));
        let element = Element {
            kind: ElementKind::EXTENSION,
            name: name.clone(),
            location,
            flags: make_element_flags(
                false,
                false,
                false,
                false,
                self.is_node_deprecated(node),
                is_private_name(&name),
            ),
            parameters: None,
            return_type: None,
            type_parameters: self.get_type_parameters_str(ast[node].type_parameters),
            aliased_type: None,
            extended_type,
        };
        self.node_outline(node, element, children)
    }

    fn new_extension_type_outline(
        &self,
        node: Id<ExtensionTypeDeclaration>,
        children: Vec<Outline>,
    ) -> Outline {
        let ast = self.ast;
        let name_token = dartr_resolver::error::support::class_name_token(ast, ast[node].name_part);
        let name = ast.tokens.lexeme(name_token).to_string();
        let type_params = class_type_parameters(ast, ast[node].name_part);
        let element = Element {
            kind: ElementKind::ExtensionType,
            name: name.clone(),
            location: Some(self.get_location_token(name_token)),
            flags: make_element_flags(
                false,
                false,
                false,
                false,
                self.is_node_deprecated(node),
                is_private_name(&name),
            ),
            parameters: None,
            return_type: None,
            type_parameters: self.get_type_parameters_str(type_params),
            aliased_type: None,
            extended_type: None,
        };
        self.node_outline(node, element, children)
    }

    fn new_constructor_outline(&self, node: Id<ConstructorDeclaration>) -> Outline {
        let ast = self.ast;
        let keyword = ast[node].new_keyword.or(ast[node].factory_keyword);
        let (mut name, mut offset, mut length) = if let Some(type_name) = ast[node].type_name {
            (
                to_source(ast, type_name.raw()),
                ast.offset(type_name),
                ast.length(type_name),
            )
        } else {
            let class_name = self
                .declared_element(node)
                .and_then(|e| self.ctx.element_data(e))
                .and_then(|d| d.enclosing)
                .and_then(|enc| self.ctx.element_data(enc))
                .and_then(|d| d.name)
                .map(|n| self.ctx.name_str(n).to_string())
                .unwrap_or_else(|| "<unknown>".to_string());
            let (o, l) = match keyword {
                Some(k) => {
                    let t = ast.tokens.get(k);
                    (t.offset, t.end() - t.offset)
                }
                None => (ast.offset(node), ast.length(node)),
            };
            (class_name, o, l)
        };
        let mut is_private = false;
        if let Some(ctor_name_token) = ast[node].name {
            let ctor_name = ast.tokens.lexeme(ctor_name_token);
            is_private = is_private_name(ctor_name);
            if ctor_name != "new" {
                name.push('.');
                name.push_str(ctor_name);
            }
            let t = ast.tokens.get(ctor_name_token);
            offset = t.offset;
            length = t.end() - t.offset;
        }
        let parameters_str = safe_to_source(ast, Some(ast[node].parameters.raw()));
        let element = Element {
            kind: ElementKind::CONSTRUCTOR,
            name,
            location: Some(self.get_location_offset_length(offset, length)),
            flags: make_element_flags(
                false,
                false,
                false,
                false,
                self.is_node_deprecated(node),
                is_private,
            ),
            parameters: Some(parameters_str),
            return_type: None,
            type_parameters: None,
            aliased_type: None,
            extended_type: None,
        };
        let contents = self.add_function_body_outlines(ast[node].body);
        self.node_outline(node, element, contents)
    }

    fn new_constructor_body_outline(&self, body: Id<PrimaryConstructorBody>) -> Outline {
        let ast = self.ast;
        let t = ast.tokens.get(ast[body].this_keyword);
        let offset = t.offset;
        let length = t.end() - t.offset;
        let mut is_private = false;
        if let Some(parent) = ast.parent(body)
            && let Some(class_decl) = ast
                .parent(parent)
                .and_then(|gp| ast.cast::<ClassDeclaration>(gp))
            && let Some(primary) =
                ast.cast::<PrimaryConstructorDeclaration>(ast[class_decl].name_part)
            && let Some(ctor_name) = ast[primary].constructor_name
        {
            is_private = is_private_name(ast.tokens.lexeme(ast[ctor_name].name));
        }
        let element = Element {
            kind: ElementKind::CONSTRUCTOR,
            name: "this".to_string(),
            location: Some(self.get_location_offset_length(offset, length)),
            flags: make_element_flags(false, false, false, false, false, is_private),
            parameters: None,
            return_type: None,
            type_parameters: None,
            aliased_type: None,
            extended_type: None,
        };
        let contents = self.add_function_body_outlines(ast[body].body);
        self.node_outline(body, element, contents)
    }

    fn new_primary_constructor_outline(
        &self,
        constructor: Id<PrimaryConstructorDeclaration>,
    ) -> Outline {
        let ast = self.ast;
        let type_name = ast[constructor].type_name;
        let t = ast.tokens.get(type_name);
        let mut name = ast.tokens.lexeme(type_name).to_string();
        let mut offset = t.offset;
        let mut length = t.end() - t.offset;
        let mut is_private = false;
        if let Some(ctor_name) = ast[constructor].constructor_name {
            let ctor_name_str = ast.tokens.lexeme(ast[ctor_name].name);
            is_private = is_private_name(ctor_name_str);
            name.push('.');
            name.push_str(ctor_name_str);
            offset = ast.offset(ctor_name);
            length = ast.length(ctor_name);
        }
        let parameters_str = safe_to_source(ast, Some(ast[constructor].formal_parameters.raw()));
        let element = Element {
            kind: ElementKind::CONSTRUCTOR,
            name,
            location: Some(self.get_location_offset_length(offset, length)),
            flags: make_element_flags(
                false,
                false,
                false,
                false,
                self.is_node_deprecated(constructor),
                is_private,
            ),
            parameters: Some(parameters_str),
            return_type: None,
            type_parameters: None,
            aliased_type: None,
            extended_type: None,
        };
        self.node_outline(constructor, element, Vec::new())
    }

    fn new_declared_field_outline(&self, parameter: Id<FormalParameter>) -> Option<Outline> {
        let ast = self.ast;
        let parts = dartr_resolver::ast_ext::formal_parameter_parts(ast, parameter.raw());
        let name_token = parts.name?;
        let name = ast.tokens.lexeme(name_token).to_string();
        let type_name = match parts.function_typed_suffix {
            None => safe_to_source(ast, parts.type_.map(|t| t.raw())),
            Some(suffix) => {
                let s = &ast[suffix];
                let ret = safe_to_source(ast, parts.type_.map(|t| t.raw()));
                let tp = safe_to_source(ast, s.type_parameters.map(|t| t.raw()));
                let fp = safe_to_source(ast, Some(s.formal_parameters.raw()));
                let q = s.question.map(|q| ast.tokens.lexeme(q)).unwrap_or("");
                format!("{ret} Function{tp}{fp}{q}")
            }
        };
        let is_final = parts
            .const_final_or_var_keyword
            .is_some_and(|k| ast.tokens.lexeme(k) == "final");
        let element = Element {
            kind: ElementKind::FIELD,
            name: name.clone(),
            location: Some(self.get_location_token(name_token)),
            flags: make_element_flags(
                false,
                false,
                is_final,
                false,
                self.is_node_deprecated(parameter),
                is_private_name(&name),
            ),
            parameters: None,
            return_type: Some(type_name),
            type_parameters: None,
            aliased_type: None,
            extended_type: None,
        };
        Some(self.node_outline(parameter, element, Vec::new()))
    }

    fn new_method_outline(&self, method: Id<MethodDeclaration>) -> Outline {
        let ast = self.ast;
        let m = &ast[method];
        let name_token = m.name;
        let name = ast.tokens.lexeme(name_token).to_string();
        let kind = match m.property_keyword.map(|t| ast.tokens.lexeme(t)) {
            Some("get") => ElementKind::GETTER,
            Some("set") => ElementKind::SETTER,
            _ => ElementKind::METHOD,
        };
        let parameters_str = m.parameters.map(|p| to_source(ast, p.raw()));
        let return_type_str = safe_to_source(ast, m.return_type.map(|t| t.raw()));
        let is_abstract = ast.is::<EmptyFunctionBody>(m.body) && m.external_keyword.is_none();
        let is_static = m
            .modifier_keyword
            .is_some_and(|t| ast.tokens.lexeme(t) == "static");
        let element = Element {
            kind,
            name: name.clone(),
            location: Some(self.get_location_token(name_token)),
            flags: make_element_flags(
                is_abstract,
                false,
                false,
                is_static,
                self.is_node_deprecated(method),
                is_private_name(&name),
            ),
            parameters: parameters_str,
            return_type: Some(return_type_str),
            type_parameters: self.get_type_parameters_str(m.type_parameters),
            aliased_type: None,
            extended_type: None,
        };
        let contents = self.add_function_body_outlines(m.body);
        self.node_outline(method, element, contents)
    }

    fn new_function_outline(&self, function: Id<FunctionDeclaration>, is_static: bool) -> Outline {
        let ast = self.ast;
        let f = &ast[function];
        let name_token = f.name;
        let name = ast.tokens.lexeme(name_token).to_string();
        let expr = &ast[f.function_expression];
        let kind = match f.property_keyword.map(|t| ast.tokens.lexeme(t)) {
            Some("get") => ElementKind::GETTER,
            Some("set") => ElementKind::SETTER,
            _ => ElementKind::FUNCTION,
        };
        let parameters_str = safe_to_source(ast, expr.parameters.map(|p| p.raw()));
        let return_type_str = safe_to_source(ast, f.return_type.map(|t| t.raw()));
        let element = Element {
            kind,
            name: name.clone(),
            location: Some(self.get_location_token(name_token)),
            flags: make_element_flags(
                false,
                false,
                false,
                is_static,
                self.is_node_deprecated(function),
                is_private_name(&name),
            ),
            parameters: Some(parameters_str),
            return_type: Some(return_type_str),
            type_parameters: self.get_type_parameters_str(expr.type_parameters),
            aliased_type: None,
            extended_type: None,
        };
        let contents = self.add_function_body_outlines(expr.body);
        self.node_outline(function, element, contents)
    }

    fn new_function_type_alias_outline(&self, node: Id<FunctionTypeAlias>) -> Outline {
        let ast = self.ast;
        let n = &ast[node];
        let name_token = n.name;
        let name = ast.tokens.lexeme(name_token).to_string();
        let parameters_str = safe_to_source(ast, Some(n.parameters.raw()));
        let return_type_str = safe_to_source(ast, n.return_type.map(|t| t.raw()));
        let element = Element {
            kind: ElementKind::FunctionTypeAlias,
            name: name.clone(),
            location: Some(self.get_location_token(name_token)),
            flags: make_element_flags(
                false,
                false,
                false,
                false,
                self.is_node_deprecated(node),
                is_private_name(&name),
            ),
            parameters: Some(parameters_str),
            return_type: Some(return_type_str),
            type_parameters: self.get_type_parameters_str(n.type_parameters),
            aliased_type: None,
            extended_type: None,
        };
        self.node_outline(node, element, Vec::new())
    }

    fn new_generic_type_alias_outline(&self, node: Id<GenericTypeAlias>) -> Outline {
        let ast = self.ast;
        let n = &ast[node];
        let name_token = n.name;
        let name = ast.tokens.lexeme(name_token).to_string();
        let aliased_type = n.type_;
        let aliased_fn = ast.cast::<GenericFunctionType>(aliased_type);
        let kind = if aliased_fn.is_some() {
            ElementKind::FunctionTypeAlias
        } else {
            ElementKind::TypeAlias
        };
        let parameters = aliased_fn.map(|f| safe_to_source(ast, Some(ast[f].parameters.raw())));
        let return_type =
            aliased_fn.map(|f| safe_to_source(ast, ast[f].return_type.map(|t| t.raw())));
        let element = Element {
            kind,
            name: name.clone(),
            location: Some(self.get_location_token(name_token)),
            flags: make_element_flags(
                false,
                false,
                false,
                false,
                self.is_node_deprecated(node),
                is_private_name(&name),
            ),
            parameters,
            return_type,
            type_parameters: self.get_type_parameters_str(n.type_parameters),
            aliased_type: Some(safe_to_source(ast, Some(aliased_type.raw()))),
            extended_type: None,
        };
        self.node_outline(node, element, Vec::new())
    }

    fn new_variable_outline(
        &self,
        type_name: &str,
        kind: ElementKind,
        variable: Id<VariableDeclaration>,
        is_static: bool,
    ) -> Outline {
        let ast = self.ast;
        let name_token = ast[variable].name;
        let name = ast.tokens.lexeme(name_token).to_string();
        let mut is_const = false;
        let mut is_final = false;
        if let Some(parent) = ast.parent(variable)
            && let Some(list) = ast.cast::<VariableDeclarationList>(parent)
            && let Some(kw) = ast[list].keyword
        {
            match ast.tokens.lexeme(kw) {
                "const" => is_const = true,
                "final" => is_final = true,
                _ => {}
            }
        }
        let element = Element {
            kind,
            name: name.clone(),
            location: Some(self.get_location_token(name_token)),
            flags: make_element_flags(
                false,
                is_const,
                is_final,
                is_static,
                self.is_node_deprecated(variable),
                is_private_name(&name),
            ),
            parameters: None,
            return_type: Some(type_name.to_string()),
            type_parameters: None,
            aliased_type: None,
            extended_type: None,
        };
        self.node_outline(variable, element, Vec::new())
    }

    fn outlines_for_members(&self, members: &[Id<ClassMember>]) -> Vec<Outline> {
        let ast = self.ast;
        let mut out = Vec::new();
        for &class_member in members {
            let m = class_member.raw();
            match ast.kind(m) {
                NodeKind::ConstructorDeclaration => {
                    out.push(self.new_constructor_outline(Id::from_raw(m)));
                }
                NodeKind::FieldDeclaration => {
                    let fd = Id::<FieldDeclaration>::from_raw(m);
                    let fields = &ast[ast[fd].fields];
                    let field_type_name = safe_to_source(ast, fields.type_.map(|t| t.raw()));
                    let is_static = ast[fd].static_keyword.is_some();
                    for &field in ast.list(fields.variables) {
                        out.push(self.new_variable_outline(
                            &field_type_name,
                            ElementKind::FIELD,
                            field,
                            is_static,
                        ));
                    }
                }
                NodeKind::MethodDeclaration => {
                    out.push(self.new_method_outline(Id::from_raw(m)));
                }
                NodeKind::PrimaryConstructorBody => {
                    out.push(self.new_constructor_body_outline(Id::from_raw(m)));
                }
                _ => {}
            }
        }
        out
    }

    fn outlines_for_primary_constructor(&self, name_part: Id<ClassNamePart>) -> Vec<Outline> {
        let Some(primary) = self.ast.cast::<PrimaryConstructorDeclaration>(name_part) else {
            return Vec::new();
        };
        if self.tables.declared_fragment.get(primary.raw()).is_none() {
            return Vec::new();
        }
        let mut outlines = vec![self.new_primary_constructor_outline(primary)];
        let params = self
            .ast
            .list(self.ast[self.ast[primary].formal_parameters].parameters);
        for &param in params {
            if let Some(el) = self.declared_element(param)
                && el.tag() == Tag::FieldFormalParameter
                && let dartr_element::AnyElement::FormalParameter(fp) = self.ctx.any(el)
                && fp.field.get().is_some()
                && let Some(outline) = self.new_declared_field_outline(param)
            {
                outlines.push(outline);
            }
        }
        outlines
    }

    fn add_function_body_outlines(&self, body: Id<FunctionBody>) -> Vec<Outline> {
        let mut contents = Vec::new();
        let mut visitor = FunctionBodyOutlinesVisitor {
            computer: self,
            contents: &mut contents,
        };
        self.ast.accept(body, &mut visitor);
        contents
    }
}

struct FunctionBodyOutlinesVisitor<'a, 'b, 'c> {
    computer: &'a OutlineComputer<'b, 'c>,
    contents: &'a mut Vec<Outline>,
}

impl AstVisitor for FunctionBodyOutlinesVisitor<'_, '_, '_> {
    fn visit_function_declaration(&mut self, _ast: &Ast, node: Id<FunctionDeclaration>) {
        self.contents
            .push(self.computer.new_function_outline(node, false));
    }

    fn visit_method_invocation(&mut self, ast: &Ast, node: Id<MethodInvocation>) {
        let name_node = ast[node].method_name;
        let Some(&elem_ref) = self.computer.tables.element.get(name_node.raw()) else {
            return;
        };
        let elem = dartr_typesystem::member::base_element(self.computer.ctx, elem_ref);
        if !matches!(
            elem.tag(),
            Tag::Method | Tag::TopLevelFunction | Tag::LocalFunction | Tag::Getter | Tag::Setter
        ) {
            return;
        }
        let unit_ast = self.computer.unit_ast();
        let is_group = is_test_group(self.computer.ctx, elem, unit_ast);
        let is_test_fn = !is_group && is_test_func(self.computer.ctx, elem, unit_ast);
        if is_group {
            let mut group_contents = Vec::new();
            {
                let mut sub = FunctionBodyOutlinesVisitor {
                    computer: self.computer,
                    contents: &mut group_contents,
                };
                ast.accept(ast[node].argument_list, &mut sub);
            }
            self.add_test_outline_node(ast, node, ElementKind::UnitTestGroup, group_contents);
        } else if is_test_fn {
            self.add_test_outline_node(ast, node, ElementKind::UnitTestTest, Vec::new());
        } else {
            ast.visit_children(node, self);
        }
    }
}

impl FunctionBodyOutlinesVisitor<'_, '_, '_> {
    fn add_test_outline_node(
        &mut self,
        ast: &Ast,
        node: Id<MethodInvocation>,
        kind: ElementKind,
        children: Vec<Outline>,
    ) {
        let name_node = ast[node].method_name;
        let exec_name = to_source(ast, name_node.raw());
        let args = ast.list(ast[ast[node].argument_list].arguments);
        let desc = extract_test_string(ast, args);
        let name = format!("{exec_name}(\"{desc}\")");
        let element = Element {
            kind,
            name,
            location: Some(self.computer.get_location_node(name_node)),
            flags: 0,
            parameters: None,
            return_type: None,
            type_parameters: None,
            aliased_type: None,
            extended_type: None,
        };
        let offset = ast.offset(node) as i64;
        let length = ast.length(node) as i64;
        self.contents.push(Outline {
            element,
            offset,
            length,
            code_offset: offset,
            code_length: length,
            children: if children.is_empty() {
                None
            } else {
                Some(children)
            },
        });
    }
}

fn extract_test_string(ast: &Ast, args: &[Id<Argument>]) -> String {
    if let Some(&first) = args.first() {
        let expr = if let Some(named) = ast.cast::<NamedArgument>(first) {
            ast[named].argument_expression
        } else {
            Id::<Expression>::from_raw(first.raw())
        };
        if let Some(s) = ast.cast::<SimpleStringLiteral>(expr) {
            return ast[s].value.to_string();
        }
        return to_source(ast, expr.raw());
    }
    "unnamed".to_string()
}

fn is_inside_test_package(ctx: &Ctx<'_>, elem: ElementId) -> bool {
    let Some(lib_id) = dartr_resolver::error::support::library_of(ctx, elem) else {
        return false;
    };
    let first_frag = ctx.get(lib_id).first_fragment();
    ctx.fragment(first_frag).source.path.ends_with("test.dart")
}

fn is_test_group(ctx: &Ctx<'_>, elem: ElementId, unit_ast: Option<UnitAst<'_>>) -> bool {
    if element_has(ctx, elem, meta_flags::IS_TEST_GROUP, unit_ast) {
        return true;
    }
    elem.tag() == Tag::TopLevelFunction
        && ctx
            .element_data(elem)
            .and_then(|d| d.name)
            .is_some_and(|n| ctx.name_str(n) == "group")
        && is_inside_test_package(ctx, elem)
}

fn is_test_func(ctx: &Ctx<'_>, elem: ElementId, unit_ast: Option<UnitAst<'_>>) -> bool {
    if element_has(ctx, elem, meta_flags::IS_TEST, unit_ast) {
        return true;
    }
    elem.tag() == Tag::TopLevelFunction
        && ctx
            .element_data(elem)
            .and_then(|d| d.name)
            .is_some_and(|n| ctx.name_str(n) == "test")
        && is_inside_test_package(ctx, elem)
}
