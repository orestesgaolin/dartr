// Dart source: pkg/analyzer/lib/src/generated/resolver.dart (ResolverVisitor)

//! [`ResolverVisitor`]: resolves the nodes of one compilation unit (static
//! types, elements, inference, flow analysis). The Dart class mixes in
//! `TypeAnalyzer` and `NullShortingMixin`; here they are the traits of
//! `dartr_flow` implemented in [`type_analyzer_impl`].
//!
//! # How the Dart code maps
//!
//! - `node.accept(resolver)` → [`ResolverVisitor::visit_node`] (generated
//!   dispatch, `src/generated/dispatch.rs`), which calls `visit_x`.
//! - `resolver.analyzeExpression(node, schema)` then `popRewrite()` →
//!   [`ResolverVisitor::resolve_expression`] (or
//!   [`ResolverVisitor::analyze_expression_node`] + [`ResolverVisitor::pop_rewrite`]).
//! - `node.recordStaticType(type, resolver: this)` →
//!   [`ResolverVisitor::record_static_type`].
//! - `diagnosticReporter.report(...)` → [`ResolverVisitor::report`].
//! - The other resolver files (`MethodInvocationResolver`, ...) are modules
//!   of this crate with free functions or structs that take
//!   `&mut ResolverVisitor`.
//!
//! Submodules: [`expressions`] (the `visitX` of expressions),
//! [`statements`], [`declarations`] (declarations, directives and other
//! nodes), [`collection_elements`], [`patterns`], [`type_analyzer_impl`].

pub mod collection_elements;
pub mod declarations;
pub mod expressions;
pub mod patterns;
pub mod statements;
pub mod type_analyzer_impl;

use dartr_ast::{Ast, Expression, Id, NodeId};
use dartr_diagnostics::{Diagnostic, LocatedDiagnostic};
use dartr_element::{
    Ctx, EId, ElemRef, ElementId, ExecutableElement, ExtensionElement, FId, InterfaceElement,
    LibraryElement, LibraryFragment, ResolutionTables, TypeId, TypeKind,
};
use dartr_flow::shared_type::{SharedTypeSchemaView, SharedTypeView};
use dartr_flow::type_analysis_result::{ExpressionTypeAnalysisResult, MatchContext, PatternResult};
use dartr_flow::type_analyzer::TypeAnalyzerOptions;
use dartr_parser::experimental_features::ExperimentalFeatures;
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_typesystem::type_system_operations::TypeSystemOperations;
use dartr_typesystem::{TypeExt, TypeSystem};

use crate::body_inference_context::BodyInferenceContext;
use crate::flow_analysis_visitor::{FlowAnalysisHelper, ResolverExpressionInfo};
use crate::options::AnalysisOptions;
use crate::scope::LibraryScopes;
use crate::shared_type_analyzer::SharedTypeAnalyzerErrors;
use crate::tables::ResolverTables;

/// A type schema (Dart `SharedTypeSchemaView`).
pub type SchemaOf = SharedTypeSchemaView<TypeId>;

/// A type (Dart `SharedTypeView`).
pub type TypeViewOf = SharedTypeView<TypeId>;

/// Dart `ExpressionTypeAnalysisResult` of the resolver.
pub type ExprResult<'a> = ExpressionTypeAnalysisResult<TypeId, ResolverExpressionInfo<'a>>;

/// Dart `PatternResult` of the resolver.
pub type PatternResultOf = PatternResult<TypeId>;

/// Dart `SharedMatchContext` of the resolver.
pub type SharedMatchContext = MatchContext<
    NodeId,
    Id<Expression>,
    Id<dartr_ast::DartPattern>,
    EId<dartr_element::PromotableElement>,
    dartr_element::Name,
>;

/// Dart `CollectionLiteralContext` (typed_literal_resolver.dart): the
/// element, key and value context types of a collection literal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CollectionLiteralContext {
    pub element_type: Option<TypeId>,
    pub key_type: Option<TypeId>,
    pub value_type: Option<TypeId>,
}

