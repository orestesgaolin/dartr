use dartr_ast::*;
use dartr_element::{
    ConstructorFragment, Ctx, ElemRef, ElementId, FragmentId, LibraryFragment, ResolutionTables,
    Tag,
};
use dartr_resolver::element_ext;
use dartr_resolver::error::support::corresponding_parameter;
use dartr_syntax::TokenId;
use dartr_typesystem::member;
use rustc_hash::FxHashMap;

use crate::convert::{convert_navigation_target_kind, line_col_from_starts};
use crate::protocol::{ElementKind, Location, NavigationRegion, NavigationTarget};

type TargetKey = (
    ElementKind,
    String,
    i64,
    i64,
    i64,
    i64,
    Option<i64>,
    Option<i64>,
);

pub struct LegacyNavigationCollector {
    pub regions: Vec<NavigationRegion>,
    region_list: Vec<((i64, i64), Vec<i64>)>,
    region_map: FxHashMap<(i64, i64), usize>,
    pub targets: Vec<NavigationTarget>,
    target_map: FxHashMap<TargetKey, i64>,
    pub files: Vec<String>,
    file_map: FxHashMap<String, i64>,
}

impl Default for LegacyNavigationCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl LegacyNavigationCollector {
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
            region_list: Vec::new(),
            region_map: FxHashMap::default(),
            targets: Vec::new(),
            target_map: FxHashMap::default(),
            files: Vec::new(),
            file_map: FxHashMap::default(),
        }
    }

    pub fn add_region(&mut self, offset: u32, length: u32, kind: ElementKind, location: Location) {
        let range = (offset as i64, length as i64);
        let target_idx = self.add_target(kind, location);
        if let Some(&idx) = self.region_map.get(&range) {
            self.region_list[idx].1.push(target_idx);
        } else {
            let idx = self.region_list.len();
            self.region_map.insert(range, idx);
            self.region_list.push((range, vec![target_idx]));
        }
    }

    fn add_file(&mut self, file: &str) -> i64 {
        if let Some(&idx) = self.file_map.get(file) {
            return idx;
        }
        let idx = self.files.len() as i64;
        self.files.push(file.to_string());
        self.file_map.insert(file.to_string(), idx);
        idx
    }

    fn add_target(&mut self, kind: ElementKind, location: Location) -> i64 {
        let key = (
            kind.clone(),
            location.file.clone(),
            location.offset,
            location.length,
            location.start_line,
            location.start_column,
            location.end_line,
            location.end_column,
        );
        if let Some(&idx) = self.target_map.get(&key) {
            return idx;
        }
        let file_index = self.add_file(&location.file);
        let idx = self.targets.len() as i64;
        let target = NavigationTarget {
            kind,
            file_index,
            offset: location.offset,
            length: location.length,
            start_line: location.start_line,
            start_column: location.start_column,
            code_offset: None,
            code_length: None,
        };
        self.targets.push(target);
        self.target_map.insert(key, idx);
        idx
    }

    pub fn create_regions(&mut self) {
        let mut list = std::mem::take(&mut self.region_list);
        list.sort_by_key(|((offset, _), _)| *offset);
        self.regions = list
            .into_iter()
            .map(|((offset, length), targets)| NavigationRegion {
                offset,
                length,
                targets,
            })
            .collect();
    }
}

pub fn compute_dart_navigation(
    ctx: &Ctx<'_>,
    ast: &Ast,
    tables: &ResolutionTables,
    unit: Id<CompilationUnit>,
    requested_offset: Option<u32>,
    requested_length: Option<u32>,
) -> LegacyNavigationCollector {
    let mut visitor = NavigationVisitor {
        ctx,
        tables,
        requested_offset,
        requested_length,
        collector: LegacyNavigationCollector::new(),
    };
    if let (Some(offset), Some(length)) = (requested_offset, requested_length) {
        if let Some(mut node) = ast.node_covering(unit, offset, length) {
            let mut n = Some(node);
            while let Some(current) = n {
                if ast.is::<Directive>(current) {
                    node = current;
                    break;
                }
                n = ast.parent(current);
            }
            let node = navigation_target_node(ast, node);
            if ast.is::<CompilationUnit>(node) {
                visitor.visit_unit_sorted(ast, unit);
            } else {
                ast.accept(node, &mut visitor);
            }
        }
    } else {
        visitor.visit_unit_sorted(ast, unit);
    }
    visitor.collector.create_regions();
    visitor.collector
}

