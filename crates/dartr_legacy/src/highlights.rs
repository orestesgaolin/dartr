use dartr_ast::*;
use dartr_element::{
    Ctx, ElementId, FragmentFlags, InterfaceElement, ResolutionTables, Tag, TypeAliasElement,
    TypeId, TypeKind, VariableFragment,
};
use dartr_resolver::element_ext::{is_enum_constant, variable_type};
use dartr_resolver::element_metadata::accessor_variable_any;
use dartr_resolver::error::support::library_of;
use dartr_syntax::{TokenId, TokenType};
use dartr_typesystem::member;

use crate::protocol::{HighlightRegion, HighlightRegionType};

pub fn compute_dart_highlights(
    ctx: &Ctx<'_>,
    ast: &Ast,
    tables: &ResolutionTables,
    unit: Id<CompilationUnit>,
) -> Vec<HighlightRegion> {
    let mut computer = HighlightsComputer {
        ctx,
        ast,
        tables,
        regions: Vec::new(),
    };
    ast.accept(unit, &mut computer);
    computer.add_comment_ranges(unit);
    computer.regions
}

struct HighlightsComputer<'a> {
    ctx: &'a Ctx<'a>,
    ast: &'a Ast,
    tables: &'a ResolutionTables,
    regions: Vec<HighlightRegion>,
}

impl<'a> HighlightsComputer<'a> {
    fn add_region(&mut self, offset: u32, length: u32, kind: HighlightRegionType) {
        self.regions.push(HighlightRegion {
            type_: kind,
            offset: offset as i64,
            length: length as i64,
        });
    }

    fn add_region_token(&mut self, token: Option<TokenId>, kind: HighlightRegionType) -> bool {
        if let Some(tok) = token {
            let t = self.ast.tokens.get(tok);
            self.add_region(t.offset, t.end() - t.offset, kind);
        }
        true
    }

    fn add_region_node(&mut self, node: impl Into<NodeId>, kind: HighlightRegionType) -> bool {
        let n = node.into();
        self.add_region(self.ast.offset(n), self.ast.length(n), kind);
        true
    }

    fn add_region_node_start_token_end(
        &mut self,
        node: impl Into<NodeId>,
        token: TokenId,
        kind: HighlightRegionType,
    ) {
        let n = node.into();
        let offset = self.ast.offset(n);
        let end = self.ast.tokens.get(token).end();
        self.add_region(offset, end.saturating_sub(offset), kind);
    }

    fn add_region_token_start_token_end(
        &mut self,
        a: TokenId,
        b: TokenId,
        kind: HighlightRegionType,
    ) {
        let offset = self.ast.tokens.get(a).offset;
        let end = self.ast.tokens.get(b).end();
        self.add_region(offset, end.saturating_sub(offset), kind);
    }

    fn add_comment_ranges(&mut self, unit: Id<CompilationUnit>) {
        let begin = self.ast.begin_token(unit.raw());
        let eof = self.ast.end_token(unit.raw());
        let mut idx = begin.0;
        loop {
            let tok_id = TokenId(idx);
            let tok = self.ast.tokens.get(tok_id);
            let mut comment_opt = tok.preceding_comments.get();
            while let Some(c_id) = comment_opt {
                let c_tok = self.ast.tokens.get(c_id);
                let lexeme = self.ast.tokens.lexeme(c_id);
                let highlight_type = match c_tok.ty {
                    TokenType::MULTI_LINE_COMMENT => {
                        if lexeme.starts_with("/**") {
                            Some(HighlightRegionType::CommentDocumentation)
                        } else {
                            Some(HighlightRegionType::CommentBlock)
                        }
                    }
                    TokenType::SINGLE_LINE_COMMENT => {
                        if lexeme.starts_with("///") {
                            Some(HighlightRegionType::CommentDocumentation)
                        } else {
                            Some(HighlightRegionType::CommentEndOfLine)
                        }
                    }
                    _ => None,
                };
                if let Some(ht) = highlight_type {
                    self.add_region_token(Some(c_id), ht);
                }
                comment_opt = c_tok.next.get();
            }
            if tok.ty == TokenType::EOF || idx >= eof.0 {
                break;
            }
            idx += 1;
        }
    }

    fn element(&self, node: impl Into<NodeId>) -> Option<ElementId> {
        self.tables
            .element
            .get(node.into())
            .map(|&e| member::base_element(self.ctx, e))
    }

    fn declared_element(&self, node: impl Into<NodeId>) -> Option<ElementId> {
        let frag = self.tables.declared_fragment.get(node.into()).copied()?;
        self.ctx.fragment_data(frag)?.element.try_get().copied()
    }

    fn write_or_read_element(&self, node: Id<SimpleIdentifier>) -> Option<ElementId> {
        dartr_resolver::error::support::write_or_read_element(self.ctx, self.ast, self.tables, node)
            .or_else(|| {
                self.tables
                    .read_element
                    .get(node.raw())
                    .map(|&e| member::base_element(self.ctx, e))
            })
            .or_else(|| self.element(node))
    }

    fn is_static_element(&self, element: ElementId) -> bool {
        match element.tag() {
            Tag::Field | Tag::TopLevelVariable => self
                .ctx
                .element_data(element)
                .and_then(|d| d.first_fragment.cast::<VariableFragment>())
                .is_some_and(|f| {
                    self.ctx
                        .store(f.store())
                        .variable_fragment(f)
                        .flags
                        .has(FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC)
                }),
            Tag::Method | Tag::Getter | Tag::Setter => {
                dartr_resolver::element_ext::first_fragment_flags(self.ctx, element)
                    .contains(FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC)
            }
            Tag::TopLevelFunction => true,
            _ => false,
        }
    }

    fn is_dynamic_type(&self, ty: TypeId) -> bool {
        matches!(self.ctx.ty(ty), TypeKind::Dynamic)
    }

    fn is_in_dart_core(&self, element: ElementId) -> bool {
        library_of(self.ctx, element).is_some_and(|lib| {
            self.ctx
                .fragment(self.ctx.get(lib).first_fragment())
                .source
                .uri
                .as_ref()
                == "dart:core"
        })
    }

    fn is_function_type(&self, ty: Option<TypeId>) -> bool {
        let Some(ty) = ty else { return false };
        match self.ctx.ty(ty) {
            TypeKind::Function(_) => true,
            TypeKind::Interface { element, .. } => {
                self.ctx
                    .interface(*element)
                    .name
                    .is_some_and(|n| self.ctx.name_str(n) == "Function")
                    && self.is_in_dart_core(element.raw())
            }
            _ => false,
        }
    }

    fn enclosing_instance_element(&self, mut node: NodeId) -> Option<ElementId> {
        while let Some(parent) = self.ast.parent(node) {
            if matches!(
                self.ast.kind(parent),
                NodeKind::ClassDeclaration
                    | NodeKind::MixinDeclaration
                    | NodeKind::EnumDeclaration
                    | NodeKind::ExtensionDeclaration
                    | NodeKind::ExtensionTypeDeclaration
            ) {
                return self.declared_element(parent);
            }
            node = parent;
        }
        None
    }