/// The inputs of the resolution of one unit that do not change while it is
/// resolved.
#[derive(Clone, Copy)]
pub struct UnitContext<'a> {
    /// Dart `definingLibrary`.
    pub library: EId<LibraryElement>,
    /// Dart `libraryFragment`.
    pub fragment: FId<LibraryFragment>,
    /// The library scopes (shared by the units of the library).
    pub scopes: &'a LibraryScopes,
    /// Dart `analysisOptions`.
    pub options: AnalysisOptions,
    /// Dart `unit.featureSet`.
    pub features: ExperimentalFeatures,
}

/// Dart `ResolverVisitor`.
///
/// One instance resolves one unit. The fields are public so that the
/// resolver files (other modules of this crate) can use them, the same as
/// the Dart resolver files use the resolver's members.
pub struct ResolverVisitor<'a> {
    /// The lookup context, with the unit's local arena.
    pub ctx: Ctx<'a>,
    /// Dart `typeSystem`.
    pub type_system: TypeSystem<'a>,
    /// The AST of the unit (a clone of the parsed AST: resolution rewrites
    /// nodes).
    pub ast: &'a mut Ast,
    /// The public resolution results (what Dart writes into the AST).
    pub tables: &'a mut ResolutionTables,
    /// Other per-node data of the resolver passes.
    pub rt: &'a mut ResolverTables,
    /// The diagnostics reported so far, in report order.
    pub diagnostics: &'a mut Vec<Diagnostic>,
    /// Dart `DiagnosticReporter.lockLevel`: diagnostics are dropped while it
    /// is not zero.
    pub lock_level: u32,
    pub unit: UnitContext<'a>,
    /// Dart `typeAnalyzerOptions`.
    pub type_analyzer_options: TypeAnalyzerOptions,
    /// Dart `flowAnalysis`.
    pub flow_analysis: FlowAnalysisHelper<'a>,
    /// Dart `errors` (`SharedTypeAnalyzerErrors`): the errors of the shared
    /// type analyzer, converted to diagnostics in report order.
    pub(crate) errors: SharedTypeAnalyzerErrors,
    /// Dart `enclosingClass`.
    pub enclosing_class: Option<EId<InterfaceElement>>,
    /// Dart `enclosingExtension`.
    pub enclosing_extension: Option<EId<ExtensionElement>>,
    /// Dart `enclosingFunction`.
    pub enclosing_function: Option<EId<ExecutableElement>>,
    /// Dart `_thisType`.
    this_type: Option<TypeId>,
    /// Dart `_bodyContext`.
    pub(crate) body_context: Option<BodyInferenceContext>,
    /// Dart `_rewriteStack`.
    rewrite_stack: Vec<Option<Id<Expression>>>,
    /// Dart `NullShortingMixin._guards` (the analyzer's guard is `Null`).
    pub(crate) guards: Vec<()>,
    /// Dart `TypeAnalyzer._dotShorthands`.
    pub(crate) dot_shorthands: Vec<(NodeId, SchemaOf)>,
}