fn navigation_target_node(ast: &Ast, node: NodeId) -> NodeId {
    let mut current = node;
    while let Some(parent) = ast.parent(current) {
        if ast.offset(current) != ast.offset(parent) {
            break;
        }
        current = parent;
    }
    let parent = ast.parent(current);
    if let Some(p) = parent
        && ast.is::<FormalParameter>(p)
    {
        current = p;
    }
    if ast.is::<TypeArgumentList>(current)
        && let Some(p) = ast.parent(current)
    {
        current = p;
    }
    current
}

fn fragment_to_plugin_kind(ctx: &Ctx<'_>, fragment: FragmentId) -> Option<ElementKind> {
    let element = ctx.fragment_data(fragment)?.element.try_get().copied()?;
    if matches!(
        element.tag(),
        Tag::Dynamic | Tag::Never | Tag::MultiplyDefined
    ) {
        return None;
    }
    if let Some(lib_frag_id) = fragment.cast::<LibraryFragment>() {
        let lib_elem = ctx.fragment(lib_frag_id).library;
        if lib_frag_id != ctx.get(lib_elem).first_fragment() {
            return Some(ElementKind::CompilationUnit);
        }
    }
    Some(convert_navigation_target_kind(element))
}

fn fragment_to_location(ctx: &Ctx<'_>, fragment: FragmentId) -> Option<Location> {
    let lib_frag_id = dartr_element::diagnostics::library_fragment_of(ctx, fragment)?;
    let lib_frag = ctx.fragment(lib_frag_id);
    let data = ctx.fragment_data(fragment)?;
    let mut name_offset = data.name_offset.or(if fragment.tag() == Tag::Label {
        data.first_token_offset
    } else {
        None
    });
    let mut name_length = data
        .name
        .map(|n| ctx.name_str(n).encode_utf16().count() as u32);
    if name_offset.is_none()
        && let Some(c) = fragment.cast::<ConstructorFragment>()
    {
        let c = ctx.fragment(c);
        if let Some(type_name) = c.type_name {
            name_offset = c.type_name_offset;
            name_length = Some(ctx.name_str(type_name).encode_utf16().count() as u32);
        } else if let Some(o) = c.new_keyword_offset {
            name_offset = Some(o);
            name_length = Some(3);
        } else if let Some(o) = c.factory_keyword_offset {
            name_offset = Some(o);
            name_length = Some(7);
        }
    }
    let (offset, length) = match (name_offset, name_length) {
        (Some(o), Some(l)) => (o, l),
        _ => (0, 0),
    };
    let (start_line, start_col) = line_col_from_starts(&lib_frag.line_starts, offset);
    let (end_line, end_col) =
        line_col_from_starts(&lib_frag.line_starts, offset.saturating_add(length));
    Some(Location {
        file: lib_frag.source.path.to_string(),
        offset: offset as i64,
        length: length as i64,
        start_line: start_line as i64,
        start_column: start_col as i64,
        end_line: Some(end_line as i64),
        end_column: Some(end_col as i64),
    })
}

fn element_fragment(ctx: &Ctx<'_>, element: ElementId) -> Option<FragmentId> {
    if matches!(
        element.tag(),
        Tag::Dynamic | Tag::Never | Tag::MultiplyDefined
    ) {
        return None;
    }
    let element = dartr_element::diagnostics::non_synthetic(ctx, element);
    Some(ctx.element_data(element)?.first_fragment)
}

struct NavigationVisitor<'c, 'a> {
    ctx: &'c Ctx<'a>,
    tables: &'c ResolutionTables,
    requested_offset: Option<u32>,
    requested_length: Option<u32>,
    collector: LegacyNavigationCollector,
}

impl NavigationVisitor<'_, '_> {
    fn within(&self, offset: u32, length: u32) -> bool {
        let Some(req_offset) = self.requested_offset else {
            return true;
        };
        let req_len = self.requested_length.unwrap_or(0);
        if offset > req_offset.saturating_add(req_len) {
            return false;
        }
        if offset.saturating_add(length) < req_offset {
            return false;
        }
        true
    }

    fn add_fragment_range(&mut self, offset: u32, length: u32, fragment: Option<FragmentId>) {
        let Some(fragment) = fragment else { return };
        if !self.within(offset, length) {
            return;
        }
        let Some(kind) = fragment_to_plugin_kind(self.ctx, fragment) else {
            return;
        };
        let Some(location) = fragment_to_location(self.ctx, fragment) else {
            return;
        };
        self.collector.add_region(offset, length, kind, location);
    }

