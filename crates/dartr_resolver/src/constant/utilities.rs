// Dart source: pkg/analyzer/lib/src/dart/constant/utilities.dart
// (with TypeAliasElementImpl.isProperRename from element.dart and
// InvocationTarget.wrongNumberOfTypeArgumentsError from
// type_instantiation_target.dart)

//! [`ConstantFinder`] (the constants of a unit to compute),
//! [`ConstantExpressionsDependenciesFinder`] (the constants that the other
//! constant expressions of a unit depend on) and [`find_references`] (Dart
//! `ReferenceFinder`).

use dartr_ast::{
    Annotation, Ast, AstVisitor, ClassDeclaration, ConstantPattern, ConstructorDeclaration,
    DotShorthandConstructorInvocation, EnumConstantDeclaration, FieldFormalParameter, Id,
    InstanceCreationExpression, Label, ListLiteral, MapPatternEntry, NodeId, NodeKind,
    RecordLiteral, RedirectingConstructorInvocation, RegularFormalParameter, RelationalPattern,
    SetOrMapLiteral, SimpleIdentifier, SuperConstructorInvocation, SuperFormalParameter,
    SwitchCase, VariableDeclaration, VariableDeclarationList,
};
use dartr_diagnostics::{LocatableDiagnostic, diag};
use dartr_element::{Ctx, EId, ElementId, FragmentFlags, Tag, TypeAliasElement, TypeId, TypeKind};
use dartr_typesystem::{TypeExt, TypeSystem, member};
use indexmap::IndexSet;

use crate::ast_ext;
use crate::constant::evaluation::{
    ConstantEvaluationEngine, ConstantTarget, NodeRef,
    dot_shorthand_constructor_invocation_is_const, is_const_constructor, list_literal_is_const,
    record_literal_is_const, set_or_map_kind, set_or_map_literal_is_const,
};
use crate::library_analyzer::ResolvedUnit;

/// Dart `ReferenceFinder`: reports the constants that the subtree [node]
/// depends on.
pub fn find_references(
    engine: &ConstantEvaluationEngine<'_>,
    node: NodeRef,
    callback: &mut dyn FnMut(ConstantTarget),
) {
    let unit = engine.unit(node.unit);
    let ctx = engine.ctx(&unit);
    let mut finder = ReferenceFinder {
        ctx: &ctx,
        unit: &unit,
        found: Vec::new(),
    };
    finder.visit(&unit.ast, node.node);
    for target in finder.found {
        callback(target);
    }
}

struct ReferenceFinder<'r> {
    ctx: &'r Ctx<'r>,
    unit: &'r ResolvedUnit,
    found: Vec<ConstantTarget>,
}

impl ReferenceFinder<'_> {
    fn visit(&mut self, ast: &Ast, node: NodeId) {
        ast.accept(node, self);
    }

    fn base_of(&self, node: NodeId) -> Option<ElementId> {
        self.unit
            .tables
            .element
            .get(node)
            .map(|&e| member::base_element(self.ctx, e))
    }
}

impl AstVisitor for ReferenceFinder<'_> {
    fn visit_dot_shorthand_constructor_invocation(
        &mut self,
        ast: &Ast,
        node: Id<DotShorthandConstructorInvocation>,
    ) {
        if dot_shorthand_constructor_invocation_is_const(ast, node)
            && let Some(constructor) = self.base_of(ast[node].constructor_name.raw())
            && constructor.tag() == Tag::Constructor
            && is_const_constructor(self.ctx, constructor)
        {
            self.found.push(ConstantTarget::Element(constructor));
        }
        ast.visit_children(node, self);
    }

    fn visit_instance_creation_expression(
        &mut self,
        ast: &Ast,
        node: Id<InstanceCreationExpression>,
    ) {
        if ast_ext::instance_creation_is_const(ast, node)
            && let Some(constructor) = self.base_of(ast[node].constructor_name.raw())
            && constructor.tag() == Tag::Constructor
            && is_const_constructor(self.ctx, constructor)
        {
            self.found.push(ConstantTarget::Element(constructor));
        }
        ast.visit_children(node, self);
    }

    fn visit_label(&mut self, _ast: &Ast, _node: Id<Label>) {
        // We are visiting the "label" part of a named expression in a
        // function call (presumably a constructor call), e.g.
        // "const C(label: ...)". We don't want to visit the SimpleIdentifier
        // for the label because that's a reference to a function parameter
        // that needs to be filled in; it's not a constant whose value we
        // depend on.
    }

    fn visit_redirecting_constructor_invocation(
        &mut self,
        ast: &Ast,
        node: Id<RedirectingConstructorInvocation>,
    ) {
        ast.visit_children(node, self);
        if let Some(target) = self.base_of(node.raw()) {
            self.found.push(ConstantTarget::Element(target));
        }
    }

    fn visit_simple_identifier(&mut self, _ast: &Ast, node: Id<SimpleIdentifier>) {
        let Some(mut element) = self.unit.tables.element.get(node).copied() else {
            return;
        };
        if member::base_element(self.ctx, element).tag() == Tag::Getter {
            match member::variable(self.ctx, element) {
                Some(v) => element = v,
                None => return,
            }
        }
        let base = member::base_element(self.ctx, element);
        if is_variable(base) && crate::element_ext::is_const(self.ctx, base) {
            self.found.push(ConstantTarget::Element(base));
        }
    }

    fn visit_super_constructor_invocation(
        &mut self,
        ast: &Ast,
        node: Id<SuperConstructorInvocation>,
    ) {
        ast.visit_children(node, self);
        if let Some(constructor) = self.base_of(node.raw()) {
            self.found.push(ConstantTarget::Element(constructor));
        }
    }
}

