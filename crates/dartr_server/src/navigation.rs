// Dart source: pkg/analyzer_plugin/lib/src/utilities/navigation/navigation_dart.dart
// Dart source: pkg/analyzer_plugin/lib/src/utilities/navigation/navigation.dart
// (NavigationCollectorImpl)

//! The navigation regions of a resolved unit at an offset (Dart
//! `computeDartNavigation` with `offset` and `length: 0`), for
//! `textDocument/definition`.

use dartr_ast::*;
use dartr_element::{Ctx, ElemRef, ElementId, FragmentId, ResolutionTables, Tag};
use dartr_resolver::element_ext;
use dartr_resolver::error::support::corresponding_parameter;
use dartr_syntax::TokenId;
use dartr_typesystem::member;

/// Dart `NavigationTarget` (with the fragment, for the code location).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    /// The element kind (part of the identity of a target).
    pub kind: &'static str,
    pub file: String,
    pub offset: u32,
    pub length: u32,
    pub fragment: Option<FragmentId>,
}

/// Dart `NavigationCollectorImpl`: regions and their targets.
#[derive(Default, Debug)]
pub struct Collector {
    /// The unique targets, in the order they were added.
    pub targets: Vec<Target>,
    /// Region ranges (offset, length) with their target indices, in the
    /// order they were added.
    pub regions: Vec<((u32, u32), Vec<usize>)>,
}

impl Collector {
    pub fn add_region(&mut self, offset: u32, length: u32, target: Target) {
        let index = match self.targets.iter().position(|t| {
            t.kind == target.kind
                && t.file == target.file
                && t.offset == target.offset
                && t.length == target.length
        }) {
            Some(i) => i,
            None => {
                self.targets.push(target);
                self.targets.len() - 1
            }
        };
        match self
            .regions
            .iter_mut()
            .find(|(r, _)| *r == (offset, length))
        {
            Some((_, list)) => list.push(index),
            None => self.regions.push(((offset, length), vec![index])),
        }
    }

    /// Dart `createRegions`: the regions sorted by offset (stable).
    pub fn sorted_regions(&self) -> Vec<((u32, u32), Vec<usize>)> {
        let mut regions = self.regions.clone();
        regions.sort_by_key(|((offset, _), _)| *offset);
        regions
    }
}

/// The library fragment that contains [fragment] (Dart
/// `Fragment.libraryFragment`).
pub fn library_fragment_of(ctx: &Ctx<'_>, mut fragment: FragmentId) -> Option<FragmentId> {
    loop {
        if fragment.tag() == Tag::Library {
            return Some(fragment);
        }
        fragment = ctx.fragment_data(fragment)?.enclosing_fragment?;
    }
}

/// The path of the file of [fragment].
pub fn fragment_path(ctx: &Ctx<'_>, fragment: FragmentId) -> Option<String> {
    let library = library_fragment_of(ctx, fragment)?;
    let library = library.cast::<dartr_element::LibraryFragment>()?;
    Some(ctx.fragment(library).source.path.to_string())
}

fn utf16_len(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}