    fn add_token_element(&mut self, ast: &Ast, token: Option<TokenId>, element: Option<ElementId>) {
        let (Some(token), Some(element)) = (token, element) else {
            return;
        };
        let t = ast.tokens.get(token);
        let fragment = element_fragment(self.ctx, element);
        self.add_fragment_range(t.offset, t.end() - t.offset, fragment);
    }

    fn add_node_element(&mut self, ast: &Ast, node: Option<NodeId>, element: Option<ElementId>) {
        let (Some(node), Some(element)) = (node, element) else {
            return;
        };
        let fragment = element_fragment(self.ctx, element);
        self.add_fragment_range(ast.offset(node), ast.length(node), fragment);
    }

    fn add_token_fragment(
        &mut self,
        ast: &Ast,
        token: Option<TokenId>,
        fragment: Option<FragmentId>,
    ) {
        let Some(token) = token else { return };
        let t = ast.tokens.get(token);
        self.add_fragment_range(t.offset, t.end() - t.offset, fragment);
    }

    fn base(&self, e: ElemRef) -> ElementId {
        member::base_element(self.ctx, e)
    }

    fn element(&self, node: impl Into<NodeId>) -> Option<ElementId> {
        self.tables.element.get(node.into()).map(|&e| self.base(e))
    }

    fn declared_fragment(&self, node: impl Into<NodeId>) -> Option<FragmentId> {
        self.tables.declared_fragment.get(node.into()).copied()
    }

    fn declared_element(&self, node: impl Into<NodeId>) -> Option<ElementId> {
        let f = self.declared_fragment(node)?;
        self.ctx.fragment_data(f)?.element.try_get().copied()
    }

    fn visit_unit_sorted(&mut self, ast: &Ast, unit: Id<CompilationUnit>) {
        let mut nodes: Vec<NodeId> = ast
            .list_raw(ast[unit].directives)
            .iter()
            .chain(ast.list_raw(ast[unit].declarations).iter())
            .copied()
            .collect();
        nodes.sort_by_key(|n| ast.offset(*n));
        for n in nodes {
            ast.accept(n, self);
        }
    }

    fn fragment_file_exists(&self, fragment: FragmentId) -> bool {
        let Some(lib_frag_id) = dartr_element::diagnostics::library_fragment_of(self.ctx, fragment)
        else {
            return false;
        };
        let path = &self.ctx.fragment(lib_frag_id).source.path;
        std::path::Path::new(&**path).is_file()
    }

    fn add_uri_directive_region(
        &mut self,
        ast: &Ast,
        uri_node: NodeId,
        uri: Option<&dartr_element::DirectiveUri>,
    ) {
        match uri {
            Some(dartr_element::DirectiveUri::Unit {
                library_fragment, ..
            }) => {
                if self.fragment_file_exists(library_fragment.raw()) {
                    self.add_fragment_range(
                        ast.offset(uri_node),
                        ast.length(uri_node),
                        Some(library_fragment.raw()),
                    );
                }
            }
            Some(dartr_element::DirectiveUri::Library { library, .. }) => {
                let first = self.ctx.get(*library).first_fragment();
                if self.fragment_file_exists(first.raw()) {
                    self.add_node_element(ast, Some(uri_node), Some(library.raw()));
                }
            }
            _ => {}
        }
    }

    fn directive_uri(&self, ast: &Ast, node: NodeId) -> Option<dartr_element::DirectiveUri> {
        let unit = ast.cast::<CompilationUnit>(ast.parent(node)?)?;
        let fragment = self.declared_fragment(unit)?.cast::<LibraryFragment>()?;
        let data = self.ctx.fragment(fragment);
        let directives = ast.list_raw(ast[unit].directives);
        if ast.is::<ImportDirective>(node) {
            let index = directives
                .iter()
                .filter(|d| ast.is::<ImportDirective>(**d))
                .position(|d| *d == node)?;
            data.library_imports
                .iter()
                .filter(|i| !i.is_synthetic)
                .nth(index)
                .map(|i| i.directive.uri.clone())
        } else if ast.is::<ExportDirective>(node) {
            let index = directives
                .iter()
                .filter(|d| ast.is::<ExportDirective>(**d))
                .position(|d| *d == node)?;
            data.library_exports
                .get(index)
                .map(|e| e.directive.uri.clone())
        } else if ast.is::<PartDirective>(node) {
            let index = directives
                .iter()
                .filter(|d| ast.is::<PartDirective>(**d))
                .position(|d| *d == node)?;
            data.parts.get(index).map(|p| p.directive.uri.clone())
        } else {
            None
        }
    }

