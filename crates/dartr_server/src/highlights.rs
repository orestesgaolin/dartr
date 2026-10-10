// Dart source: pkg/analysis_server/lib/src/computer/computer_document_highlights.dart
// Dart source: pkg/analysis_server/lib/src/utilities/extensions/element.dart (canonical)

//! Dart `DartDocumentHighlightsComputer`: the tokens of a unit that refer
//! to the element (or loop, function body) at an offset.

use dartr_ast::*;
use dartr_element::{ElemRef, ElementId, FragmentFlags, Tag};
use dartr_syntax::TokenId;
use dartr_typesystem::inheritance_manager3::{InheritanceManager3, Name};
use dartr_typesystem::member;

use crate::element_locator::Unit;

/// LSP `DocumentHighlightKind`.
pub const TEXT: u32 = 1;
pub const READ: u32 = 2;
pub const WRITE: u32 = 3;

/// Dart `Element.canonical` (analysis_server `utilities/extensions/element.dart`).
pub fn canonical(unit: &Unit<'_, '_>, element: ElementId) -> Option<ElementId> {
    let ctx = unit.ctx;
    match element.tag() {
        Tag::FieldFormalParameter => {
            let field = match ctx.any(element) {
                dartr_element::AnyElement::FormalParameter(p) => p.field.get().map(|f| f.raw()),
                _ => None,
            };
            if let Some(f) = field
                && ctx.element_data(f).and_then(|d| d.name)
                    == ctx.element_data(element).and_then(|d| d.name)
            {
                return Some(f);
            }
            Some(element)
        }
        Tag::Getter | Tag::Setter => {
            let flags = dartr_resolver::element_ext::first_fragment_flags(ctx, element);
            let variable = dartr_resolver::element_metadata::accessor_variable_any(ctx, element);
            if let Some(v) = variable {
                let declaring = v.tag() == Tag::Field
                    && dartr_resolver::element_ext::first_fragment_flags(ctx, v).contains(
                        FragmentFlags::FIELD_FRAGMENT_IS_ORIGIN_DECLARING_FORMAL_PARAMETER,
                    );
                if flags.contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE)
                    || declaring
                {
                    return Some(v);
                }
            }
            Some(element)
        }
        _ => Some(element),
    }
}

/// The highlight targets (Dart `_HighlightTargets`).
enum Targets {
    Elements(Vec<ElementId>, Vec<String>),
    Node(NodeId),
}

/// Dart `DartDocumentHighlightsComputer.compute(offset)`: the tokens and
/// their kinds, in visit order without duplicates.
pub fn compute(unit: &Unit<'_, '_>, root: Id<CompilationUnit>, offset: u32) -> Vec<(TokenId, u32)> {
    let ast = unit.ast;
    let covering = ast.node_covering(root, offset, 0);
    let Some(covering) = adjust_node(ast, offset, covering) else {
        return Vec::new();
    };
    let Some(targets) = compute_targets(unit, covering) else {
        return Vec::new();
    };
    let mut visitor = Visitor {
        unit,
        targets,
        tokens: Vec::new(),
        function_stack: Vec::new(),
    };
    ast.accept(root, &mut visitor);
    visitor.tokens
}

/// Dart `_adjustNode`.
fn adjust_node(ast: &Ast, offset: u32, node: Option<NodeId>) -> Option<NodeId> {
    let node = node?;
    if ast.is::<FormalParameterList>(node)
        && offset == ast.offset(node)
        && let Some(p) = ast.parent(node)
    {
        return Some(p);
    }
    if let Some(c) = ast.cast::<ConstructorDeclaration>(node)
        && (ast[c].type_name.is_some() || ast[c].name.is_some())
        && let Some(keyword) = ast[c].new_keyword.or(ast[c].factory_keyword)
    {
        let t = ast.tokens.get(keyword);
        if offset >= t.offset && offset <= t.end() {
            return None;
        }
    }
    Some(node)
}

