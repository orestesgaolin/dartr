// Dart source: pkg/analyzer/lib/src/dart/resolver/scope_context.dart

//! `ScopeContext`: the current lexical scope of the resolution visitor.
//!
//! The Dart class keeps the current `Scope` object and replaces it in
//! `withScope`. Here the enclosed scopes are a stack ([`ScopeContext::push`]
//! / [`ScopeContext::pop`]) on top of the library fragment scope of the
//! unit ([`crate::scope::LibraryScopes::fragment_lookup`]); a lookup walks
//! the stack from the top. The `visitX` methods of the Dart class take the
//! visitor; they are methods of [`ResolutionVisitor`] in this module
//! (`scope_visit_x`).
//!
//! Not ported: the scopes that the Dart class records on nodes
//! (`node.nameScope`, `node.bodyScope`, `node.typeParameterScope`,
//! `node.formalParameterInitializerScope`, `FormalParameter.scope`). Only
//! linking (`summary2`) reads them.

use dartr_ast::{
    Ast, ClassDeclaration, ClassTypeAlias, Comment, ConstructorDeclaration, EnumDeclaration,
    ExtensionDeclaration, ExtensionTypeDeclaration, FieldDeclaration, FunctionDeclaration,
    FunctionExpression, FunctionTypeAlias, GenericFunctionType, GenericTypeAlias, Id,
    MethodDeclaration, MixinDeclaration, NodeId, PrimaryConstructorBody,
    PrimaryConstructorDeclaration, TypeParameterList, VariableDeclarationList,
};
use dartr_element::{
    ConstructorElement, Ctx, EId, ElementId, FId, FormalParameterElement, InstanceElement,
    LibraryElement, LibraryFragment, Nullability, Tag, TypeId, TypeParameterElement,
};
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_typesystem::TypeExt;

use crate::ast_ext::formal_parameter_parts;
use crate::element_binding_visitor::primary_constructor_body_declaration;
use crate::resolution_visitor::ResolutionVisitor;
use crate::scope::{
    EnclosedLookup, EnclosedScope, EnclosedScopeKind, LibraryScopes, NameScope, ScopeLookupResult,
    library_feature_enabled,
};

/// Dart `ScopeContext`.
pub struct ScopeContext<'a> {
    ctx: Ctx<'a>,
    scopes: &'a LibraryScopes,
    /// Dart `_libraryFragment`.
    library_fragment: FId<LibraryFragment>,
    /// `_featureSet.isEnabled(Feature.wildcard_variables)`.
    wildcard_variables: bool,
    /// `_featureSet.isEnabled(Feature.primary_constructors)`.
    primary_constructors: bool,
    /// The enclosed scopes, innermost last (Dart `_nameScope` and its
    /// parents down to the library fragment scope).
    frames: Vec<EnclosedScope<'a>>,
    /// Dart `_enclosingInstanceElement`.
    enclosing_instance_element: Option<EId<InstanceElement>>,
    /// Dart `_isInStaticMember`.
    is_in_static_member: bool,
    /// A serial id of each frame of [Self::frames] (same length).
    frame_ids: Vec<u32>,
    next_frame_id: u32,
    /// The frames that a recorded lookup needs after they are popped.
    kept_frames: indexmap::IndexSet<u32>,
    /// The popped frames of [Self::kept_frames], complete (Dart keeps the
    /// scope objects on nodes; a `LocalScope` gets all the locals of its
    /// block).
    archived_frames: indexmap::IndexMap<u32, EnclosedScope<'a>>,
    /// The lookups that [Self::record_lookup] defers to
    /// [Self::finish_recorded_lookups].
    recorded_lookups: Vec<(NodeId, String, Vec<u32>)>,
}

