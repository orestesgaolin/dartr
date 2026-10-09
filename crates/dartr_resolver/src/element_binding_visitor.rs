// Dart source: pkg/analyzer/lib/src/dart/resolver/element_binding_visitor.dart

//! `ElementBindingVisitor`: binds the linked fragments of the declarations
//! of a unit to their nodes (`ResolutionTables.declared_fragment`, walking
//! the fragments with [`crate::element_walker`] in declaration order), and
//! creates the fragments and elements of local declarations (local
//! variables, local functions, function expressions, formal parameters of
//! local functions, type parameters, labels, catch parameters, declared
//! identifiers, pattern variables, generic function types) in the unit's
//! local arena (`ctx.local`).
//!
//! # Local fragments: staging
//!
//! The arenas of the local store are append-only through `&self`, and the
//! data of a stored fragment cannot change (except its slots and flags).
//! The Dart visitor changes a local fragment after it creates it (it sets
//! `typeParameters` and `formalParameters` of a local function after it
//! visits them, `metadata`, the code range). So this port keeps the new
//! fragments in staging vectors while it walks the unit, and adds them to
//! the local store at the end ([`Staging::commit`]), in creation order. The
//! id of a staged fragment is known when it is created: the index is the
//! length of the arena at the start plus the position in the staging vector
//! (the visitor is the only writer of the local store while it runs).
//!
//! Elements: Dart creates the elements of local fragments at different
//! times (some lazily, `late final element`). The commit creates exactly
//! one element per staged fragment, in the same order, so the element of
//! the staged fragment `i` of a kind is the element `i` of that kind
//! ([`Staging::element_of`]). The element fields that Dart computes from the
//! fragment (name, library, enclosing element, type parameters and formal
//! parameters of executables, the covariant flag of parameters) are set at
//! the commit.
//!
//! # Linked fragments
//!
//! The Dart visitor also sets the code range of linked fragments and the
//! metadata of fragments; here only staged fragments change. A linked
//! fragment is frozen and shared with other analysis tasks (design §2.5).
//!
//! # Annotations
//!
//! Dart `AnnotationImpl.elementAnnotation` is [`ResolverTables::element_annotation`]:
//! the fragment whose metadata has the annotation, or the library fragment
//! of the unit for annotations of directives and annotations without a
//! fragment (Dart `ElementAnnotationImpl(_libraryFragment, node)`).
//!
//! # Directives
//!
//! The import, export and part directives are matched with
//! `LibraryFragment.library_imports` / `library_exports` / `parts` by
//! position (Dart `LibraryAnalyzer._resolveDirectives` sets
//! `node.libraryImport` the same way). As Dart `_resolveLibraryImportDirective`
//! does, the prefix identifier of an import gets the prefix element
//! (`ResolutionTables.element`).

use dartr_ast::{
    Annotation, AnonymousMethodInvocation, Ast, AstVisitor, CatchClause, ClassDeclaration,
    ClassTypeAlias, CompilationUnit, ConstructorDeclaration, DeclaredIdentifier,
    DeclaredVariablePattern, EnumConstantDeclaration, EnumDeclaration, ExportDirective,
    ExtensionDeclaration, ExtensionTypeDeclaration, FieldDeclaration, FieldFormalParameter,
    FunctionDeclaration, FunctionDeclarationStatement, FunctionExpression, FunctionTypeAlias,
    GenericFunctionType, GenericTypeAlias, Id, ImportDirective, Label, LabeledStatement,
    LibraryDirective, MethodDeclaration, MixinDeclaration, NativeFunctionBody, NodeId, NodeList,
    PartDirective, PartOfDirective, PrimaryConstructorBody, PrimaryConstructorDeclaration,
    RecordTypeAnnotation, RegularFormalParameter, SuperFormalParameter, SwitchStatement,
    TopLevelVariableDeclaration, TypeParameter, VariableDeclaration, VariableDeclarationList,
};
use dartr_element::{
    ConstExprId, Ctx, EId, ElemRef, ElementAnnotation, ElementData, ElementFlags, ElementId,
    ElementStore, ExecutableElementData, ExecutableFragmentData, FId, FormalParameterElement,
    FormalParameterFragment, FragmentData, FragmentFlags, FragmentId, GenericFunctionTypeElement,
    GenericFunctionTypeFragment, LabelElement, LabelFragment, LibraryElement, LibraryFragment,
    LocalFunctionElement, LocalFunctionFragment, LocalVariableElement, LocalVariableFragment,
    Metadata, Name, ParameterKind, PatternVariableFragmentData, ResolutionTables, Tag, TypeId,
    TypeParameterElement, TypeParameterFragment, VarSlot, VariableElementData,
    VariableFragmentData,
};

use crate::ast_ext::{
    formal_parameter_parts, function_body_is_asynchronous, function_body_is_generator, is_keyword,
    name_if_not_empty, offset_if_not_empty, token_end,
};
use crate::element_walker::ElementWalker;
use crate::tables::ResolverTables;

/// Dart `unit.accept(ElementBindingVisitor.forAnalysis(fragment: fragment,
/// walker: ElementWalker.forCompilationUnit(fragment)))`.
///
/// Also sets `CompilationUnit.declaredFragment` (Dart
/// `LibraryAnalyzer._resolveFile`). Needs a context with a local arena.
pub fn bind_unit(
    ctx: &Ctx<'_>,
    ast: &Ast,
    unit: Id<CompilationUnit>,
    fragment: FId<LibraryFragment>,
    tables: &mut ResolutionTables,
    rt: &mut ResolverTables,
) {
    let local = &ctx
        .local
        .expect("element binding needs a context with a local arena")
        .store;
    tables.declared_fragment.insert(unit, fragment.raw());
    let library = ctx.fragment(fragment).library;
    let mut visitor = ElementBindingVisitor {
        ctx: *ctx,
        library,
        library_fragment: fragment,
        library_directive_index: 0,
        import_index: 0,
        export_index: 0,
        part_index: 0,
        walker: Some(ElementWalker::for_compilation_unit(ctx, fragment)),
        holder: ElementHolder::new(fragment.raw()),
        staging: Staging::new(local),
        tables,
        rt,
    };
    ast.accept(unit, &mut visitor);
    let staging = std::mem::replace(&mut visitor.staging, Staging::new(local));
    staging.commit(ctx, local, library);
}

/// Dart `ElementBindingVisitor.forPartialResolution(fragment: fragment)
/// .bindSubtree(enclosingFragment, node)`: binds the local declarations of
/// the subtree [node] (summary2 `AstResolver`: initializers, default
/// values, annotations and constructor initializers while linking). The
/// local fragments are enclosed by [enclosing_fragment]. Needs a context
/// with a local arena.
pub fn bind_subtree(
    ctx: &Ctx<'_>,
    ast: &Ast,
    fragment: FId<LibraryFragment>,
    enclosing_fragment: FragmentId,
    node: NodeId,
    tables: &mut ResolutionTables,
    rt: &mut ResolverTables,
) {
    let local = &ctx
        .local
        .expect("element binding needs a context with a local arena")
        .store;
    let library = ctx.fragment(fragment).library;
    let mut visitor = ElementBindingVisitor {
        ctx: *ctx,
        library,
        library_fragment: fragment,
        library_directive_index: 0,
        import_index: 0,
        export_index: 0,
        part_index: 0,
        walker: None,
        holder: ElementHolder::new(enclosing_fragment),
        staging: Staging::new(local),
        tables,
        rt,
    };
    ast.accept(node, &mut visitor);
    let staging = std::mem::replace(&mut visitor.staging, Staging::new(local));
    staging.commit(ctx, local, library);
}

// ------------------------------------------------------------------ staging

/// The staged fragments of one kind (see the module documentation).
struct Staged<T> {
    /// The index of the first staged fragment in the fragment arena.
    base: u32,
    /// The index of the first element of this kind in the element arena.
    element_base: u32,
    items: Vec<(Tag, T)>,
}

impl<T> Staged<T> {
    fn new(base: usize, element_base: usize) -> Self {
        Staged {
            base: base as u32,
            element_base: element_base as u32,
            items: Vec::new(),
        }
    }