    fn real_target_of_property_access(&self, pa: Id<PropertyAccess>) -> Option<NodeId> {
        let cur: NodeId = pa.raw();
        if let Some(p) = self.ast.cast::<PropertyAccess>(cur) {
            if dartr_resolver::ast_ext::property_access_is_cascaded(self.ast, p) {
                let mut anc = self.ast.parent(cur);
                while let Some(a) = anc {
                    if let Some(cascade) = self.ast.cast::<CascadeExpression>(a) {
                        return Some(self.ast[cascade].target.raw());
                    }
                    anc = self.ast.parent(a);
                }
                return None;
            }
            return Some(self.ast[p].target?.raw());
        }
        None
    }

    fn real_target_of_method_invocation(&self, mi: Id<MethodInvocation>) -> Option<NodeId> {
        dartr_resolver::ast_ext::method_invocation_real_target(self.ast, mi).map(|t| t.raw())
    }

    fn add_identifier_region(
        &mut self,
        parent: NodeId,
        name_token: TokenId,
        element: Option<ElementId>,
    ) {
        if self.add_identifier_region_keyword(name_token) {
            return;
        }
        if self.add_identifier_region_class(parent, name_token, element) {
            return;
        }
        if self.add_identifier_region_extension(name_token, element) {
            return;
        }
        if self.add_identifier_region_constructor(name_token, element) {
            return;
        }
        if self.add_identifier_region_getter_setter_declaration(parent, name_token, element) {
            return;
        }
        if self.add_identifier_region_field(parent, name_token, element) {
            return;
        }
        if self.add_identifier_region_function(parent, name_token, element) {
            return;
        }
        if self.add_identifier_region_import_prefix(name_token, element) {
            return;
        }
        if self.add_identifier_region_label(name_token, element) {
            return;
        }
        if self.add_identifier_region_local_variable(name_token, element) {
            return;
        }
        if self.add_identifier_region_method(parent, name_token, element) {
            return;
        }
        if self.add_identifier_region_parameter(name_token, element) {
            return;
        }
        if self.add_identifier_region_type_alias(name_token, element) {
            return;
        }
        if self.add_identifier_region_type_parameter(name_token, element) {
            return;
        }
        if self
            .add_identifier_region_unresolved_instance_member_reference(parent, name_token, element)
        {
            return;
        }
        self.add_region_token(Some(name_token), HighlightRegionType::IdentifierDefault);
    }

    fn add_identifier_region_keyword(&mut self, name_token: TokenId) -> bool {
        if self.ast.tokens.lexeme(name_token) == "void" {
            return self.add_region_token(Some(name_token), HighlightRegionType::KEYWORD);
        }
        false
    }

    fn add_identifier_region_class(
        &mut self,
        parent: NodeId,
        name_token: TokenId,
        element: Option<ElementId>,
    ) -> bool {
        let Some(el) = element else { return false };
        if el.cast::<InterfaceElement>().is_none() {
            return false;
        }
        let grand_parent = self.ast.parent(parent);
        let kind = if self.ast.is::<NamedType>(parent)
            && grand_parent.is_some_and(|gp| {
                self.ast.is::<ConstructorName>(gp)
                    && self
                        .ast
                        .parent(gp)
                        .is_some_and(|ggp| self.ast.is::<InstanceCreationExpression>(ggp))
            }) {
            HighlightRegionType::CONSTRUCTOR
        } else {
            match el.tag() {
                Tag::Enum => HighlightRegionType::ENUM,
                Tag::ExtensionType => HighlightRegionType::ExtensionType,
                Tag::Mixin => HighlightRegionType::MIXIN,
                _ => HighlightRegionType::CLASS,
            }
        };
        self.add_region_token(Some(name_token), kind)
    }

    fn add_identifier_region_constructor(
        &mut self,
        name_token: TokenId,
        element: Option<ElementId>,
    ) -> bool {
        let Some(el) = element else { return false };
        if el.tag() != Tag::Constructor {
            return false;
        }
        self.add_region_token(Some(name_token), HighlightRegionType::CONSTRUCTOR)
    }

    fn add_identifier_region_extension(
        &mut self,
        name_token: TokenId,
        element: Option<ElementId>,
    ) -> bool {
        let Some(el) = element else { return false };
        if el.tag() != Tag::Extension {
            return false;
        }
        self.add_region_token(Some(name_token), HighlightRegionType::EXTENSION)
    }

    fn add_identifier_region_getter_setter_declaration(
        &mut self,
        parent: NodeId,
        name_token: TokenId,
        element: Option<ElementId>,
    ) -> bool {
        if !(self.ast.is::<MethodDeclaration>(parent) || self.ast.is::<FunctionDeclaration>(parent))
        {
            return false;
        }
        let Some(el) = element else { return false };
        let is_top_level = self
            .ast
            .parent(parent)
            .is_some_and(|gp| self.ast.is::<CompilationUnit>(gp));
        let kind = match el.tag() {
            Tag::Getter => {
                if is_top_level {
                    HighlightRegionType::TopLevelGetterDeclaration
                } else if self.is_static_element(el) {
                    HighlightRegionType::StaticGetterDeclaration
                } else {
                    HighlightRegionType::InstanceGetterDeclaration
                }
            }
            Tag::Setter => {
                if is_top_level {
                    HighlightRegionType::TopLevelSetterDeclaration
                } else if self.is_static_element(el) {
                    HighlightRegionType::StaticSetterDeclaration
                } else {
                    HighlightRegionType::InstanceSetterDeclaration
                }
            }
            _ => return false,
        };
        self.add_region_token(Some(name_token), kind)
    }