fn supertype_members(unit: &Unit<'_, '_>, element: ElementId) -> Vec<ElementId> {
    let ctx = unit.ctx;
    let Some(enclosing) = ctx
        .element_data(element)
        .and_then(|d| d.enclosing)
        .and_then(|e| e.cast::<dartr_element::InterfaceElement>())
    else {
        return Vec::new();
    };
    let Some(name) = Name::for_element(ctx, ElemRef::Base(element)) else {
        return Vec::new();
    };
    let global = ctx.global();
    let manager = InheritanceManager3::new(global);
    let mut out = Vec::new();
    for &supertype in dartr_typesystem::class_hierarchy::implemented_interfaces(&global, enclosing)
    {
        let dartr_element::TypeKind::Interface { element: e, .. } = global.ty(supertype) else {
            continue;
        };
        if let Some(m) = manager.get_member(*e, name)
            && let Some(c) = canonical(unit, member::base_element(ctx, m))
        {
            out.push(c);
        }
    }
    out
}

fn element_name(unit: &Unit<'_, '_>, element: ElementId) -> Option<String> {
    let ctx = unit.ctx;
    ctx.element_data(element)
        .and_then(|d| d.name)
        .map(|n| ctx.name_str(n).to_string())
}

/// Dart `_computeTargets`.
fn compute_targets(unit: &Unit<'_, '_>, node: NodeId) -> Option<Targets> {
    if let Some(target) = target_node(unit, node) {
        return Some(Targets::Node(target));
    }
    let ast = unit.ast;
    let main = target_element(unit, node).and_then(|e| canonical(unit, e));
    let mut additional = None;
    if ast.is::<FormalParameter>(node)
        && let Some(e) = unit.declared_element(node)
        && e.tag() == Tag::FieldFormalParameter
    {
        additional = match unit.ctx.any(e) {
            dartr_element::AnyElement::FormalParameter(p) => p.field.get().map(|f| f.raw()),
            _ => None,
        };
    } else if (ast.is::<DeclaredVariablePattern>(node) || ast.is::<AssignedVariablePattern>(node))
        && let Some(p) = ast.parent(node)
        && let Some(field) = ast.cast::<PatternField>(p)
    {
        let explicit_name = ast[field].name.and_then(|n| ast[n].name);
        if explicit_name.is_none() {
            additional = unit.element(field).and_then(|e| canonical(unit, e));
        }
    }
    let mut all: Vec<ElementId> = Vec::new();
    let push = |e: ElementId, all: &mut Vec<ElementId>| {
        if !all.contains(&e) {
            all.push(e);
        }
    };
    if let Some(m) = main {
        push(m, &mut all);
    }
    if let Some(a) = additional {
        push(a, &mut all);
    }
    if let Some(m) = main {
        for s in supertype_members(unit, m) {
            push(s, &mut all);
        }
    }
    if let Some(a) = additional {
        for s in supertype_members(unit, a) {
            push(s, &mut all);
        }
    }
    let names = all.iter().filter_map(|e| element_name(unit, *e)).collect();
    Some(Targets::Elements(all, names))
}

/// Dart `_getTargetNode`.
fn target_node(unit: &Unit<'_, '_>, node: NodeId) -> Option<NodeId> {
    let ast = unit.ast;
    match ast.kind(node) {
        NodeKind::ForStatement
        | NodeKind::WhileStatement
        | NodeKind::DoStatement
        | NodeKind::SwitchStatement => Some(node),
        NodeKind::BreakStatement | NodeKind::ContinueStatement => break_target(unit, node),
        NodeKind::ReturnStatement | NodeKind::YieldStatement => {
            let mut current = ast.parent(node);
            while let Some(c) = current {
                if ast.is::<FunctionBody>(c) {
                    return Some(c);
                }
                current = ast.parent(c);
            }
            None
        }
        _ => None,
    }
}