    fn position(&self, id: FragmentId) -> Option<usize> {
        let index = id.index();
        if index < self.base {
            return None;
        }
        let position = (index - self.base) as usize;
        (position < self.items.len()).then_some(position)
    }
}

/// The fragments created while the unit is walked (see the module
/// documentation).
struct Staging {
    store: dartr_element::StoreId,
    locals: Staged<LocalVariableFragment>,
    local_functions: Staged<LocalFunctionFragment>,
    labels: Staged<LabelFragment>,
    params: Staged<FormalParameterFragment>,
    type_params: Staged<TypeParameterFragment>,
    generic_function_types: Staged<GenericFunctionTypeFragment>,
}

impl Staging {
    fn new(store: &ElementStore) -> Staging {
        let f = &store.fragments;
        let e = &store.elements;
        Staging {
            store: store.id,
            locals: Staged::new(f.locals.len(), e.locals.len()),
            local_functions: Staged::new(f.local_functions.len(), e.local_functions.len()),
            labels: Staged::new(f.labels.len(), e.labels.len()),
            params: Staged::new(f.params.len(), e.params.len()),
            type_params: Staged::new(f.type_params.len(), e.type_params.len()),
            generic_function_types: Staged::new(
                f.generic_function_types.len(),
                e.generic_function_types.len(),
            ),
        }
    }

    fn push<T>(
        store: dartr_element::StoreId,
        staged: &mut Staged<T>,
        tag: Tag,
        item: T,
    ) -> FragmentId {
        let index = staged.base + staged.items.len() as u32;
        staged.items.push((tag, item));
        FragmentId::new(store, tag, index)
    }

    fn add_local(&mut self, tag: Tag, f: LocalVariableFragment) -> FragmentId {
        Self::push(self.store, &mut self.locals, tag, f)
    }

    fn add_local_function(&mut self, f: LocalFunctionFragment) -> FragmentId {
        Self::push(self.store, &mut self.local_functions, Tag::LocalFunction, f)
    }

    fn add_label(&mut self, f: LabelFragment) -> FragmentId {
        Self::push(self.store, &mut self.labels, Tag::Label, f)
    }

    fn add_param(&mut self, tag: Tag, f: FormalParameterFragment) -> FragmentId {
        Self::push(self.store, &mut self.params, tag, f)
    }

    fn add_type_param(&mut self, f: TypeParameterFragment) -> FragmentId {
        Self::push(self.store, &mut self.type_params, Tag::TypeParameter, f)
    }

    fn add_generic_function_type(&mut self, f: GenericFunctionTypeFragment) -> FragmentId {
        Self::push(
            self.store,
            &mut self.generic_function_types,
            Tag::GenericFunctionType,
            f,
        )
    }

    /// The `FragmentImpl` data of a staged fragment, to change it.
    fn fragment_mut(&mut self, id: FragmentId) -> Option<&mut FragmentData> {
        if id.store() != self.store {
            return None;
        }
        match id.tag() {
            Tag::LocalVariable | Tag::BindPatternVariable | Tag::PatternVariable => {
                let p = self.locals.position(id)?;
                Some(&mut self.locals.items[p].1.variable.fragment)
            }
            Tag::LocalFunction => {
                let p = self.local_functions.position(id)?;
                Some(&mut self.local_functions.items[p].1.executable.fragment)
            }
            Tag::Label => {
                let p = self.labels.position(id)?;
                Some(&mut self.labels.items[p].1.fragment)
            }
            Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter => {
                let p = self.params.position(id)?;
                Some(&mut self.params.items[p].1.variable.fragment)
            }
            Tag::TypeParameter => {
                let p = self.type_params.position(id)?;
                Some(&mut self.type_params.items[p].1.fragment)
            }
            Tag::GenericFunctionType => {
                let p = self.generic_function_types.position(id)?;
                Some(&mut self.generic_function_types.items[p].1.fragment)
            }
            _ => None,
        }
    }

    fn param_mut(&mut self, id: FragmentId) -> Option<&mut FormalParameterFragment> {
        if id.store() != self.store {
            return None;
        }
        let p = self.params.position(id)?;
        Some(&mut self.params.items[p].1)
    }

    fn local_function_mut(&mut self, id: FragmentId) -> Option<&mut LocalFunctionFragment> {
        if id.store() != self.store || id.tag() != Tag::LocalFunction {
            return None;
        }
        let p = self.local_functions.position(id)?;
        Some(&mut self.local_functions.items[p].1)
    }

    fn generic_function_type_mut(
        &mut self,
        id: FragmentId,
    ) -> Option<&mut GenericFunctionTypeFragment> {
        if id.store() != self.store {
            return None;
        }
        let p = self.generic_function_types.position(id)?;
        Some(&mut self.generic_function_types.items[p].1)
    }

    /// Whether [id] is a staged fragment.
    fn is_staged(&self, id: FragmentId) -> bool {
        if id.store() != self.store {
            return false;
        }
        match id.tag() {
            Tag::LocalVariable | Tag::BindPatternVariable | Tag::PatternVariable => {
                self.locals.position(id).is_some()
            }
            Tag::LocalFunction => self.local_functions.position(id).is_some(),
            Tag::Label => self.labels.position(id).is_some(),
            Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter => {
                self.params.position(id).is_some()
            }
            Tag::TypeParameter => self.type_params.position(id).is_some(),
            Tag::GenericFunctionType => self.generic_function_types.position(id).is_some(),
            _ => false,
        }
    }

    /// The element of the staged fragment [id] (the element that
    /// [Staging::commit] creates for it).
    fn element_of(&self, id: FragmentId) -> Option<ElementId> {
        fn of<T>(
            staged: &Staged<T>,
            store: dartr_element::StoreId,
            id: FragmentId,
        ) -> Option<ElementId> {
            let p = staged.position(id)?;
            Some(ElementId::new(
                store,
                id.tag(),
                staged.element_base + p as u32,
            ))
        }
        if id.store() != self.store {
            return None;
        }
        match id.tag() {
            Tag::LocalVariable | Tag::BindPatternVariable | Tag::PatternVariable => {
                of(&self.locals, self.store, id)
            }
            Tag::LocalFunction => of(&self.local_functions, self.store, id),
            Tag::Label => of(&self.labels, self.store, id),
            Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter => {
                of(&self.params, self.store, id)
            }
            Tag::TypeParameter => of(&self.type_params, self.store, id),
            Tag::GenericFunctionType => of(&self.generic_function_types, self.store, id),
            _ => None,
        }
    }

    /// The element of any fragment: staged, or already in a store.
    fn element_of_any(&self, ctx: &Ctx<'_>, id: FragmentId) -> Option<ElementId> {
        if self.is_staged(id) {
            return self.element_of(id);
        }
        ctx.fragment_data(id)?.element.try_get().copied()
    }

