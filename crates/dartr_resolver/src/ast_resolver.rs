// Dart source: pkg/analyzer/lib/src/summary2/ast_resolver.dart

//! `AstResolver`: resolution of single expressions while a library cycle
//! is linked (summary2): the initializers of fields and top-level
//! variables whose type is inferred, and the default values of declaring
//! formal parameters (Dart `_PropertyInducingElementTypeInference`, unit
//! C10).
//!
//! # Units
//!
//! Dart resolves the node in the linking AST in place. The linker of this
//! port shares the parsed units (`Arc<ParsedUnit>`), and resolution
//! rewrites nodes, so [`LinkResolution`] resolves in a copy of the unit
//! ([`UnitCopy`]: the AST, the resolution tables and a local arena for the
//! local elements of the expression). Resolution is re-entrant: the type
//! of a variable that the initializer reads is inferred on demand
//! (`dartr_element::type_inference`), which resolves another initializer,
//! maybe of the same unit, while the first one is resolved. Each
//! resolution takes a copy out of a pool, so a nested resolution of the
//! same unit gets its own copy.
//!
//! # Scopes
//!
//! Dart uses `node.initializerScope`, which the reference resolver of the
//! linker stores on the variable declaration. Here the resolution visitor
//! pushes the same scopes before it visits the expression: the type
//! parameters and the instance (or extension) scope of the enclosing
//! instance element, the static flag of a field, and the constructor
//! initializer scope of the primary constructor for the initializer of a
//! non-late instance field (Dart `ScopeContext.visitVariableDeclarationList`).
//!
//! # Results
//!
//! The diagnostics are dropped (Dart `DiagnosticListener.nullListener`).
//! The static type of the expression can mention elements of the local
//! arena (the parameter elements of a function expression, the type
//! parameters of a generic function expression); [`LinkResolution`] returns
//! a type with these replaced by elements of the linked cycle (Dart keeps
//! the local elements alive in the type).

use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
use std::sync::Arc;

use dartr_ast::{
    Ast, Expression, FieldDeclaration, Id, NodeId, VariableDeclaration, VariableDeclarationList,
};
use dartr_ast_builder::ParsedUnit;
use dartr_diagnostics::Diagnostic;
use dartr_element::{
    Ctx, EId, ElemRef, FId, FnParam, FormalParameterElement, FragmentId, FunctionTypeData,
    InstanceElement, InterfaceElement, LibraryElement, LibraryFragment, LocalArena, NamedType,
    ResolutionTables, Tag, TypeId, TypeKind, TypeParameterElement,
};
use indexmap::IndexMap;

use crate::options::AnalysisOptions;
use crate::resolution_visitor::ResolutionVisitor;
use crate::resolver::{ResolverVisitor, UnitContext};
use crate::scope::LibraryScopes;
use crate::tables::ResolverTables;

/// A copy of a unit to resolve expressions in.
struct UnitCopy {
    ast: Ast,
    tables: ResolutionTables,
    rt: ResolverTables,
    local: LocalArena,
}

/// What to resolve: Dart `AstResolver(linker, libraryFragment, scope,
/// analysisOptions, enclosingClassElement:)` and the arguments of
/// `resolveExpression`.
pub struct ExpressionRequest<'r> {
    pub library: EId<LibraryElement>,
    /// The library fragment of the unit (Dart `libraryFragment`).
    pub fragment: FId<LibraryFragment>,
    pub source: ExpressionSource<'r>,
    pub options: AnalysisOptions,
    /// The instance element that encloses the declaration (its scopes).
    pub enclosing_instance: Option<EId<InstanceElement>>,
    /// Dart `enclosingClassElement`.
    pub enclosing_class: Option<EId<InterfaceElement>>,
    /// Dart `contextType`.
    pub context_type: TypeId,
    /// Dart `inScopePrimaryConstructorParameters`.
    pub in_scope_primary_constructor_parameters: Option<&'r [EId<FormalParameterElement>]>,
}

