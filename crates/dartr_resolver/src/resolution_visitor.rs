// Dart source: pkg/analyzer/lib/src/dart/resolver/resolution_visitor.dart
// (ResolutionVisitor, _PatternVariableBinderVisitor, _VariableBinder,
// _VariableBinderErrors)
// Dart source: pkg/analyzer/lib/src/diagnostic/diagnostic_factory.dart
// (duplicateDefinitionForNodes)

//! `ResolutionVisitor`: the pass before the `ResolverVisitor`. It walks the
//! unit with the lexical scopes ([`crate::scope_context`]), resolves type
//! annotations ([`crate::named_type_resolver`],
//! [`crate::record_type_annotation_resolver`]), sets the types of
//! declarations with explicit types (local variables, formal parameters),
//! rewrites nodes whose meaning depends on the scope
//! ([`crate::ast_rewrite`]), and records the scope lookup result of each
//! `SimpleIdentifier` (`ResolverTables.scope_lookup_result`) and the element
//! of each identifier that refers to a promotable local
//! (`ResolutionTables.element`).
//!
//! # Where the Dart node fields go
//!
//! - `NamedType.type`, `GenericFunctionType.type`, `RecordTypeAnnotation.type`,
//!   `FormalParameter.explicitFragmentType`: `ResolutionTables.annotation_type`.
//! - `NamedType.element`, `ImportPrefixReference.element`,
//!   `SimpleIdentifier.element`, `AssignedVariablePattern.element`,
//!   `LabelReference.element`: `ResolutionTables.element`.
//! - `SimpleIdentifier.scopeLookupResult`, `BreakStatement.target`,
//!   `ContinueStatement.target`, `LocalVariableInfo`,
//!   `GuardedPattern.variables`, `SwitchStatementCaseGroup.variables`,
//!   `PatternVariableDeclaration.elements`, `ForEachPartsWithPattern.variables`:
//!   [`ResolverTables`].
//! - The scopes that Dart records on nodes (`nameScope`, `bodyScope`):
//!   not kept (see [`crate::scope_context`]).
//!
//! # Element types
//!
//! Dart sets `element.type` (formal parameters, local variables),
//! `element.bound` (type parameters) and `element.returnType` on the
//! elements of the declarations it visits. Here only local elements (in
//! the unit's local arena) change: linked elements are frozen and shared
//! with other analysis tasks (design §2.5), and linking already set the
//! same types on them.
//!
//! Not ported: import usage tracking in `visitHideCombinator` /
//! `visitShowCombinator` (see [`crate::scope`]); `dataForTesting`.

use std::sync::Arc;

use dartr_ast::{
    AnonymousMethodInvocation, AssignedVariablePattern, Ast, AstVisitorMut, Block,
    BlockFunctionBody, BreakStatement, CatchClause, ClassDeclaration, ClassTypeAlias, Comment,
    ConstructorDeclaration, ConstructorFieldInitializer, ConstructorName, ContinueStatement,
    DeclaredIdentifier, DeclaredVariablePattern, DoStatement, DotShorthandConstructorInvocation,
    DotShorthandInvocation, DotShorthandPropertyAccess, EmptyClassBody, EmptyEnumBody,
    EnumDeclaration, ExtendsClause, ExtensionDeclaration, ExtensionTypeDeclaration,
    FieldDeclaration, FieldFormalParameter, ForEachPartsWithDeclaration,
    ForEachPartsWithIdentifier, ForEachPartsWithPattern, ForElement, ForPartsWithDeclarations,
    ForPartsWithExpression, ForPartsWithPattern, ForStatement, FunctionDeclaration,
    FunctionExpression, FunctionTypeAlias, GenericFunctionType, GenericTypeAlias, GuardedPattern,
    Id, IfElement, IfStatement, ImplementsClause, ImportDirective, InstanceCreationExpression,
    LabelReference, LabeledStatement, LibraryDirective, MethodDeclaration, MethodInvocation,
    MixinDeclaration, MixinOnClause, NodeId, NodeList, PatternAssignment,
    PatternVariableDeclaration, PatternVariableDeclarationStatement, PrefixedIdentifier,
    PrimaryConstructorBody, PropertyAccess, RecordTypeAnnotation, RedirectingConstructorInvocation,
    RegularFormalParameter, SimpleIdentifier, SuperConstructorInvocation, SuperFormalParameter,
    SwitchExpression, SwitchStatement, TypeParameter, VariableDeclaration, VariableDeclarationList,
    WhileStatement, WithClause,
};
use dartr_diagnostics::{
    Diagnostic, DiagnosticMessage, LocatableDiagnostic, LocatedDiagnostic, diag,
};
use dartr_element::diagnostics::type_arg;
use dartr_element::{
    AnyElement, Ctx, EId, ElemRef, ElementData, ElementId, ExtensionTypeElement, FId, FnParam,
    FormalParameterElement, FragmentData, FragmentFlags, FragmentId, GenericFunctionTypeElement,
    InterfaceElement, JoinPatternVariableElement, JoinPatternVariableFragment, LibraryElement,
    LibraryFragment, LocalVariableElement, LocalVariableFragment, Name, Nullability,
    PatternVariableFragment, PatternVariableFragmentData, ResolutionTables, Tag, TypeId, TypeKind,
    TypeParameterElement, VarSlot, VariableElementData, VariableFragmentData,
};
use dartr_flow::type_analyzer::{JoinedPatternVariableInconsistency, TypeAnalyzerErrorsBase};
use dartr_flow::variable_bindings::{VariableBinder, VariableBinderErrors};
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_type_analyzer::variable_bindings::VariableBinderState;
use dartr_typesystem::element_type::formal_parameter_as_fn_param;
use dartr_typesystem::{TypeExt, TypeSystem};
use indexmap::IndexMap;

use crate::ast_ext::{
    formal_parameter_parts, identifier_name, is_keyword, method_invocation_real_target,
    simple_identifier_in_setter_context, statement_unlabeled, token_end,
};
use crate::ast_rewrite::AstRewriter;
use crate::element_binding_visitor::switch_member_labels;
use crate::named_type_resolver::{
    NamedTypeEnv, NamedTypeResolver, should_ignore_undefined_named_type,
};
use crate::record_type_annotation_resolver::RecordTypeAnnotationResolver;
use crate::resolver::UnitContext;
use crate::scope::{
    LabelScopes, NameScope, UnlabeledBreakContinueContext, library_feature_enabled,
};
use crate::scope_context::ScopeContext;
use crate::tables::ResolverTables;

/// Dart `unit.accept(ResolutionVisitor(libraryFragment: ..., nameScope:
/// libraryFragment.scope, ...))`. Needs a context with a local arena.
pub fn resolve_unit<'a>(
    ctx: &Ctx<'a>,
    unit_ctx: UnitContext<'a>,
    ast: &mut Ast,
    unit: Id<dartr_ast::CompilationUnit>,
    tables: &mut ResolutionTables,
    rt: &mut ResolverTables,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut visitor = ResolutionVisitor::new(*ctx, unit_ctx, tables, rt, diagnostics);
    ast.accept_mut(unit, &mut visitor);
    for (node, result) in visitor.scope_context.finish_recorded_lookups() {
        visitor.rt.this_scope_lookup.insert(node, (result.getter, result.setter));
    }
}

/// The key of a join of pattern variables (Dart `Object key` of
/// `joinPatternVariables`): the logical-or pattern, or the switch statement
/// case group (identified by its last member).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum JoinKey {
    LogicalOr(NodeId),
    SharedCase(NodeId),
}

impl From<NodeId> for JoinKey {
    fn from(node: NodeId) -> Self {
        JoinKey::LogicalOr(node)
    }
}

/// Dart `ResolutionVisitor`.
pub struct ResolutionVisitor<'c, 'a> {
    pub(crate) ctx: Ctx<'a>,
    pub(crate) unit: UnitContext<'a>,
    pub(crate) type_system: TypeSystem<'a>,
    pub(crate) tables: &'c mut ResolutionTables,
    pub(crate) rt: &'c mut ResolverTables,
    /// Dart `_diagnosticReporter`.
    pub(crate) diagnostics: &'c mut Vec<Diagnostic>,
    /// Dart `_scopeContext`.
    pub(crate) scope_context: ScopeContext<'a>,
    /// Dart `_namedTypeResolver`.
    pub(crate) named_type_resolver: NamedTypeResolver,
    /// Dart `_patternVariables`.
    pattern_variables: VariableBinderState<Name, ElementId, JoinKey>,
    /// The Dart `LabelScope` objects.
    label_scopes: LabelScopes,
    /// Dart `_labelScope`.
    label_scope: Option<usize>,
    /// Dart `_unlabeledBreakContinueContext`.
    unlabeled_break_continue_context: UnlabeledBreakContinueContext,
    /// Dart `_enclosingClosure`.
    enclosing_closure: Option<ElementId>,
    /// Dart `_libraryDirectiveIndex`.
    library_directive_index: u32,
    /// `_libraryElement.featureSet.isEnabled(Feature.wildcard_variables)`.
    wildcard_variables: bool,
    /// The path and URI of the unit (Dart `_diagnosticReporter.source`).
    source_path: Arc<str>,
    source_uri: Arc<str>,
}

impl<'c, 'a> ResolutionVisitor<'c, 'a> {
    /// Dart `ResolutionVisitor(...)`.
    pub fn new(
        ctx: Ctx<'a>,
        unit: UnitContext<'a>,
        tables: &'c mut ResolutionTables,
        rt: &'c mut ResolverTables,
        diagnostics: &'c mut Vec<Diagnostic>,
    ) -> ResolutionVisitor<'c, 'a> {
        let source = ctx.fragment(unit.fragment).source.clone();
        ResolutionVisitor {
            ctx,
            unit,
            type_system: TypeSystem::new(ctx),
            tables,
            rt,
            diagnostics,
            scope_context: ScopeContext::new(ctx, unit.scopes, unit.library, unit.fragment),
            named_type_resolver: NamedTypeResolver::new(
                &ctx,
                unit.library,
                unit.fragment,
                unit.options.strict_inference,
                unit.options.strict_casts,
            ),
            pattern_variables: VariableBinderState::new(),
            label_scopes: LabelScopes::default(),
            label_scope: None,
            unlabeled_break_continue_context: UnlabeledBreakContinueContext::root(),
            enclosing_closure: None,
            library_directive_index: 0,
            wildcard_variables: library_feature_enabled(
                &ctx,
                unit.library,
                ExperimentalFlag::WildcardVariables,
            ),
            source_path: source.path,
            source_uri: source.uri,
        }
    }