    /// Adds the staged fragments and their elements to the local store.
    fn commit(self, ctx: &Ctx<'_>, store: &ElementStore, library: EId<LibraryElement>) {
        let enclosing_of = |f: &FragmentData| -> Option<ElementId> {
            f.enclosing_fragment
                .and_then(|e| self.element_of_any(ctx, e))
        };
        let element_data = |f: &FragmentData, first: FragmentId, enclosing: Option<ElementId>| {
            let mut data = ElementData::new(f.name, first);
            data.library = Some(library);
            data.enclosing = enclosing;
            data
        };
        let elements_of = |ids: &[FId<TypeParameterFragment>]| -> Vec<EId<TypeParameterElement>> {
            ids.iter()
                .filter_map(|f| self.element_of_any(ctx, f.raw()))
                .filter_map(|e| e.cast())
                .collect()
        };
        let params_of = |ids: &[FId<FormalParameterFragment>]| -> Vec<EId<FormalParameterElement>> {
            ids.iter()
                .filter_map(|f| self.element_of_any(ctx, f.raw()))
                .filter_map(|e| e.cast())
                .collect()
        };

        // Compute the element data before the fragments move.
        let mut local_elements = Vec::new();
        for (i, (tag, f)) in self.locals.items.iter().enumerate() {
            let id = FragmentId::new(self.store, *tag, self.locals.base + i as u32);
            let e = LocalVariableElement {
                variable: VariableElementData::new(element_data(f, id, enclosing_of(f))),
                type_: VarSlot::with(TypeId::INVALID),
            };
            local_elements.push((*tag, e));
        }
        let mut function_elements = Vec::new();
        for (i, (_, f)) in self.local_functions.items.iter().enumerate() {
            let id = FragmentId::new(
                self.store,
                Tag::LocalFunction,
                self.local_functions.base + i as u32,
            );
            // Dart `LocalFunctionElementImpl.enclosingElement` is `null`.
            let mut executable = ExecutableElementData::new(element_data(f, id, None));
            executable.type_params = elements_of(&f.type_params);
            executable.formal_params = params_of(&f.formal_params);
            function_elements.push(LocalFunctionElement { executable });
        }
        let mut label_elements = Vec::new();
        for (i, (_, f)) in self.labels.items.iter().enumerate() {
            let id = FragmentId::new(self.store, Tag::Label, self.labels.base + i as u32);
            // Dart `LabelElementImpl.enclosingElement` is `null`.
            label_elements.push(LabelElement {
                element: element_data(f, id, None),
            });
        }
        let mut param_elements = Vec::new();
        for (i, (tag, f)) in self.params.items.iter().enumerate() {
            let id = FragmentId::new(self.store, *tag, self.params.base + i as u32);
            let covariant = f
                .flags
                .has(FragmentFlags::FORMAL_PARAMETER_FRAGMENT_IS_EXPLICITLY_COVARIANT);
            let e = FormalParameterElement {
                variable: VariableElementData::new(element_data(f, id, enclosing_of(f))),
                kind: f.parameter_kind,
                type_: VarSlot::with(TypeId::INVALID),
                base_formal_parameter: None,
                field: VarSlot::new(),
            };
            param_elements.push((*tag, e, covariant));
        }
        let mut type_param_elements = Vec::new();
        for (i, (_, f)) in self.type_params.items.iter().enumerate() {
            let id = FragmentId::new(
                self.store,
                Tag::TypeParameter,
                self.type_params.base + i as u32,
            );
            type_param_elements.push(TypeParameterElement::new(element_data(
                f,
                id,
                enclosing_of(f),
            )));
        }
        let mut gft_elements = Vec::new();
        for (i, (_, f)) in self.generic_function_types.items.iter().enumerate() {
            let id = FragmentId::new(
                self.store,
                Tag::GenericFunctionType,
                self.generic_function_types.base + i as u32,
            );
            gft_elements.push(GenericFunctionTypeElement {
                element: element_data(f, id, enclosing_of(f)),
                type_params: elements_of(&f.type_params),
                formal_params: params_of(&f.formal_params),
                return_type: VarSlot::new(),
                type_: VarSlot::new(),
            });
        }

        // Fragments, then elements, in staging order; then the links.
        let mut fragment_ids: Vec<(FragmentId, ElementId)> = Vec::new();
        let check = |expected: FragmentId, actual: FragmentId| {
            debug_assert_eq!(expected, actual, "staged fragment id mismatch");
        };

        for (i, ((tag, f), (etag, e))) in self
            .locals
            .items
            .into_iter()
            .zip(local_elements)
            .enumerate()
        {
            let expected = FragmentId::new(self.store, tag, self.locals.base + i as u32);
            let fid = match tag {
                Tag::BindPatternVariable => store
                    .add_fragment::<dartr_element::BindPatternVariableFragment>(f)
                    .raw(),
                Tag::PatternVariable => store
                    .add_fragment::<dartr_element::PatternVariableFragment>(f)
                    .raw(),
                _ => store.add_fragment::<LocalVariableFragment>(f).raw(),
            };
            check(expected, fid);
            let eid = match etag {
                Tag::BindPatternVariable => store
                    .add::<dartr_element::BindPatternVariableElement>(e)
                    .raw(),
                Tag::PatternVariable => store.add::<dartr_element::PatternVariableElement>(e).raw(),
                _ => store.add::<LocalVariableElement>(e).raw(),
            };
            fragment_ids.push((fid, eid));
        }
        for (f, e) in self
            .local_functions
            .items
            .into_iter()
            .zip(function_elements)
        {
            let fid = store.add_fragment::<LocalFunctionFragment>(f.1).raw();
            let eid = store.add::<LocalFunctionElement>(e).raw();
            fragment_ids.push((fid, eid));
        }
        for (f, e) in self.labels.items.into_iter().zip(label_elements) {
            let fid = store.add_fragment::<LabelFragment>(f.1).raw();
            let eid = store.add::<LabelElement>(e).raw();
            fragment_ids.push((fid, eid));
        }
        for ((tag, f), (_, e, covariant)) in self.params.items.into_iter().zip(param_elements) {
            let (fid, eid) = match tag {
                Tag::FieldFormalParameter => (
                    store
                        .add_fragment::<dartr_element::FieldFormalParameterFragment>(f)
                        .raw(),
                    store
                        .add::<dartr_element::FieldFormalParameterElement>(e)
                        .raw(),
                ),
                Tag::SuperFormalParameter => (
                    store
                        .add_fragment::<dartr_element::SuperFormalParameterFragment>(f)
                        .raw(),
                    store
                        .add::<dartr_element::SuperFormalParameterElement>(e)
                        .raw(),
                ),
                _ => (
                    store.add_fragment::<FormalParameterFragment>(f).raw(),
                    store.add::<FormalParameterElement>(e).raw(),
                ),
            };
            if let Some(data) = store.element_data(eid) {
                data.flags.set(
                    ElementFlags::FORMAL_PARAMETER_ELEMENT_IS_COVARIANT,
                    covariant,
                );
            }
            fragment_ids.push((fid, eid));
        }
        for (f, e) in self.type_params.items.into_iter().zip(type_param_elements) {
            let fid = store.add_fragment::<TypeParameterFragment>(f.1).raw();
            let eid = store.add::<TypeParameterElement>(e).raw();
            fragment_ids.push((fid, eid));
        }
        for (f, e) in self
            .generic_function_types
            .items
            .into_iter()
            .zip(gft_elements)
        {
            let fid = store.add_fragment::<GenericFunctionTypeFragment>(f.1).raw();
            let eid = store.add::<GenericFunctionTypeElement>(e).raw();
            fragment_ids.push((fid, eid));
        }
        for (fid, eid) in fragment_ids {
            if let Some(f) = store.fragment_data(fid) {
                f.element.set_once(eid);
            }
        }
    }
}

// ------------------------------------------------------------------ holder

/// Dart `ElementHolder`: the container of new fragments.
#[derive(Debug)]
struct ElementHolder {
    fragment: FragmentId,
    type_parameters: Vec<FId<TypeParameterFragment>>,
    formal_parameters: Vec<FId<FormalParameterFragment>>,
}

impl ElementHolder {
    fn new(fragment: FragmentId) -> ElementHolder {
        ElementHolder {
            fragment,
            type_parameters: Vec::new(),
            formal_parameters: Vec::new(),
        }
    }
}

// ------------------------------------------------------------------ fragments

fn local_variable_fragment(name: Option<Name>, first_token_offset: u32) -> LocalVariableFragment {
    let mut fragment = FragmentData::new(name, None);
    fragment.first_token_offset = Some(first_token_offset);
    LocalVariableFragment {
        variable: VariableFragmentData::new(fragment),
        pattern: PatternVariableFragmentData::default(),
    }
}

fn local_function_fragment(name: Option<Name>, first_token_offset: u32) -> LocalFunctionFragment {
    let mut fragment = FragmentData::new(name, None);
    fragment.first_token_offset = Some(first_token_offset);
    LocalFunctionFragment {
        executable: ExecutableFragmentData::new(fragment),
    }
}

fn set_code_range(f: &mut FragmentData, offset: u32, length: u32) {
    f.code_offset = Some(offset);
    f.code_length = Some(length);
}

// ------------------------------------------------------------------ visitor