    fn add_identifier_region_field(
        &mut self,
        parent: NodeId,
        name_token: TokenId,
        element: Option<ElementId>,
    ) -> bool {
        let mut kind = None;
        if let Some(el) = element {
            match el.tag() {
                Tag::Field => {
                    if is_enum_constant(self.ctx, el) {
                        kind = Some(HighlightRegionType::EnumConstant);
                    } else if self.is_static_element(el) {
                        kind = Some(HighlightRegionType::StaticFieldDeclaration);
                    } else {
                        kind = Some(HighlightRegionType::InstanceFieldReference);
                    }
                }
                Tag::TopLevelVariable => {
                    kind = Some(HighlightRegionType::TopLevelVariableDeclaration);
                }
                Tag::Getter => {
                    let var = accessor_variable_any(self.ctx, el);
                    if var.is_some_and(|v| v.tag() == Tag::TopLevelVariable) {
                        kind = Some(HighlightRegionType::TopLevelGetterReference);
                    } else if var
                        .is_some_and(|v| v.tag() == Tag::Field && is_enum_constant(self.ctx, v))
                    {
                        kind = Some(HighlightRegionType::EnumConstant);
                    } else if self.is_static_element(el) {
                        kind = Some(HighlightRegionType::StaticGetterReference);
                    } else {
                        kind = Some(HighlightRegionType::InstanceGetterReference);
                    }
                }
                Tag::Setter => {
                    let var = accessor_variable_any(self.ctx, el);
                    if var.is_some_and(|v| v.tag() == Tag::TopLevelVariable) {
                        kind = Some(HighlightRegionType::TopLevelSetterReference);
                    } else if var
                        .is_some_and(|v| v.tag() == Tag::Field && is_enum_constant(self.ctx, v))
                    {
                        kind = Some(HighlightRegionType::EnumConstant);
                    } else if self.is_static_element(el) {
                        kind = Some(HighlightRegionType::StaticSetterReference);
                    } else {
                        kind = Some(HighlightRegionType::InstanceSetterReference);
                    }
                }
                _ => {}
            }
        } else {
            let mut static_type = None;
            if let Some(pa) = self.ast.cast::<PropertyAccess>(parent)
                && self.ast[self.ast[pa].property_name].token == name_token
            {
                if let Some(rt) = self.real_target_of_property_access(pa) {
                    static_type = self.tables.static_type.get(rt).copied();
                }
            } else if !self.ast.is::<PrefixedIdentifier>(parent)
                && let Some(enc) = self.enclosing_instance_element(parent)
                && let Some(ext) = enc.cast::<dartr_element::ExtensionElement>()
            {
                static_type = self.ctx.get(ext).extended_type.get();
            }
            if let Some(st) = static_type
                && let TypeKind::Record {
                    positional, named, ..
                } = self.ctx.ty(st)
            {
                let lexeme = self.ast.tokens.lexeme(name_token);
                let pos_fields = self.ctx.list(*positional);
                let named_fields = self.ctx.list(*named);
                let has_field = if let Some(rest) = lexeme.strip_prefix('$')
                    && let Ok(idx) = rest.parse::<usize>()
                {
                    idx >= 1 && idx <= pos_fields.len()
                } else {
                    named_fields
                        .iter()
                        .any(|nf| self.ctx.name_str(nf.name) == lexeme)
                };
                kind = Some(if has_field {
                    HighlightRegionType::InstanceGetterReference
                } else {
                    HighlightRegionType::UnresolvedInstanceMemberReference
                });
            }
        }
        if let Some(k) = kind {
            return self.add_region_token(Some(name_token), k);
        }
        false
    }

    fn add_identifier_region_function(
        &mut self,
        parent: NodeId,
        name_token: TokenId,
        element: Option<ElementId>,
    ) -> bool {
        let Some(el) = element else { return false };
        if !matches!(el.tag(), Tag::TopLevelFunction | Tag::LocalFunction) {
            return false;
        }
        let is_invocation = self
            .ast
            .cast::<MethodInvocation>(parent)
            .is_some_and(|mi| self.ast[self.ast[mi].method_name].token == name_token);
        let is_top_level = el.tag() == Tag::TopLevelFunction;
        let kind = match (is_top_level, is_invocation) {
            (true, true) => HighlightRegionType::TopLevelFunctionReference,
            (true, false) => HighlightRegionType::TopLevelFunctionTearOff,
            (false, true) => HighlightRegionType::LocalFunctionReference,
            (false, false) => HighlightRegionType::LocalFunctionTearOff,
        };
        self.add_region_token(Some(name_token), kind)
    }

    fn add_identifier_region_import_prefix(
        &mut self,
        name_token: TokenId,
        element: Option<ElementId>,
    ) -> bool {
        let Some(el) = element else { return false };
        if el.tag() != Tag::Prefix {
            return false;
        }
        self.add_region_token(Some(name_token), HighlightRegionType::ImportPrefix)
    }

    fn add_identifier_region_label(
        &mut self,
        name_token: TokenId,
        element: Option<ElementId>,
    ) -> bool {
        let Some(el) = element else { return false };
        if el.tag() != Tag::Label {
            return false;
        }
        self.add_region_token(Some(name_token), HighlightRegionType::LABEL)
    }

    fn add_identifier_region_local_variable(
        &mut self,
        name_token: TokenId,
        element: Option<ElementId>,
    ) -> bool {
        let Some(el) = element else { return false };
        if !matches!(
            el.tag(),
            Tag::LocalVariable
                | Tag::PatternVariable
                | Tag::BindPatternVariable
                | Tag::JoinPatternVariable
        ) {
            return false;
        }
        let ty = variable_type(self.ctx, el);
        let kind = if self.is_dynamic_type(ty) {
            HighlightRegionType::DynamicLocalVariableReference
        } else {
            HighlightRegionType::LocalVariableReference
        };
        self.add_region_token(Some(name_token), kind)
    }

    fn is_call_method(&self, parent: NodeId, name_token: TokenId) -> bool {
        if self.ast.tokens.lexeme(name_token) != "call" {
            return false;
        }
        let enclosing_instance_fn = self
            .enclosing_instance_element(parent)
            .and_then(|e| e.cast::<dartr_element::ExtensionElement>())
            .is_some_and(|ext| self.is_function_type(self.ctx.get(ext).extended_type.get()));
        if let Some(mi) = self.ast.cast::<MethodInvocation>(parent)
            && self.ast[self.ast[mi].method_name].token == name_token
        {
            let target_is_fn = self
                .real_target_of_method_invocation(mi)
                .is_some_and(|rt| self.is_function_type(self.tables.static_type.get(rt).copied()));
            if target_is_fn || enclosing_instance_fn {
                return true;
            }
        }
        if let Some(pi) = self.ast.cast::<PrefixedIdentifier>(parent)
            && self.ast[self.ast[pi].identifier].token == name_token
            && self.is_function_type(
                self.tables
                    .static_type
                    .get(self.ast[pi].prefix.raw())
                    .copied(),
            )
        {
            return true;
        }
        if let Some(pa) = self.ast.cast::<PropertyAccess>(parent)
            && self.ast[self.ast[pa].property_name].token == name_token
            && self
                .real_target_of_property_access(pa)
                .is_some_and(|rt| self.is_function_type(self.tables.static_type.get(rt).copied()))
        {
            return true;
        }
        false
    }

    fn add_identifier_region_method(
        &mut self,
        parent: NodeId,
        name_token: TokenId,
        element: Option<ElementId>,
    ) -> bool {
        let is_invocation = self
            .ast
            .cast::<MethodInvocation>(parent)
            .is_some_and(|mi| self.ast[self.ast[mi].method_name].token == name_token)
            || self
                .ast
                .cast::<DotShorthandInvocation>(parent)
                .is_some_and(|dsi| self.ast[self.ast[dsi].member_name].token == name_token);
        if self.is_call_method(parent, name_token) {
            return self.add_region_token(
                Some(name_token),
                if is_invocation {
                    HighlightRegionType::InstanceMethodReference
                } else {
                    HighlightRegionType::InstanceMethodTearOff
                },
            );
        }
        let Some(el) = element else { return false };
        if el.tag() != Tag::Method {
            return false;
        }
        let is_static = self.is_static_element(el);
        let kind = match (is_static, is_invocation) {
            (true, true) => HighlightRegionType::StaticMethodReference,
            (true, false) => HighlightRegionType::StaticMethodTearOff,
            (false, true) => HighlightRegionType::InstanceMethodReference,
            (false, false) => HighlightRegionType::InstanceMethodTearOff,
        };
        self.add_region_token(Some(name_token), kind)
    }