/// Dart `Fragment.toLocation()` (the offset and length of the name).
pub fn fragment_target(ctx: &Ctx<'_>, fragment: FragmentId) -> Option<Target> {
    let element = ctx.fragment_data(fragment)?.element.try_get().copied();
    if let Some(e) = element
        && matches!(e.tag(), Tag::Dynamic | Tag::MultiplyDefined)
    {
        return None;
    }
    let file = fragment_path(ctx, fragment)?;
    let data = ctx.fragment_data(fragment)?;
    // `LabelFragmentImpl.nameOffset` is `firstTokenOffset`.
    let mut name_offset = data.name_offset.or(if fragment.tag() == Tag::Label {
        data.first_token_offset
    } else {
        None
    });
    let mut name_length = data.name.map(|n| utf16_len(ctx.name_str(n)));
    if name_offset.is_none()
        && let Some(c) = fragment.cast::<dartr_element::ConstructorFragment>()
    {
        let c = ctx.fragment(c);
        if let Some(type_name) = c.type_name {
            name_offset = c.type_name_offset;
            name_length = Some(utf16_len(ctx.name_str(type_name)));
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
    Some(Target {
        kind: element.map(|e| e.tag().name()).unwrap_or("UNKNOWN"),
        file,
        offset,
        length,
        fragment: Some(fragment),
    })
}

/// The first fragment of `element.nonSynthetic` (Dart
/// `_addRegionForElement`).
pub fn element_fragment(ctx: &Ctx<'_>, element: ElementId) -> Option<FragmentId> {
    if matches!(
        element.tag(),
        Tag::Dynamic | Tag::Never | Tag::MultiplyDefined
    ) {
        return None;
    }
    let element = dartr_element::diagnostics::non_synthetic(ctx, element);
    Some(ctx.element_data(element)?.first_fragment)
}

/// Dart `computeDartNavigation(collector, unit, offset, 0)`.
pub fn compute(
    ctx: &Ctx<'_>,
    ast: &Ast,
    tables: &ResolutionTables,
    unit: Id<CompilationUnit>,
    offset: u32,
) -> Collector {
    let mut visitor = NavigationVisitor {
        ctx,
        tables,
        offset,
        collector: Collector::default(),
    };
    // Dart `_getNodeForRange`: the node covering the offset, or its
    // directive.
    let Some(mut node) = ast.node_covering(unit, offset, 0) else {
        return visitor.collector;
    };
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
    visitor.collector
}

/// Dart `_getNavigationTargetNode`.
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

struct NavigationVisitor<'c, 'a> {
    ctx: &'c Ctx<'a>,
    tables: &'c ResolutionTables,
    offset: u32,
    collector: Collector,
}

impl NavigationVisitor<'_, '_> {
    /// Dart `_isWithinRequestedRange(offset, length)`.
    fn within(&self, offset: u32, length: u32) -> bool {
        !(offset > self.offset || offset + length < self.offset)
    }

    fn add_fragment_range(&mut self, offset: u32, length: u32, fragment: Option<FragmentId>) {
        let Some(fragment) = fragment else { return };
        if !self.within(offset, length) {
            return;
        }
        let Some(target) = fragment_target(self.ctx, fragment) else {
            return;
        };
        self.collector.add_region(offset, length, target);
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

    /// Dart `visitCompilationUnit`: directives and declarations by offset.
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

    /// Dart `_addUriDirectiveRegion`.
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
                if let Some(path) = fragment_path(self.ctx, library_fragment.raw())
                    && std::path::Path::new(&path).is_file()
                {
                    self.add_fragment_range(
                        ast.offset(uri_node),
                        ast.length(uri_node),
                        Some(library_fragment.raw()),
                    );
                }
            }
            Some(dartr_element::DirectiveUri::Library { library, .. }) => {
                let first = self.ctx.get(*library).first_fragment();
                if let Some(path) = fragment_path(self.ctx, first.raw())
                    && std::path::Path::new(&path).is_file()
                {
                    self.add_node_element(ast, Some(uri_node), Some(library.raw()));
                }
            }
            _ => {}
        }
    }

    /// The directive URI element of the import or export [node] (the n-th
    /// directive of its kind).
    fn directive_uri(&self, ast: &Ast, node: NodeId) -> Option<dartr_element::DirectiveUri> {
        let unit = ast.cast::<CompilationUnit>(ast.parent(node)?)?;
        let fragment = self
            .declared_fragment(unit)?
            .cast::<dartr_element::LibraryFragment>()?;
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
        let fragment = self.declared_fragment(node);
        // Dart `super.visitDeclaredIdentifier`: the name is not visited (a
        // token), the type is.
        let _ = fragment;
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
        let element = self.element(node);
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
        // The declaration adds the name region (Dart adds it twice; the
        // collector keeps one target per region and target).
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
                        Target {
                            kind: "FILE",
                            file: source.path.to_string(),
                            offset: 0,
                            length: 0,
                            fragment: None,
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
            .and_then(|f| self.ctx.fragment_data(f))
            .and_then(|f| f.enclosing_fragment);
        let anchor = ast[node]
            .library_name
            .map(|n| n.raw())
            .or(ast[node].uri.map(|u| u.raw()));
        if let Some(anchor) = anchor {
            self.add_fragment_range(ast.offset(anchor), ast.length(anchor), fragment);
        }
        ast.visit_children(node, self);
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
                    .map(|n| utf16_len(self.ctx.name_str(n)));
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

impl NavigationVisitor<'_, '_> {
    /// The regions of an import prefix reference (all fragments of the
    /// prefix).
    fn add_prefix_regions(&mut self, ast: &Ast, name: TokenId, element: Option<ElementId>) {
        let Some(element) = element else { return };
        let t = ast.tokens.get(name);
        for f in self.fragments_of(element) {
            self.add_fragment_range(t.offset, t.end() - t.offset, Some(f));
        }
    }

    /// Dart `element.fragments`.
    fn fragments_of(&self, element: ElementId) -> Vec<FragmentId> {
        let mut result = Vec::new();
        let mut f = self.ctx.element_data(element).map(|d| d.first_fragment);
        while let Some(id) = f {
            result.push(id);
            f = self.ctx.fragment_data(id).and_then(|d| d.next_fragment);
        }
        result
    }

    /// The interface element of the type of the variable [e], if the type
    /// is an interface type.
    fn interface_of_variable_type(&self, e: ElementId) -> Option<ElementId> {
        let ty = dartr_resolver::element_ext::variable_type(self.ctx, e);
        match self.ctx.ty(ty) {
            dartr_element::TypeKind::Interface { element, .. } => Some(element.raw()),
            _ => None,
        }
    }
}