/// Dart `ElementBindingVisitor`.
struct ElementBindingVisitor<'c, 'a> {
    ctx: Ctx<'a>,
    library: EId<LibraryElement>,
    /// Dart `_libraryFragment`.
    library_fragment: FId<LibraryFragment>,
    /// Dart `_libraryDirectiveIndex`.
    library_directive_index: u32,
    /// The index of the next import directive in `library_imports`.
    import_index: usize,
    /// The index of the next export directive in `library_exports`.
    export_index: usize,
    /// The index of the next part directive in `parts`.
    part_index: usize,
    /// Dart `_elementWalker`.
    walker: Option<ElementWalker>,
    /// Dart `_elementHolder`.
    holder: ElementHolder,
    staging: Staging,
    tables: &'c mut ResolutionTables,
    rt: &'c mut ResolverTables,
}

impl<'c, 'a> ElementBindingVisitor<'c, 'a> {
    fn name(&self, text: &str) -> Name {
        self.ctx.name(text)
    }

    /// Dart `_withElementWalker`.
    fn with_walker<R>(
        &mut self,
        walker: Option<ElementWalker>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let previous = std::mem::replace(&mut self.walker, walker);
        let r = f(self);
        self.walker = previous;
        r
    }

    /// Dart `_withElementHolder`; returns the holder after [f].
    fn with_holder(&mut self, holder: ElementHolder, f: impl FnOnce(&mut Self)) -> ElementHolder {
        let previous = std::mem::replace(&mut self.holder, holder);
        f(self);
        std::mem::replace(&mut self.holder, previous)
    }

    fn visit_opt(&mut self, ast: &Ast, node: Option<impl Into<NodeId>>) {
        if let Some(n) = node {
            ast.accept(n, self);
        }
    }

    fn visit_list<T: ?Sized>(&mut self, ast: &Ast, list: NodeList<T>) {
        for &n in ast.list_raw(list) {
            ast.accept(n, self);
        }
    }

    fn set_declared(&mut self, node: impl Into<NodeId>, fragment: FragmentId) {
        self.tables.declared_fragment.insert(node, fragment);
    }

    /// Dart `_setElementAnnotations`: the [nodes] get the annotations of
    /// [owner] when the counts match.
    fn set_element_annotations(
        &mut self,
        ast: &Ast,
        nodes: NodeList<Annotation>,
        annotation_count: usize,
        owner: FragmentId,
    ) {
        let nodes = ast.list(nodes);
        if nodes.len() != annotation_count {
            return;
        }
        for &n in nodes {
            self.rt.element_annotation.insert(n, owner);
        }
    }

    /// Dart `_setOrCreateMetadataElements`.
    fn set_or_create_metadata_elements(
        &mut self,
        ast: &Ast,
        fragment: FragmentId,
        annotations: NodeList<Annotation>,
    ) {
        let nodes: Vec<Id<Annotation>> = ast.list(annotations).to_vec();
        if nodes.is_empty() {
            return;
        }
        let existing = if self.staging.is_staged(fragment) {
            0
        } else {
            self.ctx
                .fragment_data(fragment)
                .map(|f| f.metadata.annotations.len())
                .unwrap_or(0)
        };
        if existing != 0 && existing == nodes.len() {
            self.set_element_annotations(ast, annotations, existing, fragment);
        }
        self.with_walker(None, |v| {
            for &n in &nodes {
                ast.accept(n, v);
            }
        });
        if self.walker.is_none() {
            if let Some(f) = self.staging.fragment_mut(fragment) {
                f.metadata = Metadata {
                    annotations: nodes
                        .iter()
                        .map(|&n| ElementAnnotation {
                            library_fragment: self.library_fragment,
                            annotation_ast: ConstExprId(n.raw()),
                        })
                        .collect(),
                    metadata_flags: dartr_element::OnceSlot::with(
                        dartr_element::compute_metadata_flags(
                            ast,
                            &nodes.iter().map(|&a| a.into()).collect::<Vec<NodeId>>(),
                        ),
                    ),
                };
                for &n in &nodes {
                    self.rt.element_annotation.insert(n, fragment);
                }
            }
        }
    }

    /// Dart `_buildLabelElements`.
    fn build_label_elements(&mut self, ast: &Ast, labels: NodeList<Label>, on_switch_member: bool) {
        for &label in ast.list(labels) {
            let name = self.name(ast.tokens.lexeme(ast[label].name));
            let mut fragment = FragmentData::new(Some(name), None);
            fragment.first_token_offset = Some(ast.offset(label));
            fragment.enclosing_fragment = Some(self.holder.fragment);
            let id = self.staging.add_label(LabelFragment {
                fragment,
                on_switch_member,
            });
            self.set_declared(label, id);
        }
    }

    /// Dart `_visitFormalParameter`.
    fn visit_formal_parameter(
        &mut self,
        ast: &Ast,
        node: NodeId,
        create: impl FnOnce(&mut Self) -> (Tag, FormalParameterFragment),
    ) {
        let parts = formal_parameter_parts(ast, node);
        let fragment: FragmentId = if let Some(walker) = self.walker.as_mut() {
            match walker.get_parameter() {
                Some(f) => f.raw(),
                None => return,
            }
        } else {
            let (tag, mut f) = create(self);
            f.variable.fragment.enclosing_fragment = Some(self.holder.fragment);
            set_code_range(&mut f.variable.fragment, ast.offset(node), ast.length(node));
            let flags = &f.variable.fragment.flags;
            flags.set(
                FragmentFlags::VARIABLE_FRAGMENT_IS_CONST,
                parts.is_const(ast),
            );
            flags.set(
                FragmentFlags::FORMAL_PARAMETER_FRAGMENT_IS_EXPLICITLY_COVARIANT,
                parts.covariant_keyword.is_some(),
            );
            flags.set(
                FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL,
                parts.is_final(ast),
            );
            let id = self.staging.add_param(tag, f);
            self.holder.formal_parameters.push(FId::from_raw(id));
            id
        };
        self.set_declared(node, fragment);

        self.set_or_create_metadata_elements(ast, fragment, parts.metadata);

        if let Some(suffix) = parts.function_typed_suffix {
            let suffix_type_parameters = ast[suffix].type_parameters;
            let suffix_parameters = ast[suffix].formal_parameters;
            // The elements of the holder's children are created by the
            // commit (Dart `TypeParameterElementImpl(firstFragment:)`,
            // `initElement`).
            let _holder = self.with_holder(ElementHolder::new(fragment), |v| {
                v.with_walker(None, |v| {
                    v.visit_opt(ast, parts.documentation_comment);
                    v.visit_opt(ast, parts.type_);
                    v.visit_opt(ast, suffix_type_parameters);
                    ast.accept(suffix_parameters, v);
                });
            });
        } else {
            self.visit_opt(ast, parts.documentation_comment);
            self.visit_opt(ast, parts.type_);
        }

        if let Some(default_clause) = parts.default_clause {
            let value = ast[default_clause].value;
            if self.walker.is_none() {
                if let Some(f) = self.staging.param_mut(fragment) {
                    f.variable.constant_initializer = Some(ConstExprId(value.raw()));
                }
            }
            self.with_walker(None, |v| {
                v.with_holder(ElementHolder::new(fragment), |v| {
                    ast.accept(value, v);
                });
            });
        }
    }

    fn new_formal_parameter_fragment(
        &self,
        ast: &Ast,
        node: NodeId,
        name: Option<dartr_syntax::TokenId>,
        kind: ParameterKind,
    ) -> FormalParameterFragment {
        let name_text = name
            .and_then(|t| name_if_not_empty(ast, t))
            .map(|n| self.name(n));
        let name_offset = name.and_then(|t| offset_if_not_empty(ast, t));
        let mut fragment = FragmentData::new(name_text, name_offset);
        fragment.first_token_offset = Some(ast.offset(node));
        FormalParameterFragment {
            variable: VariableFragmentData::new(fragment),
            parameter_kind: kind,
            private_name: None,
        }
    }