    fn add_prefix_regions(&mut self, ast: &Ast, name: TokenId, element: Option<ElementId>) {
        let Some(element) = element else { return };
        let t = ast.tokens.get(name);
        for f in self.fragments_of(element) {
            self.add_fragment_range(t.offset, t.end() - t.offset, Some(f));
        }
    }

    fn fragments_of(&self, element: ElementId) -> Vec<FragmentId> {
        let mut result = Vec::new();
        let mut f = self.ctx.element_data(element).map(|d| d.first_fragment);
        while let Some(id) = f {
            result.push(id);
            f = self.ctx.fragment_data(id).and_then(|d| d.next_fragment);
        }
        result
    }

    fn interface_of_variable_type(&self, e: ElementId) -> Option<ElementId> {
        let ty = dartr_resolver::element_ext::variable_type(self.ctx, e);
        match self.ctx.ty(ty) {
            dartr_element::TypeKind::Interface { element, .. } => Some(element.raw()),
            _ => None,
        }
    }
}

impl AstVisitor for NavigationVisitor<'_, '_> {
    fn visit_annotation(&mut self, ast: &Ast, node: Id<Annotation>) {
        let mut element = self.element(node);
        if let Some(e) = element
            && e.tag() == Tag::Constructor
            && dartr_element::diagnostics::non_synthetic(self.ctx, e) != e
        {
            element = self.ctx.element_data(e).and_then(|d| d.enclosing);
        }
        let name = ast[node].name;
        if let Some(p) = ast.cast::<PrefixedIdentifier>(name) {
            let mut prefix_element = self.element(ast[p].prefix);
            if prefix_element.is_some_and(|e| e.tag() == Tag::Class) {
                prefix_element = element;
            }
            self.add_node_element(ast, Some(ast[p].prefix.raw()), prefix_element);
            self.add_node_element(ast, Some(ast[p].identifier.raw()), element);
        } else {
            self.add_node_element(ast, Some(name.raw()), element);
        }
        self.add_node_element(ast, ast[node].constructor_name.map(|c| c.raw()), element);
        if let Some(t) = ast[node].type_arguments {
            ast.accept(t, self);
        }
        if let Some(a) = ast[node].arguments {
            ast.accept(a, self);
        }
    }

    fn visit_assignment_expression(&mut self, ast: &Ast, node: Id<AssignmentExpression>) {
        ast.accept(ast[node].left_hand_side, self);
        let element = self.element(node);
        self.add_token_element(ast, Some(ast[node].operator), element);
        ast.accept(ast[node].right_hand_side, self);
    }

    fn visit_binary_expression(&mut self, ast: &Ast, node: Id<BinaryExpression>) {
        ast.accept(ast[node].left_operand, self);
        let element = self.element(node);
        self.add_token_element(ast, Some(ast[node].operator), element);
        ast.accept(ast[node].right_operand, self);
    }

    fn visit_catch_clause_parameter(&mut self, ast: &Ast, node: Id<CatchClauseParameter>) {
        let fragment = self.declared_fragment(node);
        if fragment.is_none() {
            return;
        }
        self.add_token_fragment(ast, Some(ast[node].name), fragment);
        ast.visit_children(node, self);
    }

    fn visit_class_declaration(&mut self, ast: &Ast, node: Id<ClassDeclaration>) {
        let name = dartr_resolver::error::support::class_name_token(ast, ast[node].name_part);
        let fragment = self.declared_fragment(node);
        self.add_token_fragment(ast, Some(name), fragment);
        ast.visit_children(node, self);
    }

    fn visit_compilation_unit(&mut self, ast: &Ast, node: Id<CompilationUnit>) {
        self.visit_unit_sorted(ast, node);
    }

    fn visit_constructor_declaration(&mut self, ast: &Ast, node: Id<ConstructorDeclaration>) {
        for &m in ast.list_raw(ast[node].metadata) {
            ast.accept(m, self);
        }
        let element = self.declared_element(node);
        match ast[node].name {
            None => {
                let anchor = ast[node].new_keyword.or(ast[node].factory_keyword);
                match anchor {
                    Some(t) => self.add_token_element(ast, Some(t), element),
                    None => {
                        self.add_node_element(ast, ast[node].type_name.map(|t| t.raw()), element)
                    }
                }
            }
            Some(name) => {
                if let Some(t) = ast[node].type_name {
                    ast.accept(t, self);
                }
                self.add_token_element(
                    ast,
                    ast[node].new_keyword.or(ast[node].factory_keyword),
                    element,
                );
                let fragment = self.declared_fragment(node);
                self.add_token_fragment(ast, Some(name), fragment);
            }
        }
        ast.accept(ast[node].parameters, self);
        for &i in ast.list_raw(ast[node].initializers) {
            ast.accept(i, self);
        }
        if let Some(r) = ast[node].redirected_constructor {
            ast.accept(r, self);
        }
        ast.accept(ast[node].body, self);
    }