    fn library(&self) -> EId<LibraryElement> {
        self.unit.library
    }

    fn library_fragment(&self) -> FId<LibraryFragment> {
        self.unit.fragment
    }

    // ------------------------------------------------------------ visiting

    /// `node.accept(this)`.
    pub(crate) fn visit(&mut self, ast: &mut Ast, node: impl Into<NodeId>) {
        ast.accept_mut(node.into(), self);
    }

    /// `node?.accept(this)`.
    pub(crate) fn visit_opt(&mut self, ast: &mut Ast, node: Option<impl Into<NodeId>>) {
        if let Some(n) = node {
            self.visit(ast, n);
        }
    }

    /// `list.accept(this)`.
    pub(crate) fn visit_list<T: ?Sized>(&mut self, ast: &mut Ast, list: NodeList<T>) {
        let items = ast.list_raw(list).to_vec();
        for n in items {
            self.visit(ast, n);
        }
    }

    /// `node.visitChildren(this)`.
    pub(crate) fn visit_children(&mut self, ast: &mut Ast, node: NodeId) {
        ast.visit_children_mut(node, self);
    }

    /// `node.visitChildrenWithHooks(this, visitX: (_) {})`: the children
    /// without [skip].
    fn visit_children_except(&mut self, ast: &mut Ast, node: NodeId, skip: Option<NodeId>) {
        for child in ast.children(node) {
            if Some(child) != skip {
                self.visit(ast, child);
            }
        }
    }

    // ------------------------------------------------------------ elements

    /// The element of the declared fragment of [node]
    /// (`node.declaredFragment!.element`).
    pub(crate) fn declared_element(&self, node: NodeId) -> Option<ElementId> {
        let fragment = *self.tables.declared_fragment.get(node)?;
        self.ctx.fragment_data(fragment)?.element.try_get().copied()
    }

    fn declared_fragment(&self, node: NodeId) -> Option<FragmentId> {
        self.tables.declared_fragment.get(node).copied()
    }

    /// Dart `TypeAnnotation.type` (`typeOrThrow`, `InvalidType` when not
    /// set).
    fn annotation_type(&self, node: impl Into<NodeId>) -> TypeId {
        self.tables
            .annotation_type
            .get(node)
            .copied()
            .unwrap_or(TypeId::INVALID)
    }

    /// Whether [e] is in the local arena of the unit (see the module
    /// documentation, "Element types").
    fn is_local(&self, e: ElementId) -> bool {
        self.ctx
            .local
            .is_some_and(|local| local.store.id == e.store())
    }

    /// Dart `element.type = t` of a local variable or formal parameter.
    fn set_variable_type(&self, e: ElementId, t: TypeId) {
        if !self.is_local(e) {
            return;
        }
        match self.ctx.any(e) {
            AnyElement::LocalVariable(v) => v.type_.set(Some(t)),
            AnyElement::FormalParameter(p) => p.type_.set(Some(t)),
            _ => {}
        }
    }

    /// The type of a variable element (`InvalidType` when not set).
    fn variable_type(&self, e: ElementId) -> TypeId {
        crate::element_ext::variable_type(&self.ctx, e)
    }