/// Dart `BreakStatement.target` / `ContinueStatement.target`.
fn break_target(unit: &Unit<'_, '_>, node: NodeId) -> Option<NodeId> {
    let ast = unit.ast;
    let label = ast
        .cast::<BreakStatement>(node)
        .and_then(|b| ast[b].label)
        .or_else(|| {
            ast.cast::<ContinueStatement>(node)
                .and_then(|c| ast[c].label)
        });
    let element = label.and_then(|l| unit.element(l));
    if label.is_some() && element.is_none() {
        return None;
    }
    dartr_resolver::flow_analysis_visitor::get_label_target(
        ast,
        unit.tables,
        unit.ctx,
        node,
        element,
        ast.is::<BreakStatement>(node),
    )
    .map(|s| s.raw())
}

/// Dart `_getTargetElement`.
fn target_element(unit: &Unit<'_, '_>, node: NodeId) -> Option<ElementId> {
    let ast = unit.ast;
    if ast.is::<PrimaryConstructorBody>(node) {
        return None;
    }
    if ast.is::<NamedType>(node)
        && let Some(p) = ast.parent(node)
        && let Some(c) = ast.cast::<ConstructorName>(p)
        && ast[c].name.is_none()
        && let Some(e) = unit.element(c)
    {
        return Some(e);
    }
    if ast.is::<Identifier>(node)
        && let Some(p) = ast.parent(node)
        && let Some(c) = ast.cast::<ConstructorDeclaration>(p)
        && ast[c].name.is_some()
        && ast[c].type_name.map(|t| t.raw()) == Some(node)
    {
        return unit
            .declared_element(c)
            .and_then(|e| unit.ctx.element_data(e))
            .and_then(|d| d.enclosing);
    }
    if ast.is::<DeclaredVariablePattern>(node)
        && let Some(e) = unit.declared_element(node)
        && let Some(join) = dartr_resolver::element_ext::pattern_variable_join(unit.ctx, e)
    {
        return Some(join);
    }
    unit.locate(node)
}

struct Visitor<'u, 'c, 'a> {
    unit: &'u Unit<'c, 'a>,
    targets: Targets,
    tokens: Vec<(TokenId, u32)>,
    function_stack: Vec<NodeId>,
}

impl Visitor<'_, '_, '_> {
    fn add_node_occurrence(&mut self, node: Option<NodeId>, token: TokenId, kind: u32) {
        if let (Some(n), Targets::Node(target)) = (node, &self.targets)
            && n == *target
            && !self.tokens.contains(&(token, kind))
        {
            self.tokens.push((token, kind));
        }
    }

    fn add_occurrence(&mut self, element: Option<ElementId>, token: Option<TokenId>, kind: u32) {
        let (Some(element), Some(token)) = (element, token) else {
            return;
        };
        let Some(canonical) = canonical(self.unit, element) else {
            return;
        };
        let Targets::Elements(elements, names) = &self.targets else {
            return;
        };
        let Some(name) = element_name(self.unit, canonical) else {
            return;
        };
        if !names.contains(&name) {
            return;
        }
        let matches = elements.contains(&canonical)
            || supertype_members(self.unit, canonical)
                .iter()
                .any(|e| elements.contains(e));
        if matches && !self.tokens.contains(&(token, kind)) {
            self.tokens.push((token, kind));
        }
    }

    fn declared(&self, node: impl Into<NodeId>) -> Option<ElementId> {
        self.unit.declared_element(node)
    }
}