/// Where the expression to resolve is.
pub enum ExpressionSource<'r> {
    /// An expression of a unit.
    Unit {
        parsed: &'r Arc<ParsedUnit>,
        /// The fragments of the declarations of the unit (the linker's
        /// `declaredFragment` of the nodes).
        declared_fragments: &'r IndexMap<NodeId, FragmentId>,
        /// The node that has the expression: a `VariableDeclaration` (its
        /// initializer) or a formal parameter (its default value).
        owner: NodeId,
    },
    /// A synthetic expression of the linker (Dart: the initializer of the
    /// synthetic `VariableDeclaration` of an enum constant): [expression]
    /// in [ast], the const expressions of the cycle.
    Synthetic {
        ast: &'r Ast,
        expression: NodeId,
        /// The unit of the declaration: its features, and the dot
        /// shorthands (Dart `isDotShorthand`) of the nodes that the
        /// expression copies (the copies keep the offsets).
        unit: &'r Arc<ParsedUnit>,
    },
}

/// The `AstResolver`s of one linked cycle: the library scopes and the unit
/// copies (see the module documentation).
#[derive(Default)]
pub struct LinkResolution {
    scopes: RefCell<IndexMap<EId<LibraryElement>, Rc<LibraryScopes>>>,
    /// The free copies of each unit; `None`: of the synthetic expressions.
    pool: RefCell<IndexMap<Option<FId<LibraryFragment>>, Vec<UnitCopy>>>,
}

/// The expression of [owner]: the initializer of a variable declaration,
/// or the default value of a formal parameter.
fn owner_expression(ast: &Ast, owner: NodeId) -> Option<Id<Expression>> {
    if let Some(v) = ast.cast::<VariableDeclaration>(owner) {
        return ast[v].initializer;
    }
    let parts = crate::ast_ext::formal_parameter_parts(ast, owner);
    parts.default_clause.map(|d| ast[d].value)
}

impl LinkResolution {
    pub fn new() -> LinkResolution {
        LinkResolution::default()
    }

    fn library_scopes(&self, ctx: &Ctx<'_>, library: EId<LibraryElement>) -> Rc<LibraryScopes> {
        if let Some(s) = self.scopes.borrow().get(&library) {
            return s.clone();
        }
        let scopes = Rc::new(LibraryScopes::build(ctx, library));
        self.scopes.borrow_mut().insert(library, scopes.clone());
        scopes
    }

    fn take_copy(&self, ctx: &Ctx<'_>, request: &ExpressionRequest<'_>) -> UnitCopy {
        let key = pool_key(request);
        if let Some(copy) = self.pool.borrow_mut().get_mut(&key).and_then(|v| v.pop()) {
            return copy;
        }
        let mut tables = ResolutionTables::new();
        let mut rt = ResolverTables::new();
        let ast = match &request.source {
            ExpressionSource::Unit {
                parsed,
                declared_fragments,
                ..
            } => {
                for (&node, &fragment) in *declared_fragments {
                    tables.declared_fragment.insert(node, fragment);
                }
                tables
                    .declared_fragment
                    .insert(parsed.unit, request.fragment.raw());
                for &node in &parsed.dot_shorthands {
                    rt.dot_shorthand.insert(node, ());
                }
                parsed.ast.clone()
            }
            ExpressionSource::Synthetic { ast, .. } => (*ast).clone(),
        };
        UnitCopy {
            ast,
            tables,
            rt,
            local: ctx.world.generation.new_local_arena(),
        }
    }

    fn put_copy(&self, fragment: Option<FId<LibraryFragment>>, copy: UnitCopy) {
        self.pool
            .borrow_mut()
            .entry(fragment)
            .or_default()
            .push(copy);
    }