fn is_variable(e: ElementId) -> bool {
    e.is::<dartr_element::VariableElement>()
}

/// Dart `ConstantFinder`: the elements and annotations of [unit] whose
/// constant values need to be computed.
pub fn find_constants(ctx: &Ctx<'_>, unit_index: u32, unit: &ResolvedUnit) -> Vec<ConstantTarget> {
    let mut finder = ConstantFinder {
        ctx,
        unit,
        unit_index,
        constants_to_compute: Vec::new(),
        treat_final_instance_var_as_const: false,
    };
    unit.ast.accept(unit.unit, &mut finder);
    finder.constants_to_compute
}

/// Dart `ConstantFinder`.
pub struct ConstantFinder<'r> {
    ctx: &'r Ctx<'r>,
    unit: &'r ResolvedUnit,
    unit_index: u32,
    /// The elements and AST nodes whose constant values need to be
    /// computed.
    pub constants_to_compute: Vec<ConstantTarget>,
    /// A flag indicating whether instance variables marked as "final"
    /// should be treated as "const".
    treat_final_instance_var_as_const: bool,
}

impl ConstantFinder<'_> {
    fn element_of_declaration(&self, node: NodeId) -> Option<ElementId> {
        let fragment = *self.unit.tables.declared_fragment.get(node)?;
        self.ctx.fragment_data(fragment)?.element.try_get().copied()
    }

    fn visit_formal_parameter(&mut self, has_default_clause: bool, node: NodeId) {
        if has_default_clause && let Some(element) = self.element_of_declaration(node) {
            self.constants_to_compute
                .push(ConstantTarget::Element(element));
        }
    }
}