    /// The `FragmentImpl` data of the first fragment of [e].
    fn first_fragment_data(&self, e: ElementId) -> Option<&'a FragmentData> {
        let data = self.ctx.element_data(e)?;
        self.ctx.fragment_data(data.first_fragment)
    }

    /// The pattern data of the first fragment of a pattern variable.
    fn pattern_data(&self, e: ElementId) -> Option<&'a PatternVariableFragmentData> {
        let v = e.cast::<LocalVariableElement>()?;
        let first = self.ctx.get(v).first_fragment();
        Some(&self.ctx.fragment(first).pattern)
    }

    // ------------------------------------------------------------ diagnostics

    fn report(&mut self, d: LocatedDiagnostic) {
        self.diagnostics.push(d.into_diagnostic());
    }

    fn report_at_node(&mut self, ast: &Ast, d: LocatableDiagnostic, node: impl Into<NodeId>) {
        let node = node.into();
        self.report(d.at_offset(ast.offset(node) as usize, ast.length(node) as usize));
    }

    fn report_at_token(&mut self, ast: &Ast, d: LocatableDiagnostic, token: dartr_syntax::TokenId) {
        let offset = ast.tokens.offset(token);
        let length = token_end(ast, token) - offset;
        self.report(d.at_offset(offset as usize, length as usize));
    }

    // ------------------------------------------------------------ helpers

    /// Dart `_withUnlabeledBreakContinueContextNested(statement, ...)`, the
    /// start; returns the previous context.
    fn nest_unlabeled_context(&mut self, statement: NodeId) -> UnlabeledBreakContinueContext {
        let nested = self.unlabeled_break_continue_context.nest(statement);
        std::mem::replace(&mut self.unlabeled_break_continue_context, nested)
    }

    /// Dart `_visitStatementInScope`.
    fn visit_statement_in_scope(&mut self, ast: &mut Ast, statement: Option<NodeId>) {
        let Some(statement) = statement else {
            return;
        };
        if let Some(block) = ast.cast::<Block>(statement) {
            self.visit_block_impl(ast, block);
        } else {
            let scope = self.scope_context.push_local_scope();
            self.define_local_elements(ast, scope, &[statement]);
            self.visit(ast, statement);
            self.scope_context.pop();
        }
    }

    /// Dart `visitBlock`.
    fn visit_block_impl(&mut self, ast: &mut Ast, node: Id<Block>) {
        let scope = self.scope_context.push_local_scope();
        let statements = ast.list_raw(ast[node].statements).to_vec();
        self.define_local_elements(ast, scope, &statements);
        self.visit_list(ast, ast[node].statements);
        self.scope_context.pop();
    }

    /// Dart `_defineLocalElements`.
    fn define_local_elements(&mut self, ast: &Ast, scope: usize, statements: &[NodeId]) {
        for &statement in statements {
            let statement = statement_unlabeled(ast, statement);
            if let Some(s) = ast.cast::<dartr_ast::FunctionDeclarationStatement>(statement) {
                let declaration = ast[s].function_declaration;
                if let Some(e) = self.declared_element(declaration.raw()) {
                    self.scope_context.add_local(scope, e);
                }
            } else if let Some(s) = ast.cast::<PatternVariableDeclarationStatement>(statement) {
                let declaration = ast[s].declaration;
                self.define_pattern_variable_declaration_elements(ast, scope, declaration);
            } else if let Some(s) = ast.cast::<dartr_ast::VariableDeclarationStatement>(statement) {
                let variables = ast[s].variables;
                for e in self.declared_elements(ast, variables) {
                    self.scope_context.add_local(scope, e);
                }
            }
        }
    }

    /// Dart `VariableDeclarationList.declaredElements` (the local variable
    /// elements).
    fn declared_elements(&self, ast: &Ast, list: Id<VariableDeclarationList>) -> Vec<ElementId> {
        ast.list(ast[list].variables)
            .iter()
            .filter_map(|&v| self.declared_element(v.raw()))
            .filter(|e| e.is::<LocalVariableElement>())
            .collect()
    }

    /// Dart `_definePatternVariableDeclarationElements`.
    fn define_pattern_variable_declaration_elements(
        &mut self,
        ast: &Ast,
        scope: usize,
        declaration: Id<PatternVariableDeclaration>,
    ) {
        let pattern = ast[declaration].pattern;
        let variables = self.compute_declared_pattern_variables(ast, pattern.raw());
        self.rt
            .pattern_variable_declaration_elements
            .insert(declaration, variables.clone());
        for e in variables {
            self.scope_context.add_local(scope, e);
        }
    }

    /// Dart `_computeDeclaredPatternVariables`: the bind pattern variables.
    fn compute_declared_pattern_variables(&mut self, ast: &Ast, pattern: NodeId) -> Vec<ElementId> {
        self.compute_pattern_variables(ast, pattern, None)
            .into_values()
            .filter(|e| e.tag() == Tag::BindPatternVariable)
            .collect()
    }

    /// Dart `_computePatternVariables`.
    fn compute_pattern_variables(
        &mut self,
        ast: &Ast,
        pattern: NodeId,
        shared_case_scope_key: Option<JoinKey>,
    ) -> IndexMap<Name, ElementId> {
        let mut state = std::mem::take(&mut self.pattern_variables);
        state.case_pattern_start();
        {
            let mut binder = JoinBuilder { ctx: self.ctx };
            let mut errors = BinderErrors {
                ctx: self.ctx,
                ast,
                diagnostics: &mut *self.diagnostics,
                source_path: &self.source_path,
            };
            bind_pattern_variables(
                &self.ctx,
                &self.tables.declared_fragment,
                ast,
                pattern,
                &mut state,
                &mut binder,
                &mut errors,
            );
        }
        let result = state.case_pattern_finish(shared_case_scope_key.as_ref());
        self.pattern_variables = state;
        result
    }

    /// Dart `_nestLabelScopes`.
    fn nest_label_scopes(
        &mut self,
        ast: &Ast,
        outer: Option<usize>,
        labels: NodeList<dartr_ast::Label>,
        node: NodeId,
    ) -> Option<usize> {
        let mut current = outer;
        for &label in ast.list(labels) {
            if let Some(element) = self.declared_element(label.raw()) {
                current = Some(self.label_scopes.push(current, element, node));
            }
        }
        current
    }

    /// Dart `_lookupBreakOrContinueTarget`.
    fn lookup_break_or_continue_target(
        &mut self,
        ast: &Ast,
        parent_node: NodeId,
        label_node: Option<Id<LabelReference>>,
        is_continue: bool,
    ) -> Option<NodeId> {
        let Some(label_node) = label_node else {
            return if is_continue {
                self.unlabeled_break_continue_context.continue_target(ast)
            } else {
                self.unlabeled_break_continue_context.break_target()
            };
        };
        let label_name = ast.tokens.lexeme(ast[label_node].name).to_string();
        let Some(defining_scope) =
            self.label_scopes
                .lookup(&self.ctx, self.label_scope, &label_name)
        else {
            self.report_at_node(ast, diag::label_undefined(&label_name), label_node);
            return None;
        };
        self.tables
            .element
            .insert(label_node, ElemRef::Base(defining_scope.element));
        if let Some(enclosing_closure) = self.enclosing_closure {
            let label_container = self
                .first_fragment_data(defining_scope.element)
                .and_then(|f| f.enclosing_fragment);
            let closure_fragment = self
                .ctx
                .element_data(enclosing_closure)
                .map(|d| d.first_fragment);
            if label_container != closure_fragment {
                self.report_at_node(ast, diag::label_in_outer_scope(&label_name), label_node);
            }
        }
        let node = defining_scope.node;
        let is_loop_or_member = ast.is::<DoStatement>(node)
            || ast.is::<ForStatement>(node)
            || ast.is::<dartr_ast::SwitchMember>(node)
            || ast.is::<WhileStatement>(node);
        if is_continue && !is_loop_or_member {
            self.report_at_node(ast, diag::continue_label_invalid(), parent_node);
        }
        Some(node)
    }

    /// Dart `_resolveGuardedPattern`.
    fn resolve_guarded_pattern(
        &mut self,
        ast: &mut Ast,
        guarded_pattern: Id<GuardedPattern>,
        shared_case_scope_key: Option<JoinKey>,
        then: Option<&mut dyn FnMut(&mut Self, &mut Ast)>,
    ) {
        let (pattern, when_clause) = (
            ast[guarded_pattern].pattern,
            ast[guarded_pattern].when_clause,
        );
        let variables = self.compute_pattern_variables(ast, pattern.raw(), shared_case_scope_key);
        // Matched variables are available in `whenClause`.
        let scope = self.scope_context.push_local_scope();
        for &e in variables.values() {
            self.scope_context.add_local(scope, e);
        }
        self.rt
            .guarded_pattern_variables
            .insert(guarded_pattern, variables.clone());
        self.visit(ast, pattern);

        for &e in variables.values() {
            if let Some(p) = self.pattern_data(e) {
                p.is_visiting_when_clause.set(true);
            }
        }
        self.visit_opt(ast, when_clause);
        for &e in variables.values() {
            if let Some(p) = self.pattern_data(e) {
                p.is_visiting_when_clause.set(false);
            }
        }

        if let Some(then) = then {
            then(self, ast);
        }
        self.scope_context.pop();
    }

    /// Dart `_resolveType`: resolves [named_type] and reports a diagnostic
    /// when the type is not valid in the [clause] of the [declaration].
    pub(crate) fn resolve_type(
        &mut self,
        ast: &mut Ast,
        declaration: Option<NodeId>,
        clause: Option<NodeId>,
        named_type: Id<dartr_ast::NamedType>,
    ) {
        self.named_type_resolver.class_hierarchy_named_type = Some(named_type);
        self.visit_named_type_impl(ast, named_type);
        self.named_type_resolver.class_hierarchy_named_type = None;

        if self.named_type_resolver.has_error_reported {
            return;
        }

        let t = self.annotation_type(named_type);

        if let Some(enclosing) = self.named_type_resolver.enclosing_class {
            if let Some(extension_type) = enclosing.raw().cast::<ExtensionTypeElement>() {
                self.verify_extension_element_implements(ast, extension_type, named_type, t);
                return;
            }
        }

        // Dart `NamedType.isSynthetic`.
        let name_token = ast[named_type].name;
        if crate::ast_ext::token_is_synthetic(ast, name_token)
            && ast[named_type].type_arguments.is_none()
        {
            return;
        }

        let element = self.ctx.interface_element(t).map(|e| e.raw());
        match element.map(|e| e.tag()) {
            Some(Tag::Class) => return,
            Some(Tag::Mixin)
                if clause.is_some_and(|c| {
                    ast.is::<ImplementsClause>(c)
                        || ast.is::<MixinOnClause>(c)
                        || ast.is::<WithClause>(c)
                }) =>
            {
                return;
            }
            _ => {}
        }

        if should_ignore_undefined_named_type(
            &self.ctx,
            self.scope_context.library_scopes(),
            self.library_fragment(),
            ast,
            named_type,
        ) {
            return;
        }

        let diagnostic = match clause {
            None => declaration
                .filter(|&d| ast.is::<ClassTypeAlias>(d))
                .map(|_| diag::mixin_with_non_class_superclass()),
            Some(c) if ast.is::<ExtendsClause>(c) => declaration
                .and_then(|d| ast.cast::<ClassDeclaration>(d))
                .map(|d| {
                    if ast[d].with_clause.is_none() {
                        diag::extends_non_class()
                    } else {
                        diag::mixin_with_non_class_superclass()
                    }
                }),
            Some(c) if ast.is::<ImplementsClause>(c) => Some(diag::implements_non_class()),
            Some(c) if ast.is::<MixinOnClause>(c) => {
                Some(diag::mixin_super_class_constraint_non_interface())
            }
            Some(c) if ast.is::<WithClause>(c) => Some(diag::mixin_of_non_class()),
            Some(_) => None,
        };
        // Dart asserts that a code was found.
        let Some(diagnostic) = diagnostic else {
            return;
        };
        let first_token = ast[named_type]
            .import_prefix
            .map(|p| ast[p].name)
            .unwrap_or(name_token);
        let offset = ast.tokens.offset(first_token);
        let length = token_end(ast, name_token) - offset;
        self.report(diagnostic.at_offset(offset as usize, length as usize));
    }

    /// Dart `_resolveTypes`.
    fn resolve_types(
        &mut self,
        ast: &mut Ast,
        declaration: Option<NodeId>,
        clause: NodeId,
        named_types: NodeList<dartr_ast::NamedType>,
    ) {
        for named_type in ast.list(named_types).to_vec() {
            self.resolve_type(ast, declaration, Some(clause), named_type);
        }
    }

    /// Dart `_setExplicitFormalParameterType`.
    fn set_explicit_formal_parameter_type(&mut self, ast: &Ast, node: NodeId) -> Option<TypeId> {
        let parts = formal_parameter_parts(ast, node);
        let result = if let Some(suffix) = parts.function_typed_suffix {
            let type_parameters: Vec<EId<TypeParameterElement>> = ast[suffix]
                .type_parameters
                .map(|l| ast.list(ast[l].type_parameters).to_vec())
                .unwrap_or_default()
                .into_iter()
                .filter_map(|tp| self.declared_element(tp.raw()))
                .filter_map(|e| e.cast())
                .collect();
            let formal_parameters: Vec<EId<FormalParameterElement>> = ast
                .list(ast[ast[suffix].formal_parameters].parameters)
                .iter()
                .filter_map(|&p| self.declared_element(p.raw()))
                .filter_map(|e| e.cast())
                .collect();
            let return_type = parts
                .type_
                .map(|t| self.annotation_type(t))
                .unwrap_or(TypeId::DYNAMIC);
            let nullability = if ast[suffix].question.is_some() {
                Nullability::Question
            } else {
                Nullability::None
            };
            Some(self.function_type(
                &type_parameters,
                &formal_parameters,
                return_type,
                nullability,
            ))
        } else {
            parts
                .type_
                .and_then(|t| self.tables.annotation_type.get(t).copied())
        };
        match result {
            Some(t) => {
                self.tables.annotation_type.insert(node, t);
            }
            None => {
                self.tables.annotation_type.remove(node);
            }
        }
        result
    }

    /// Dart `FunctionTypeImpl(typeParameters:, formalParameters:,
    /// returnType:, nullabilitySuffix:)`.
    fn function_type(
        &self,
        type_parameters: &[EId<TypeParameterElement>],
        formal_parameters: &[EId<FormalParameterElement>],
        return_type: TypeId,
        nullability: Nullability,
    ) -> TypeId {
        let params: Vec<FnParam> = formal_parameters
            .iter()
            .map(|&p| formal_parameter_as_fn_param(&self.ctx, p))
            .collect();
        self.ctx
            .function_type(type_parameters, &params, return_type, nullability, None)
    }

    /// Dart `_verifyExtensionElementImplements`.
    fn verify_extension_element_implements(
        &mut self,
        ast: &Ast,
        declared_element: EId<ExtensionTypeElement>,
        node: Id<dartr_ast::NamedType>,
        t: TypeId,
    ) {
        let ctx = self.ctx;
        if !self.type_system.is_valid_extension_type_superinterface(t) {
            self.report_at_node(
                ast,
                diag::extension_type_implements_disallowed_type(type_arg(&ctx, t)),
                node,
            );
            return;
        }
        if ctx.get(declared_element).fields.is_empty() {
            return;
        }
        let declared_representation = ctx.extension_type_representation(declared_element);
        if self.type_system.is_subtype_of(declared_representation, t) {
            return;
        }

        // When `type` is an extension type.
        if let TypeKind::Interface { element, .. } = *ctx.ty(t) {
            let has_representation = element
                .raw()
                .cast::<ExtensionTypeElement>()
                .is_some_and(|e| !ctx.get(e).fields.is_empty());
            if has_representation {
                if let Some(implemented_representation) = ctx.representation_type(t) {
                    if !self
                        .type_system
                        .is_subtype_of(declared_representation, implemented_representation)
                    {
                        let implemented_name = ctx.element_name(element.raw()).unwrap_or("");
                        let declared_name = ctx.element_name(declared_element.raw()).unwrap_or("");
                        self.report_at_node(
                            ast,
                            diag::extension_type_implements_representation_not_supertype(
                                type_arg(&ctx, implemented_representation),
                                implemented_name,
                                type_arg(&ctx, declared_representation),
                                declared_name,
                            ),
                            node,
                        );
                    }
                    return;
                }
            }
        }

        self.report_at_node(
            ast,
            diag::extension_type_implements_not_supertype(
                type_arg(&ctx, t),
                type_arg(&ctx, declared_representation),
            ),
            node,
        );
    }

    /// Dart `_visitForLoopParts`.
    fn visit_for_loop_parts(&mut self, ast: &mut Ast, scope: usize, node: NodeId) {
        if let Some(n) = ast.cast::<ForEachPartsWithDeclaration>(node) {
            let (iterable, loop_variable) = (ast[n].iterable, ast[n].loop_variable);
            self.visit(ast, iterable);
            if let Some(e) = self.declared_element(loop_variable.raw()) {
                self.scope_context.add_local(scope, e);
            }
            self.visit(ast, loop_variable);
        } else if let Some(n) = ast.cast::<ForEachPartsWithIdentifier>(node) {
            let (iterable, identifier) = (ast[n].iterable, ast[n].identifier);
            self.visit(ast, iterable);
            self.visit(ast, identifier);
        } else if let Some(n) = ast.cast::<ForEachPartsWithPattern>(node) {
            let (iterable, pattern, metadata) = (ast[n].iterable, ast[n].pattern, ast[n].metadata);
            self.visit(ast, iterable);
            let variables = self.compute_declared_pattern_variables(ast, pattern.raw());
            self.rt
                .for_each_pattern_variables
                .insert(n, variables.clone());
            for e in variables {
                self.scope_context.add_local(scope, e);
            }
            self.visit(ast, pattern);
            self.visit_list(ast, metadata);
        } else if let Some(n) = ast.cast::<ForPartsWithDeclarations>(node) {
            let (variables, condition, updaters) =
                (ast[n].variables, ast[n].condition, ast[n].updaters);
            for e in self.declared_elements(ast, variables) {
                self.scope_context.add_local(scope, e);
            }
            self.visit(ast, variables);
            self.visit_opt(ast, condition);
            self.visit_list(ast, updaters);
        } else if let Some(n) = ast.cast::<ForPartsWithExpression>(node) {
            let (initialization, condition, updaters) =
                (ast[n].initialization, ast[n].condition, ast[n].updaters);
            self.visit_opt(ast, initialization);
            self.visit_opt(ast, condition);
            self.visit_list(ast, updaters);
        } else if let Some(n) = ast.cast::<ForPartsWithPattern>(node) {
            let (variables, condition, updaters) =
                (ast[n].variables, ast[n].condition, ast[n].updaters);
            self.define_pattern_variable_declaration_elements(ast, scope, variables);
            self.visit(ast, variables);
            self.visit_opt(ast, condition);
            self.visit_list(ast, updaters);
        }
    }

    /// Dart `visitNamedType`.
    fn visit_named_type_impl(&mut self, ast: &mut Ast, node: Id<dartr_ast::NamedType>) {
        let type_arguments = ast[node].type_arguments;
        self.visit_opt(ast, type_arguments);

        let mut env = NamedTypeEnv {
            ctx: self.ctx,
            type_system: self.type_system,
            scope: &self.scope_context,
            tables: &mut *self.tables,
            diagnostics: &mut *self.diagnostics,
        };
        self.named_type_resolver.resolve(&mut env, ast, node);

        if let Some(rewrite_result) = self.named_type_resolver.rewrite_result {
            self.visit(ast, rewrite_result);
        }
    }

    /// The function or method body visit of Dart `visitXFunctionBody`:
    /// `node.localVariableInfo = _localVariableInfo` is
    /// `ResolverTables.potentially_mutated_in_scope` (one per unit).
    fn visit_function_body(&mut self, ast: &mut Ast, node: NodeId) {
        self.visit_children(ast, node);
    }

    fn ast_rewriter<'r>(&'r mut self) -> AstRewriter<'r, 'a> {
        AstRewriter {
            ctx: self.ctx,
            scope: &self.scope_context,
            tables: &mut *self.tables,
            diagnostics: &mut *self.diagnostics,
        }
    }
}