    /// Dart `AstResolver.resolveExpression(getNode, contextType:,
    /// inScopePrimaryConstructorParameters:)` followed by
    /// `getNode().typeOrThrow`: the static type of the expression, with no
    /// local elements. `None` when the owner has no expression or the
    /// resolution panicked.
    pub fn resolve_expression(
        &self,
        ctx: &Ctx<'_>,
        request: &ExpressionRequest<'_>,
    ) -> Option<TypeId> {
        let scopes = self.library_scopes(ctx, request.library);
        let mut copy = self.take_copy(ctx, request);
        let result = catch_unwind(AssertUnwindSafe(|| {
            resolve_in_copy(ctx, &scopes, &mut copy, request)
        }));
        match result {
            Ok(t) => {
                self.put_copy(pool_key(request), copy);
                t
            }
            // The copy may be inconsistent after a panic: drop it.
            Err(_) => None,
        }
    }
}

fn pool_key(request: &ExpressionRequest<'_>) -> Option<FId<LibraryFragment>> {
    match request.source {
        ExpressionSource::Unit { .. } => Some(request.fragment),
        ExpressionSource::Synthetic { .. } => None,
    }
}

/// The node that has the expression in [ast] (a copy of the source of
/// [request]). A synthetic expression gets a synthetic
/// `VariableDeclaration` parent (Dart `ElementBuilder` creates one for each
/// enum constant).
fn owner_in_copy(
    ast: &mut Ast,
    rt: &mut ResolverTables,
    request: &ExpressionRequest<'_>,
) -> NodeId {
    match request.source {
        ExpressionSource::Unit { owner, .. } => owner,
        ExpressionSource::Synthetic {
            expression, unit, ..
        } => {
            if let Some(parent) = ast.parent(expression) {
                return parent;
            }
            mark_copied_dot_shorthands(ast, rt, expression, unit);
            let name = ast.tokens.push_synthetic_string(
                dartr_syntax::TokenType::IDENTIFIER,
                "",
                0,
                0,
                Some(0),
            );
            let metadata = ast.new_list(std::iter::empty());
            ast.add(VariableDeclaration {
                documentation_comment: None,
                metadata,
                name,
                equals: None,
                initializer: Some(Id::from_raw(expression)),
            })
            .raw()
        }
    }
}

/// Dart `isDotShorthand` of the nodes of the copied subtree [root]: the
/// nodes with the offset, length and kind of a dot shorthand of [unit].
fn mark_copied_dot_shorthands(ast: &Ast, rt: &mut ResolverTables, root: NodeId, unit: &ParsedUnit) {
    if unit.dot_shorthands.is_empty() {
        return;
    }
    let key = |a: &Ast, n: NodeId| (a.offset(n), a.length(n), a.kind(n));
    let originals: Vec<_> = unit
        .dot_shorthands
        .iter()
        .map(|&n| key(&unit.ast, n))
        .collect();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if originals.contains(&key(ast, n)) {
            rt.dot_shorthand.insert(n, ());
        }
        stack.extend(ast.children(n));
    }
}