    fn visit_constructor_name(&mut self, ast: &Ast, node: Id<ConstructorName>) {
        let Some(element) = self.element(node) else {
            return;
        };
        let named_type = ast[node].type_;
        if let Some(prefix) = ast[named_type].import_prefix {
            let prefix_element = self.element(prefix);
            self.add_prefix_regions(ast, ast[prefix].name, prefix_element);
        }
        let class_target = if ast[node].name.is_some() {
            self.element(named_type)
        } else {
            Some(element)
        };
        let class_target =
            class_target.map(|e| dartr_element::diagnostics::non_synthetic(self.ctx, e));
        self.add_token_element(ast, Some(ast[named_type].name), class_target);
        if let Some(t) = ast[named_type].type_arguments {
            ast.accept(t, self);
        }
        if let Some(name) = ast[node].name {
            self.add_node_element(ast, Some(name.raw()), Some(element));
        }
    }

    fn visit_declared_identifier(&mut self, ast: &Ast, node: Id<DeclaredIdentifier>) {
        if ast[node].type_.is_none()
            && let Some(keyword) = ast[node].keyword
            && ast.tokens.lexeme(keyword) == "var"
            && let Some(e) = self.declared_element(node)
            && let Some(element) = self.interface_of_variable_type(e)
        {
            self.add_token_element(ast, Some(keyword), Some(element));
        }
        ast.visit_children(node, self);
    }

    fn visit_declared_variable_pattern(&mut self, ast: &Ast, node: Id<DeclaredVariablePattern>) {
        let element = self.declared_element(node);
        match element.and_then(|e| element_ext::pattern_variable_join(self.ctx, e)) {
            Some(join) => {
                for v in element_ext::join_pattern_variable_components(self.ctx, join) {
                    self.add_token_element(ast, Some(ast[node].name), Some(v));
                }
            }
            None => self.add_token_element(ast, Some(ast[node].name), element),
        }
        ast.visit_children(node, self);
    }

    fn visit_enum_constant_declaration(&mut self, ast: &Ast, node: Id<EnumConstantDeclaration>) {
        let constructor = self.element(node);
        self.add_token_element(ast, Some(ast[node].name), constructor);
        if let Some(arguments) = ast[node].arguments {
            if let Some(selector) = ast[arguments].constructor_selector {
                let name = ast[selector].name;
                self.add_node_element(ast, Some(name.raw()), constructor);
            }
            if let Some(t) = ast[arguments].type_arguments {
                ast.accept(t, self);
            }
            ast.accept(ast[arguments].argument_list, self);
        }
    }

    fn visit_enum_declaration(&mut self, ast: &Ast, node: Id<EnumDeclaration>) {
        let name = dartr_resolver::error::support::class_name_token(ast, ast[node].name_part);
        let fragment = self.declared_fragment(node);
        self.add_token_fragment(ast, Some(name), fragment);
        ast.visit_children(node, self);
    }

    fn visit_export_directive(&mut self, ast: &Ast, node: Id<ExportDirective>) {
        let uri = self.directive_uri(ast, node.raw());
        self.add_uri_directive_region(ast, ast[node].uri.raw(), uri.as_ref());
        ast.visit_children(node, self);
    }

    fn visit_extension_declaration(&mut self, ast: &Ast, node: Id<ExtensionDeclaration>) {
        let fragment = self.declared_fragment(node);
        self.add_token_fragment(ast, ast[node].name, fragment);
        ast.visit_children(node, self);
    }

    fn visit_extension_override(&mut self, ast: &Ast, node: Id<ExtensionOverride>) {
        let element = self.element(node);
        self.add_token_element(ast, Some(ast[node].name), element);
        ast.visit_children(node, self);
    }

    fn visit_extension_type_declaration(&mut self, ast: &Ast, node: Id<ExtensionTypeDeclaration>) {
        let name = dartr_resolver::error::support::class_name_token(ast, ast[node].name_part);
        let fragment = self.declared_fragment(node);
        self.add_token_fragment(ast, Some(name), fragment);
        ast.visit_children(node, self);
    }