impl<'a> ScopeContext<'a> {
    /// Dart `ScopeContext(libraryFragment:, nameScope: libraryFragment.scope)`.
    pub fn new(
        ctx: Ctx<'a>,
        scopes: &'a LibraryScopes,
        library: EId<LibraryElement>,
        library_fragment: FId<LibraryFragment>,
    ) -> ScopeContext<'a> {
        ScopeContext {
            ctx,
            scopes,
            library_fragment,
            wildcard_variables: library_feature_enabled(
                &ctx,
                library,
                ExperimentalFlag::WildcardVariables,
            ),
            primary_constructors: library_feature_enabled(
                &ctx,
                library,
                ExperimentalFlag::PrimaryConstructors,
            ),
            frames: Vec::new(),
            enclosing_instance_element: None,
            is_in_static_member: false,
            frame_ids: Vec::new(),
            next_frame_id: 0,
            kept_frames: indexmap::IndexSet::new(),
            archived_frames: indexmap::IndexMap::new(),
            recorded_lookups: Vec::new(),
        }
    }

    /// The library scopes of the unit.
    pub fn library_scopes(&self) -> &'a LibraryScopes {
        self.scopes
    }

    /// Dart `enclosingInstanceElement`.
    pub fn enclosing_instance_element(&self) -> Option<EId<InstanceElement>> {
        self.enclosing_instance_element
    }

    /// Sets Dart `_enclosingInstanceElement`; returns the previous value.
    pub fn set_enclosing_instance_element(
        &mut self,
        element: Option<EId<InstanceElement>>,
    ) -> Option<EId<InstanceElement>> {
        std::mem::replace(&mut self.enclosing_instance_element, element)
    }

    /// Sets Dart `_isInStaticMember`; returns the previous value.
    pub fn set_in_static_member(&mut self, value: bool) -> bool {
        std::mem::replace(&mut self.is_in_static_member, value)
    }

    /// Whether wildcard variables are enabled for the library.
    pub fn wildcard_variables(&self) -> bool {
        self.wildcard_variables
    }

    /// Dart `withScope(scope, ...)`, the start: [scope] becomes the current
    /// scope. Returns its index in the stack (for [Self::add_local]).
    pub fn push(&mut self, scope: EnclosedScope<'a>) -> usize {
        self.frames.push(scope);
        self.frame_ids.push(self.next_frame_id);
        self.next_frame_id += 1;
        self.frames.len() - 1
    }

    /// Dart `withScope(scope, ...)`, the end: the enclosing scope becomes
    /// the current scope again.
    pub fn pop(&mut self) {
        let frame = self.frames.pop();
        if let (Some(frame), Some(id)) = (frame, self.frame_ids.pop())
            && self.kept_frames.contains(&id)
        {
            self.archived_frames.insert(id, frame);
        }
    }

    /// Records the lookup of [id] in the current scope for [node], done in
    /// [Self::finish_recorded_lookups] when the scopes are complete. This
    /// stands for Dart `ScopeResolverVisitor.getNodeNameScope(node)
    /// .lookup(id)` after resolution (the linter `resolveNameInScope`).
    pub fn record_lookup(&mut self, node: NodeId, id: &str) {
        self.kept_frames.extend(self.frame_ids.iter().copied());
        self.recorded_lookups.push((node, id.to_string(), self.frame_ids.clone()));
    }

    /// The results of the lookups of [Self::record_lookup].
    pub fn finish_recorded_lookups(&mut self) -> Vec<(NodeId, ScopeLookupResult)> {
        let lookups = std::mem::take(&mut self.recorded_lookups);
        let mut results = Vec::with_capacity(lookups.len());
        for (node, id, chain) in lookups {
            let frames: Vec<&EnclosedScope<'a>> = chain
                .iter()
                .filter_map(|frame_id| {
                    self.archived_frames.get(frame_id).or_else(|| {
                        let index = self.frame_ids.iter().position(|i| i == frame_id)?;
                        self.frames.get(index)
                    })
                })
                .collect();
            if frames.len() != chain.len() {
                continue;
            }
            results.push((node, self.lookup_in_frames(&frames, frames.len(), &id)));
        }
        self.archived_frames.clear();
        self.kept_frames.clear();
        results
    }

    /// [Self::lookup_below] over a recorded chain of frames.
    fn lookup_in_frames(&self, frames: &[&EnclosedScope<'a>], top: usize, id: &str) -> ScopeLookupResult {
        let ctx = &self.ctx;
        for i in (0..top).rev() {
            let frame = frames[i];
            if frame.kind == EnclosedScopeKind::DocumentationComment {
                let result = self.lookup_in_frames(frames, i, id);
                if result.getter.is_some() || result.setter.is_some() {
                    return result;
                }
                return self.scopes.doc_import_lookup(id);
            }
            if let EnclosedLookup::Found(r) = frame.lookup_here(ctx, id) {
                return r;
            }
        }
        self.scopes.fragment_lookup(ctx, self.library_fragment, id)
    }

    /// The number of enclosed scopes (to check that pushes and pops match).
    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    /// Dart `withLocalScope`, the start: pushes a new `LocalScope`.
    pub fn push_local_scope(&mut self) -> usize {
        self.push(EnclosedScope::local_scope(self.wildcard_variables))
    }

    /// Dart `LocalScope.add(element)` of the local scope at [index].
    pub fn add_local(&mut self, index: usize, element: ElementId) {
        let ctx = self.ctx;
        if let Some(scope) = self.frames.get_mut(index) {
            debug_assert_eq!(scope.kind, EnclosedScopeKind::Local);
            scope.add(&ctx, element);
        }
    }

    /// Dart `withTypeParameterScope(elements, ...)`, the start.
    pub fn push_type_parameter_scope(&mut self, elements: &[EId<TypeParameterElement>]) -> usize {
        let scope =
            EnclosedScope::type_parameter_scope(&self.ctx, elements, self.wildcard_variables);
        self.push(scope)
    }

    /// Dart `withFormalParameterScope(elements, ...)`, the start.
    pub fn push_formal_parameter_scope(
        &mut self,
        elements: &[EId<FormalParameterElement>],
    ) -> usize {
        let scope =
            EnclosedScope::formal_parameter_scope(&self.ctx, elements, self.wildcard_variables);
        self.push(scope)
    }

    /// Dart `withConstructorInitializerScope(element, ...)`, the start.
    pub fn push_constructor_initializer_scope(
        &mut self,
        element: EId<ConstructorElement>,
    ) -> usize {
        let scope = EnclosedScope::constructor_initializer_scope(
            &self.ctx,
            element,
            self.wildcard_variables,
        );
        self.push(scope)
    }

    /// Dart `withPrimaryParameterScope(element, ...)`, the start.
    pub fn push_primary_parameter_scope(&mut self, element: EId<ConstructorElement>) -> usize {
        let scope =
            EnclosedScope::primary_parameter_scope(&self.ctx, element, self.wildcard_variables);
        self.push(scope)
    }

    /// Dart `withInstanceScope(element, ...)`, the start: also sets the
    /// enclosing instance element. Returns the previous enclosing instance
    /// element for [Self::pop_instance_scope].
    pub fn push_instance_scope(
        &mut self,
        element: EId<InstanceElement>,
    ) -> Option<EId<InstanceElement>> {
        let scope = EnclosedScope::instance_scope(&self.ctx, element);
        self.push(scope);
        self.set_enclosing_instance_element(Some(element))
    }

    /// Dart `withExtensionScope(element, ...)`, the start.
    pub fn push_extension_scope(
        &mut self,
        element: EId<dartr_element::ExtensionElement>,
    ) -> Option<EId<InstanceElement>> {
        let scope = EnclosedScope::extension_scope(&self.ctx, element);
        self.push(scope);
        self.set_enclosing_instance_element(Some(element.upcast()))
    }

    /// The end of [Self::push_instance_scope] / [Self::push_extension_scope]
    /// (Dart `_withEnclosingInstanceElement` resets the element to `null`).
    pub fn pop_instance_scope(&mut self, _previous: Option<EId<InstanceElement>>) {
        self.pop();
        self.enclosing_instance_element = None;
    }

    /// Dart `instantiateTypeParameter(element:, nullability:)`.
    pub fn instantiate_type_parameter(
        &self,
        element: EId<TypeParameterElement>,
        nullability: Nullability,
    ) -> TypeId {
        if self.is_in_static_member {
            let enclosing = self.ctx.get(element).enclosing;
            if enclosing.is_some_and(|e| e.is::<InstanceElement>()) {
                return TypeId::INVALID;
            }
        }
        self.ctx.type_parameter_type(element, nullability)
    }

    /// The lookup in the scopes below index [top] of the stack.
    fn lookup_below(&self, top: usize, id: &str) -> ScopeLookupResult {
        let ctx = &self.ctx;
        for i in (0..top).rev() {
            let frame = &self.frames[i];
            if frame.kind == EnclosedScopeKind::DocumentationComment {
                // Dart `DocumentationCommentScope.lookup`: the inner scope
                // first, then the doc imports.
                let result = self.lookup_below(i, id);
                if result.getter.is_some() || result.setter.is_some() {
                    return result;
                }
                return self.scopes.doc_import_lookup(id);
            }
            if let EnclosedLookup::Found(r) = frame.lookup_here(ctx, id) {
                return r;
            }
        }
        self.scopes.fragment_lookup(ctx, self.library_fragment, id)
    }
}