fn resolve_in_copy(
    link_ctx: &Ctx<'_>,
    scopes: &LibraryScopes,
    copy: &mut UnitCopy,
    request: &ExpressionRequest<'_>,
) -> Option<TypeId> {
    let UnitCopy {
        ast,
        tables,
        rt,
        local,
    } = copy;
    let owner = owner_in_copy(ast, rt, request);
    let expression = owner_expression(ast, owner)?;
    let unit_features = match &request.source {
        ExpressionSource::Unit { parsed, .. } => parsed.feature_set,
        ExpressionSource::Synthetic { unit, .. } => unit.feature_set,
    };
    let library_features = link_ctx.get(request.library).feature_set.clone();
    let ctx = Ctx {
        local: Some(&*local),
        features: &library_features,
        ..*link_ctx
    };
    let unit_ctx = UnitContext {
        library: request.library,
        fragment: request.fragment,
        scopes,
        options: request.options,
        features: unit_features,
    };
    let mut diagnostics: Vec<Diagnostic> = Vec::new();

    // ElementBindingVisitor.forPartialResolution(...).bindSubtree(...)
    crate::element_binding_visitor::bind_subtree(
        &ctx,
        ast,
        request.fragment,
        request.fragment.raw(),
        expression.raw(),
        tables,
        rt,
    );

    // node.accept(_resolutionVisitor), in the initializer scope.
    {
        let mut visitor = ResolutionVisitor::new(ctx, unit_ctx, tables, rt, &mut diagnostics);
        visitor.enter_initializer_scope(ast, owner, request);
        visitor.visit(ast, expression);
    }

    // Node may have been rewritten so get it again.
    let expression = owner_expression(ast, owner)?;
    let parent = ast.parent(expression)?;
    let result = {
        let mut resolver = ResolverVisitor::new(ctx, ast, tables, rt, &mut diagnostics, unit_ctx);
        // _prepareEnclosingDeclarations
        resolver.prepare_enclosing_declarations(request.enclosing_class, None);
        {
            let ast = &*resolver.ast;
            let tables = &*resolver.tables;
            resolver.flow_analysis.body_or_initializer_enter(
                ast,
                tables,
                parent,
                request.in_scope_primary_constructor_parameters,
                None,
            );
        }
        resolver.analyze_expression_node(expression, request.context_type);
        let rewritten = resolver.pop_rewrite().unwrap_or(expression);
        resolver.flow_analysis.body_or_initializer_exit();
        resolver.static_type(rewritten)
    };
    let t = result.unwrap_or(TypeId::INVALID);
    Some(Globalizer::new(&ctx).globalize(t))
}

impl ResolutionVisitor<'_, '_> {
    /// Pushes the scopes of Dart `node.initializerScope` for the
    /// expression of [request] (see the module documentation).
    fn enter_initializer_scope(
        &mut self,
        ast: &Ast,
        owner: NodeId,
        request: &ExpressionRequest<'_>,
    ) {
        let Some(instance) = request.enclosing_instance else {
            return;
        };
        let type_parameters =
            crate::scope_context::element_type_parameters(&self.ctx, instance.raw());
        self.scope_context
            .push_type_parameter_scope(&type_parameters);
        if instance.raw().tag() == Tag::Extension {
            self.scope_context
                .push_extension_scope(EId::from_raw(instance.raw()));
        } else {
            self.named_type_resolver.enclosing_class = request.enclosing_class;
            self.scope_context.push_instance_scope(instance);
        }
        // ScopeContext.visitFieldDeclaration, visitVariableDeclarationList
        let list = ast
            .parent(owner)
            .and_then(|p| ast.cast::<VariableDeclarationList>(p));
        let field_declaration = list
            .and_then(|l| ast.parent(l))
            .and_then(|p| ast.cast::<FieldDeclaration>(p));
        if let (Some(list), Some(field_declaration)) = (list, field_declaration) {
            let is_static = ast[field_declaration].static_keyword.is_some();
            self.scope_context.set_in_static_member(is_static);
            if !is_static
                && ast[list].late_keyword.is_none()
                && let Some(c) = crate::scope_context::primary_constructor_of(&self.ctx, instance)
            {
                self.scope_context.push_constructor_initializer_scope(c);
            }
        }
    }
}

/// Replaces the local elements in a type by elements of the linked cycle
/// (see the module documentation).
struct Globalizer<'c, 'a> {
    ctx: &'c Ctx<'a>,
    type_parameters: IndexMap<EId<TypeParameterElement>, EId<TypeParameterElement>>,
}

impl<'c, 'a> Globalizer<'c, 'a> {
    fn new(ctx: &'c Ctx<'a>) -> Self {
        Globalizer {
            ctx,
            type_parameters: IndexMap::new(),
        }
    }