impl AstVisitor for ConstantFinder<'_> {
    fn visit_annotation(&mut self, ast: &Ast, node: Id<Annotation>) {
        ast.visit_children(node, self);
        // Analyzer ignores annotations on "part of" directives and on enum
        // constant declarations.
        let parent = ast.parent(node).map(|p| ast.kind(p));
        if matches!(
            parent,
            Some(
                NodeKind::PartDirective
                    | NodeKind::PartOfDirective
                    | NodeKind::EnumConstantDeclaration
            )
        ) {
            return;
        }
        self.constants_to_compute
            .push(ConstantTarget::Annotation(NodeRef::new(
                self.unit_index,
                node,
            )));
    }

    fn visit_class_declaration(&mut self, ast: &Ast, node: Id<ClassDeclaration>) {
        let prev = self.treat_final_instance_var_as_const;
        let has_const_constructor = self
            .element_of_declaration(node.raw())
            .and_then(|e| e.cast::<dartr_element::InterfaceElement>())
            .is_some_and(|e| {
                self.ctx
                    .interface(e)
                    .constructors
                    .iter()
                    .any(|c| is_const_constructor(self.ctx, c.raw()))
            });
        let name_part_is_primary =
            ast.kind(ast[node].name_part) == NodeKind::PrimaryConstructorDeclaration;
        if has_const_constructor && !name_part_is_primary {
            // Instance vars marked "final" need to be included in the
            // dependency graph, since constant constructors implicitly use
            // the values in their initializers.
            self.treat_final_instance_var_as_const = true;
        }
        ast.visit_children(node, self);
        self.treat_final_instance_var_as_const = prev;
    }

    fn visit_constructor_declaration(&mut self, ast: &Ast, node: Id<ConstructorDeclaration>) {
        ast.visit_children(node, self);
        if ast[node].const_keyword.is_some()
            && let Some(element) = self.element_of_declaration(node.raw())
        {
            self.constants_to_compute
                .push(ConstantTarget::Element(element));
            if let Some(executable) = element.cast::<dartr_element::ExecutableElement>() {
                for p in &self.ctx.executable(executable).formal_params {
                    self.constants_to_compute
                        .push(ConstantTarget::Element(p.raw()));
                }
            }
        }
    }

    fn visit_enum_constant_declaration(&mut self, ast: &Ast, node: Id<EnumConstantDeclaration>) {
        ast.visit_children(node, self);
        if let Some(element) = self.element_of_declaration(node.raw()) {
            self.constants_to_compute
                .push(ConstantTarget::Element(element));
        }
    }

    fn visit_field_formal_parameter(&mut self, ast: &Ast, node: Id<FieldFormalParameter>) {
        ast.visit_children(node, self);
        self.visit_formal_parameter(ast[node].default_clause.is_some(), node.raw());
    }

    fn visit_regular_formal_parameter(&mut self, ast: &Ast, node: Id<RegularFormalParameter>) {
        ast.visit_children(node, self);
        self.visit_formal_parameter(ast[node].default_clause.is_some(), node.raw());
    }

    fn visit_super_formal_parameter(&mut self, ast: &Ast, node: Id<SuperFormalParameter>) {
        ast.visit_children(node, self);
        self.visit_formal_parameter(ast[node].default_clause.is_some(), node.raw());
    }

    fn visit_variable_declaration(&mut self, ast: &Ast, node: Id<VariableDeclaration>) {
        ast.visit_children(node, self);
        if ast[node].initializer.is_none() {
            return;
        }
        let Some(element) = self.element_of_declaration(node.raw()) else {
            return;
        };
        let list = ast
            .parent(node)
            .and_then(|p| ast.cast::<VariableDeclarationList>(p));
        let keyword = list.and_then(|l| ast[l].keyword);
        let is_const = ast_ext::is_keyword(ast, keyword, "const");
        let is_final = ast_ext::is_keyword(ast, keyword, "final");
        let is_instance_field = element.tag() == Tag::Field
            && !crate::element_ext::first_fragment_flags(self.ctx, element)
                .contains(FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC);
        if is_const || (self.treat_final_instance_var_as_const && is_instance_field && is_final) {
            self.constants_to_compute
                .push(ConstantTarget::Element(element));
        }
    }
}

/// Dart `ConstantExpressionsDependenciesFinder`: the constants that the
/// constant expressions of [unit] (const collection literals, const
/// instance creations, switch cases, patterns, keys of non-const maps and
/// elements of non-const sets) depend on.
pub fn find_dependencies(
    engine: &ConstantEvaluationEngine<'_>,
    unit_index: u32,
) -> IndexSet<ConstantTarget> {
    let unit = engine.unit(unit_index);
    let ctx = engine.ctx(&unit);
    let mut finder = DependenciesFinder {
        engine,
        ctx: &ctx,
        unit: &unit,
        unit_index,
        dependencies: IndexSet::new(),
    };
    unit.ast.accept(unit.unit, &mut finder);
    finder.dependencies
}

struct DependenciesFinder<'r, 'a> {
    engine: &'r ConstantEvaluationEngine<'a>,
    ctx: &'r Ctx<'r>,
    unit: &'r ResolvedUnit,
    unit_index: u32,
    dependencies: IndexSet<ConstantTarget>,
}

impl DependenciesFinder<'_, '_> {
    /// Dart `_find(node)`.
    fn find(&mut self, node: NodeId) {
        let dependencies = &mut self.dependencies;
        find_references(self.engine, NodeRef::new(self.unit_index, node), &mut |t| {
            dependencies.insert(t);
        });
    }
}

impl AstVisitor for DependenciesFinder<'_, '_> {
    fn visit_constant_pattern(&mut self, ast: &Ast, node: Id<ConstantPattern>) {
        self.find(ast[node].expression.raw());
    }

    fn visit_dot_shorthand_constructor_invocation(
        &mut self,
        ast: &Ast,
        node: Id<DotShorthandConstructorInvocation>,
    ) {
        if dot_shorthand_constructor_invocation_is_const(ast, node) {
            self.find(node.raw());
        } else {
            ast.visit_children(node, self);
        }
    }