    /// The executable fragment of a function or method declaration from the
    /// walker (Dart `getGetter` / `getSetter` / `getFunction`).
    fn walker_executable(&mut self, is_getter: bool, is_setter: bool) -> Option<FragmentId> {
        let walker = self.walker.as_mut()?;
        if is_getter {
            walker.get_getter().map(|f| f.raw())
        } else if is_setter {
            walker.get_setter().map(|f| f.raw())
        } else {
            walker.get_function().map(|f| f.raw())
        }
    }
}

impl AstVisitor for ElementBindingVisitor<'_, '_> {
    fn visit_annotation(&mut self, ast: &Ast, node: Id<Annotation>) {
        if self.rt.element_annotation.get(node).is_none() && self.walker.is_none() {
            // Dart `ElementAnnotationImpl(_libraryFragment, node)`.
            self.rt
                .element_annotation
                .insert(node, self.library_fragment.raw());
        }
        self.with_walker(None, |v| ast.visit_children(node, v));
    }

    fn visit_anonymous_method_invocation(
        &mut self,
        ast: &Ast,
        node: Id<AnonymousMethodInvocation>,
    ) {
        let mut f = local_function_fragment(None, ast.offset(node));
        f.executable.fragment.enclosing_fragment = Some(self.holder.fragment);
        let flags = &f.executable.fragment.flags;
        flags.set(
            FragmentFlags::EXECUTABLE_FRAGMENT_HAS_IMPLICIT_RETURN_TYPE,
            true,
        );
        flags.set(FragmentFlags::EXECUTABLE_FRAGMENT_IS_ASYNCHRONOUS, false);
        flags.set(FragmentFlags::EXECUTABLE_FRAGMENT_IS_GENERATOR, false);
        set_code_range(
            &mut f.executable.fragment,
            ast.offset(node),
            ast.length(node),
        );
        let fragment = self.staging.add_local_function(f);
        self.set_declared(node, fragment);

        let holder = self.with_holder(ElementHolder::new(fragment), |v| {
            ast.visit_children(node, v);
        });
        if let Some(f) = self.staging.local_function_mut(fragment) {
            f.type_params = Vec::new();
            f.formal_params = holder.formal_parameters;
        }
    }

    fn visit_catch_clause(&mut self, ast: &Ast, node: Id<CatchClause>) {
        self.with_walker(None, |v| {
            let n = &ast[node];
            let (exception_type, exception_parameter, stack_trace_parameter) = (
                n.exception_type,
                n.exception_parameter,
                n.stack_trace_parameter,
            );
            if let Some(exception_node) = exception_parameter {
                let name_token = ast[exception_node].name;
                let name = name_if_not_empty(ast, name_token).map(|n| v.name(n));
                let mut f = local_variable_fragment(name, ast.offset(exception_node));
                f.variable.fragment.name_offset = offset_if_not_empty(ast, name_token);
                f.variable.fragment.enclosing_fragment = Some(v.holder.fragment);
                let flags = &f.variable.fragment.flags;
                flags.set(FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL, true);
                if exception_type.is_none() {
                    flags.set(FragmentFlags::VARIABLE_FRAGMENT_HAS_IMPLICIT_TYPE, true);
                }
                let offset = ast.tokens.offset(name_token);
                set_code_range(
                    &mut f.variable.fragment,
                    offset,
                    token_end(ast, name_token) - offset,
                );
                let id = v.staging.add_local(Tag::LocalVariable, f);
                v.set_declared(exception_node, id);
            }
            if let Some(stack_trace_node) = stack_trace_parameter {
                let name_token = ast[stack_trace_node].name;
                let name = name_if_not_empty(ast, name_token).map(|n| v.name(n));
                let mut f = local_variable_fragment(name, ast.offset(stack_trace_node));
                f.variable.fragment.name_offset = offset_if_not_empty(ast, name_token);
                f.variable.fragment.enclosing_fragment = Some(v.holder.fragment);
                let flags = &f.variable.fragment.flags;
                flags.set(FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL, true);
                flags.set(FragmentFlags::VARIABLE_FRAGMENT_HAS_IMPLICIT_TYPE, true);
                let offset = ast.tokens.offset(name_token);
                set_code_range(
                    &mut f.variable.fragment,
                    offset,
                    token_end(ast, name_token) - offset,
                );
                let id = v.staging.add_local(Tag::LocalVariable, f);
                v.set_declared(stack_trace_node, id);
            }
            ast.visit_children(node, v);
        });
    }

    fn visit_class_declaration(&mut self, ast: &Ast, node: Id<ClassDeclaration>) {
        let Some(fragment) = self.walker.as_mut().and_then(|w| w.get_class()) else {
            return;
        };
        self.set_declared(node, fragment.raw());
        self.set_or_create_metadata_elements(ast, fragment.raw(), ast[node].metadata);
        let walker = ElementWalker::for_class(&self.ctx, fragment);
        self.with_walker(Some(walker), |v| ast.visit_children(node, v));
    }

    fn visit_class_type_alias(&mut self, ast: &Ast, node: Id<ClassTypeAlias>) {
        let Some(fragment) = self.walker.as_mut().and_then(|w| w.get_class()) else {
            return;
        };
        self.set_declared(node, fragment.raw());
        self.set_or_create_metadata_elements(ast, fragment.raw(), ast[node].metadata);
        let walker = ElementWalker::for_class(&self.ctx, fragment);
        self.with_walker(Some(walker), |v| ast.visit_children(node, v));
    }

    fn visit_constructor_declaration(&mut self, ast: &Ast, node: Id<ConstructorDeclaration>) {
        let Some(fragment) = self.walker.as_mut().and_then(|w| w.get_constructor()) else {
            return;
        };
        self.set_declared(node, fragment.raw());
        let n = &ast[node];
        let (metadata, type_name, parameters, initializers, redirected, body) = (
            n.metadata,
            n.type_name,
            n.parameters,
            n.initializers,
            n.redirected_constructor,
            n.body,
        );
        self.set_or_create_metadata_elements(ast, fragment.raw(), metadata);
        let ctx = self.ctx;
        self.with_holder(ElementHolder::new(fragment.raw()), |v| {
            v.with_walker(None, |v| {
                v.visit_opt(ast, type_name);
                let walker = ElementWalker::for_executable(&ctx, FId::from_raw(fragment.raw()));
                v.with_walker(Some(walker), |v| ast.accept(parameters, v));
                v.visit_list(ast, initializers);
                v.visit_opt(ast, redirected);
                ast.accept(body, v);
            });
        });
    }

    fn visit_declared_identifier(&mut self, ast: &Ast, node: Id<DeclaredIdentifier>) {
        let n = &ast[node];
        let (name_token, keyword, type_, metadata) = (n.name, n.keyword, n.type_, n.metadata);
        let name = name_if_not_empty(ast, name_token).map(|t| self.name(t));
        let mut f = local_variable_fragment(name, ast.offset(node));
        f.variable.fragment.name_offset = offset_if_not_empty(ast, name_token);
        f.variable.fragment.enclosing_fragment = Some(self.holder.fragment);
        let flags = &f.variable.fragment.flags;
        flags.set(
            FragmentFlags::VARIABLE_FRAGMENT_IS_CONST,
            is_keyword(ast, keyword, "const"),
        );
        flags.set(
            FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL,
            is_keyword(ast, keyword, "final"),
        );
        if type_.is_none() {
            flags.set(FragmentFlags::VARIABLE_FRAGMENT_HAS_IMPLICIT_TYPE, true);
        }
        set_code_range(&mut f.variable.fragment, ast.offset(node), ast.length(node));
        let fragment = self.staging.add_local(Tag::LocalVariable, f);
        self.set_declared(node, fragment);
        self.set_or_create_metadata_elements(ast, fragment, metadata);
        ast.visit_children(node, self);
    }

    fn visit_declared_variable_pattern(&mut self, ast: &Ast, node: Id<DeclaredVariablePattern>) {
        if self.walker.is_none() {
            let n = &ast[node];
            let (name_token, keyword) = (n.name, n.keyword);
            let name = self.name(ast.tokens.lexeme(name_token));
            let mut f = local_variable_fragment(Some(name), ast.offset(node));
            f.pattern.node = Some(node.raw());
            f.variable.fragment.name_offset = Some(ast.tokens.offset(name_token));
            f.variable.fragment.enclosing_fragment = Some(self.holder.fragment);
            f.variable.fragment.flags.set(
                FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL,
                is_keyword(ast, keyword, "final"),
            );
            let offset = ast.tokens.offset(name_token);
            set_code_range(
                &mut f.variable.fragment,
                offset,
                token_end(ast, name_token) - offset,
            );
            let fragment = self.staging.add_local(Tag::BindPatternVariable, f);
            self.set_declared(node, fragment);
        }
        ast.visit_children(node, self);
    }