    fn add_identifier_region_parameter(
        &mut self,
        name_token: TokenId,
        element: Option<ElementId>,
    ) -> bool {
        let Some(el) = element else { return false };
        if !matches!(
            el.tag(),
            Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter
        ) {
            return false;
        }
        let ty = variable_type(self.ctx, el);
        let kind = if self.is_dynamic_type(ty) {
            HighlightRegionType::DynamicParameterReference
        } else {
            HighlightRegionType::ParameterReference
        };
        self.add_region_token(Some(name_token), kind)
    }

    fn add_identifier_region_type_alias(
        &mut self,
        name_token: TokenId,
        element: Option<ElementId>,
    ) -> bool {
        let Some(el) = element else { return false };
        let Some(ta) = el.cast::<TypeAliasElement>() else {
            return false;
        };
        let is_fn = self
            .ctx
            .get(ta)
            .aliased_type
            .get()
            .is_some_and(|ty| matches!(self.ctx.ty(ty), TypeKind::Function(_)));
        let kind = if is_fn {
            HighlightRegionType::FunctionTypeAlias
        } else {
            HighlightRegionType::TypeAlias
        };
        self.add_region_token(Some(name_token), kind)
    }

    fn add_identifier_region_type_parameter(
        &mut self,
        name_token: TokenId,
        element: Option<ElementId>,
    ) -> bool {
        let Some(el) = element else { return false };
        if el.tag() != Tag::TypeParameter {
            return false;
        }
        self.add_region_token(Some(name_token), HighlightRegionType::TypeParameter)
    }

    fn add_identifier_region_unresolved_instance_member_reference(
        &mut self,
        parent: NodeId,
        name_token: TokenId,
        element: Option<ElementId>,
    ) -> bool {
        if element.is_some() {
            return false;
        }
        let mut decorate = false;
        if let Some(mi) = self.ast.cast::<MethodInvocation>(parent) {
            if self.ast[self.ast[mi].method_name].token == name_token
                && let Some(rt) = self.real_target_of_method_invocation(mi)
                && self.is_dynamic_expression(rt)
            {
                decorate = true;
            }
        } else if let Some(pi) = self.ast.cast::<PrefixedIdentifier>(parent) {
            decorate = self.ast[self.ast[pi].identifier].token == name_token;
        } else if let Some(pa) = self.ast.cast::<PropertyAccess>(parent) {
            decorate = self.ast[self.ast[pa].property_name].token == name_token;
        }
        if decorate {
            return self.add_region_token(
                Some(name_token),
                HighlightRegionType::UnresolvedInstanceMemberReference,
            );
        }
        false
    }

    fn is_dynamic_expression(&self, expr: NodeId) -> bool {
        self.tables
            .static_type
            .get(expr)
            .is_some_and(|&ty| matches!(self.ctx.ty(ty), TypeKind::Dynamic | TypeKind::Invalid))
    }

    fn add_regions_function_body(&mut self, keyword: Option<TokenId>, star: Option<TokenId>) {
        if let Some(kw) = keyword {
            let offset = self.ast.tokens.get(kw).offset;
            let end = match star {
                Some(s) => self.ast.tokens.get(s).end(),
                None => self.ast.tokens.get(kw).end(),
            };
            self.add_region(
                offset,
                end.saturating_sub(offset),
                HighlightRegionType::KEYWORD,
            );
        }
    }

    fn add_regions_configurations(&mut self, configs: NodeList<Configuration>) {
        for &cfg in self.ast.list(configs) {
            self.add_region_token(Some(self.ast[cfg].if_keyword), HighlightRegionType::KEYWORD);
        }
    }
}