    fn visit_instance_creation_expression(
        &mut self,
        ast: &Ast,
        node: Id<InstanceCreationExpression>,
    ) {
        if ast_ext::instance_creation_is_const(ast, node) {
            self.find(node.raw());
        } else {
            ast.visit_children(node, self);
        }
    }

    fn visit_list_literal(&mut self, ast: &Ast, node: Id<ListLiteral>) {
        if list_literal_is_const(ast, node) {
            self.find(node.raw());
        } else {
            ast.visit_children(node, self);
        }
    }

    fn visit_map_pattern_entry(&mut self, ast: &Ast, node: Id<MapPatternEntry>) {
        self.find(ast[node].key.raw());
        ast.visit_children(node, self);
    }

    fn visit_record_literal(&mut self, ast: &Ast, node: Id<RecordLiteral>) {
        if record_literal_is_const(ast, node) {
            self.find(node.raw());
        } else {
            ast.visit_children(node, self);
        }
    }

    fn visit_relational_pattern(&mut self, ast: &Ast, node: Id<RelationalPattern>) {
        self.find(ast[node].operand.raw());
    }

    fn visit_set_or_map_literal(&mut self, ast: &Ast, node: Id<SetOrMapLiteral>) {
        if set_or_map_literal_is_const(ast, node) {
            self.find(node.raw());
        } else {
            let (is_set, is_map) = set_or_map_kind(self.unit, self.ctx, node);
            if is_map || is_set {
                // Values of keys are computed to check that they are
                // unique; values of sets are computed to check that they are
                // unique.
                for &entry in ast.list_raw(ast[node].elements) {
                    self.find(entry);
                }
            }
            ast.visit_children(node, self);
        }
    }

    fn visit_switch_case(&mut self, ast: &Ast, node: Id<SwitchCase>) {
        self.find(ast[node].expression.raw());
        for &s in ast.list_raw(ast[node].statements) {
            ast.accept(s, self);
        }
    }
}

/// Dart `TypeAliasElementImpl.isProperRename`.
pub fn is_proper_rename(ctx: &Ctx<'_>, alias: EId<TypeAliasElement>) -> bool {
    let data = ctx.get(alias);
    let Some(aliased_type) = data.aliased_type.get() else {
        return false;
    };
    let TypeKind::Interface { element, args, .. } = *ctx.ty(aliased_type) else {
        return false;
    };
    let type_parameters = &data.type_params;
    let aliased_parameters = ctx.interface(element).type_params.clone();
    let type_arguments = ctx.list(args);
    if type_parameters.len() != aliased_parameters.len() {
        return false;
    }
    let ts = TypeSystem::new(*ctx);
    for i in 0..type_parameters.len() {
        let bound = ctx
            .type_parameter_bound(type_parameters[i])
            .unwrap_or(TypeId::DYNAMIC);
        let aliased_bound = ctx
            .type_parameter_bound(aliased_parameters[i])
            .unwrap_or(TypeId::DYNAMIC);
        if !ts.is_subtype_of(bound, aliased_bound) || !ts.is_subtype_of(aliased_bound, bound) {
            return false;
        }
        if let TypeKind::TypeParameter { param, .. } = *ctx.ty(type_arguments[i])
            && param != type_parameters[i]
        {
            return false;
        }
    }
    true
}

/// Dart `DartObjectComputer.typeInstantiate`'s
/// `target.wrongNumberOfTypeArgumentsError(...)`: an executable element
/// target when [function] is a `SimpleIdentifier` of an executable,
/// otherwise a function typed expression of [raw_type].
pub fn wrong_number_of_type_arguments_error(
    ctx: &Ctx<'_>,
    unit: &ResolvedUnit,
    function: Id<dartr_ast::Expression>,
    raw_type: TypeId,
    type_parameter_count: usize,
    type_argument_count: usize,
) -> LocatableDiagnostic {
    if unit.ast.kind(function) == NodeKind::SimpleIdentifier
        && let Some(&e) = unit.tables.element.get(function)
    {
        let base = member::base_element(ctx, e);
        if base.is::<dartr_element::ExecutableElement>() {
            return diag::wrong_number_of_type_arguments_element(
                base.kind().display_name(),
                ctx.element_name(base).unwrap_or(""),
                type_parameter_count as i64,
                type_argument_count as i64,
            );
        }
    }
    diag::wrong_number_of_type_arguments_function(
        dartr_element::diagnostics::type_arg(ctx, raw_type),
        type_parameter_count as i64,
        type_argument_count as i64,
    )
}