    fn globalize(&mut self, t: TypeId) -> TypeId {
        if !t.is_local() {
            return t;
        }
        let ctx = self.ctx;
        match *ctx.ty(t) {
            TypeKind::Interface {
                element,
                args,
                nullability,
                alias,
            } => {
                let args = self.list(args);
                let alias = alias.map(|a| self.alias(a));
                ctx.intern(TypeKind::Interface {
                    element,
                    args,
                    nullability,
                    alias,
                })
            }
            TypeKind::Record {
                positional,
                named,
                nullability,
                alias,
            } => {
                let positional = self.list(positional);
                let named_items: Vec<NamedType> = ctx
                    .list(named)
                    .iter()
                    .map(|n| NamedType {
                        name: n.name,
                        ty: self.globalize(n.ty),
                    })
                    .collect();
                let named = ctx.intern_list(&named_items);
                let alias = alias.map(|a| self.alias(a));
                ctx.intern(TypeKind::Record {
                    positional,
                    named,
                    nullability,
                    alias,
                })
            }
            TypeKind::TypeParameter {
                param,
                nullability,
                promoted_bound,
                alias,
            } => {
                let param = if param.raw().store().is_local() {
                    match self.type_parameters.get(&param) {
                        Some(&p) => p,
                        // A type parameter that is not bound in the type.
                        None => return TypeId::DYNAMIC,
                    }
                } else {
                    param
                };
                let promoted_bound = promoted_bound.map(|b| self.globalize(b));
                let alias = alias.map(|a| self.alias(a));
                ctx.intern(TypeKind::TypeParameter {
                    param,
                    nullability,
                    promoted_bound,
                    alias,
                })
            }
            TypeKind::Function(f) => {
                let store = dartr_typesystem::type_ext::fresh_store(&ctx.global());
                let mut fresh: Vec<(EId<TypeParameterElement>, EId<TypeParameterElement>)> =
                    Vec::new();
                let type_params: Vec<EId<TypeParameterElement>> = ctx
                    .list(f.type_params)
                    .iter()
                    .map(|&p| {
                        if !p.raw().store().is_local() {
                            return p;
                        }
                        let data = ctx.get(p);
                        let name = data.name;
                        let fragment = store.add_fragment::<dartr_element::TypeParameterFragment>(
                            dartr_element::TypeParameterFragment {
                                fragment: dartr_element::FragmentData::new(name, None),
                            },
                        );
                        let mut element = TypeParameterElement::new(
                            dartr_element::ElementData::new(name, fragment.raw()),
                        );
                        element.variance = data.variance;
                        let id: EId<TypeParameterElement> = store.add(element);
                        store.fragment(fragment).element.set_once(id.raw());
                        self.type_parameters.insert(p, id);
                        fresh.push((p, id));
                        id
                    })
                    .collect();
                for &(old, new) in &fresh {
                    let bound = ctx.get(old).bound.get().map(|b| self.globalize(b));
                    ctx.get(new).bound.set(bound);
                }
                let params: Vec<FnParam> = ctx
                    .list(f.params)
                    .iter()
                    .map(|p| FnParam {
                        name: p.name,
                        kind: p.kind,
                        ty: self.globalize(p.ty),
                        covariant: p.covariant,
                        element: p.element.filter(|e| !is_local_ref(*e)),
                    })
                    .collect();
                let ret = self.globalize(f.ret);
                let alias = f.alias.map(|a| self.alias(a));
                ctx.intern(TypeKind::Function(FunctionTypeData {
                    type_params: ctx.intern_list(&type_params),
                    params: ctx.intern_list(&params),
                    required_positional: f.required_positional,
                    ret,
                    nullability: f.nullability,
                    alias,
                }))
            }
            _ => t,
        }
    }

    fn list(&mut self, list: dartr_element::TypeList) -> dartr_element::TypeList {
        let items: Vec<TypeId> = self
            .ctx
            .list(list)
            .iter()
            .map(|&t| self.globalize(t))
            .collect();
        self.ctx.intern_list(&items)
    }

    fn alias(&mut self, alias: dartr_element::AliasId) -> dartr_element::AliasId {
        if !alias.is_local() {
            return alias;
        }
        let a = *self.ctx.alias(alias);
        let args = self.list(a.args);
        self.ctx.intern_alias(dartr_element::AliasRef { args, ..a })
    }
}

fn is_local_ref(e: ElemRef) -> bool {
    match e {
        ElemRef::Base(b) => b.store().is_local(),
        ElemRef::Member(m) => m.is_local(),
    }
}