    fn visit_field_formal_parameter(&mut self, ast: &Ast, node: Id<FieldFormalParameter>) {
        if let Some(e) = self.declared_element(node)
            && e.tag() == Tag::FieldFormalParameter
        {
            let field = match self.ctx.any(e) {
                dartr_element::AnyElement::FormalParameter(p) => p.field.get().map(|f| f.raw()),
                _ => None,
            };
            self.add_token_element(ast, Some(ast[node].this_keyword), field);
            self.add_token_element(ast, Some(ast[node].name), field);
        }
        if let Some(t) = ast[node].type_ {
            ast.accept(t, self);
        }
        if let Some(s) = ast[node].function_typed_suffix {
            ast.accept(s, self);
        }
        if let Some(d) = ast[node].default_clause {
            ast.accept(d, self);
        }
    }

    fn visit_function_declaration(&mut self, ast: &Ast, node: Id<FunctionDeclaration>) {
        let fragment = self.declared_fragment(node);
        self.add_token_fragment(ast, Some(ast[node].name), fragment);
        ast.visit_children(node, self);
    }

    fn visit_function_type_alias(&mut self, ast: &Ast, node: Id<FunctionTypeAlias>) {
        let fragment = self.declared_fragment(node);
        self.add_token_fragment(ast, Some(ast[node].name), fragment);
        ast.visit_children(node, self);
    }

    fn visit_generic_type_alias(&mut self, ast: &Ast, node: Id<GenericTypeAlias>) {
        let fragment = self.declared_fragment(node);
        self.add_token_fragment(ast, Some(ast[node].name), fragment);
        ast.visit_children(node, self);
    }

    fn visit_import_directive(&mut self, ast: &Ast, node: Id<ImportDirective>) {
        let uri = self.directive_uri(ast, node.raw());
        self.add_uri_directive_region(ast, ast[node].uri.raw(), uri.as_ref());
        ast.visit_children(node, self);
    }

    fn visit_import_prefix_reference(&mut self, ast: &Ast, node: Id<ImportPrefixReference>) {
        let element = self.element(node);
        self.add_prefix_regions(ast, ast[node].name, element);
    }

    fn visit_index_expression(&mut self, ast: &Ast, node: Id<IndexExpression>) {
        ast.visit_children(node, self);
        let element = self
            .tables
            .write_element
            .get(node.raw())
            .or_else(|| self.tables.read_element.get(node.raw()))
            .or_else(|| self.tables.element.get(node.raw()))
            .map(|&e| self.base(e));
        self.add_token_element(ast, Some(ast[node].left_bracket), element);
        self.add_token_element(ast, Some(ast[node].right_bracket), element);
    }

    fn visit_instance_creation_expression(
        &mut self,
        ast: &Ast,
        node: Id<InstanceCreationExpression>,
    ) {
        let constructor_name = ast[node].constructor_name;
        if self.element(constructor_name).is_none() {
            let class = self.element(ast[constructor_name].type_);
            self.add_node_element(ast, Some(constructor_name.raw()), class);
        }
        ast.visit_children(node, self);
    }

    fn visit_label_reference(&mut self, ast: &Ast, node: Id<LabelReference>) {
        let element = self.element(node);
        self.add_token_element(ast, Some(ast[node].name), element);
        ast.visit_children(node, self);
    }

    fn visit_library_directive(&mut self, ast: &Ast, node: Id<LibraryDirective>) {
        let element = self.element(node).or_else(|| {
            let unit = ast.parent(node)?;
            let frag = self.declared_fragment(unit)?.cast::<LibraryFragment>()?;
            Some(self.ctx.fragment(frag).library.raw())
        });
        self.add_node_element(ast, ast[node].name.map(|n| n.raw()), element);
        ast.visit_children(node, self);
    }

    fn visit_method_declaration(&mut self, ast: &Ast, node: Id<MethodDeclaration>) {
        let fragment = self.declared_fragment(node);
        self.add_token_fragment(ast, Some(ast[node].name), fragment);
        ast.visit_children(node, self);
    }

    fn visit_mixin_declaration(&mut self, ast: &Ast, node: Id<MixinDeclaration>) {
        let fragment = self.declared_fragment(node);
        self.add_token_fragment(ast, Some(ast[node].name), fragment);
        ast.visit_children(node, self);
    }

    fn visit_named_argument(&mut self, ast: &Ast, node: Id<NamedArgument>) {
        let parameter = corresponding_parameter(self.ctx, ast, self.tables, node.raw());
        self.add_token_element(ast, Some(ast[node].name), parameter);
        ast.accept(ast[node].argument_expression, self);
    }

    fn visit_named_type(&mut self, ast: &Ast, node: Id<NamedType>) {
        if let Some(prefix) = ast[node].import_prefix {
            ast.accept(prefix, self);
        }
        let element = self.element(node);
        self.add_token_element(ast, Some(ast[node].name), element);
        if let Some(t) = ast[node].type_arguments {
            ast.accept(t, self);
        }
    }