impl AstVisitor for Visitor<'_, '_, '_> {
    fn visit_node(&mut self, ast: &Ast, node: NodeId) {
        // Dart `GeneralizingAstVisitor`: the abstract kinds.
        if ast.is::<FormalParameter>(node) {
            let element = self.declared(node);
            let name = dartr_resolver::ast_ext::formal_parameter_parts(ast, node).name;
            if let Some(e) = element
                && e.tag() == Tag::FieldFormalParameter
            {
                let field = match self.unit.ctx.any(e) {
                    dartr_element::AnyElement::FormalParameter(p) => p.field.get().map(|f| f.raw()),
                    _ => None,
                };
                if field.is_some() {
                    self.add_occurrence(Some(e), name, WRITE);
                    self.add_occurrence(field, name, WRITE);
                }
            } else {
                self.add_occurrence(element, name, WRITE);
            }
        }
        if ast.is::<FunctionBody>(node) {
            self.function_stack.push(node);
            ast.visit_children(node, self);
            self.function_stack.pop();
            return;
        }
        if let Some(t) = ast.cast::<GenericTypeAlias>(node) {
            let e = self.declared(node);
            self.add_occurrence(e, Some(ast[t].name), WRITE);
        } else if let Some(t) = ast.cast::<FunctionTypeAlias>(node) {
            let e = self.declared(node);
            self.add_occurrence(e, Some(ast[t].name), WRITE);
        } else if let Some(t) = ast.cast::<ClassTypeAlias>(node) {
            let e = self.declared(node);
            self.add_occurrence(e, Some(ast[t].name), WRITE);
        }
        ast.visit_children(node, self);
    }

    fn visit_assigned_variable_pattern(&mut self, ast: &Ast, node: Id<AssignedVariablePattern>) {
        let element = self.unit.element(node);
        self.add_occurrence(element, Some(ast[node].name), WRITE);
        ast.visit_children(node, self);
    }

    fn visit_break_statement(&mut self, ast: &Ast, node: Id<BreakStatement>) {
        let target = break_target(self.unit, node.raw());
        self.add_node_occurrence(target, ast[node].break_keyword, TEXT);
        ast.visit_children(node, self);
    }

    fn visit_catch_clause_parameter(&mut self, ast: &Ast, node: Id<CatchClauseParameter>) {
        let e = self.declared(node);
        self.add_occurrence(e, Some(ast[node].name), WRITE);
        ast.visit_children(node, self);
    }

    fn visit_class_declaration(&mut self, ast: &Ast, node: Id<ClassDeclaration>) {
        let e = self.declared(node);
        let name = dartr_resolver::error::support::class_name_token(ast, ast[node].name_part);
        self.add_occurrence(e, Some(name), WRITE);
        ast.visit_children(node, self);
    }

    fn visit_constructor_declaration(&mut self, ast: &Ast, node: Id<ConstructorDeclaration>) {
        let e = self.declared(node);
        let token = ast[node]
            .name
            .or(ast[node].type_name.map(|t| ast.begin_token(t.raw())))
            .or(ast[node].new_keyword)
            .or(ast[node].factory_keyword);
        self.add_occurrence(e, token, WRITE);
        ast.visit_children(node, self);
    }

    fn visit_constructor_name(&mut self, ast: &Ast, node: Id<ConstructorName>) {
        if ast[node].name.is_none() {
            let element = self.unit.element(node);
            if element.is_some() {
                let named_type = ast[node].type_;
                self.add_occurrence(element, Some(ast[named_type].name), READ);
            }
            if let Some(prefix) = ast[ast[node].type_].import_prefix {
                ast.accept(prefix, self);
            }
            return;
        }
        ast.visit_children(node, self);
    }

    fn visit_continue_statement(&mut self, ast: &Ast, node: Id<ContinueStatement>) {
        let target = break_target(self.unit, node.raw());
        self.add_node_occurrence(target, ast[node].continue_keyword, TEXT);
        ast.visit_children(node, self);
    }

    fn visit_declared_identifier(&mut self, ast: &Ast, node: Id<DeclaredIdentifier>) {
        let e = self.declared(node);
        self.add_occurrence(e, Some(ast[node].name), WRITE);
        ast.visit_children(node, self);
    }

    fn visit_declared_variable_pattern(&mut self, ast: &Ast, node: Id<DeclaredVariablePattern>) {
        let e = self.declared(node);
        let target = e
            .and_then(|e| dartr_resolver::element_ext::pattern_variable_join(self.unit.ctx, e))
            .or(e);
        self.add_occurrence(target, Some(ast[node].name), WRITE);
        ast.visit_children(node, self);
    }

    fn visit_do_statement(&mut self, ast: &Ast, node: Id<DoStatement>) {
        self.add_node_occurrence(Some(node.raw()), ast[node].do_keyword, TEXT);
        ast.visit_children(node, self);
    }

    fn visit_enum_constant_declaration(&mut self, ast: &Ast, node: Id<EnumConstantDeclaration>) {
        let e = self.declared(node);
        self.add_occurrence(e, Some(ast[node].name), WRITE);
        ast.visit_children(node, self);
    }

    fn visit_enum_declaration(&mut self, ast: &Ast, node: Id<EnumDeclaration>) {
        let e = self.declared(node);
        let name = dartr_resolver::error::support::class_name_token(ast, ast[node].name_part);
        self.add_occurrence(e, Some(name), WRITE);
        ast.visit_children(node, self);
    }

    fn visit_extension_declaration(&mut self, ast: &Ast, node: Id<ExtensionDeclaration>) {
        let e = self.declared(node);
        self.add_occurrence(e, ast[node].name, WRITE);
        ast.visit_children(node, self);
    }

    fn visit_extension_override(&mut self, ast: &Ast, node: Id<ExtensionOverride>) {
        let e = self.unit.element(node);
        self.add_occurrence(e, Some(ast[node].name), WRITE);
        ast.visit_children(node, self);
    }

    fn visit_extension_type_declaration(&mut self, ast: &Ast, node: Id<ExtensionTypeDeclaration>) {
        let e = self.declared(node);
        let name = dartr_resolver::error::support::class_name_token(ast, ast[node].name_part);
        self.add_occurrence(e, Some(name), WRITE);
        ast.visit_children(node, self);
    }

    fn visit_for_statement(&mut self, ast: &Ast, node: Id<ForStatement>) {
        self.add_node_occurrence(Some(node.raw()), ast[node].for_keyword, TEXT);
        ast.visit_children(node, self);
    }

    fn visit_function_declaration(&mut self, ast: &Ast, node: Id<FunctionDeclaration>) {
        let e = self.declared(node);
        self.add_occurrence(e, Some(ast[node].name), WRITE);
        ast.visit_children(node, self);
    }

    fn visit_import_prefix_reference(&mut self, ast: &Ast, node: Id<ImportPrefixReference>) {
        let e = self.unit.element(node);
        self.add_occurrence(e, Some(ast[node].name), READ);
        ast.visit_children(node, self);
    }

    fn visit_method_declaration(&mut self, ast: &Ast, node: Id<MethodDeclaration>) {
        let e = self.declared(node);
        self.add_occurrence(e, Some(ast[node].name), WRITE);
        ast.visit_children(node, self);
    }

    fn visit_mixin_declaration(&mut self, ast: &Ast, node: Id<MixinDeclaration>) {
        let e = self.declared(node);
        self.add_occurrence(e, Some(ast[node].name), WRITE);
        ast.visit_children(node, self);
    }

    fn visit_named_argument(&mut self, ast: &Ast, node: Id<NamedArgument>) {
        let parameter = dartr_resolver::error::support::corresponding_parameter(
            self.unit.ctx,
            ast,
            self.unit.tables,
            node.raw(),
        );
        self.add_occurrence(parameter, Some(ast[node].name), WRITE);
        ast.accept(ast[node].argument_expression, self);
    }

    fn visit_named_type(&mut self, ast: &Ast, node: Id<NamedType>) {
        let e = self.unit.element(node);
        self.add_occurrence(e, Some(ast[node].name), READ);
        ast.visit_children(node, self);
    }

    fn visit_pattern_field(&mut self, ast: &Ast, node: Id<PatternField>) {
        let pattern = ast[node].pattern;
        let mut name = ast[node].name.and_then(|n| ast[n].name);
        if name.is_none() {
            if let Some(p) = ast.cast::<DeclaredVariablePattern>(pattern) {
                name = Some(ast[p].name);
            } else if let Some(p) = ast.cast::<AssignedVariablePattern>(pattern) {
                name = Some(ast[p].name);
            }
        }
        let e = self.unit.element(node);
        self.add_occurrence(e, name, WRITE);
        ast.visit_children(node, self);
    }

    fn visit_primary_constructor_name(&mut self, ast: &Ast, node: Id<PrimaryConstructorName>) {
        if let Some(p) = ast.parent(node)
            && ast.is::<PrimaryConstructorDeclaration>(p)
        {
            let e = self.declared(p);
            self.add_occurrence(e, Some(ast[node].name), WRITE);
        }
        ast.visit_children(node, self);
    }

    fn visit_return_statement(&mut self, ast: &Ast, node: Id<ReturnStatement>) {
        let function = self.function_stack.last().copied();
        self.add_node_occurrence(function, ast[node].return_keyword, TEXT);
        ast.visit_children(node, self);
    }

    fn visit_simple_identifier(&mut self, ast: &Ast, node: Id<SimpleIdentifier>) {
        if let Some(p) = ast.parent(node)
            && let Some(c) = ast.cast::<ConstructorDeclaration>(p)
            && ast[c].name.is_none()
            && ast[c].type_name == Some(node)
        {
            return;
        }
        let element = self.unit.write_or_read_element(node);
        let write = dartr_resolver::error::support::write_or_read_element(
            self.unit.ctx,
            ast,
            self.unit.tables,
            node,
        )
        .is_some()
            && is_write(self.unit, node);
        self.add_occurrence(
            element,
            Some(ast[node].token),
            if write { WRITE } else { READ },
        );
        ast.visit_children(node, self);
    }

    fn visit_switch_statement(&mut self, ast: &Ast, node: Id<SwitchStatement>) {
        self.add_node_occurrence(Some(node.raw()), ast[node].switch_keyword, TEXT);
        ast.visit_children(node, self);
    }

    fn visit_type_parameter(&mut self, ast: &Ast, node: Id<TypeParameter>) {
        let e = self.declared(node);
        self.add_occurrence(e, Some(ast[node].name), WRITE);
        ast.visit_children(node, self);
    }

    fn visit_variable_declaration(&mut self, ast: &Ast, node: Id<VariableDeclaration>) {
        let e = self.declared(node);
        self.add_occurrence(e, Some(ast[node].name), WRITE);
        ast.visit_children(node, self);
    }

    fn visit_while_statement(&mut self, ast: &Ast, node: Id<WhileStatement>) {
        self.add_node_occurrence(Some(node.raw()), ast[node].while_keyword, TEXT);
        ast.visit_children(node, self);
    }

    fn visit_yield_statement(&mut self, ast: &Ast, node: Id<YieldStatement>) {
        let function = self.function_stack.last().copied();
        self.add_node_occurrence(function, ast[node].yield_keyword, TEXT);
        ast.visit_children(node, self);
    }
}