// ------------------------------------------------------------------ visitor

impl AstVisitorMut for ResolutionVisitor<'_, '_> {
    fn visit_annotation(&mut self, ast: &mut Ast, node: Id<dartr_ast::Annotation>) {
        let constructor_name = ast[node].constructor_name.map(|c| c.raw());
        self.visit_children_except(ast, node.raw(), constructor_name);
    }

    fn visit_anonymous_method_invocation(
        &mut self,
        ast: &mut Ast,
        node: Id<AnonymousMethodInvocation>,
    ) {
        let (target, parameters, body) = (ast[node].target, ast[node].parameters, ast[node].body);
        self.visit_opt(ast, target);
        let scope = self.scope_context.push_local_scope();
        if let Some(parameters) = parameters {
            self.add_formal_parameter_list(ast, scope, parameters);
            self.visit(ast, parameters);
        }
        self.visit(ast, body);
        self.scope_context.pop();
    }

    fn visit_assigned_variable_pattern(
        &mut self,
        ast: &mut Ast,
        node: Id<AssignedVariablePattern>,
    ) {
        let name_token = ast[node].name;
        let name = ast.tokens.lexeme(name_token).to_string();
        let element = self.scope_context.lookup(&name).getter;
        match element {
            Some(e) => {
                self.tables.element.insert(node, ElemRef::Base(e));
            }
            None => {
                self.tables.element.remove(node);
            }
        }

        if let Some(e) = element {
            if crate::element_ext::is_promotable_element(e) {
                self.rt.potentially_mutated_in_scope.insert(e);
            }
        }

        match element {
            None => {
                // Recovery: the code might try to refer to an instance field.
                let enclosing = self
                    .scope_context
                    .enclosing_instance_element()
                    .and_then(|e| e.raw().cast::<InterfaceElement>());
                if let Some(enclosing) = enclosing {
                    let manager = dartr_typesystem::inheritance_manager3::InheritanceManager3::new(
                        self.ctx.global(),
                    );
                    let member_name = dartr_typesystem::inheritance_manager3::Name::for_library(
                        &self.ctx,
                        Some(self.library()),
                        &name,
                    )
                    .for_setter(&self.ctx);
                    if let Some(member) = manager.get_member(enclosing, member_name) {
                        self.tables.element.insert(node, member);
                        self.report_at_token(
                            ast,
                            diag::pattern_assignment_not_local_variable(),
                            name_token,
                        );
                        return;
                    }
                }
                self.report_at_token(ast, diag::undefined_identifier(&name), name_token);
            }
            Some(e) => {
                let is_local_variable_or_parameter =
                    crate::element_ext::is_local_variable(e) || e.is::<FormalParameterElement>();
                if !is_local_variable_or_parameter {
                    self.report_at_token(
                        ast,
                        diag::pattern_assignment_not_local_variable(),
                        name_token,
                    );
                }
            }
        }
    }

    fn visit_block(&mut self, ast: &mut Ast, node: Id<Block>) {
        self.visit_block_impl(ast, node);
    }

    fn visit_block_function_body(&mut self, ast: &mut Ast, node: Id<BlockFunctionBody>) {
        let previous = std::mem::replace(
            &mut self.unlabeled_break_continue_context,
            UnlabeledBreakContinueContext::root(),
        );
        self.visit_function_body(ast, node.raw());
        self.unlabeled_break_continue_context = previous;
    }

    fn visit_break_statement(&mut self, ast: &mut Ast, node: Id<BreakStatement>) {
        let label = ast[node].label;
        let target = self.lookup_break_or_continue_target(ast, node.raw(), label, false);
        match target {
            Some(t) => {
                self.rt.break_continue_target.insert(node, t);
            }
            None => {
                self.rt.break_continue_target.remove(node);
            }
        }
    }

    fn visit_catch_clause(&mut self, ast: &mut Ast, node: Id<CatchClause>) {
        let n = &ast[node];
        let (exception_type, exception_parameter, stack_trace_parameter, body) = (
            n.exception_type,
            n.exception_parameter,
            n.stack_trace_parameter,
            n.body,
        );
        self.visit_opt(ast, exception_type);

        let scope = self.scope_context.push_local_scope();
        if let Some(exception_node) = exception_parameter {
            if let Some(e) = self.declared_element(exception_node.raw()) {
                self.scope_context.add_local(scope, e);
                let t = match exception_type {
                    Some(type_node) => self.annotation_type(type_node),
                    None => self.ctx.tp.object_type(),
                };
                self.set_variable_type(e, t);
            }
        }
        if let Some(stack_trace_node) = stack_trace_parameter {
            if let Some(e) = self.declared_element(stack_trace_node.raw()) {
                self.scope_context.add_local(scope, e);
                self.set_variable_type(e, self.ctx.tp.stack_trace_type());
            }
        }
        self.visit(ast, body);
        self.scope_context.pop();
    }

    fn visit_class_declaration(&mut self, ast: &mut Ast, node: Id<ClassDeclaration>) {
        let element = self
            .declared_element(node.raw())
            .and_then(|e| e.cast::<InterfaceElement>());
        self.named_type_resolver.enclosing_class = element;
        self.scope_visit_class_declaration(ast, node);
        self.named_type_resolver.enclosing_class = None;
    }

    fn visit_class_type_alias(&mut self, ast: &mut Ast, node: Id<ClassTypeAlias>) {
        let element = self
            .declared_element(node.raw())
            .and_then(|e| e.cast::<InterfaceElement>());
        self.named_type_resolver.enclosing_class = element;
        self.scope_visit_class_type_alias(ast, node);
        self.named_type_resolver.enclosing_class = None;
    }

    fn visit_comment(&mut self, ast: &mut Ast, node: Id<Comment>) {
        self.scope_visit_documentation_comment(ast, node);
    }

    fn visit_constructor_declaration(&mut self, ast: &mut Ast, node: Id<ConstructorDeclaration>) {
        self.scope_visit_constructor_declaration(ast, node);
    }

    fn visit_constructor_field_initializer(
        &mut self,
        ast: &mut Ast,
        node: Id<ConstructorFieldInitializer>,
    ) {
        let field_name = ast[node].field_name.raw();
        self.visit_children_except(ast, node.raw(), Some(field_name));
    }

    fn visit_constructor_name(&mut self, ast: &mut Ast, node: Id<ConstructorName>) {
        let name = ast[node].name.map(|n| n.raw());
        self.visit_children_except(ast, node.raw(), name);
    }

    fn visit_continue_statement(&mut self, ast: &mut Ast, node: Id<ContinueStatement>) {
        let label = ast[node].label;
        let target = self.lookup_break_or_continue_target(ast, node.raw(), label, true);
        match target {
            Some(t) => {
                self.rt.break_continue_target.insert(node, t);
            }
            None => {
                self.rt.break_continue_target.remove(node);
            }
        }
    }