    fn visit_name_with_type_parameters(&mut self, ast: &Ast, node: Id<NameWithTypeParameters>) {
        if let Some(parent) = ast.parent(node)
            && (ast.is::<ClassDeclaration>(parent) || ast.is::<EnumDeclaration>(parent))
        {
            let fragment = self.declared_fragment(parent);
            self.add_token_fragment(ast, Some(ast[node].type_name), fragment);
        }
        ast.visit_children(node, self);
    }

    fn visit_part_directive(&mut self, ast: &Ast, node: Id<PartDirective>) {
        let uri = self.directive_uri(ast, node.raw());
        let uri_node = ast[node].uri;
        match uri {
            Some(dartr_element::DirectiveUri::Unit {
                library_fragment, ..
            }) => {
                self.add_fragment_range(
                    ast.offset(uri_node),
                    ast.length(uri_node),
                    Some(library_fragment.raw()),
                );
            }
            Some(dartr_element::DirectiveUri::Source { source, .. }) => {
                let (o, l) = (ast.offset(uri_node), ast.length(uri_node));
                if self.within(o, l) {
                    self.collector.add_region(
                        o,
                        l,
                        ElementKind::FILE,
                        Location {
                            file: source.path.to_string(),
                            offset: 0,
                            length: 0,
                            start_line: 0,
                            start_column: 0,
                            end_line: Some(0),
                            end_column: Some(0),
                        },
                    );
                }
            }
            _ => {}
        }
        ast.visit_children(node, self);
    }

    fn visit_part_of_directive(&mut self, ast: &Ast, node: Id<PartOfDirective>) {
        let unit = ast.parent(node);
        let fragment = unit
            .and_then(|u| self.declared_fragment(u))
            .and_then(|f| f.cast::<LibraryFragment>())
            .and_then(|lf| crate::convert::enclosing_library_fragment(self.ctx, lf))
            .map(|lf| lf.raw());
        let anchor = ast[node]
            .library_name
            .map(|n| n.raw())
            .or(ast[node].uri.map(|u| u.raw()));
        if let Some(anchor) = anchor {
            self.add_fragment_range(ast.offset(anchor), ast.length(anchor), fragment);
        }
        ast.visit_children(node, self);
    }

    fn visit_pattern_field(&mut self, ast: &Ast, node: Id<PatternField>) {
        if let Some(name_node) = ast[node].name {
            let name_token = ast[name_node].name.or_else(|| {
                let pat = ast[node].pattern;
                if let Some(vp) = ast.cast::<DeclaredVariablePattern>(pat) {
                    Some(ast[vp].name)
                } else {
                    ast.cast::<AssignedVariablePattern>(pat)
                        .map(|vp| ast[vp].name)
                }
            });
            let element = self.element(node);
            self.add_token_element(ast, name_token, element);
        }
        ast.accept(ast[node].pattern, self);
    }

    fn visit_postfix_expression(&mut self, ast: &Ast, node: Id<PostfixExpression>) {
        ast.visit_children(node, self);
        let element = self.element(node);
        self.add_token_element(ast, Some(ast[node].operator), element);
    }

    fn visit_prefix_expression(&mut self, ast: &Ast, node: Id<PrefixExpression>) {
        let element = self.element(node);
        self.add_token_element(ast, Some(ast[node].operator), element);
        ast.visit_children(node, self);
    }

    fn visit_primary_constructor_declaration(
        &mut self,
        ast: &Ast,
        node: Id<PrimaryConstructorDeclaration>,
    ) {
        if let Some(parent) = ast.parent(node) {
            let fragment = self.declared_fragment(parent);
            self.add_token_fragment(ast, Some(ast[node].type_name), fragment);
        }
        ast.visit_children(node, self);
    }

    fn visit_primary_constructor_name(&mut self, ast: &Ast, node: Id<PrimaryConstructorName>) {
        if let Some(parent) = ast.parent(node)
            && ast.is::<PrimaryConstructorDeclaration>(parent)
        {
            let fragment = self.declared_fragment(parent);
            self.add_token_fragment(ast, Some(ast[node].name), fragment);
        }
        ast.visit_children(node, self);
    }

    fn visit_redirecting_constructor_invocation(
        &mut self,
        ast: &Ast,
        node: Id<RedirectingConstructorInvocation>,
    ) {
        let element = self
            .element(node)
            .map(|e| dartr_element::diagnostics::non_synthetic(self.ctx, e));
        self.add_token_element(ast, Some(ast[node].this_keyword), element);
        self.add_node_element(ast, ast[node].constructor_name.map(|c| c.raw()), element);
        ast.accept(ast[node].argument_list, self);
    }