    fn visit_enum_constant_declaration(&mut self, ast: &Ast, node: Id<EnumConstantDeclaration>) {
        let Some(fragment) = self.walker.as_mut().and_then(|w| w.get_variable()) else {
            return;
        };
        self.set_declared(node, fragment.raw());
        let (metadata, arguments) = (ast[node].metadata, ast[node].arguments);
        self.set_or_create_metadata_elements(ast, fragment.raw(), metadata);
        if let Some(arguments) = arguments {
            self.with_walker(None, |v| {
                v.with_holder(ElementHolder::new(fragment.raw()), |v| {
                    ast.accept(arguments, v);
                });
            });
        }
    }

    fn visit_enum_declaration(&mut self, ast: &Ast, node: Id<EnumDeclaration>) {
        let Some(fragment) = self.walker.as_mut().and_then(|w| w.get_enum()) else {
            return;
        };
        self.set_declared(node, fragment.raw());
        self.set_or_create_metadata_elements(ast, fragment.raw(), ast[node].metadata);
        let walker = ElementWalker::for_enum(&self.ctx, fragment);
        self.with_walker(Some(walker), |v| ast.visit_children(node, v));
    }

    fn visit_export_directive(&mut self, ast: &Ast, node: Id<ExportDirective>) {
        let index = self.export_index;
        self.export_index += 1;
        let count = self
            .ctx
            .fragment(self.library_fragment)
            .library_exports
            .get(index)
            .map(|e| e.directive.metadata.annotations.len());
        if let Some(count) = count {
            self.set_element_annotations(
                ast,
                ast[node].metadata,
                count,
                self.library_fragment.raw(),
            );
        }
        self.with_walker(None, |v| ast.visit_children(node, v));
    }

    fn visit_extension_declaration(&mut self, ast: &Ast, node: Id<ExtensionDeclaration>) {
        let Some(fragment) = self.walker.as_mut().and_then(|w| w.get_extension()) else {
            return;
        };
        self.set_declared(node, fragment.raw());
        self.set_or_create_metadata_elements(ast, fragment.raw(), ast[node].metadata);
        let walker = ElementWalker::for_extension(&self.ctx, fragment);
        self.with_walker(Some(walker), |v| ast.visit_children(node, v));
    }

    fn visit_extension_type_declaration(&mut self, ast: &Ast, node: Id<ExtensionTypeDeclaration>) {
        let Some(fragment) = self.walker.as_mut().and_then(|w| w.get_extension_type()) else {
            return;
        };
        self.set_declared(node, fragment.raw());
        self.set_or_create_metadata_elements(ast, fragment.raw(), ast[node].metadata);
        let walker = ElementWalker::for_extension_type(&self.ctx, fragment);
        self.with_walker(Some(walker), |v| ast.visit_children(node, v));
    }

    fn visit_field_formal_parameter(&mut self, ast: &Ast, node: Id<FieldFormalParameter>) {
        let (name, kind) = (ast[node].name, ast[node].kind);
        self.visit_formal_parameter(ast, node.raw(), |v| {
            let f = v.new_formal_parameter_fragment(ast, node.raw(), Some(name), kind);
            (Tag::FieldFormalParameter, f)
        });
    }

    fn visit_function_declaration(&mut self, ast: &Ast, node: Id<FunctionDeclaration>) {
        let n = &ast[node];
        let (expression, metadata, return_type, external_keyword, property_keyword) = (
            n.function_expression,
            n.metadata,
            n.return_type,
            n.external_keyword,
            n.property_keyword,
        );
        let (type_parameters, parameters, body) = {
            let e = &ast[expression];
            (e.type_parameters, e.parameters, e.body)
        };
        let is_getter = is_keyword(ast, property_keyword, "get");
        let is_setter = is_keyword(ast, property_keyword, "set");

        let fragment: FragmentId = if self.walker.is_some() {
            let Some(fragment) = self.walker_executable(is_getter, is_setter) else {
                return;
            };
            self.set_declared(node, fragment);
            self.set_declared(expression, fragment);
            fragment
        } else {
            let Some(&fragment) = self.tables.declared_fragment.get(node) else {
                return;
            };
            self.set_declared(expression, fragment);
            let offset = ast.offset(node);
            let length = ast.length(node);
            let is_asynchronous = function_body_is_asynchronous(ast, body.raw());
            let is_generator = function_body_is_generator(ast, body.raw());
            let is_external = external_keyword.is_some() || ast.is::<NativeFunctionBody>(body);
            let is_complete =
                external_keyword.is_some() || !ast.is::<dartr_ast::EmptyFunctionBody>(body);
            if let Some(f) = self.staging.local_function_mut(fragment) {
                set_code_range(&mut f.executable.fragment, offset, length);
                let flags = &f.executable.fragment.flags;
                if is_external {
                    flags.set(FragmentFlags::EXECUTABLE_FRAGMENT_IS_EXTERNAL, true);
                }
                flags.set(FragmentFlags::FRAGMENT_IS_COMPLETE, is_complete);
                flags.set(
                    FragmentFlags::EXECUTABLE_FRAGMENT_IS_ASYNCHRONOUS,
                    is_asynchronous,
                );
                flags.set(
                    FragmentFlags::EXECUTABLE_FRAGMENT_IS_GENERATOR,
                    is_generator,
                );
                if return_type.is_none() {
                    flags.set(
                        FragmentFlags::EXECUTABLE_FRAGMENT_HAS_IMPLICIT_RETURN_TYPE,
                        true,
                    );
                }
            }
            fragment
        };

        self.set_or_create_metadata_elements(ast, fragment, metadata);

        let ctx = self.ctx;
        let walker_present = self.walker.is_some();
        self.with_holder(ElementHolder::new(fragment), |v| {
            v.visit_opt(ast, return_type);
            if walker_present {
                let walker = ElementWalker::for_executable(&ctx, FId::from_raw(fragment));
                v.with_walker(Some(walker), |v| {
                    v.visit_opt(ast, type_parameters);
                    v.visit_opt(ast, parameters);
                });
                v.with_walker(None, |v| ast.accept(body, v));
            } else {
                v.visit_opt(ast, type_parameters);
                let type_params = std::mem::take(&mut v.holder.type_parameters);
                if let Some(f) = v.staging.local_function_mut(fragment) {
                    f.type_params = type_params;
                }
                v.visit_opt(ast, parameters);
                let formal_params = std::mem::take(&mut v.holder.formal_parameters);
                if let Some(f) = v.staging.local_function_mut(fragment) {
                    f.formal_params = formal_params;
                }
                ast.accept(body, v);
            }
        });
    }

    fn visit_function_declaration_statement(
        &mut self,
        ast: &Ast,
        node: Id<FunctionDeclarationStatement>,
    ) {
        let function_node = ast[node].function_declaration;
        let name_token = ast[function_node].name;
        let expression = ast[function_node].function_expression;
        let name = name_if_not_empty(ast, name_token).map(|t| self.name(t));
        let mut f = local_function_fragment(name, ast.offset(node));
        f.executable.fragment.name_offset = offset_if_not_empty(ast, name_token);
        f.executable.fragment.enclosing_fragment = Some(self.holder.fragment);
        let fragment = self.staging.add_local_function(f);
        self.set_declared(function_node, fragment);
        self.set_declared(expression, fragment);
        ast.visit_children(node, self);
    }