    fn visit_declared_identifier(&mut self, ast: &mut Ast, node: Id<DeclaredIdentifier>) {
        self.visit_children(ast, node.raw());
        if let Some(e) = self.declared_element(node.raw()) {
            let t = match ast[node].type_ {
                Some(type_node) => self.annotation_type(type_node),
                None => TypeId::DYNAMIC,
            };
            self.set_variable_type(e, t);
        }
    }

    fn visit_declared_variable_pattern(
        &mut self,
        ast: &mut Ast,
        node: Id<DeclaredVariablePattern>,
    ) {
        let type_node = ast[node].type_;
        self.visit_opt(ast, type_node);

        let Some(fragment) = self.declared_fragment(node.raw()) else {
            return;
        };
        let Some(e) = self.declared_element(node.raw()) else {
            return;
        };
        if !self.is_local(e) {
            return;
        }
        let Some(fragment_data) = self.ctx.fragment_data(fragment) else {
            return;
        };
        match type_node {
            Some(t) => {
                let t = self.annotation_type(t);
                self.set_variable_type(e, t);
            }
            None => {
                fragment_data
                    .flags
                    .set(FragmentFlags::VARIABLE_FRAGMENT_HAS_IMPLICIT_TYPE, true);
            }
        }

        let pattern_context = crate::ast_ext::pattern_context(ast, node.raw());
        let is_final =
            if let Some(c) = pattern_context.and_then(|c| ast.cast::<ForEachPartsWithPattern>(c)) {
                is_keyword(ast, Some(ast[c].keyword), "final")
            } else if let Some(c) =
                pattern_context.and_then(|c| ast.cast::<PatternVariableDeclaration>(c))
            {
                is_keyword(ast, Some(ast[c].keyword), "final")
            } else {
                is_keyword(ast, ast[node].keyword, "final")
            };
        fragment_data
            .flags
            .set(FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL, is_final);
    }

    fn visit_do_statement(&mut self, ast: &mut Ast, node: Id<DoStatement>) {
        let (body, condition) = (ast[node].body, ast[node].condition);
        let previous = self.nest_unlabeled_context(node.raw());
        self.visit_statement_in_scope(ast, Some(body.raw()));
        self.visit(ast, condition);
        self.unlabeled_break_continue_context = previous;
    }

    fn visit_dot_shorthand_constructor_invocation(
        &mut self,
        ast: &mut Ast,
        node: Id<DotShorthandConstructorInvocation>,
    ) {
        let skip = ast[node].constructor_name.raw();
        self.visit_children_except(ast, node.raw(), Some(skip));
    }

    fn visit_dot_shorthand_invocation(&mut self, ast: &mut Ast, node: Id<DotShorthandInvocation>) {
        let skip = ast[node].member_name.raw();
        self.visit_children_except(ast, node.raw(), Some(skip));
    }

    fn visit_dot_shorthand_property_access(
        &mut self,
        ast: &mut Ast,
        node: Id<DotShorthandPropertyAccess>,
    ) {
        let skip = ast[node].property_name.raw();
        self.visit_children_except(ast, node.raw(), Some(skip));
    }

    fn visit_empty_class_body(&mut self, _ast: &mut Ast, _node: Id<EmptyClassBody>) {}

    fn visit_empty_enum_body(&mut self, _ast: &mut Ast, _node: Id<EmptyEnumBody>) {}

    fn visit_enum_declaration(&mut self, ast: &mut Ast, node: Id<EnumDeclaration>) {
        let element = self
            .declared_element(node.raw())
            .and_then(|e| e.cast::<InterfaceElement>());
        self.named_type_resolver.enclosing_class = element;
        self.scope_visit_enum_declaration(ast, node);
        self.named_type_resolver.enclosing_class = None;
    }

    fn visit_extends_clause(&mut self, ast: &mut Ast, node: Id<ExtendsClause>) {
        let superclass = ast[node].superclass;
        let declaration = ast.parent(node);
        self.resolve_type(ast, declaration, Some(node.raw()), superclass);
    }

    fn visit_extension_declaration(&mut self, ast: &mut Ast, node: Id<ExtensionDeclaration>) {
        self.scope_visit_extension_declaration(ast, node);
    }

    fn visit_extension_type_declaration(
        &mut self,
        ast: &mut Ast,
        node: Id<ExtensionTypeDeclaration>,
    ) {
        let element = self
            .declared_element(node.raw())
            .and_then(|e| e.cast::<InterfaceElement>());
        self.named_type_resolver.enclosing_class = element;
        self.scope_visit_extension_type_declaration(ast, node);
        self.named_type_resolver.enclosing_class = None;
    }

    fn visit_field_declaration(&mut self, ast: &mut Ast, node: Id<FieldDeclaration>) {
        self.scope_visit_field_declaration(ast, node);
    }

    fn visit_field_formal_parameter(&mut self, ast: &mut Ast, node: Id<FieldFormalParameter>) {
        self.scope_visit_formal_parameter(ast, node.raw());
        self.set_explicit_formal_parameter_type(ast, node.raw());
    }

    // Dart throws `StateError('Should not be invoked')` in the next
    // methods: the parents visit these nodes. Visit the children to stay
    // robust.
    fn visit_for_each_parts_with_declaration(
        &mut self,
        ast: &mut Ast,
        node: Id<ForEachPartsWithDeclaration>,
    ) {
        self.visit_children(ast, node.raw());
    }

    fn visit_for_each_parts_with_pattern(
        &mut self,
        ast: &mut Ast,
        node: Id<ForEachPartsWithPattern>,
    ) {
        self.visit_children(ast, node.raw());
    }

    fn visit_for_parts_with_pattern(&mut self, ast: &mut Ast, node: Id<ForPartsWithPattern>) {
        self.visit_children(ast, node.raw());
    }

    fn visit_for_element(&mut self, ast: &mut Ast, node: Id<ForElement>) {
        let (parts, body) = (ast[node].for_loop_parts, ast[node].body);
        let scope = self.scope_context.push_local_scope();
        self.visit_for_loop_parts(ast, scope, parts.raw());
        self.scope_context.push_local_scope();
        self.visit(ast, body);
        self.scope_context.pop();
        self.scope_context.pop();
    }

    fn visit_for_statement(&mut self, ast: &mut Ast, node: Id<ForStatement>) {
        let (parts, body) = (ast[node].for_loop_parts, ast[node].body);
        let previous = self.nest_unlabeled_context(node.raw());
        let scope = self.scope_context.push_local_scope();
        self.visit_for_loop_parts(ast, scope, parts.raw());
        self.visit_statement_in_scope(ast, Some(body.raw()));
        self.scope_context.pop();
        self.unlabeled_break_continue_context = previous;
    }

    fn visit_function_declaration(&mut self, ast: &mut Ast, node: Id<FunctionDeclaration>) {
        let element = self.declared_element(node.raw());
        let closure = element.filter(|e| e.tag() == Tag::LocalFunction);
        let previous = std::mem::replace(&mut self.enclosing_closure, closure);
        self.scope_visit_function_declaration(ast, node);
        self.enclosing_closure = previous;

        if let Some(e) = closure {
            if self.is_local(e) {
                let return_type = ast[node]
                    .return_type
                    .map(|t| self.annotation_type(t))
                    .unwrap_or(TypeId::DYNAMIC);
                if let AnyElement::LocalFunction(f) = self.ctx.any(e) {
                    f.return_type.set(Some(return_type));
                }
            }
        }
    }

    fn visit_function_expression(&mut self, ast: &mut Ast, node: Id<FunctionExpression>) {
        let element = self.declared_element(node.raw());
        let closure = element.filter(|e| e.tag() == Tag::LocalFunction);
        let previous = std::mem::replace(&mut self.enclosing_closure, closure);
        self.scope_visit_function_expression(ast, node);
        self.enclosing_closure = previous;
    }

    fn visit_function_type_alias(&mut self, ast: &mut Ast, node: Id<FunctionTypeAlias>) {
        self.scope_visit_function_type_alias(ast, node);
    }

    fn visit_generic_function_type(&mut self, ast: &mut Ast, node: Id<GenericFunctionType>) {
        self.scope_visit_generic_function_type(ast, node);

        let return_type = ast[node]
            .return_type
            .map(|t| self.annotation_type(t))
            .unwrap_or(TypeId::DYNAMIC);
        let nullability = if ast[node].question.is_some() {
            Nullability::Question
        } else {
            Nullability::None
        };
        let element = self
            .declared_element(node.raw())
            .and_then(|e| e.cast::<GenericFunctionTypeElement>());
        let t = match element {
            Some(e) => {
                let data = self.ctx.get(e);
                let t = self.function_type(
                    &data.type_params,
                    &data.formal_params,
                    return_type,
                    nullability,
                );
                if self.is_local(e.raw()) {
                    data.return_type.set(Some(return_type));
                    data.type_.set(Some(t));
                }
                t
            }
            None => self.function_type(&[], &[], return_type, nullability),
        };
        self.tables.annotation_type.insert(node, t);
    }

    fn visit_generic_type_alias(&mut self, ast: &mut Ast, node: Id<GenericTypeAlias>) {
        self.scope_visit_generic_type_alias(ast, node);
    }

    fn visit_if_element(&mut self, ast: &mut Ast, node: Id<IfElement>) {
        let n = &ast[node];
        let (expression, case_clause, then_element, else_element) =
            (n.expression, n.case_clause, n.then_element, n.else_element);
        match case_clause {
            Some(case_clause) => {
                self.visit(ast, expression);
                let guarded_pattern = ast[case_clause].guarded_pattern;
                let mut then = |v: &mut Self, ast: &mut Ast| v.visit(ast, then_element);
                self.resolve_guarded_pattern(ast, guarded_pattern, None, Some(&mut then));
                self.visit_opt(ast, else_element);
            }
            None => self.visit_children(ast, node.raw()),
        }
    }