impl NameScope for ScopeContext<'_> {
    /// Dart `nameScope.lookup(id)`.
    fn lookup(&self, id: &str) -> ScopeLookupResult {
        self.lookup_below(self.frames.len(), id)
    }
}

/// The elements of the declared fragments of [nodes] (type parameters).
fn type_parameter_elements(
    v: &ResolutionVisitor<'_, '_>,
    ast: &Ast,
    list: Option<Id<TypeParameterList>>,
) -> Vec<EId<TypeParameterElement>> {
    let Some(list) = list else {
        return Vec::new();
    };
    ast.list(ast[list].type_parameters)
        .iter()
        .filter_map(|&tp| v.declared_element(tp.raw()))
        .filter_map(|e| e.cast())
        .collect()
}

impl<'a> ResolutionVisitor<'_, 'a> {
    /// Dart `withTypeParameterList(typeParameterList, ...)`, the start;
    /// returns whether a scope was pushed.
    pub(crate) fn push_type_parameter_list(
        &mut self,
        ast: &Ast,
        list: Option<Id<TypeParameterList>>,
    ) -> bool {
        if list.is_none() {
            return false;
        }
        let elements = type_parameter_elements(self, ast, list);
        self.scope_context.push_type_parameter_scope(&elements);
        true
    }

    /// The type parameters of the element of a declaration (Dart
    /// `element.typeParameters`).
    fn declared_type_parameters(&self, node: NodeId) -> Vec<EId<TypeParameterElement>> {
        let Some(e) = self.declared_element(node) else {
            return Vec::new();
        };
        element_type_parameters(&self.ctx, e)
    }

    /// The formal parameters of the element of a declaration (Dart
    /// `element.formalParameters`).
    fn declared_formal_parameters(&self, node: NodeId) -> Vec<EId<FormalParameterElement>> {
        let Some(e) = self.declared_element(node) else {
            return Vec::new();
        };
        match self.ctx.any(e) {
            dartr_element::AnyElement::Method(x) => x.formal_params.clone(),
            dartr_element::AnyElement::Constructor(x) => x.formal_params.clone(),
            dartr_element::AnyElement::Getter(x) => x.formal_params.clone(),
            dartr_element::AnyElement::Setter(x) => x.formal_params.clone(),
            dartr_element::AnyElement::TopLevelFunction(x) => x.formal_params.clone(),
            dartr_element::AnyElement::LocalFunction(x) => x.formal_params.clone(),
            dartr_element::AnyElement::GenericFunctionType(x) => x.formal_params.clone(),
            _ => Vec::new(),
        }
    }

    /// Dart `ScopeContext.visitClassDeclaration`.
    pub(crate) fn scope_visit_class_declaration(
        &mut self,
        ast: &mut Ast,
        node: Id<ClassDeclaration>,
    ) {
        let n = &ast[node];
        let (metadata, name_part, extends, with, implements, native, doc, body) = (
            n.metadata,
            n.name_part,
            n.extends_clause,
            n.with_clause,
            n.implements_clause,
            n.native_clause,
            n.documentation_comment,
            n.body,
        );
        self.visit_list(ast, metadata);
        let type_parameters = self.declared_type_parameters(node.raw());
        self.scope_context
            .push_type_parameter_scope(&type_parameters);
        let name_part_type_parameters = class_name_part_type_parameters(ast, name_part.raw());
        self.visit_opt(ast, name_part_type_parameters);
        self.visit_opt(ast, extends);
        self.visit_opt(ast, with);
        self.visit_opt(ast, implements);
        self.visit_opt(ast, native);
        if let Some(element) = self.declared_element(node.raw()).and_then(|e| e.cast()) {
            let previous = self.scope_context.push_instance_scope(element);
            self.visit_opt(ast, doc);
            if let Some(pc) = ast.cast::<PrimaryConstructorDeclaration>(name_part) {
                let formal_parameters = ast[pc].formal_parameters;
                self.visit(ast, formal_parameters);
            }
            self.visit(ast, body);
            self.scope_context.pop_instance_scope(previous);
        }
        self.scope_context.pop();
    }

    /// Dart `ScopeContext.visitClassTypeAlias` with the `visitSuperclass`
    /// of the resolution visitor.
    pub(crate) fn scope_visit_class_type_alias(&mut self, ast: &mut Ast, node: Id<ClassTypeAlias>) {
        let n = &ast[node];
        let (metadata, type_parameters, superclass, with, implements, doc) = (
            n.metadata,
            n.type_parameters,
            n.superclass,
            n.with_clause,
            n.implements_clause,
            n.documentation_comment,
        );
        self.visit_list(ast, metadata);
        let elements = self.declared_type_parameters(node.raw());
        self.scope_context.push_type_parameter_scope(&elements);
        self.visit_opt(ast, type_parameters);
        self.resolve_type(ast, Some(node.raw()), None, superclass);
        self.visit(ast, with);
        self.visit_opt(ast, implements);
        if let Some(element) = self.declared_element(node.raw()).and_then(|e| e.cast()) {
            let previous = self.scope_context.push_instance_scope(element);
            self.visit_opt(ast, doc);
            self.scope_context.pop_instance_scope(previous);
        }
        self.scope_context.pop();
    }

    /// Dart `ScopeContext.visitConstructorDeclaration`.
    pub(crate) fn scope_visit_constructor_declaration(
        &mut self,
        ast: &mut Ast,
        node: Id<ConstructorDeclaration>,
    ) {
        let n = &ast[node];
        let (metadata, type_name, parameters, initializers, doc, redirected, body) = (
            n.metadata,
            n.type_name,
            n.parameters,
            n.initializers,
            n.documentation_comment,
            n.redirected_constructor,
            n.body,
        );
        self.visit_list(ast, metadata);
        self.visit_opt(ast, type_name);
        self.visit(ast, parameters);
        let element = self
            .declared_element(node.raw())
            .and_then(|e| e.cast::<ConstructorElement>());
        if let Some(element) = element {
            self.scope_context
                .push_constructor_initializer_scope(element);
            self.visit_list(ast, initializers);
            self.visit_opt(ast, doc);
            self.scope_context.pop();
        } else {
            self.visit_list(ast, initializers);
            self.visit_opt(ast, doc);
        }
        self.visit_opt(ast, redirected);
        let formal_parameters = self.declared_formal_parameters(node.raw());
        self.scope_context
            .push_formal_parameter_scope(&formal_parameters);
        self.visit(ast, body);
        self.scope_context.pop();
    }

    /// Dart `ScopeContext.visitDocumentationComment`.
    pub(crate) fn scope_visit_documentation_comment(&mut self, ast: &mut Ast, node: Id<Comment>) {
        self.scope_context
            .push(EnclosedScope::documentation_comment_scope());
        self.visit_children(ast, node.raw());
        self.scope_context.pop();
    }

    /// Dart `ScopeContext.visitEnumDeclaration`.
    pub(crate) fn scope_visit_enum_declaration(
        &mut self,
        ast: &mut Ast,
        node: Id<EnumDeclaration>,
    ) {
        let n = &ast[node];
        let (metadata, name_part, with, implements, doc, body) = (
            n.metadata,
            n.name_part,
            n.with_clause,
            n.implements_clause,
            n.documentation_comment,
            n.body,
        );
        self.visit_list(ast, metadata);
        let type_parameters = self.declared_type_parameters(node.raw());
        self.scope_context
            .push_type_parameter_scope(&type_parameters);
        let name_part_type_parameters = class_name_part_type_parameters(ast, name_part.raw());
        self.visit_opt(ast, name_part_type_parameters);
        self.visit_opt(ast, with);
        self.visit_opt(ast, implements);
        if let Some(element) = self.declared_element(node.raw()).and_then(|e| e.cast()) {
            let previous = self.scope_context.push_instance_scope(element);
            self.visit_opt(ast, doc);
            if let Some(pc) = ast.cast::<PrimaryConstructorDeclaration>(name_part) {
                let formal_parameters = ast[pc].formal_parameters;
                self.visit(ast, formal_parameters);
            }
            self.visit(ast, body);
            self.scope_context.pop_instance_scope(previous);
        }
        self.scope_context.pop();
    }

    /// Dart `ScopeContext.visitExtensionDeclaration`.
    pub(crate) fn scope_visit_extension_declaration(
        &mut self,
        ast: &mut Ast,
        node: Id<ExtensionDeclaration>,
    ) {
        let n = &ast[node];
        let (metadata, type_parameters, on_clause, doc, body) = (
            n.metadata,
            n.type_parameters,
            n.on_clause,
            n.documentation_comment,
            n.body,
        );
        self.visit_list(ast, metadata);
        let elements = self.declared_type_parameters(node.raw());
        self.scope_context.push_type_parameter_scope(&elements);
        self.visit_opt(ast, type_parameters);
        self.visit_opt(ast, on_clause);
        if let Some(element) = self.declared_element(node.raw()).and_then(|e| e.cast()) {
            let previous = self.scope_context.push_extension_scope(element);
            self.visit_opt(ast, doc);
            self.visit(ast, body);
            self.scope_context.pop_instance_scope(previous);
        }
        self.scope_context.pop();
    }

    /// Dart `ScopeContext.visitExtensionTypeDeclaration`.
    pub(crate) fn scope_visit_extension_type_declaration(
        &mut self,
        ast: &mut Ast,
        node: Id<ExtensionTypeDeclaration>,
    ) {
        let n = &ast[node];
        let (metadata, name_part, implements, doc, body) = (
            n.metadata,
            n.name_part,
            n.implements_clause,
            n.documentation_comment,
            n.body,
        );
        self.visit_list(ast, metadata);
        let type_parameters = self.declared_type_parameters(node.raw());
        self.scope_context
            .push_type_parameter_scope(&type_parameters);
        let name_part_type_parameters = class_name_part_type_parameters(ast, name_part.raw());
        self.visit_opt(ast, name_part_type_parameters);
        self.visit_opt(ast, implements);
        let primary_formal_parameters = ast
            .cast::<PrimaryConstructorDeclaration>(name_part)
            .map(|pc| ast[pc].formal_parameters);
        let element = self.declared_element(node.raw()).and_then(|e| e.cast());
        if self.scope_context.primary_constructors {
            if let Some(element) = element {
                let previous = self.scope_context.push_instance_scope(element);
                self.visit_opt(ast, doc);
                self.visit_opt(ast, primary_formal_parameters);
                self.visit(ast, body);
                self.scope_context.pop_instance_scope(previous);
            }
        } else {
            self.visit_opt(ast, primary_formal_parameters);
            if let Some(element) = element {
                let previous = self.scope_context.push_instance_scope(element);
                self.visit_opt(ast, doc);
                self.visit(ast, body);
                self.scope_context.pop_instance_scope(previous);
            }
        }
        self.scope_context.pop();
    }

    /// Dart `ScopeContext.visitFieldDeclaration`.
    pub(crate) fn scope_visit_field_declaration(
        &mut self,
        ast: &mut Ast,
        node: Id<FieldDeclaration>,
    ) {
        let is_static = ast[node].static_keyword.is_some();
        let outer = self.scope_context.set_in_static_member(is_static);
        self.visit_children(ast, node.raw());
        self.scope_context.set_in_static_member(outer);
    }

    /// Dart `ScopeContext.visitFormalParameter`.
    pub(crate) fn scope_visit_formal_parameter(&mut self, ast: &mut Ast, node: NodeId) {
        let parts = formal_parameter_parts(ast, node);
        self.visit_list(ast, parts.metadata);
        self.visit_opt(ast, parts.documentation_comment);
        match parts.function_typed_suffix {
            None => self.visit_opt(ast, parts.type_),
            Some(suffix) => {
                let (type_parameters, formal_parameters) =
                    (ast[suffix].type_parameters, ast[suffix].formal_parameters);
                let pushed = self.push_type_parameter_list(ast, type_parameters);
                self.visit_opt(ast, parts.type_);
                self.visit_opt(ast, type_parameters);
                self.visit(ast, formal_parameters);
                if pushed {
                    self.scope_context.pop();
                }
            }
        }
        self.visit_opt(ast, parts.default_clause);
    }

    /// Dart `ScopeContext.visitFunctionDeclaration`.
    pub(crate) fn scope_visit_function_declaration(
        &mut self,
        ast: &mut Ast,
        node: Id<FunctionDeclaration>,
    ) {
        let n = &ast[node];
        let (metadata, return_type, expression, doc) = (
            n.metadata,
            n.return_type,
            n.function_expression,
            n.documentation_comment,
        );
        let (type_parameters, parameters, body) = {
            let e = &ast[expression];
            (e.type_parameters, e.parameters, e.body)
        };
        self.visit_list(ast, metadata);
        let elements = self.declared_type_parameters(node.raw());
        self.scope_context.push_type_parameter_scope(&elements);
        self.visit_opt(ast, return_type);
        self.visit_opt(ast, type_parameters);
        self.visit_opt(ast, parameters);
        let formal_parameters = self.declared_formal_parameters(node.raw());
        self.scope_context
            .push_formal_parameter_scope(&formal_parameters);
        self.visit_opt(ast, doc);
        self.visit(ast, body);
        self.scope_context.pop();
        self.scope_context.pop();
    }

    /// Dart `ScopeContext.visitFunctionExpression`.
    pub(crate) fn scope_visit_function_expression(
        &mut self,
        ast: &mut Ast,
        node: Id<FunctionExpression>,
    ) {
        let (type_parameters, parameters, body) = {
            let e = &ast[node];
            (e.type_parameters, e.parameters, e.body)
        };
        let pushed = self.push_type_parameter_list(ast, type_parameters);
        self.visit_opt(ast, type_parameters);
        self.visit_opt(ast, parameters);
        let formal_parameters = self.declared_formal_parameters(node.raw());
        self.scope_context
            .push_formal_parameter_scope(&formal_parameters);
        self.visit(ast, body);
        self.scope_context.pop();
        if pushed {
            self.scope_context.pop();
        }
    }

    /// Dart `ScopeContext.visitFunctionTypeAlias`.
    pub(crate) fn scope_visit_function_type_alias(
        &mut self,
        ast: &mut Ast,
        node: Id<FunctionTypeAlias>,
    ) {
        let n = &ast[node];
        let (metadata, return_type, type_parameters, parameters, doc) = (
            n.metadata,
            n.return_type,
            n.type_parameters,
            n.parameters,
            n.documentation_comment,
        );
        self.visit_list(ast, metadata);
        let elements = self.declared_type_parameters(node.raw());
        self.scope_context.push_type_parameter_scope(&elements);
        self.visit_opt(ast, return_type);
        self.visit_opt(ast, type_parameters);
        self.visit(ast, parameters);
        let scope = self.scope_context.push_local_scope();
        self.add_formal_parameter_list(ast, scope, parameters);
        self.visit_opt(ast, doc);
        self.scope_context.pop();
        self.scope_context.pop();
    }

    /// Dart `ScopeContext.visitGenericFunctionType`.
    pub(crate) fn scope_visit_generic_function_type(
        &mut self,
        ast: &mut Ast,
        node: Id<GenericFunctionType>,
    ) {
        let n = &ast[node];
        let (type_parameters, parameters, return_type) =
            (n.type_parameters, n.parameters, n.return_type);
        let pushed = self.push_type_parameter_list(ast, type_parameters);
        self.visit_opt(ast, type_parameters);
        self.visit(ast, parameters);
        self.visit_opt(ast, return_type);
        if pushed {
            self.scope_context.pop();
        }
    }

    /// Dart `ScopeContext.visitGenericTypeAlias`.
    pub(crate) fn scope_visit_generic_type_alias(
        &mut self,
        ast: &mut Ast,
        node: Id<GenericTypeAlias>,
    ) {
        let n = &ast[node];
        let (metadata, type_parameters, type_, doc) = (
            n.metadata,
            n.type_parameters,
            n.type_,
            n.documentation_comment,
        );
        self.visit_list(ast, metadata);
        let elements = self.declared_type_parameters(node.raw());
        self.scope_context.push_type_parameter_scope(&elements);
        self.visit_opt(ast, type_parameters);
        self.visit(ast, type_);
        // The type node may be rewritten? No: type annotations are not
        // rewritten, so `type_` is still the child.
        if let Some(function_type) = ast.cast::<GenericFunctionType>(type_) {
            let (ft_type_parameters, ft_parameters) = (
                ast[function_type].type_parameters,
                ast[function_type].parameters,
            );
            let pushed = self.push_type_parameter_list(ast, ft_type_parameters);
            let scope = self.scope_context.push_local_scope();
            self.add_formal_parameter_list(ast, scope, ft_parameters);
            self.visit_opt(ast, doc);
            self.scope_context.pop();
            if pushed {
                self.scope_context.pop();
            }
        } else {
            self.visit_opt(ast, doc);
        }
        self.scope_context.pop();
    }

    /// Dart `ScopeContext.visitMethodDeclaration`.
    pub(crate) fn scope_visit_method_declaration(
        &mut self,
        ast: &mut Ast,
        node: Id<MethodDeclaration>,
    ) {
        let n = &ast[node];
        let (metadata, modifier, return_type, type_parameters, parameters, doc, body) = (
            n.metadata,
            n.modifier_keyword,
            n.return_type,
            n.type_parameters,
            n.parameters,
            n.documentation_comment,
            n.body,
        );
        let is_static = modifier.is_some_and(|m| ast.tokens.lexeme(m) == "static");
        self.visit_list(ast, metadata);
        let outer = self.scope_context.set_in_static_member(is_static);
        let elements = self.declared_type_parameters(node.raw());
        self.scope_context.push_type_parameter_scope(&elements);
        self.visit_opt(ast, return_type);
        self.visit_opt(ast, type_parameters);
        self.visit_opt(ast, parameters);
        let formal_parameters = self.declared_formal_parameters(node.raw());
        self.scope_context
            .push_formal_parameter_scope(&formal_parameters);
        self.visit_opt(ast, doc);
        self.visit(ast, body);
        self.scope_context.pop();
        self.scope_context.pop();
        self.scope_context.set_in_static_member(outer);
    }

    /// Dart `ScopeContext.visitMixinDeclaration`.
    pub(crate) fn scope_visit_mixin_declaration(
        &mut self,
        ast: &mut Ast,
        node: Id<MixinDeclaration>,
    ) {
        let n = &ast[node];
        let (metadata, type_parameters, on_clause, implements, doc, body) = (
            n.metadata,
            n.type_parameters,
            n.on_clause,
            n.implements_clause,
            n.documentation_comment,
            n.body,
        );
        self.visit_list(ast, metadata);
        let elements = self.declared_type_parameters(node.raw());
        self.scope_context.push_type_parameter_scope(&elements);
        self.visit_opt(ast, type_parameters);
        self.visit_opt(ast, on_clause);
        self.visit_opt(ast, implements);
        if let Some(element) = self.declared_element(node.raw()).and_then(|e| e.cast()) {
            let previous = self.scope_context.push_instance_scope(element);
            self.visit_opt(ast, doc);
            self.visit(ast, body);
            self.scope_context.pop_instance_scope(previous);
        }
        self.scope_context.pop();
    }

    /// Dart `ScopeContext.visitPrimaryConstructorBody`.
    pub(crate) fn scope_visit_primary_constructor_body(
        &mut self,
        ast: &mut Ast,
        node: Id<PrimaryConstructorBody>,
    ) {
        let element = primary_constructor_body_declaration(ast, node)
            .and_then(|d| self.declared_element(d.raw()))
            .and_then(|e| e.cast::<ConstructorElement>());
        let n = &ast[node];
        let (metadata, initializers, doc, body) =
            (n.metadata, n.initializers, n.documentation_comment, n.body);
        self.visit_list(ast, metadata);
        match element {
            Some(e) => {
                self.scope_context.push_constructor_initializer_scope(e);
                self.visit_list(ast, initializers);
                self.scope_context.pop();
                self.scope_context.push_primary_parameter_scope(e);
                self.visit_opt(ast, doc);
                self.visit(ast, body);
                self.scope_context.pop();
            }
            None => {
                self.visit_list(ast, initializers);
                self.visit_opt(ast, doc);
                self.visit(ast, body);
            }
        }
    }

    /// Dart `ScopeContext.visitVariableDeclarationList`.
    pub(crate) fn scope_visit_variable_declaration_list(
        &mut self,
        ast: &mut Ast,
        node: Id<VariableDeclarationList>,
    ) {
        let n = &ast[node];
        let (metadata, type_, late, variables) = (n.metadata, n.type_, n.late_keyword, n.variables);
        self.visit_list(ast, metadata);
        self.visit_opt(ast, type_);

        // Use different scope for instance non-late field initializers.
        let mut pushed = false;
        if let Some(field_declaration) = ast
            .parent(node)
            .and_then(|p| ast.cast::<FieldDeclaration>(p))
        {
            if ast[field_declaration].static_keyword.is_none() && late.is_none() {
                let primary_constructor = self
                    .scope_context
                    .enclosing_instance_element()
                    .and_then(|e| primary_constructor_of(&self.ctx, e));
                if let Some(c) = primary_constructor {
                    self.scope_context.push_constructor_initializer_scope(c);
                    pushed = true;
                }
            }
        }
        self.visit_list(ast, variables);
        if pushed {
            self.scope_context.pop();
        }
    }

    /// Dart `LocalScopeExtension.addFormalParameterList`.
    pub(crate) fn add_formal_parameter_list(
        &mut self,
        ast: &Ast,
        scope: usize,
        list: Id<dartr_ast::FormalParameterList>,
    ) {
        for &p in ast.list(ast[list].parameters) {
            if let Some(e) = self.declared_element(p.raw()) {
                self.scope_context.add_local(scope, e);
            }
        }
    }
}