    fn visit_function_expression(&mut self, ast: &Ast, node: Id<FunctionExpression>) {
        if ast
            .parent(node)
            .is_some_and(|p| ast.is::<FunctionDeclaration>(p))
        {
            ast.visit_children(node, self);
            return;
        }
        let n = &ast[node];
        let (type_parameters, parameters, body) = (n.type_parameters, n.parameters, n.body);
        let mut f = local_function_fragment(None, ast.offset(node));
        f.executable.fragment.enclosing_fragment = Some(self.holder.fragment);
        let flags = &f.executable.fragment.flags;
        flags.set(
            FragmentFlags::EXECUTABLE_FRAGMENT_HAS_IMPLICIT_RETURN_TYPE,
            true,
        );
        flags.set(
            FragmentFlags::EXECUTABLE_FRAGMENT_IS_ASYNCHRONOUS,
            function_body_is_asynchronous(ast, body.raw()),
        );
        flags.set(
            FragmentFlags::EXECUTABLE_FRAGMENT_IS_GENERATOR,
            function_body_is_generator(ast, body.raw()),
        );
        set_code_range(
            &mut f.executable.fragment,
            ast.offset(node),
            ast.length(node),
        );
        let fragment = self.staging.add_local_function(f);
        self.set_declared(node, fragment);

        self.with_holder(ElementHolder::new(fragment), |v| {
            v.visit_opt(ast, type_parameters);
            let type_params = std::mem::take(&mut v.holder.type_parameters);
            if let Some(f) = v.staging.local_function_mut(fragment) {
                f.type_params = type_params;
            }
            v.visit_opt(ast, parameters);
            let formal_params = std::mem::take(&mut v.holder.formal_parameters);
            if let Some(f) = v.staging.local_function_mut(fragment) {
                f.formal_params = formal_params;
            }
            ast.accept(body, v);
        });
    }

    fn visit_function_type_alias(&mut self, ast: &Ast, node: Id<FunctionTypeAlias>) {
        let Some(fragment) = self.walker.as_mut().and_then(|w| w.get_typedef()) else {
            return;
        };
        self.set_declared(node, fragment.raw());
        let n = &ast[node];
        let (metadata, type_parameters, return_type, parameters) =
            (n.metadata, n.type_parameters, n.return_type, n.parameters);
        self.set_or_create_metadata_elements(ast, fragment.raw(), metadata);
        let walker = ElementWalker::for_typedef(&self.ctx, fragment);
        // The formal parameters are enclosed by the typedef fragment (Dart
        // `fragment.encloseElements(holder.formalParameters)`); the commit
        // creates their elements (Dart `initElement`).
        self.with_holder(ElementHolder::new(fragment.raw()), |v| {
            v.with_walker(Some(walker), |v| {
                v.visit_opt(ast, type_parameters);
                v.with_walker(None, |v| {
                    v.visit_opt(ast, return_type);
                    ast.accept(parameters, v);
                });
            });
        });
    }

    fn visit_generic_function_type(&mut self, ast: &Ast, node: Id<GenericFunctionType>) {
        let mut fragment_data = FragmentData::new(None, None);
        fragment_data.first_token_offset = Some(ast.offset(node));
        // Dart `_libraryFragment.encloseElement(fragment)`.
        fragment_data.enclosing_fragment = Some(self.library_fragment.raw());
        set_code_range(&mut fragment_data, ast.offset(node), ast.length(node));
        let fragment = self
            .staging
            .add_generic_function_type(GenericFunctionTypeFragment {
                fragment: fragment_data,
                type_params: Vec::new(),
                formal_params: Vec::new(),
                is_nullable: ast[node].question.is_some(),
            });
        self.set_declared(node, fragment);

        let holder = self.with_holder(ElementHolder::new(fragment), |v| {
            v.with_walker(None, |v| ast.visit_children(node, v));
        });
        if let Some(f) = self.staging.generic_function_type_mut(fragment) {
            f.type_params = holder.type_parameters;
            f.formal_params = holder.formal_parameters;
        }
    }

    fn visit_generic_type_alias(&mut self, ast: &Ast, node: Id<GenericTypeAlias>) {
        let Some(fragment) = self.walker.as_mut().and_then(|w| w.get_typedef()) else {
            return;
        };
        self.set_declared(node, fragment.raw());
        self.set_or_create_metadata_elements(ast, fragment.raw(), ast[node].metadata);
        let walker = ElementWalker::for_generic_type_alias(&self.ctx, fragment);
        self.with_walker(Some(walker), |v| ast.visit_children(node, v));
    }

    fn visit_import_directive(&mut self, ast: &Ast, node: Id<ImportDirective>) {
        let index = self.import_index;
        self.import_index += 1;
        let ctx = self.ctx;
        let import = ctx
            .fragment(self.library_fragment)
            .library_imports
            .get(index);
        if let Some(import) = import {
            let count = import.directive.metadata.annotations.len();
            self.set_element_annotations(
                ast,
                ast[node].metadata,
                count,
                self.library_fragment.raw(),
            );
            // Dart `LibraryAnalyzer._resolveLibraryImportDirective`:
            // `directive.prefix?.element = element.prefix?.element`.
            if let Some(prefix_node) = ast[node].prefix {
                let prefix_element = import
                    .prefix
                    .and_then(|p| ctx.fragment(p).element.try_get().copied());
                if let Some(e) = prefix_element {
                    self.tables.element.insert(prefix_node, ElemRef::Base(e));
                }
            }
        }
        self.with_walker(None, |v| ast.visit_children(node, v));
    }

    fn visit_labeled_statement(&mut self, ast: &Ast, node: Id<LabeledStatement>) {
        self.build_label_elements(ast, ast[node].labels, false);
        ast.visit_children(node, self);
    }

    fn visit_library_directive(&mut self, ast: &Ast, node: Id<LibraryDirective>) {
        self.library_directive_index += 1;
        if self.library_directive_index == 1 {
            let count = self.ctx.get(self.library).metadata.annotations.len();
            self.set_element_annotations(
                ast,
                ast[node].metadata,
                count,
                self.library_fragment.raw(),
            );
        }
        self.with_walker(None, |v| ast.visit_children(node, v));
    }

    fn visit_method_declaration(&mut self, ast: &Ast, node: Id<MethodDeclaration>) {
        let n = &ast[node];
        let (property_keyword, metadata, return_type, type_parameters, parameters, body) = (
            n.property_keyword,
            n.metadata,
            n.return_type,
            n.type_parameters,
            n.parameters,
            n.body,
        );
        let is_getter = is_keyword(ast, property_keyword, "get");
        let is_setter = is_keyword(ast, property_keyword, "set");
        let Some(fragment) = self.walker_executable(is_getter, is_setter) else {
            return;
        };
        self.set_declared(node, fragment);
        self.set_or_create_metadata_elements(ast, fragment, metadata);

        self.visit_opt(ast, return_type);
        let walker = ElementWalker::for_executable(&self.ctx, FId::from_raw(fragment));
        self.with_walker(Some(walker), |v| {
            v.visit_opt(ast, type_parameters);
            v.visit_opt(ast, parameters);
        });
        self.with_holder(ElementHolder::new(fragment), |v| {
            v.with_walker(None, |v| ast.accept(body, v));
        });
    }

    fn visit_mixin_declaration(&mut self, ast: &Ast, node: Id<MixinDeclaration>) {
        let Some(fragment) = self.walker.as_mut().and_then(|w| w.get_mixin()) else {
            return;
        };
        self.set_declared(node, fragment.raw());
        self.set_or_create_metadata_elements(ast, fragment.raw(), ast[node].metadata);
        let walker = ElementWalker::for_mixin(&self.ctx, fragment);
        self.with_walker(Some(walker), |v| ast.visit_children(node, v));
    }

    fn visit_part_directive(&mut self, ast: &Ast, node: Id<PartDirective>) {
        let index = self.part_index;
        self.part_index += 1;
        let count = self
            .ctx
            .fragment(self.library_fragment)
            .parts
            .get(index)
            .map(|p| p.directive.metadata.annotations.len());
        if let Some(count) = count {
            self.set_element_annotations(
                ast,
                ast[node].metadata,
                count,
                self.library_fragment.raw(),
            );
        }
        self.with_walker(None, |v| ast.visit_children(node, v));
    }

    fn visit_part_of_directive(&mut self, ast: &Ast, node: Id<PartOfDirective>) {
        self.with_walker(None, |v| ast.visit_children(node, v));
    }