    fn visit_if_statement(&mut self, ast: &mut Ast, node: Id<IfStatement>) {
        let n = &ast[node];
        let (expression, case_clause, then_statement, else_statement) = (
            n.expression,
            n.case_clause,
            n.then_statement,
            n.else_statement,
        );
        self.visit(ast, expression);
        match case_clause {
            Some(case_clause) => {
                let guarded_pattern = ast[case_clause].guarded_pattern;
                let mut then = |v: &mut Self, ast: &mut Ast| {
                    v.visit_statement_in_scope(ast, Some(then_statement.raw()));
                };
                self.resolve_guarded_pattern(ast, guarded_pattern, None, Some(&mut then));
            }
            None => {
                self.visit_statement_in_scope(ast, Some(then_statement.raw()));
            }
        }
        self.visit_statement_in_scope(ast, else_statement.map(|s| s.raw()));
    }

    fn visit_implements_clause(&mut self, ast: &mut Ast, node: Id<ImplementsClause>) {
        let declaration = ast.parent(node);
        let interfaces = ast[node].interfaces;
        self.resolve_types(ast, declaration, node.raw(), interfaces);
    }

    fn visit_import_directive(&mut self, ast: &mut Ast, node: Id<ImportDirective>) {
        // The annotations were bound by the element binding visitor (Dart
        // `_setElementAnnotations`).
        let prefix = ast[node].prefix.map(|p| p.raw());
        self.visit_children_except(ast, node.raw(), prefix);
    }

    fn visit_instance_creation_expression(
        &mut self,
        ast: &mut Ast,
        node: Id<InstanceCreationExpression>,
    ) {
        let library = self.library();
        let enclosing = self.scope_context.enclosing_instance_element();
        let had_type_arguments = {
            let type_node = ast[ast[node].constructor_name].type_;
            ast[type_node].type_arguments.is_some()
        };
        let new_node = self
            .ast_rewriter()
            .instance_creation_expression(ast, node, library, enclosing);
        if new_node != node.raw() {
            if had_type_arguments
                && !library_feature_enabled(
                    &self.ctx,
                    library,
                    ExperimentalFlag::ConstructorTearoffs,
                )
            {
                let is_function_reference_call = ast
                    .cast::<MethodInvocation>(new_node)
                    .and_then(|m| ast[m].target)
                    .is_some_and(|t| ast.is::<dartr_ast::FunctionReference>(t));
                if is_function_reference_call {
                    // `a<...>.m(...)` where `a` is not a class or a type
                    // alias needs the constructor tear-offs feature.
                    self.report_at_node(ast, diag::sdk_version_constructor_tearoffs(), node);
                }
            }
            self.visit(ast, new_node);
            return;
        }
        self.visit_children(ast, node.raw());
    }

    fn visit_labeled_statement(&mut self, ast: &mut Ast, node: Id<LabeledStatement>) {
        let unlabeled = statement_unlabeled(ast, node.raw());
        let labels = ast[node].labels;
        let label_scope = self.nest_label_scopes(ast, self.label_scope, labels, unlabeled);
        let previous = std::mem::replace(&mut self.label_scope, label_scope);
        self.visit(ast, unlabeled);
        self.label_scope = previous;
    }

    fn visit_library_directive(&mut self, ast: &mut Ast, node: Id<LibraryDirective>) {
        self.library_directive_index += 1;
        self.visit_children(ast, node.raw());
    }

    fn visit_method_declaration(&mut self, ast: &mut Ast, node: Id<MethodDeclaration>) {
        self.scope_visit_method_declaration(ast, node);
    }

    fn visit_method_invocation(&mut self, ast: &mut Ast, node: Id<MethodInvocation>) {
        let new_node = self.ast_rewriter().method_invocation(ast, node);
        if new_node != node.raw() {
            self.visit(ast, new_node);
            return;
        }
        let method_name = ast[node].method_name.raw();
        if method_invocation_real_target(ast, node).is_none() {
            self.visit_children(ast, node.raw());
        } else {
            self.visit_children_except(ast, node.raw(), Some(method_name));
        }
    }

    fn visit_mixin_declaration(&mut self, ast: &mut Ast, node: Id<MixinDeclaration>) {
        self.scope_visit_mixin_declaration(ast, node);
    }

    fn visit_mixin_on_clause(&mut self, ast: &mut Ast, node: Id<MixinOnClause>) {
        let declaration = ast.parent(node);
        let constraints = ast[node].superclass_constraints;
        self.resolve_types(ast, declaration, node.raw(), constraints);
    }

    fn visit_named_type(&mut self, ast: &mut Ast, node: Id<dartr_ast::NamedType>) {
        self.visit_named_type_impl(ast, node);
    }

    fn visit_pattern_assignment(&mut self, ast: &mut Ast, node: Id<PatternAssignment>) {
        let (pattern, expression) = (ast[node].pattern, ast[node].expression);
        let scope = self.scope_context.push_local_scope();
        let variables = self.compute_declared_pattern_variables(ast, pattern.raw());
        for e in variables {
            self.scope_context.add_local(scope, e);
        }
        self.visit(ast, pattern);
        self.visit(ast, expression);
        self.scope_context.pop();
    }

    fn visit_pattern_variable_declaration_statement(
        &mut self,
        ast: &mut Ast,
        node: Id<PatternVariableDeclarationStatement>,
    ) {
        let declaration = ast[node].declaration;
        self.visit(ast, declaration);
    }

    fn visit_prefixed_identifier(&mut self, ast: &mut Ast, node: Id<PrefixedIdentifier>) {
        let new_node = self.ast_rewriter().prefixed_identifier_node(ast, node);
        if new_node != node.raw() {
            self.visit(ast, new_node);
            return;
        }
        let identifier = ast[node].identifier.raw();
        self.visit_children_except(ast, node.raw(), Some(identifier));
    }

    fn visit_primary_constructor_body(&mut self, ast: &mut Ast, node: Id<PrimaryConstructorBody>) {
        self.scope_visit_primary_constructor_body(ast, node);
    }

    fn visit_this_expression(&mut self, ast: &mut Ast, node: Id<dartr_ast::ThisExpression>) {
        // For the linter `unnecessary_this` (`resolveNameInScope`).
        let name = match ast.parent(node) {
            Some(parent) if ast.kind(parent) == dartr_ast::NodeKind::PropertyAccess => {
                let name = ast[Id::<PropertyAccess>::from_raw(parent)].property_name;
                Some(ast.tokens.lexeme(ast[name].token).to_string())
            }
            Some(parent) if ast.kind(parent) == dartr_ast::NodeKind::MethodInvocation => {
                let name = ast[Id::<MethodInvocation>::from_raw(parent)].method_name;
                Some(ast.tokens.lexeme(ast[name].token).to_string())
            }
            _ => None,
        };
        if let Some(name) = name {
            self.scope_context.record_lookup(node.raw(), &name);
        }
    }

    fn visit_property_access(&mut self, ast: &mut Ast, node: Id<PropertyAccess>) {
        let new_node = self.ast_rewriter().property_access(ast, node);
        if new_node != node.raw() {
            self.visit(ast, new_node);
            return;
        }
        let property_name = ast[node].property_name.raw();
        self.visit_children_except(ast, node.raw(), Some(property_name));
    }

    fn visit_record_type_annotation(&mut self, ast: &mut Ast, node: Id<RecordTypeAnnotation>) {
        self.visit_children(ast, node.raw());
        let mut resolver = RecordTypeAnnotationResolver {
            ctx: self.ctx,
            wildcard_variables: self.wildcard_variables,
            source_path: &self.source_path,
            source_uri: &self.source_uri,
            tables: &mut *self.tables,
            diagnostics: &mut *self.diagnostics,
        };
        resolver.resolve(ast, node);
    }

    fn visit_redirecting_constructor_invocation(
        &mut self,
        ast: &mut Ast,
        node: Id<RedirectingConstructorInvocation>,
    ) {
        let skip = ast[node].constructor_name.map(|c| c.raw());
        self.visit_children_except(ast, node.raw(), skip);
    }

    fn visit_regular_formal_parameter(&mut self, ast: &mut Ast, node: Id<RegularFormalParameter>) {
        let (suffix, type_node) = (ast[node].function_typed_suffix, ast[node].type_);
        if suffix.is_some() {
            self.scope_visit_formal_parameter(ast, node.raw());
        } else {
            self.visit_children(ast, node.raw());
        }

        let explicit_fragment_type = self.set_explicit_formal_parameter_type(ast, node.raw());

        let Some(fragment) = self.declared_fragment(node.raw()) else {
            return;
        };
        let Some(e) = self.declared_element(node.raw()) else {
            return;
        };
        let is_first = self
            .ctx
            .fragment_data(fragment)
            .is_some_and(|f| f.previous_fragment.is_none());
        if is_first {
            if let Some(t) = explicit_fragment_type {
                self.set_variable_type(e, t);
            } else if type_node.is_some() {
                self.set_variable_type(e, TypeId::DYNAMIC);
            } else if self.variable_type(e) == TypeId::INVALID {
                // Dart: identical(type, InvalidTypeImpl.instance).
                self.set_variable_type(e, TypeId::DYNAMIC);
            }
        }
    }

    fn visit_simple_identifier(&mut self, ast: &mut Ast, node: Id<SimpleIdentifier>) {
        let new_node = self.ast_rewriter().simple_identifier_node(ast, node);
        if new_node != node.raw() {
            self.visit(ast, new_node);
            return;
        }

        let name = identifier_name(ast, node).to_string();
        let scope_lookup_result = self.scope_context.lookup(&name);
        self.rt
            .scope_lookup_result
            .insert(node, scope_lookup_result);

        let Some(element) = scope_lookup_result.getter else {
            return;
        };
        if !crate::element_ext::is_promotable_element(element) {
            return;
        }
        self.tables.element.insert(node, ElemRef::Base(element));

        if element.tag() == Tag::JoinPatternVariable {
            if let Some(p) = self.pattern_data(element) {
                p.references.lock().push(node.raw());
            }
        }

        if simple_identifier_in_setter_context(ast, node) {
            self.rt.potentially_mutated_in_scope.insert(element);
            let is_pattern_variable = matches!(
                element.tag(),
                Tag::PatternVariable | Tag::BindPatternVariable | Tag::JoinPatternVariable
            );
            if is_pattern_variable
                && self
                    .pattern_data(element)
                    .is_some_and(|p| p.is_visiting_when_clause.get())
            {
                self.report_at_node(ast, diag::pattern_variable_assignment_inside_guard(), node);
            }
        }
    }