/// The type parameters of a class name part (Dart
/// `namePart.typeParameters`).
fn class_name_part_type_parameters(ast: &Ast, name_part: NodeId) -> Option<Id<TypeParameterList>> {
    if let Some(n) = ast.cast::<dartr_ast::NameWithTypeParameters>(name_part) {
        ast[n].type_parameters
    } else if let Some(p) = ast.cast::<PrimaryConstructorDeclaration>(name_part) {
        ast[p].type_parameters
    } else {
        None
    }
}

/// Dart `element.typeParameters` for the elements that have them.
pub(crate) fn element_type_parameters(
    ctx: &Ctx<'_>,
    e: ElementId,
) -> Vec<EId<TypeParameterElement>> {
    use dartr_element::AnyElement as A;
    match ctx.any(e) {
        A::Class(x) => x.type_params.clone(),
        A::Enum(x) => x.type_params.clone(),
        A::Mixin(x) => x.type_params.clone(),
        A::ExtensionType(x) => x.type_params.clone(),
        A::Extension(x) => x.type_params.clone(),
        A::Method(x) => x.type_params.clone(),
        A::Constructor(x) => x.type_params.clone(),
        A::Getter(x) => x.type_params.clone(),
        A::Setter(x) => x.type_params.clone(),
        A::TopLevelFunction(x) => x.type_params.clone(),
        A::LocalFunction(x) => x.type_params.clone(),
        A::TypeAlias(x) => x.type_params.clone(),
        A::GenericFunctionType(x) => x.type_params.clone(),
        _ => Vec::new(),
    }
}

/// Dart `InterfaceElementImpl.primaryConstructor`: the constructor whose
/// fragment is a primary constructor.
pub(crate) fn primary_constructor_of(
    ctx: &Ctx<'_>,
    element: EId<InstanceElement>,
) -> Option<EId<ConstructorElement>> {
    if !matches!(
        element.raw().tag(),
        Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType
    ) {
        return None;
    }
    let interface = ctx.interface(element.raw().cast()?);
    interface.constructors.iter().copied().find(|&c| {
        let first = ctx.get(c).first_fragment();
        ctx.fragment(first)
            .flags
            .has(dartr_element::FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_PRIMARY)
    })
}