impl<'a> ResolverVisitor<'a> {
    /// Dart `ResolverVisitor(...)`.
    pub fn new(
        ctx: Ctx<'a>,
        ast: &'a mut Ast,
        tables: &'a mut ResolutionTables,
        rt: &'a mut ResolverTables,
        diagnostics: &'a mut Vec<Diagnostic>,
        unit: UnitContext<'a>,
    ) -> ResolverVisitor<'a> {
        let type_system = TypeSystem::new(ctx);
        let type_analyzer_options =
            crate::type_analyzer_options::compute_type_analyzer_options(unit.features);
        let operations = TypeSystemOperations::new(type_system, unit.options.strict_casts);
        ResolverVisitor {
            ctx,
            type_system,
            ast,
            tables,
            rt,
            diagnostics,
            lock_level: 0,
            unit,
            type_analyzer_options,
            flow_analysis: FlowAnalysisHelper::new(operations, type_analyzer_options),
            errors: SharedTypeAnalyzerErrors::default(),
            enclosing_class: None,
            enclosing_extension: None,
            enclosing_function: None,
            this_type: None,
            body_context: None,
            rewrite_stack: Vec::new(),
            guards: Vec::new(),
            dot_shorthands: Vec::new(),
        }
    }

    // ------------------------------------------------------------ features

    /// Whether [flag] is enabled for the unit (Dart `_featureSet.isEnabled`).
    pub fn is_enabled(&self, flag: ExperimentalFlag) -> bool {
        self.unit.features.is_experiment_enabled(flag)
    }

    /// Dart `isConstructorTearoffsEnabled`.
    pub fn is_constructor_tearoffs_enabled(&self) -> bool {
        self.is_enabled(ExperimentalFlag::ConstructorTearoffs)
    }

    /// Dart `genericMetadataIsEnabled`.
    pub fn generic_metadata_is_enabled(&self) -> bool {
        self.is_enabled(ExperimentalFlag::GenericMetadata)
    }

    /// Dart `inferenceUsingBoundsIsEnabled`.
    pub fn inference_using_bounds_is_enabled(&self) -> bool {
        self.is_enabled(ExperimentalFlag::InferenceUsingBounds)
    }

    // ------------------------------------------------------------ diagnostics

    /// Dart `diagnosticReporter.report(...)`.
    ///
    /// The pending errors of the shared type analyzer are written first, so
    /// that the diagnostics stay in report order.
    pub fn report(&mut self, diagnostic: LocatedDiagnostic) {
        self.flush_type_analyzer_errors();
        if self.lock_level != 0 {
            return;
        }
        self.diagnostics.push(diagnostic.into_diagnostic());
    }

    /// Dart `diagnostic.at(node)`: locates [diagnostic] at the offset and
    /// length of [node].
    pub fn at(
        &self,
        diagnostic: dartr_diagnostics::LocatableDiagnostic,
        node: impl Into<NodeId>,
    ) -> LocatedDiagnostic {
        let node = node.into();
        diagnostic.at_offset(self.ast.offset(node) as usize, self.ast.length(node) as usize)
    }

    /// Dart `diagnostic.at(token)`.
    pub fn at_token(
        &self,
        diagnostic: dartr_diagnostics::LocatableDiagnostic,
        token: dartr_syntax::TokenId,
    ) -> LocatedDiagnostic {
        let t = self.ast.tokens.get(token);
        diagnostic.at_offset(t.offset as usize, (t.end() - t.offset) as usize)
    }

    /// Converts the errors that the shared type analyzer reported since the
    /// last call into diagnostics.
    pub fn flush_type_analyzer_errors(&mut self) {
        if self.errors.is_empty() {
            return;
        }
        let pending = self.errors.take();
        for error in pending {
            if let Some(d) = crate::shared_type_analyzer::to_diagnostic(self, error) {
                if self.lock_level == 0 {
                    self.diagnostics.push(d.into_diagnostic());
                }
            }
        }
    }

    /// Dart `flow` (`flowAnalysis.flow!`): the flow analysis of the current
    /// body. Panics when no body or initializer is being resolved.
    pub fn flow(&mut self) -> &mut crate::flow_analysis_visitor::ResolverFlow<'a> {
        self.flow_analysis
            .flow
            .as_mut()
            .expect("flow analysis is not active")
    }

    // ------------------------------------------------------------ syntax

    /// The lexeme of [token] (Dart `token.lexeme`).
    pub fn lexeme(&self, token: dartr_syntax::TokenId) -> &str {
        self.ast.tokens.lexeme(token)
    }

    /// Dart `node.visitChildren(resolver)`: visits the children of [node] in
    /// `visitChildren` order. The children are read before the first visit,
    /// so a child may be rewritten while it is visited.
    pub fn visit_children(&mut self, node: impl Into<NodeId>) {
        let children = self.ast.children(node.into());
        for child in children {
            self.visit_node(child);
        }
    }

    /// Dart `node?.accept(resolver)`.
    pub fn visit_opt(&mut self, node: Option<impl Into<NodeId>>) {
        if let Some(node) = node {
            self.visit_node(node.into());
        }
    }

    /// Dart `nodeList.accept(resolver)`: visits each node of [list].
    pub fn visit_list<T: ?Sized>(&mut self, list: dartr_ast::NodeList<T>) {
        let items = self.ast.list_raw(list).to_vec();
        for item in items {
            self.visit_node(item);
        }
    }

    // ------------------------------------------------------------ expressions

    /// Dart `analyzeExpression(node, SharedTypeSchemaView(schema))` (the
    /// `TypeAnalyzer` mixin): dispatches [node] with the context type
    /// [schema], finishes null shorting, and leaves the (possibly rewritten)
    /// node on the rewrite stack; the caller must [`Self::pop_rewrite`].
    pub fn analyze_expression_node(&mut self, node: Id<Expression>, schema: TypeId) -> ExprResult<'a> {
        dartr_flow::type_analyzer::TypeAnalyzer::analyze_expression(
            self,
            node,
            SchemaOf::new(schema),
            false,
            false,
            false,
        )
    }

    /// Dart `analyzeExpression(node, schema); popRewrite()!`: resolves
    /// [node] with the context type [context] and returns the node that is
    /// in its place after resolution (a rewrite may replace it).
    pub fn resolve_expression(&mut self, node: Id<Expression>, context: TypeId) -> Id<Expression> {
        self.analyze_expression_node(node, context);
        self.pop_rewrite().expect("rewritten expression")
    }

    /// Dart `popRewrite()`.
    pub fn pop_rewrite(&mut self) -> Option<Id<Expression>> {
        self.rewrite_stack.pop().expect("rewrite stack")
    }

    /// Dart `peekRewrite()`.
    pub fn peek_rewrite(&self) -> Option<Id<Expression>> {
        *self.rewrite_stack.last().expect("rewrite stack")
    }

    /// Dart `pushRewrite(expression)`.
    pub fn push_rewrite(&mut self, expression: Option<Id<Expression>>) {
        self.rewrite_stack.push(expression);
    }

    /// Dart `rewriteStackDepth`.
    pub fn rewrite_stack_depth(&self) -> usize {
        self.rewrite_stack.len()
    }

    /// Dart `replaceExpression(oldNode, newNode, parent: parent)`: puts
    /// [new] in the slot of [old] (the parent of [old], or [parent]), and
    /// updates the top of the rewrite stack.
    ///
    /// [new] must be created with `ast.add` (it adopts its children).
    pub fn replace_expression(
        &mut self,
        old: Id<Expression>,
        new: Id<Expression>,
        parent: Option<NodeId>,
    ) {
        if let Some(Some(top)) = self.rewrite_stack.last()
            && *top == old
        {
            *self.rewrite_stack.last_mut().unwrap() = Some(new);
        }
        match parent {
            Some(parent) => self
                .ast
                .replace_child(parent, old.raw(), new.raw()),
            None => self.ast.replace_with(old, new),
        }
    }

    /// The static type of [expression] (Dart `expression.staticType`).
    pub fn static_type(&self, expression: impl Into<NodeId>) -> Option<TypeId> {
        self.tables.static_type.get(expression).copied()
    }

    /// Dart `expression.typeOrThrow`.
    pub fn type_or_throw(&self, expression: impl Into<NodeId>) -> TypeId {
        let node = expression.into();
        match self.tables.static_type.get(node) {
            Some(&t) => t,
            None => panic!(
                "no static type for {:?} at {}",
                self.ast.kind(node),
                self.ast.offset(node)
            ),
        }
    }

    /// Dart `ExpressionImpl.recordStaticType(type, resolver: this)`: sets the
    /// static type of [expression]; a bottom type ends the flow (Dart
    /// `flow?.handleExit()`).
    pub fn record_static_type(&mut self, expression: impl Into<NodeId>, ty: TypeId) {
        self.tables.static_type.insert(expression.into(), ty);
        if self.ctx.is_bottom(ty) {
            if let Some(flow) = self.flow_analysis.flow.as_mut() {
                dartr_flow::flow_analysis::FlowAnalysis::handle_exit(flow);
            }
        }
    }

    /// Sets the static type of [expression] without the flow analysis side
    /// effect (Dart assignments to `staticType` / `setPseudoExpressionStaticType`).
    pub fn set_static_type(&mut self, expression: impl Into<NodeId>, ty: TypeId) {
        self.tables.static_type.insert(expression.into(), ty);
    }

    /// Dart `node.element = element` of an identifier or another node with
    /// an element.
    pub fn set_element(&mut self, node: impl Into<NodeId>, element: Option<ElemRef>) {
        let node = node.into();
        match element {
            Some(e) => {
                self.tables.element.insert(node, e);
            }
            None => {
                self.tables.element.remove(node);
            }
        }
    }

    /// The element of [node] (Dart `node.element`).
    pub fn element(&self, node: impl Into<NodeId>) -> Option<ElemRef> {
        self.tables.element.get(node).copied()
    }

    /// Dart `checkUnreachableNode(node)`.
    pub fn check_unreachable_node(&mut self, node: impl Into<NodeId>) {
        self.flow_analysis.check_unreachable_node(node.into());
    }

    // ------------------------------------------------------------ enclosing

    /// Dart `thisType`: the type of `this` before promotion.
    pub fn this_type(&self) -> Option<TypeId> {
        self.this_type
    }

    /// Dart `effectiveThisType`: the type of `this` after promotion.
    pub fn effective_this_type(&self) -> Option<TypeId> {
        if let Some(flow) = &self.flow_analysis.flow
            && let Some(t) = dartr_flow::flow_analysis::FlowAnalysis::promoted_type_of_this(flow)
        {
            return Some(t.unwrap_type_view());
        }
        self.this_type
    }

    /// Dart `prepareEnclosingDeclarations`.
    pub fn prepare_enclosing_declarations(
        &mut self,
        enclosing_class: Option<EId<InterfaceElement>>,
        enclosing_executable: Option<EId<ExecutableElement>>,
    ) {
        self.enclosing_class = enclosing_class;
        self.setup_this_type();
        self.enclosing_function = enclosing_executable;
    }

    /// Dart `_setupThisType`.
    pub(crate) fn setup_this_type(&mut self) {
        if let Some(class) = self.enclosing_class {
            self.this_type = Some(self.ctx.interface_this_type(class));
            return;
        }
        if let Some(extension) = self.enclosing_extension {
            let data = self.ctx.get(extension);
            self.this_type = data.extended_type.get();
            return;
        }
        self.this_type = None;
    }

    /// Sets `_thisType` directly (Dart code that assigns `_thisType`).
    pub(crate) fn set_this_type(&mut self, ty: Option<TypeId>) {
        self.this_type = ty;
    }

    /// Dart `bodyContext`.
    pub fn body_context(&self) -> Option<&BodyInferenceContext> {
        self.body_context.as_ref()
    }

    /// Dart `bodyContext` (mutable).
    pub fn body_context_mut(&mut self) -> Option<&mut BodyInferenceContext> {
        self.body_context.as_mut()
    }

    // ------------------------------------------------------------ fallback

    /// The fallback of a node kind whose resolver is not ported yet: resolves
    /// the expressions below [node] with an empty context (so that the
    /// ported node kinds inside get their types), without entering function
    /// bodies, and gives [node] the type `dynamic`.
    ///
    /// Every use is a stub of a resolver file (see the crate `README.md`).
    pub fn fallback_expression(&mut self, node: Id<Expression>) {
        self.fallback_visit_expressions_below(node.raw());
        self.record_static_type(node, TypeId::DYNAMIC);
    }

    /// The expressions below [node], resolved with an empty context; does not
    /// descend into function bodies or statements.
    pub fn fallback_visit_expressions_below(&mut self, node: NodeId) {
        let children = self.ast.children(node);
        for child in children {
            let kind = self.ast.kind(child);
            if crate::generated::dispatch::is_expression_kind(kind) {
                if self.flow_analysis.is_active() {
                    self.resolve_expression(Id::from_raw(child), TypeId::UNKNOWN);
                }
            } else if self.ast.is::<dartr_ast::FunctionBody>(child)
                || self.ast.is::<dartr_ast::Statement>(child)
            {
                // Not entered: needs the enclosing executable set up.
            } else {
                self.fallback_visit_expressions_below(child);
            }
        }
    }

    /// The type `dynamic` if [ty] is an invalid type (Dart
    /// `type is InvalidType ? DynamicType : type` in several places).
    pub fn invalid_to_dynamic(&self, ty: TypeId) -> TypeId {
        match self.ctx.ty(ty) {
            TypeKind::Invalid => TypeId::DYNAMIC,
            _ => ty,
        }
    }

    /// The element id of [node]'s element, if it is a base element.
    pub fn base_element(&self, node: impl Into<NodeId>) -> Option<ElementId> {
        match self.element(node)? {
            ElemRef::Base(e) => Some(e),
            ElemRef::Member(m) => Some(self.ctx.member(m).base),
        }
    }
}