    fn visit_super_constructor_invocation(
        &mut self,
        ast: &mut Ast,
        node: Id<SuperConstructorInvocation>,
    ) {
        let skip = ast[node].constructor_name.map(|c| c.raw());
        self.visit_children_except(ast, node.raw(), skip);
    }

    fn visit_super_formal_parameter(&mut self, ast: &mut Ast, node: Id<SuperFormalParameter>) {
        self.scope_visit_formal_parameter(ast, node.raw());
        self.set_explicit_formal_parameter_type(ast, node.raw());
    }

    fn visit_switch_expression(&mut self, ast: &mut Ast, node: Id<SwitchExpression>) {
        let (expression, cases) = (ast[node].expression, ast[node].cases);
        self.visit(ast, expression);
        for case in ast.list(cases).to_vec() {
            let (guarded_pattern, case_expression) =
                (ast[case].guarded_pattern, ast[case].expression);
            let mut then = |v: &mut Self, ast: &mut Ast| v.visit(ast, case_expression);
            self.resolve_guarded_pattern(ast, guarded_pattern, None, Some(&mut then));
        }
    }

    fn visit_switch_statement(&mut self, ast: &mut Ast, node: Id<SwitchStatement>) {
        let (expression, members) = (ast[node].expression, ast[node].members);
        let members: Vec<NodeId> = ast.list_raw(members).to_vec();

        let mut label_scope = self.label_scope;
        for &member in &members {
            if let Some(labels) = switch_member_labels(ast, member) {
                label_scope = self.nest_label_scopes(ast, label_scope, labels, member);
            }
        }

        let previous_context = self.nest_unlabeled_context(node.raw());
        let previous_label_scope = std::mem::replace(&mut self.label_scope, label_scope);

        self.visit(ast, expression);

        for group in member_groups(ast, &members) {
            let last = *group.members.last().expect("a group has members");
            let key = JoinKey::SharedCase(last);
            self.pattern_variables
                .switch_statement_shared_case_scope_start(key);
            for &member in &group.members {
                if let Some(m) = ast.cast::<dartr_ast::SwitchCase>(member) {
                    let e = ast[m].expression;
                    self.visit(ast, e);
                } else if ast.is::<dartr_ast::SwitchDefault>(member) {
                    self.pattern_variables
                        .switch_statement_shared_case_scope_empty(&key);
                } else if let Some(m) = ast.cast::<dartr_ast::SwitchPatternCase>(member) {
                    let guarded_pattern = ast[m].guarded_pattern;
                    self.resolve_guarded_pattern(ast, guarded_pattern, Some(key), None);
                }
            }
            if group.has_labels {
                self.pattern_variables
                    .switch_statement_shared_case_scope_empty(&key);
            }
            let variables = {
                let mut state = std::mem::take(&mut self.pattern_variables);
                let mut binder = JoinBuilder { ctx: self.ctx };
                let variables = state.switch_statement_shared_case_scope_finish(&mut binder, key);
                self.pattern_variables = state;
                variables
            };
            self.rt
                .switch_case_group_variables
                .insert(last, variables.clone());

            let scope = self.scope_context.push_local_scope();
            let statements = switch_member_statements(ast, last);
            let statement_ids: Vec<NodeId> = statements
                .map(|s| ast.list_raw(s).to_vec())
                .unwrap_or_default();
            self.define_local_elements(ast, scope, &statement_ids);
            for &e in variables.values() {
                self.scope_context.add_local(scope, e);
            }
            for s in statement_ids {
                self.visit(ast, s);
            }
            self.scope_context.pop();
        }

        self.label_scope = previous_label_scope;
        self.unlabeled_break_continue_context = previous_context;
    }

    fn visit_type_parameter(&mut self, ast: &mut Ast, node: Id<TypeParameter>) {
        let (metadata, bound) = (ast[node].metadata, ast[node].bound);
        self.visit_list(ast, metadata);
        if let Some(bound) = bound {
            self.visit(ast, bound);
            let Some(fragment) = self.declared_fragment(node.raw()) else {
                return;
            };
            let is_first = self
                .ctx
                .fragment_data(fragment)
                .is_some_and(|f| f.previous_fragment.is_none());
            if is_first {
                if let Some(e) = self
                    .declared_element(node.raw())
                    .and_then(|e| e.cast::<TypeParameterElement>())
                {
                    if self.is_local(e.raw()) {
                        let t = self.annotation_type(bound);
                        self.ctx.get(e).bound.set(Some(t));
                    }
                }
            }
        }
    }

    fn visit_variable_declaration(&mut self, ast: &mut Ast, node: Id<VariableDeclaration>) {
        if let Some(e) = self.declared_element(node.raw()) {
            if e.is::<LocalVariableElement>() {
                let list = ast
                    .parent(node)
                    .and_then(|p| ast.cast::<VariableDeclarationList>(p));
                let t = match list.and_then(|l| ast[l].type_) {
                    Some(type_node) => self.annotation_type(type_node),
                    None => TypeId::DYNAMIC,
                };
                self.set_variable_type(e, t);
            }
        }
        let initializer = ast[node].initializer;
        self.visit_opt(ast, initializer);
    }

    fn visit_variable_declaration_list(
        &mut self,
        ast: &mut Ast,
        node: Id<VariableDeclarationList>,
    ) {
        self.scope_visit_variable_declaration_list(ast, node);
    }

    fn visit_while_statement(&mut self, ast: &mut Ast, node: Id<WhileStatement>) {
        let (condition, body) = (ast[node].condition, ast[node].body);
        let previous = self.nest_unlabeled_context(node.raw());
        self.visit(ast, condition);
        self.visit_statement_in_scope(ast, Some(body.raw()));
        self.unlabeled_break_continue_context = previous;
    }

    fn visit_with_clause(&mut self, ast: &mut Ast, node: Id<WithClause>) {
        let declaration = ast.parent(node);
        for named_type in ast.list(ast[node].mixin_types).to_vec() {
            self.named_type_resolver.with_clause_named_type = Some(named_type);
            self.resolve_type(ast, declaration, Some(node.raw()), named_type);
            self.named_type_resolver.with_clause_named_type = None;
        }
    }
}

// ------------------------------------------------------------------ switch groups

/// Dart `SwitchStatementCaseGroup`.
struct SwitchStatementCaseGroup {
    members: Vec<NodeId>,
    has_labels: bool,
}

/// Dart `SwitchStatementImpl._computeMemberGroups`.
fn member_groups(ast: &Ast, members: &[NodeId]) -> Vec<SwitchStatementCaseGroup> {
    let mut groups = Vec::new();
    let mut group_members = Vec::new();
    let mut group_has_labels = false;
    for &member in members {
        group_members.push(member);
        group_has_labels |= switch_member_labels(ast, member).is_some_and(|l| !l.is_empty());
        let has_statements = switch_member_statements(ast, member).is_some_and(|s| !s.is_empty());
        if has_statements {
            groups.push(SwitchStatementCaseGroup {
                members: std::mem::take(&mut group_members),
                has_labels: group_has_labels,
            });
            group_has_labels = false;
        }
    }
    if !group_members.is_empty() {
        groups.push(SwitchStatementCaseGroup {
            members: group_members,
            has_labels: group_has_labels,
        });
    }
    groups
}

/// Dart `SwitchMember.statements`.
fn switch_member_statements(ast: &Ast, member: NodeId) -> Option<NodeList<dartr_ast::Statement>> {
    if let Some(m) = ast.cast::<dartr_ast::SwitchCase>(member) {
        Some(ast[m].statements)
    } else if let Some(m) = ast.cast::<dartr_ast::SwitchDefault>(member) {
        Some(ast[m].statements)
    } else {
        ast.cast::<dartr_ast::SwitchPatternCase>(member)
            .map(|m| ast[m].statements)
    }
}

// ------------------------------------------------------------------ pattern variables

/// Dart `_VariableBinder.joinPatternVariables`: creates join pattern
/// variables in the local arena.
struct JoinBuilder<'a> {
    ctx: Ctx<'a>,
}

impl VariableBinder for JoinBuilder<'_> {
    type Node = NodeId;
    type Variable = ElementId;
    type Key = JoinKey;

    fn join_pattern_variables(
        &mut self,
        key: JoinKey,
        components: Vec<ElementId>,
        inconsistency: JoinedPatternVariableInconsistency,
    ) -> ElementId {
        let ctx = self.ctx;
        let first = components[0];
        let pattern_data = |e: ElementId| -> Option<&PatternVariableFragmentData> {
            let v = e.cast::<LocalVariableElement>()?;
            Some(&ctx.fragment(ctx.get(v).first_fragment()).pattern)
        };
        let first_fragment =
            |e: ElementId| -> Option<FragmentId> { ctx.element_data(e).map(|d| d.first_fragment) };
        let expanded: Vec<ElementId> = match key {
            JoinKey::LogicalOr(_) => {
                let mut result = Vec::new();
                for &c in &components {
                    if c.tag() == Tag::JoinPatternVariable {
                        let variables = pattern_data(c)
                            .map(|p| p.variables.clone())
                            .unwrap_or_default();
                        for v in variables {
                            if let Some(e) =
                                ctx.fragment_data(v.raw()).and_then(|f| f.element.try_get())
                            {
                                result.push(*e);
                            }
                        }
                    } else {
                        result.push(c);
                    }
                }
                result
            }
            JoinKey::SharedCase(_) => components.clone(),
        };

        // Dart `inconsistency.maxWithAll(components.whereType<Join>()...)`.
        let mut result_inconsistency = inconsistency;
        for &c in &components {
            if c.tag() == Tag::JoinPatternVariable {
                if let Some(p) = pattern_data(c) {
                    let other =
                        from_element_inconsistency(p.inconsistency.get().unwrap_or_default());
                    result_inconsistency = result_inconsistency.max_with(other);
                }
            }
        }

        let name = ctx.element_data(first).and_then(|d| d.name);
        let enclosing_fragment = first_fragment(first)
            .and_then(|f| ctx.fragment_data(f))
            .and_then(|f| f.enclosing_fragment);
        let mut fragment_data = FragmentData::new(name, None);
        fragment_data.enclosing_fragment = enclosing_fragment;
        let variables: Vec<FId<PatternVariableFragment>> = expanded
            .iter()
            .filter_map(|&e| first_fragment(e))
            .map(FId::from_raw)
            .collect();
        let pattern = PatternVariableFragmentData {
            variables: variables.clone(),
            inconsistency: VarSlot::with(to_element_inconsistency(result_inconsistency)),
            ..PatternVariableFragmentData::default()
        };
        let store = &ctx
            .local
            .expect("pattern variables need a context with a local arena")
            .store;
        let fragment = store.add_fragment::<JoinPatternVariableFragment>(LocalVariableFragment {
            variable: VariableFragmentData::new(fragment_data),
            pattern,
        });
        for v in &variables {
            ctx.fragment(*v).pattern.join.set(Some(fragment));
        }
        let library = ctx.element_data(first).and_then(|d| d.library);
        let enclosing = enclosing_fragment
            .and_then(|f| ctx.fragment_data(f))
            .and_then(|f| f.element.try_get().copied());
        let mut element_data = ElementData::new(name, fragment.raw());
        element_data.library = library;
        element_data.enclosing = enclosing;
        let element = store.add::<JoinPatternVariableElement>(LocalVariableElement {
            variable: VariableElementData::new(element_data),
            type_: VarSlot::with(TypeId::INVALID),
        });
        store.fragment(fragment).element.set_once(element.raw());
        element.raw()
    }
}