    fn visit_regular_formal_parameter(&mut self, ast: &Ast, node: Id<RegularFormalParameter>) {
        if let Some(name) = ast[node].name {
            let fragment = self.declared_fragment(node);
            self.add_token_fragment(ast, Some(name), fragment);
        }
        ast.visit_children(node, self);
    }

    fn visit_simple_identifier(&mut self, ast: &Ast, node: Id<SimpleIdentifier>) {
        let element =
            dartr_resolver::error::support::write_or_read_element(self.ctx, ast, self.tables, node)
                .or_else(|| {
                    self.tables
                        .read_element
                        .get(node.raw())
                        .map(|&e| self.base(e))
                });
        let Some(element) = element else { return };
        match element.tag() {
            Tag::Prefix => {
                let length = self
                    .ctx
                    .element_data(element)
                    .and_then(|d| d.name)
                    .map(|n| self.ctx.name_str(n).encode_utf16().count() as u32);
                let fragments = self.fragments_of(element);
                for f in fragments {
                    if let Some(length) = length {
                        self.add_fragment_range(ast.offset(node), length, Some(f));
                    }
                }
            }
            Tag::JoinPatternVariable => {
                for v in element_ext::join_pattern_variable_components(self.ctx, element) {
                    self.add_node_element(ast, Some(node.raw()), Some(v));
                }
            }
            _ => self.add_node_element(ast, Some(node.raw()), Some(element)),
        }
    }

    fn visit_super_constructor_invocation(
        &mut self,
        ast: &Ast,
        node: Id<SuperConstructorInvocation>,
    ) {
        let element = self
            .element(node)
            .map(|e| dartr_element::diagnostics::non_synthetic(self.ctx, e));
        self.add_token_element(ast, Some(ast[node].super_keyword), element);
        self.add_node_element(ast, ast[node].constructor_name.map(|c| c.raw()), element);
        ast.accept(ast[node].argument_list, self);
    }

    fn visit_super_formal_parameter(&mut self, ast: &Ast, node: Id<SuperFormalParameter>) {
        if let Some(e) = self.declared_element(node) {
            let super_parameter = (e.tag() == Tag::SuperFormalParameter)
                .then(|| {
                    dartr_resolver::constant::evaluation::super_constructor_parameter(self.ctx, e)
                })
                .flatten()
                .map(|p| self.base(p));
            self.add_token_element(ast, Some(ast[node].super_keyword), super_parameter);
            self.add_token_element(ast, Some(ast[node].name), super_parameter);
        }
        if let Some(t) = ast[node].type_ {
            ast.accept(t, self);
        }
        if let Some(s) = ast[node].function_typed_suffix {
            ast.accept(s, self);
        }
        if let Some(d) = ast[node].default_clause {
            ast.accept(d, self);
        }
    }

    fn visit_type_parameter(&mut self, ast: &Ast, node: Id<TypeParameter>) {
        let fragment = self.declared_fragment(node);
        self.add_token_fragment(ast, Some(ast[node].name), fragment);
        ast.visit_children(node, self);
    }

    fn visit_variable_declaration(&mut self, ast: &Ast, node: Id<VariableDeclaration>) {
        let fragment = self.declared_fragment(node);
        self.add_token_fragment(ast, Some(ast[node].name), fragment);
        ast.visit_children(node, self);
    }

    fn visit_variable_declaration_list(&mut self, ast: &Ast, node: Id<VariableDeclarationList>) {
        if ast[node].type_.is_none()
            && let Some(keyword) = ast[node].keyword
            && ast.tokens.lexeme(keyword) == "var"
        {
            let variables = ast.list(ast[node].variables).to_vec();
            let mut common: Option<ElementId> = None;
            let mut ok = !variables.is_empty();
            for (i, v) in variables.iter().enumerate() {
                let element = self
                    .declared_element(*v)
                    .and_then(|e| self.interface_of_variable_type(e));
                match (i, element) {
                    (_, None) => {
                        ok = false;
                        break;
                    }
                    (0, Some(e)) => common = Some(e),
                    (_, Some(e)) => {
                        if common != Some(e) {
                            ok = false;
                            break;
                        }
                    }
                }
            }
            if ok && let Some(e) = common {
                self.add_token_element(ast, Some(keyword), Some(e));
            }
        }
        ast.visit_children(node, self);
    }
}