impl AstVisitor for HighlightsComputer<'_> {
    fn visit_annotation(&mut self, ast: &Ast, node: Id<Annotation>) {
        match ast[node].arguments {
            None => {
                self.add_region_node(node, HighlightRegionType::ANNOTATION);
            }
            Some(args) => {
                let begin = ast.begin_token(args.raw());
                let end = ast.end_token(args.raw());
                self.add_region_node_start_token_end(node, begin, HighlightRegionType::ANNOTATION);
                self.add_region_token(Some(end), HighlightRegionType::ANNOTATION);
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_as_expression(&mut self, ast: &Ast, node: Id<AsExpression>) {
        self.add_region_token(Some(ast[node].as_operator), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_assert_statement(&mut self, ast: &Ast, node: Id<AssertStatement>) {
        self.add_region_token(Some(ast[node].assert_keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_assigned_variable_pattern(&mut self, ast: &Ast, node: Id<AssignedVariablePattern>) {
        self.add_region_token(
            Some(ast[node].name),
            HighlightRegionType::LocalVariableReference,
        );
        ast.visit_children(node, self);
    }

    fn visit_await_expression(&mut self, ast: &Ast, node: Id<AwaitExpression>) {
        self.add_region_token(Some(ast[node].await_keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_block_function_body(&mut self, ast: &Ast, node: Id<BlockFunctionBody>) {
        self.add_regions_function_body(ast[node].keyword, ast[node].star);
        ast.visit_children(node, self);
    }

    fn visit_boolean_literal(&mut self, ast: &Ast, node: Id<BooleanLiteral>) {
        self.add_region_node(node, HighlightRegionType::KEYWORD);
        self.add_region_node(node, HighlightRegionType::LiteralBoolean);
        ast.visit_children(node, self);
    }

    fn visit_break_statement(&mut self, ast: &Ast, node: Id<BreakStatement>) {
        self.add_region_token(Some(ast[node].break_keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_case_clause(&mut self, ast: &Ast, node: Id<CaseClause>) {
        self.add_region_token(Some(ast[node].case_keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_cast_pattern(&mut self, ast: &Ast, node: Id<CastPattern>) {
        self.add_region_token(Some(ast[node].as_token), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_catch_clause(&mut self, ast: &Ast, node: Id<CatchClause>) {
        self.add_region_token(ast[node].catch_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(ast[node].on_keyword, HighlightRegionType::KEYWORD);
        if let Some(exc) = ast[node].exception_parameter {
            self.add_region_token(
                Some(ast[exc].name),
                HighlightRegionType::LocalVariableDeclaration,
            );
        }
        if let Some(st) = ast[node].stack_trace_parameter {
            self.add_region_token(
                Some(ast[st].name),
                HighlightRegionType::LocalVariableDeclaration,
            );
        }
        ast.visit_children(node, self);
    }

    fn visit_class_declaration(&mut self, ast: &Ast, node: Id<ClassDeclaration>) {
        let c = &ast[node];
        self.add_region_token(c.augment_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(c.abstract_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(c.sealed_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(c.base_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(c.interface_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(c.final_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(c.mixin_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(Some(c.class_keyword), HighlightRegionType::KEYWORD);
        let type_name = dartr_resolver::error::support::class_name_token(ast, c.name_part);
        self.add_region_token(Some(type_name), HighlightRegionType::CLASS);
        ast.visit_children(node, self);
    }

    fn visit_class_type_alias(&mut self, ast: &Ast, node: Id<ClassTypeAlias>) {
        let c = &ast[node];
        self.add_region_token(c.abstract_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(c.sealed_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(c.base_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(c.interface_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(c.final_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(c.mixin_keyword, HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_constant_pattern(&mut self, ast: &Ast, node: Id<ConstantPattern>) {
        self.add_region_token(ast[node].const_keyword, HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_constructor_declaration(&mut self, ast: &Ast, node: Id<ConstructorDeclaration>) {
        let c = &ast[node];
        self.add_region_token(c.augment_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(c.external_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(c.factory_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(c.const_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(c.new_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(c.name, HighlightRegionType::CONSTRUCTOR);
        ast.visit_children(node, self);
    }

    fn visit_constructor_reference(&mut self, ast: &Ast, node: Id<ConstructorReference>) {
        let cn = ast[node].constructor_name;
        ast.accept(ast[cn].type_, self);
        if let Some(name) = ast[cn].name {
            self.add_region_node(name, HighlightRegionType::ConstructorTearOff);
        }
    }

    fn visit_constructor_selector(&mut self, _ast: &Ast, node: Id<ConstructorSelector>) {
        self.add_region_node(self.ast[node].name, HighlightRegionType::CONSTRUCTOR);
    }

    fn visit_continue_statement(&mut self, ast: &Ast, node: Id<ContinueStatement>) {
        self.add_region_token(
            Some(ast[node].continue_keyword),
            HighlightRegionType::KEYWORD,
        );
        ast.visit_children(node, self);
    }

    fn visit_declared_identifier(&mut self, ast: &Ast, node: Id<DeclaredIdentifier>) {
        self.add_region_token(ast[node].keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(
            Some(ast[node].name),
            HighlightRegionType::LocalVariableDeclaration,
        );
        ast.visit_children(node, self);
    }

    fn visit_declared_variable_pattern(&mut self, ast: &Ast, node: Id<DeclaredVariablePattern>) {
        self.add_region_token(ast[node].keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(
            Some(ast[node].name),
            HighlightRegionType::LocalVariableDeclaration,
        );
        ast.visit_children(node, self);
    }

    fn visit_do_statement(&mut self, ast: &Ast, node: Id<DoStatement>) {
        self.add_region_token(Some(ast[node].do_keyword), HighlightRegionType::KEYWORD);
        self.add_region_token(Some(ast[node].while_keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_dot_shorthand_property_access(
        &mut self,
        ast: &Ast,
        node: Id<DotShorthandPropertyAccess>,
    ) {
        let prop = ast[node].property_name;
        let el = self.element(prop);
        if el.is_some_and(|e| e.tag() == Tag::Constructor) {
            self.add_region_node(prop, HighlightRegionType::ConstructorTearOff);
        } else {
            self.add_identifier_region(node.raw(), ast[prop].token, el);
        }
    }

    fn visit_dotted_name(&mut self, ast: &Ast, node: Id<DottedName>) {
        if ast
            .parent(node.raw())
            .is_some_and(|p| ast.is::<Configuration>(p))
        {
            let begin = ast.begin_token(node.raw());
            let end = ast.end_token(node.raw());
            let mut idx = begin.0;
            while idx <= end.0 {
                let tok = TokenId(idx);
                if ast.tokens.get(tok).ty != TokenType::PERIOD {
                    self.add_region_token(Some(tok), HighlightRegionType::IdentifierDefault);
                }
                idx += 1;
            }
        } else {
            self.add_region_node(node, HighlightRegionType::LibraryName);
        }
        ast.visit_children(node, self);
    }

    fn visit_double_literal(&mut self, ast: &Ast, node: Id<DoubleLiteral>) {
        self.add_region_node(node, HighlightRegionType::LiteralDouble);
        ast.visit_children(node, self);
    }

    fn visit_enum_constant_declaration(&mut self, ast: &Ast, node: Id<EnumConstantDeclaration>) {
        self.add_region_token(Some(ast[node].name), HighlightRegionType::EnumConstant);
        ast.visit_children(node, self);
    }

    fn visit_enum_declaration(&mut self, ast: &Ast, node: Id<EnumDeclaration>) {
        self.add_region_token(Some(ast[node].enum_keyword), HighlightRegionType::KEYWORD);
        let type_name = dartr_resolver::error::support::class_name_token(ast, ast[node].name_part);
        self.add_region_token(Some(type_name), HighlightRegionType::ENUM);
        ast.visit_children(node, self);
    }

    fn visit_export_directive(&mut self, ast: &Ast, node: Id<ExportDirective>) {
        self.add_region_node(node, HighlightRegionType::DIRECTIVE);
        self.add_region_token(Some(ast[node].export_keyword), HighlightRegionType::KEYWORD);
        self.add_regions_configurations(ast[node].configurations);
        ast.visit_children(node, self);
    }

    fn visit_expression_function_body(&mut self, ast: &Ast, node: Id<ExpressionFunctionBody>) {
        self.add_regions_function_body(ast[node].keyword, ast[node].star);
        ast.visit_children(node, self);
    }

    fn visit_extends_clause(&mut self, ast: &Ast, node: Id<ExtendsClause>) {
        self.add_region_token(
            Some(ast[node].extends_keyword),
            HighlightRegionType::KEYWORD,
        );
        ast.visit_children(node, self);
    }

    fn visit_extension_declaration(&mut self, ast: &Ast, node: Id<ExtensionDeclaration>) {
        let e = &ast[node];
        self.add_region_token(e.augment_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(Some(e.extension_keyword), HighlightRegionType::KEYWORD);
        self.add_region_token(e.name, HighlightRegionType::EXTENSION);
        ast.visit_children(node, self);
    }

    fn visit_extension_on_clause(&mut self, ast: &Ast, node: Id<ExtensionOnClause>) {
        self.add_region_token(Some(ast[node].on_keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_extension_override(&mut self, ast: &Ast, node: Id<ExtensionOverride>) {
        self.add_region_token(Some(ast[node].name), HighlightRegionType::EXTENSION);
        ast.visit_children(node, self);
    }

    fn visit_extension_type_declaration(&mut self, ast: &Ast, node: Id<ExtensionTypeDeclaration>) {
        let e = &ast[node];
        self.add_region_token(Some(e.extension_keyword), HighlightRegionType::KEYWORD);
        self.add_region_token(Some(e.type_keyword), HighlightRegionType::KEYWORD);
        let type_name = dartr_resolver::error::support::class_name_token(ast, e.name_part);
        self.add_region_token(Some(type_name), HighlightRegionType::ExtensionType);
        ast.visit_children(node, self);
    }

    fn visit_field_declaration(&mut self, ast: &Ast, node: Id<FieldDeclaration>) {
        let f = &ast[node];
        self.add_region_token(f.abstract_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(f.external_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(f.static_keyword, HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_field_formal_parameter(&mut self, ast: &Ast, node: Id<FieldFormalParameter>) {
        let f = &ast[node];
        self.add_region_token(f.required_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(f.const_final_or_var_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(Some(f.this_keyword), HighlightRegionType::KEYWORD);
        self.add_region_token(Some(f.name), HighlightRegionType::InstanceFieldReference);
        ast.visit_children(node, self);
    }

    fn visit_for_each_parts_with_declaration(
        &mut self,
        ast: &Ast,
        node: Id<ForEachPartsWithDeclaration>,
    ) {
        self.add_region_token(Some(ast[node].in_keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_for_each_parts_with_identifier(
        &mut self,
        ast: &Ast,
        node: Id<ForEachPartsWithIdentifier>,
    ) {
        self.add_region_token(Some(ast[node].in_keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_for_each_parts_with_pattern(&mut self, ast: &Ast, node: Id<ForEachPartsWithPattern>) {
        self.add_region_token(Some(ast[node].keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_for_element(&mut self, ast: &Ast, node: Id<ForElement>) {
        self.add_region_token(ast[node].await_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(Some(ast[node].for_keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_for_statement(&mut self, ast: &Ast, node: Id<ForStatement>) {
        self.add_region_token(ast[node].await_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(Some(ast[node].for_keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_function_declaration(&mut self, ast: &Ast, node: Id<FunctionDeclaration>) {
        let f = &ast[node];
        self.add_region_token(f.augment_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(f.external_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(f.property_keyword, HighlightRegionType::KEYWORD);
        let prop = f.property_keyword.map(|t| ast.tokens.lexeme(t));
        let is_top_level = ast
            .parent(node.raw())
            .is_some_and(|p| ast.is::<CompilationUnit>(p));
        let name_type = match prop {
            Some("get") => HighlightRegionType::TopLevelGetterDeclaration,
            Some("set") => HighlightRegionType::TopLevelSetterDeclaration,
            _ if is_top_level => HighlightRegionType::TopLevelFunctionDeclaration,
            _ => HighlightRegionType::LocalFunctionDeclaration,
        };
        self.add_region_token(Some(f.name), name_type);
        ast.visit_children(node, self);
    }

    fn visit_function_type_alias(&mut self, ast: &Ast, node: Id<FunctionTypeAlias>) {
        self.add_region_token(
            Some(ast[node].typedef_keyword),
            HighlightRegionType::KEYWORD,
        );
        self.add_region_token(Some(ast[node].name), HighlightRegionType::FunctionTypeAlias);
        ast.visit_children(node, self);
    }

    fn visit_generic_function_type(&mut self, ast: &Ast, node: Id<GenericFunctionType>) {
        self.add_region_token(
            Some(ast[node].function_keyword),
            HighlightRegionType::KEYWORD,
        );
        ast.visit_children(node, self);
    }

    fn visit_generic_type_alias(&mut self, ast: &Ast, node: Id<GenericTypeAlias>) {
        self.add_region_token(
            Some(ast[node].typedef_keyword),
            HighlightRegionType::KEYWORD,
        );
        let name_type = if ast.is::<GenericFunctionType>(ast[node].type_) {
            HighlightRegionType::FunctionTypeAlias
        } else {
            HighlightRegionType::TypeAlias
        };
        self.add_region_token(Some(ast[node].name), name_type);
        ast.visit_children(node, self);
    }

    fn visit_hide_combinator(&mut self, ast: &Ast, node: Id<HideCombinator>) {
        self.add_region_token(Some(ast[node].keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_if_element(&mut self, ast: &Ast, node: Id<IfElement>) {
        self.add_region_token(Some(ast[node].if_keyword), HighlightRegionType::KEYWORD);
        self.add_region_token(ast[node].else_keyword, HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_if_statement(&mut self, ast: &Ast, node: Id<IfStatement>) {
        self.add_region_token(Some(ast[node].if_keyword), HighlightRegionType::KEYWORD);
        self.add_region_token(ast[node].else_keyword, HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_implements_clause(&mut self, ast: &Ast, node: Id<ImplementsClause>) {
        self.add_region_token(
            Some(ast[node].implements_keyword),
            HighlightRegionType::KEYWORD,
        );
        ast.visit_children(node, self);
    }

    fn visit_import_directive(&mut self, ast: &Ast, node: Id<ImportDirective>) {
        let d = &ast[node];
        self.add_region_node(node, HighlightRegionType::DIRECTIVE);
        self.add_region_token(Some(d.import_keyword), HighlightRegionType::KEYWORD);
        self.add_region_token(d.deferred_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(d.as_keyword, HighlightRegionType::KEYWORD);
        self.add_regions_configurations(d.configurations);
        ast.visit_children(node, self);
    }

    fn visit_import_prefix_reference(&mut self, _ast: &Ast, node: Id<ImportPrefixReference>) {
        self.add_region_token(Some(self.ast[node].name), HighlightRegionType::ImportPrefix);
    }

    fn visit_instance_creation_expression(
        &mut self,
        ast: &Ast,
        node: Id<InstanceCreationExpression>,
    ) {
        self.add_region_token(ast[node].keyword, HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_integer_literal(&mut self, ast: &Ast, node: Id<IntegerLiteral>) {
        self.add_region_node(node, HighlightRegionType::LiteralInteger);
        ast.visit_children(node, self);
    }

    fn visit_interpolation_string(&mut self, ast: &Ast, node: Id<InterpolationString>) {
        self.add_region_node(node, HighlightRegionType::LiteralString);
        ast.visit_children(node, self);
    }

    fn visit_is_expression(&mut self, ast: &Ast, node: Id<IsExpression>) {
        self.add_region_token(Some(ast[node].is_operator), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_label(&mut self, ast: &Ast, node: Id<Label>) {
        self.add_region_token(Some(ast[node].name), HighlightRegionType::LABEL);
        ast.visit_children(node, self);
    }

    fn visit_label_reference(&mut self, ast: &Ast, node: Id<LabelReference>) {
        self.add_region_token(Some(ast[node].name), HighlightRegionType::LABEL);
        ast.visit_children(node, self);
    }

    fn visit_library_directive(&mut self, ast: &Ast, node: Id<LibraryDirective>) {
        self.add_region_node(node, HighlightRegionType::DIRECTIVE);
        self.add_region_token(
            Some(ast[node].library_keyword),
            HighlightRegionType::KEYWORD,
        );
        ast.visit_children(node, self);
    }

    fn visit_list_literal(&mut self, ast: &Ast, node: Id<ListLiteral>) {
        self.add_region_node(node, HighlightRegionType::LiteralList);
        self.add_region_token(ast[node].const_keyword, HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_method_declaration(&mut self, ast: &Ast, node: Id<MethodDeclaration>) {
        let m = &ast[node];
        self.add_region_token(m.augment_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(m.external_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(m.modifier_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(m.operator_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(m.property_keyword, HighlightRegionType::KEYWORD);
        let prop = m.property_keyword.map(|t| ast.tokens.lexeme(t));
        let is_static = m
            .modifier_keyword
            .is_some_and(|t| ast.tokens.lexeme(t) == "static");
        let name_type = match (prop, is_static) {
            (Some("get"), true) => HighlightRegionType::StaticGetterDeclaration,
            (Some("get"), false) => HighlightRegionType::InstanceGetterDeclaration,
            (Some("set"), true) => HighlightRegionType::StaticSetterDeclaration,
            (Some("set"), false) => HighlightRegionType::InstanceSetterDeclaration,
            (_, true) => HighlightRegionType::StaticMethodDeclaration,
            (_, false) => HighlightRegionType::InstanceMethodDeclaration,
        };
        self.add_region_token(Some(m.name), name_type);
        ast.visit_children(node, self);
    }

    fn visit_mixin_declaration(&mut self, ast: &Ast, node: Id<MixinDeclaration>) {
        let m = &ast[node];
        self.add_region_token(m.augment_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(m.base_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(Some(m.mixin_keyword), HighlightRegionType::KEYWORD);
        self.add_region_token(Some(m.name), HighlightRegionType::MIXIN);
        ast.visit_children(node, self);
    }

    fn visit_mixin_on_clause(&mut self, ast: &Ast, node: Id<MixinOnClause>) {
        self.add_region_token(Some(ast[node].on_keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_named_argument(&mut self, ast: &Ast, node: Id<NamedArgument>) {
        let parameter = dartr_resolver::error::support::corresponding_parameter(
            self.ctx,
            ast,
            self.tables,
            node.raw(),
        );
        if let Some(param) = parameter {
            let ty = variable_type(self.ctx, param);
            let kind = if self.is_dynamic_type(ty) {
                HighlightRegionType::DynamicParameterReference
            } else {
                HighlightRegionType::ParameterReference
            };
            self.add_region_token(Some(ast[node].name), kind);
        } else {
            self.add_identifier_region(node.raw(), ast[node].name, None);
        }
        ast.accept(ast[node].argument_expression, self);
    }

    fn visit_named_type(&mut self, ast: &Ast, node: Id<NamedType>) {
        if let Some(prefix) = ast[node].import_prefix {
            self.add_region_token(Some(ast[prefix].name), HighlightRegionType::ImportPrefix);
        }
        let el = self.element(node);
        let ty = self
            .tables
            .annotation_type
            .get(node)
            .copied()
            .or_else(|| self.tables.static_type.get(node.raw()).copied());
        let is_dynamic = (ty.is_some_and(|t| matches!(self.ctx.ty(t), TypeKind::Dynamic))
            || el.is_some_and(|e| e.tag() == Tag::Dynamic))
            && ast.tokens.lexeme(ast[node].name) == "dynamic";
        let is_never = ty.is_some_and(|t| matches!(self.ctx.ty(t), TypeKind::Never(_)))
            || el.is_some_and(|e| e.tag() == Tag::Never);
        if is_dynamic || is_never {
            self.add_region_token(
                Some(ast[node].name),
                if is_dynamic {
                    HighlightRegionType::TypeNameDynamic
                } else {
                    HighlightRegionType::CLASS
                },
            );
            return;
        }
        self.add_identifier_region(node.raw(), ast[node].name, el);
        if let Some(ta) = ast[node].type_arguments {
            ast.accept(ta, self);
        }
    }

    fn visit_native_clause(&mut self, ast: &Ast, node: Id<NativeClause>) {
        self.add_region_token(Some(ast[node].native_keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_native_function_body(&mut self, ast: &Ast, node: Id<NativeFunctionBody>) {
        self.add_region_token(Some(ast[node].native_keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_null_literal(&mut self, ast: &Ast, node: Id<NullLiteral>) {
        self.add_region_token(Some(ast[node].literal), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_part_directive(&mut self, ast: &Ast, node: Id<PartDirective>) {
        self.add_region_node(node, HighlightRegionType::DIRECTIVE);
        self.add_region_token(Some(ast[node].part_keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_part_of_directive(&mut self, ast: &Ast, node: Id<PartOfDirective>) {
        self.add_region_node(node, HighlightRegionType::DIRECTIVE);
        self.add_region_token_start_token_end(
            ast[node].part_keyword,
            ast[node].of_keyword,
            HighlightRegionType::KEYWORD,
        );
        ast.visit_children(node, self);
    }

    fn visit_pattern_field(&mut self, ast: &Ast, node: Id<PatternField>) {
        if let Some(name_node) = ast[node].name
            && let Some(name) = ast[name_node].name
        {
            let el = self.element(node);
            let kind = if el.is_some_and(|e| e.tag() == Tag::Method) {
                HighlightRegionType::InstanceMethodTearOff
            } else {
                HighlightRegionType::InstanceGetterReference
            };
            self.add_region_token(Some(name), kind);
        }
        ast.visit_children(node, self);
    }

    fn visit_pattern_variable_declaration(
        &mut self,
        ast: &Ast,
        node: Id<PatternVariableDeclaration>,
    ) {
        self.add_region_token(Some(ast[node].keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_primary_constructor_body(&mut self, ast: &Ast, node: Id<PrimaryConstructorBody>) {
        self.add_region_token(Some(ast[node].this_keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_primary_constructor_declaration(
        &mut self,
        ast: &Ast,
        node: Id<PrimaryConstructorDeclaration>,
    ) {
        self.add_region_token(ast[node].const_keyword, HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_primary_constructor_name(&mut self, ast: &Ast, node: Id<PrimaryConstructorName>) {
        self.add_region_token(Some(ast[node].name), HighlightRegionType::CONSTRUCTOR);
        ast.visit_children(node, self);
    }

    fn visit_record_literal(&mut self, ast: &Ast, node: Id<RecordLiteral>) {
        self.add_region_node(node, HighlightRegionType::LiteralRecord);
        self.add_region_token(ast[node].const_keyword, HighlightRegionType::KEYWORD);
        for &field in ast.list(ast[node].fields) {
            if let Some(named) = ast.cast::<RecordLiteralNamedField>(field) {
                self.add_region_token(
                    Some(ast[named].name),
                    HighlightRegionType::ParameterReference,
                );
                ast.accept(ast[named].field_expression, self);
            } else {
                ast.accept(field, self);
            }
        }
    }

    fn visit_record_type_annotation(&mut self, ast: &Ast, node: Id<RecordTypeAnnotation>) {
        for &f in ast.list(ast[node].positional_fields) {
            self.add_region_token(ast[f].name, HighlightRegionType::FIELD);
        }
        if let Some(named) = ast[node].named_fields {
            for &f in ast.list(ast[named].fields) {
                self.add_region_token(Some(ast[f].name), HighlightRegionType::FIELD);
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_regular_formal_parameter(&mut self, ast: &Ast, node: Id<RegularFormalParameter>) {
        let p = &ast[node];
        self.add_region_token(p.required_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(p.covariant_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(p.const_final_or_var_keyword, HighlightRegionType::KEYWORD);
        let is_dynamic = self
            .declared_element(node)
            .is_some_and(|el| self.is_dynamic_type(variable_type(self.ctx, el)));
        self.add_region_token(
            p.name,
            if is_dynamic {
                HighlightRegionType::DynamicParameterDeclaration
            } else {
                HighlightRegionType::ParameterDeclaration
            },
        );
        ast.visit_children(node, self);
    }

    fn visit_rethrow_expression(&mut self, ast: &Ast, node: Id<RethrowExpression>) {
        self.add_region_token(
            Some(ast[node].rethrow_keyword),
            HighlightRegionType::KEYWORD,
        );
        ast.visit_children(node, self);
    }

    fn visit_return_statement(&mut self, ast: &Ast, node: Id<ReturnStatement>) {
        self.add_region_token(Some(ast[node].return_keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_set_or_map_literal(&mut self, ast: &Ast, node: Id<SetOrMapLiteral>) {
        let is_map =
            self.tables
                .static_type
                .get(node.raw())
                .is_some_and(|&ty| match self.ctx.ty(ty) {
                    TypeKind::Interface { element, .. } => {
                        self.ctx
                            .interface(*element)
                            .name
                            .is_some_and(|n| self.ctx.name_str(n) == "Map")
                            && self.is_in_dart_core(element.raw())
                    }
                    _ => false,
                })
                || ast
                    .list(ast[node].elements)
                    .iter()
                    .any(|&e| ast.is::<MapLiteralEntry>(e));
        if is_map {
            self.add_region_node(node, HighlightRegionType::LiteralMap);
        }
        self.add_region_token(ast[node].const_keyword, HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_show_combinator(&mut self, ast: &Ast, node: Id<ShowCombinator>) {
        self.add_region_token(Some(ast[node].keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_simple_identifier(&mut self, ast: &Ast, node: Id<SimpleIdentifier>) {
        if let Some(parent) = ast.parent(node.raw()) {
            let el = self.write_or_read_element(node);
            self.add_identifier_region(parent, ast[node].token, el);
        }
        ast.visit_children(node, self);
    }

    fn visit_simple_string_literal(&mut self, ast: &Ast, node: Id<SimpleStringLiteral>) {
        self.add_region_node(node, HighlightRegionType::LiteralString);
        ast.visit_children(node, self);
    }

    fn visit_super_constructor_invocation(
        &mut self,
        ast: &Ast,
        node: Id<SuperConstructorInvocation>,
    ) {
        self.add_region_token(Some(ast[node].super_keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_super_expression(&mut self, ast: &Ast, node: Id<SuperExpression>) {
        self.add_region_token(Some(ast[node].super_keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_super_formal_parameter(&mut self, ast: &Ast, node: Id<SuperFormalParameter>) {
        let p = &ast[node];
        self.add_region_token(p.required_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(p.const_final_or_var_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(Some(p.super_keyword), HighlightRegionType::KEYWORD);
        let is_dynamic = self
            .declared_element(node)
            .is_some_and(|el| self.is_dynamic_type(variable_type(self.ctx, el)));
        self.add_region_token(
            Some(p.name),
            if is_dynamic {
                HighlightRegionType::DynamicParameterDeclaration
            } else {
                HighlightRegionType::ParameterDeclaration
            },
        );
        ast.visit_children(node, self);
    }

    fn visit_switch_case(&mut self, ast: &Ast, node: Id<SwitchCase>) {
        self.add_region_token(Some(ast[node].keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_switch_default(&mut self, ast: &Ast, node: Id<SwitchDefault>) {
        self.add_region_token(Some(ast[node].keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_switch_expression(&mut self, ast: &Ast, node: Id<SwitchExpression>) {
        self.add_region_token(Some(ast[node].switch_keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_switch_pattern_case(&mut self, ast: &Ast, node: Id<SwitchPatternCase>) {
        self.add_region_token(Some(ast[node].keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_switch_statement(&mut self, ast: &Ast, node: Id<SwitchStatement>) {
        self.add_region_token(Some(ast[node].switch_keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_this_expression(&mut self, ast: &Ast, node: Id<ThisExpression>) {
        self.add_region_token(Some(ast[node].this_keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_throw_expression(&mut self, ast: &Ast, node: Id<ThrowExpression>) {
        self.add_region_token(Some(ast[node].throw_keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_top_level_variable_declaration(
        &mut self,
        ast: &Ast,
        node: Id<TopLevelVariableDeclaration>,
    ) {
        self.add_region_token(ast[node].augment_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(ast[node].external_keyword, HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_try_statement(&mut self, ast: &Ast, node: Id<TryStatement>) {
        self.add_region_token(Some(ast[node].try_keyword), HighlightRegionType::KEYWORD);
        self.add_region_token(ast[node].finally_keyword, HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_type_parameter(&mut self, ast: &Ast, node: Id<TypeParameter>) {
        self.add_region_token(Some(ast[node].name), HighlightRegionType::TypeParameter);
        self.add_region_token(ast[node].extends_keyword, HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_variable_declaration(&mut self, ast: &Ast, node: Id<VariableDeclaration>) {
        if let Some(el) = self.declared_element(node) {
            match el.tag() {
                Tag::Field => {
                    let kind = if self.is_static_element(el) {
                        HighlightRegionType::StaticFieldDeclaration
                    } else {
                        HighlightRegionType::InstanceFieldDeclaration
                    };
                    self.add_region_token(Some(ast[node].name), kind);
                }
                Tag::LocalVariable
                | Tag::PatternVariable
                | Tag::BindPatternVariable
                | Tag::JoinPatternVariable => {
                    let kind = if self.is_dynamic_type(variable_type(self.ctx, el)) {
                        HighlightRegionType::DynamicLocalVariableDeclaration
                    } else {
                        HighlightRegionType::LocalVariableDeclaration
                    };
                    self.add_region_token(Some(ast[node].name), kind);
                }
                Tag::TopLevelVariable => {
                    self.add_region_token(
                        Some(ast[node].name),
                        HighlightRegionType::TopLevelVariableDeclaration,
                    );
                }
                _ => {}
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_variable_declaration_list(&mut self, ast: &Ast, node: Id<VariableDeclarationList>) {
        self.add_region_token(ast[node].late_keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(ast[node].keyword, HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_when_clause(&mut self, ast: &Ast, node: Id<WhenClause>) {
        self.add_region_token(Some(ast[node].when_keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_while_statement(&mut self, ast: &Ast, node: Id<WhileStatement>) {
        self.add_region_token(Some(ast[node].while_keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_wildcard_pattern(&mut self, ast: &Ast, node: Id<WildcardPattern>) {
        self.add_region_token(ast[node].keyword, HighlightRegionType::KEYWORD);
        self.add_region_token(
            Some(ast[node].name),
            HighlightRegionType::LocalVariableDeclaration,
        );
        ast.visit_children(node, self);
    }

    fn visit_with_clause(&mut self, ast: &Ast, node: Id<WithClause>) {
        self.add_region_token(Some(ast[node].with_keyword), HighlightRegionType::KEYWORD);
        ast.visit_children(node, self);
    }

    fn visit_yield_statement(&mut self, ast: &Ast, node: Id<YieldStatement>) {
        let kw = ast[node].yield_keyword;
        let offset = ast.tokens.get(kw).offset;
        let end = match ast[node].star {
            Some(s) => ast.tokens.get(s).end(),
            None => ast.tokens.get(kw).end(),
        };
        self.add_region(
            offset,
            end.saturating_sub(offset),
            HighlightRegionType::KEYWORD,
        );
        ast.visit_children(node, self);
    }
}