    fn visit_primary_constructor_body(&mut self, ast: &Ast, node: Id<PrimaryConstructorBody>) {
        let declaration = primary_constructor_body_declaration(ast, node);
        let fragment = declaration.and_then(|d| self.tables.declared_fragment.get(d).copied());
        if let Some(fragment) = fragment {
            self.with_holder(ElementHolder::new(fragment), |v| {
                v.with_walker(None, |v| ast.visit_children(node, v));
            });
        } else {
            self.with_walker(None, |v| ast.visit_children(node, v));
        }
    }

    fn visit_primary_constructor_declaration(
        &mut self,
        ast: &Ast,
        node: Id<PrimaryConstructorDeclaration>,
    ) {
        let Some(fragment) = self.walker.as_mut().and_then(|w| w.get_constructor()) else {
            return;
        };
        self.set_declared(node, fragment.raw());
        let (formal_parameters, type_parameters) =
            (ast[node].formal_parameters, ast[node].type_parameters);
        let ctx = self.ctx;
        self.with_holder(ElementHolder::new(fragment.raw()), |v| {
            let walker = ElementWalker::for_executable(&ctx, FId::from_raw(fragment.raw()));
            v.with_walker(Some(walker), |v| ast.accept(formal_parameters, v));
        });
        self.visit_opt(ast, type_parameters);
    }

    fn visit_record_type_annotation(&mut self, ast: &Ast, node: Id<RecordTypeAnnotation>) {
        self.with_walker(None, |v| ast.visit_children(node, v));
    }

    fn visit_regular_formal_parameter(&mut self, ast: &Ast, node: Id<RegularFormalParameter>) {
        let n = &ast[node];
        let (name, kind, type_, suffix) = (n.name, n.kind, n.type_, n.function_typed_suffix);
        self.visit_formal_parameter(ast, node.raw(), |v| {
            let f = v.new_formal_parameter_fragment(ast, node.raw(), name, kind);
            if type_.is_none() && suffix.is_none() {
                f.variable
                    .fragment
                    .flags
                    .set(FragmentFlags::VARIABLE_FRAGMENT_HAS_IMPLICIT_TYPE, true);
            }
            (Tag::FormalParameter, f)
        });
    }

    fn visit_super_formal_parameter(&mut self, ast: &Ast, node: Id<SuperFormalParameter>) {
        let (name, kind) = (ast[node].name, ast[node].kind);
        self.visit_formal_parameter(ast, node.raw(), |v| {
            let f = v.new_formal_parameter_fragment(ast, node.raw(), Some(name), kind);
            (Tag::SuperFormalParameter, f)
        });
    }

    fn visit_switch_statement(&mut self, ast: &Ast, node: Id<SwitchStatement>) {
        for &member in ast.list(ast[node].members) {
            let labels = switch_member_labels(ast, member.raw());
            if let Some(labels) = labels {
                self.build_label_elements(ast, labels, true);
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_type_parameter(&mut self, ast: &Ast, node: Id<TypeParameter>) {
        let name_token = ast[node].name;
        let fragment: FragmentId = if let Some(walker) = self.walker.as_mut() {
            match walker.get_type_parameter() {
                Some(f) => f.raw(),
                None => return,
            }
        } else {
            let name = self.name(ast.tokens.lexeme(name_token));
            let mut f = FragmentData::new(Some(name), Some(ast.tokens.offset(name_token)));
            f.first_token_offset = Some(ast.offset(node));
            f.enclosing_fragment = Some(self.holder.fragment);
            set_code_range(&mut f, ast.offset(node), ast.length(node));
            let id = self
                .staging
                .add_type_param(TypeParameterFragment { fragment: f });
            self.holder.type_parameters.push(FId::from_raw(id));
            id
        };
        self.set_declared(node, fragment);
        self.set_or_create_metadata_elements(ast, fragment, ast[node].metadata);
        ast.visit_children(node, self);
    }

    fn visit_variable_declaration(&mut self, ast: &Ast, node: Id<VariableDeclaration>) {
        let Some(variable_list) = ast
            .parent(node)
            .and_then(|p| ast.cast::<VariableDeclarationList>(p))
        else {
            ast.visit_children(node, self);
            return;
        };
        let Some(declaration_parent) = ast.parent(variable_list) else {
            return;
        };
        let list = &ast[variable_list];
        let (list_type, list_keyword, list_late, list_metadata, list_variables) = (
            list.type_,
            list.keyword,
            list.late_keyword,
            list.metadata,
            list.variables,
        );

        let fragment: FragmentId = if let Some(walker) = self.walker.as_mut() {
            match walker.get_variable() {
                Some(f) => f.raw(),
                None => return,
            }
        } else {
            let name_token = ast[node].name;
            let name = name_if_not_empty(ast, name_token).map(|t| self.name(t));
            let mut f = local_variable_fragment(name, ast.offset(node));
            f.variable.fragment.enclosing_fragment = Some(self.holder.fragment);
            let flags = &f.variable.fragment.flags;
            flags.set(
                FragmentFlags::VARIABLE_FRAGMENT_HAS_IMPLICIT_TYPE,
                list_type.is_none(),
            );
            flags.set(
                FragmentFlags::NON_PARAMETER_VARIABLE_FRAGMENT_HAS_INITIALIZER,
                ast[node].initializer.is_some(),
            );
            flags.set(
                FragmentFlags::VARIABLE_FRAGMENT_IS_CONST,
                is_keyword(ast, list_keyword, "const"),
            );
            flags.set(
                FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL,
                is_keyword(ast, list_keyword, "final"),
            );
            flags.set(
                FragmentFlags::VARIABLE_FRAGMENT_IS_LATE,
                list_late.is_some(),
            );
            f.variable.fragment.name_offset = offset_if_not_empty(ast, name_token);
            self.staging.add_local(Tag::LocalVariable, f)
        };
        self.set_declared(node, fragment);

        let annotations = if let Some(d) = ast.cast::<FieldDeclaration>(declaration_parent) {
            ast[d].metadata
        } else if let Some(d) = ast.cast::<TopLevelVariableDeclaration>(declaration_parent) {
            ast[d].metadata
        } else {
            list_metadata
        };
        self.set_or_create_metadata_elements(ast, fragment, annotations);

        let offset = if ast.list(list_variables).first() == Some(&node) {
            ast.offset(declaration_parent)
        } else {
            ast.offset(node)
        };
        let end = ast.end(node);
        if let Some(f) = self.staging.fragment_mut(fragment) {
            set_code_range(f, offset, end.saturating_sub(offset));
        }

        self.with_walker(None, |v| {
            v.with_holder(ElementHolder::new(fragment), |v| {
                ast.visit_children(node, v);
            });
        });
    }
}

/// Dart `PrimaryConstructorBodyImpl.declaration`.
pub fn primary_constructor_body_declaration(
    ast: &Ast,
    node: Id<PrimaryConstructorBody>,
) -> Option<Id<PrimaryConstructorDeclaration>> {
    let grand_parent = ast.parent(ast.parent(node)?)?;
    let name_part = if let Some(c) = ast.cast::<ClassDeclaration>(grand_parent) {
        ast[c].name_part.raw()
    } else if let Some(e) = ast.cast::<EnumDeclaration>(grand_parent) {
        ast[e].name_part.raw()
    } else {
        let e = ast.cast::<ExtensionTypeDeclaration>(grand_parent)?;
        ast[e].name_part.raw()
    };
    ast.cast::<PrimaryConstructorDeclaration>(name_part)
}

/// Dart `SwitchMember.labels`.
pub fn switch_member_labels(ast: &Ast, member: NodeId) -> Option<NodeList<Label>> {
    if let Some(m) = ast.cast::<dartr_ast::SwitchCase>(member) {
        Some(ast[m].labels)
    } else if let Some(m) = ast.cast::<dartr_ast::SwitchDefault>(member) {
        Some(ast[m].labels)
    } else {
        ast.cast::<dartr_ast::SwitchPatternCase>(member)
            .map(|m| ast[m].labels)
    }
}