/// Dart `node.writeElement != null`: the identifier is the target of an
/// assignment, prefix or postfix expression with a write element.
fn is_write(unit: &Unit<'_, '_>, node: Id<SimpleIdentifier>) -> bool {
    let ast = unit.ast;
    let mut current = node.raw();
    let mut parent = ast.parent(current);
    // `a.b = ...` / `o.b = ...`: the property name of the target.
    if let Some(p) = parent
        && (ast
            .cast::<PrefixedIdentifier>(p)
            .is_some_and(|pi| ast[pi].identifier == node)
            || ast
                .cast::<PropertyAccess>(p)
                .is_some_and(|pa| ast[pa].property_name == node))
    {
        current = p;
        parent = ast.parent(p);
    }
    let Some(p) = parent else { return false };
    let target_of = |n: NodeId| -> bool {
        if let Some(a) = ast.cast::<AssignmentExpression>(p) {
            return ast[a].left_hand_side.raw() == n;
        }
        if let Some(a) = ast.cast::<PostfixExpression>(p) {
            return ast[a].operand.raw() == n;
        }
        if let Some(a) = ast.cast::<PrefixExpression>(p) {
            return ast[a].operand.raw() == n;
        }
        false
    };
    target_of(current) && unit.tables.write_element.get(p).is_some()
}