fn to_element_inconsistency(
    i: JoinedPatternVariableInconsistency,
) -> dartr_element::JoinedPatternVariableInconsistency {
    use dartr_element::JoinedPatternVariableInconsistency as E;
    match i {
        JoinedPatternVariableInconsistency::None => E::None,
        JoinedPatternVariableInconsistency::LogicalOr => E::LogicalOr,
        JoinedPatternVariableInconsistency::SharedCaseAbsent => E::SharedCaseAbsent,
        JoinedPatternVariableInconsistency::SharedCaseHasLabel => E::SharedCaseHasLabel,
        JoinedPatternVariableInconsistency::DifferentFinalityOrType => E::DifferentFinalityOrType,
    }
}

fn from_element_inconsistency(
    i: dartr_element::JoinedPatternVariableInconsistency,
) -> JoinedPatternVariableInconsistency {
    use dartr_element::JoinedPatternVariableInconsistency as E;
    match i {
        E::None => JoinedPatternVariableInconsistency::None,
        E::LogicalOr => JoinedPatternVariableInconsistency::LogicalOr,
        E::SharedCaseAbsent => JoinedPatternVariableInconsistency::SharedCaseAbsent,
        E::SharedCaseHasLabel => JoinedPatternVariableInconsistency::SharedCaseHasLabel,
        E::DifferentFinalityOrType => JoinedPatternVariableInconsistency::DifferentFinalityOrType,
    }
}

/// Dart `_VariableBinderErrors`.
struct BinderErrors<'r, 'a> {
    ctx: Ctx<'a>,
    ast: &'r Ast,
    diagnostics: &'r mut Vec<Diagnostic>,
    source_path: &'r str,
}

impl BinderErrors<'_, '_> {
    /// The name token of the declared variable pattern of a bind pattern
    /// variable (Dart `element.node.name`).
    fn name_token(&self, e: ElementId) -> Option<dartr_syntax::TokenId> {
        let v = e.cast::<LocalVariableElement>()?;
        let node = self
            .ctx
            .fragment(self.ctx.get(v).first_fragment())
            .pattern
            .node?;
        let pattern = self.ast.cast::<DeclaredVariablePattern>(node)?;
        Some(self.ast[pattern].name)
    }
}

impl TypeAnalyzerErrorsBase for BinderErrors<'_, '_> {
    fn assert_in_error_recovery(&mut self) {
        // Dart: `throw UnimplementedError()`; the binder does not call it.
    }
}

impl VariableBinderErrors for BinderErrors<'_, '_> {
    type Node = NodeId;
    type Variable = ElementId;
    type Name = Name;

    fn duplicate_variable_pattern(
        &mut self,
        name: Name,
        original: ElementId,
        duplicate: ElementId,
    ) {
        let ast = self.ast;
        let (Some(duplicate_token), Some(original_token)) =
            (self.name_token(duplicate), self.name_token(original))
        else {
            return;
        };
        // Dart `DiagnosticFactory.duplicateDefinitionForNodes`.
        let original_offset = ast.tokens.offset(original_token);
        let original_length = token_end(ast, original_token) - original_offset;
        let duplicate_offset = ast.tokens.offset(duplicate_token);
        let duplicate_length = token_end(ast, duplicate_token) - duplicate_offset;
        let d = diag::duplicate_variable_pattern(self.ctx.name_str(name))
            .with_context_messages([DiagnosticMessage {
                file_path: self.source_path.to_string(),
                offset: original_offset as i64,
                length: original_length as i64,
                message: "The first definition of this name.".to_string(),
                url: None,
            }])
            .at_offset(duplicate_offset as usize, duplicate_length as usize);
        self.diagnostics.push(d.into_diagnostic());
        if let Some(v) = duplicate.cast::<LocalVariableElement>() {
            let first = self.ctx.get(v).first_fragment();
            self.ctx.fragment(first).pattern.is_duplicate.set(true);
        }
    }

    fn logical_or_pattern_branch_missing_variable(
        &mut self,
        node: NodeId,
        has_in_left: bool,
        name: Name,
        _variable: ElementId,
    ) {
        let ast = self.ast;
        let Some(or_pattern) = ast.cast::<dartr_ast::LogicalOrPattern>(node) else {
            return;
        };
        let target = if has_in_left {
            ast[or_pattern].right_operand
        } else {
            ast[or_pattern].left_operand
        };
        let d = diag::missing_variable_pattern(self.ctx.name_str(name))
            .at_offset(ast.offset(target) as usize, ast.length(target) as usize);
        self.diagnostics.push(d.into_diagnostic());
    }
}

/// Dart `_PatternVariableBinderVisitor`: adds the variables that [node]
/// declares to the binder.
fn bind_pattern_variables(
    ctx: &Ctx<'_>,
    declared_fragment: &dartr_ast::NodeMap<FragmentId>,
    ast: &Ast,
    node: NodeId,
    state: &mut VariableBinderState<Name, ElementId, JoinKey>,
    binder: &mut JoinBuilder<'_>,
    errors: &mut BinderErrors<'_, '_>,
) {
    use dartr_ast::NodeKind as K;
    let visit = |n: NodeId,
                 state: &mut VariableBinderState<Name, ElementId, JoinKey>,
                 binder: &mut JoinBuilder<'_>,
                 errors: &mut BinderErrors<'_, '_>| {
        bind_pattern_variables(ctx, declared_fragment, ast, n, state, binder, errors);
    };
    match ast.kind(node) {
        K::AssignedVariablePattern
        | K::ConstantPattern
        | K::RelationalPattern
        | K::WildcardPattern => {}
        K::CastPattern => {
            let p = ast[Id::<dartr_ast::CastPattern>::from_raw(node)].pattern;
            visit(p.raw(), state, binder, errors);
        }
        K::DeclaredVariablePattern => {
            let pattern = Id::<DeclaredVariablePattern>::from_raw(node);
            let element = declared_fragment
                .get(node)
                .and_then(|&f| ctx.fragment_data(f))
                .and_then(|f| f.element.try_get().copied());
            if let Some(element) = element {
                let name = ctx.name(ast.tokens.lexeme(ast[pattern].name));
                state.add(Some(errors), name, element);
            }
        }
        K::ListPattern => {
            for &e in ast.list_raw(ast[Id::<dartr_ast::ListPattern>::from_raw(node)].elements) {
                visit(e, state, binder, errors);
            }
        }
        K::LogicalAndPattern => {
            let p = &ast[Id::<dartr_ast::LogicalAndPattern>::from_raw(node)];
            let (l, r) = (p.left_operand, p.right_operand);
            visit(l.raw(), state, binder, errors);
            visit(r.raw(), state, binder, errors);
        }
        K::LogicalOrPattern => {
            let p = &ast[Id::<dartr_ast::LogicalOrPattern>::from_raw(node)];
            let (l, r) = (p.left_operand, p.right_operand);
            state.logical_or_pattern_start();
            visit(l.raw(), state, binder, errors);
            state.logical_or_pattern_finish_left();
            visit(r.raw(), state, binder, errors);
            state.logical_or_pattern_finish(binder, Some(errors), node);
        }
        K::MapPattern => {
            for &e in ast.list_raw(ast[Id::<dartr_ast::MapPattern>::from_raw(node)].elements) {
                visit(e, state, binder, errors);
            }
        }
        K::MapPatternEntry => {
            let v = ast[Id::<dartr_ast::MapPatternEntry>::from_raw(node)].value;
            visit(v.raw(), state, binder, errors);
        }
        K::NullAssertPattern => {
            let p = ast[Id::<dartr_ast::NullAssertPattern>::from_raw(node)].pattern;
            visit(p.raw(), state, binder, errors);
        }
        K::NullCheckPattern => {
            let p = ast[Id::<dartr_ast::NullCheckPattern>::from_raw(node)].pattern;
            visit(p.raw(), state, binder, errors);
        }
        K::ObjectPattern => {
            for &f in ast.list_raw(ast[Id::<dartr_ast::ObjectPattern>::from_raw(node)].fields) {
                visit(f, state, binder, errors);
            }
        }
        K::ParenthesizedPattern => {
            let p = ast[Id::<dartr_ast::ParenthesizedPattern>::from_raw(node)].pattern;
            visit(p.raw(), state, binder, errors);
        }
        K::PatternField => {
            let p = ast[Id::<dartr_ast::PatternField>::from_raw(node)].pattern;
            visit(p.raw(), state, binder, errors);
        }
        K::RecordPattern => {
            for &f in ast.list_raw(ast[Id::<dartr_ast::RecordPattern>::from_raw(node)].fields) {
                visit(f, state, binder, errors);
            }
        }
        K::RestPatternElement => {
            if let Some(p) = ast[Id::<dartr_ast::RestPatternElement>::from_raw(node)].pattern {
                visit(p.raw(), state, binder, errors);
            }
        }
        // Dart `ThrowingAstVisitor`: other nodes are not patterns.
        _ => {}
    }
}
