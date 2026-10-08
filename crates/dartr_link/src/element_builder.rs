// Dart source: pkg/analyzer/lib/src/summary2/element_builder.dart
// (FragmentBuilder, ElementBuilder, _EnclosingContext), and the element
// and fragment constructors of pkg/analyzer/lib/src/dart/element/element.dart

//! Builds fragments from the AST of each unit ([`FragmentBuilder`], a Dart
//! `ThrowingAstVisitor`), then elements from the fragments
//! ([`ElementBuilder`]): one element per declaration (or per chain of
//! augmenting fragments), the synthetic getters and setters of variables,
//! the synthetic variables of getters and setters, and the formal
//! parameter and type parameter elements.
//!
//! Expressions that linking keeps (constant initializers, default values,
//! annotations, the initializers of const constructors) are copied into the
//! cycle's `ConstExprs`. The synthetic AST of enums (the `InstanceCreation`
//! initializer of each constant, the `values` list literal and its
//! `List<E>` type) is built in the `ConstExprs` too.
//!
//! Augmentations (an experiment): fragments are chained as in Dart; when
//! an augmentation has more or fewer type parameters or formal parameters
//! than the augmented fragment, the synthetic fragments of
//! `_linkTypeParameters` / `_linkFormalParameters` are not created, the
//! common prefix is linked.

use std::sync::Arc;

use dartr_ast::*;
use dartr_ast::NamedType;
use dartr_ast_builder::ParsedUnit;
use dartr_element::*;
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_syntax::{TokenId, TokenType};
use indexmap::{IndexMap, IndexSet};

use crate::ast_util::*;
use crate::informative_data::fragment_data_mut;
use crate::library_builder::{ImplicitEnumNodes, LibraryBuilder};
use crate::link::{Linker, LinkerCore};
use crate::reference::{MemberReferenceKind, RefId, TopLevelReferenceKind};

// ---- fragment and element constructors ----

fn fd(name: Option<Name>) -> FragmentData {
    FragmentData::new(name, None)
}

fn ed(name: Option<Name>, first: FragmentId) -> ElementData {
    ElementData::new(name, first)
}

pub fn new_constructor_fragment(fragment: FragmentData) -> ConstructorFragment {
    ConstructorFragment {
        executable: ExecutableFragmentData::new(fragment),
        constant_initializers: OnceSlot::new(),
        new_keyword_offset: None,
        factory_keyword_offset: None,
        type_name: None,
        type_name_offset: None,
        period_offset: None,
        name_end: None,
        this_keyword_offset: None,
    }
}

fn new_field_fragment(name: Option<Name>) -> FieldFragment {
    FieldFragment {
        property: PropertyInducingFragmentData {
            variable: VariableFragmentData::new(fd(name)),
            induced_getter: None,
            induced_setter: None,
        },
        inherits_covariant: BoolSlot::new(false),
    }
}

fn new_top_level_variable_fragment(name: Option<Name>) -> TopLevelVariableFragment {
    TopLevelVariableFragment {
        property: PropertyInducingFragmentData {
            variable: VariableFragmentData::new(fd(name)),
            induced_getter: None,
            induced_setter: None,
        },
    }
}

fn new_getter_fragment(name: Option<Name>) -> GetterFragment {
    GetterFragment {
        accessor: PropertyAccessorFragmentData {
            executable: ExecutableFragmentData::new(fd(name)),
            inducing_variable: None,
        },
    }
}

fn new_setter_fragment(name: Option<Name>) -> SetterFragment {
    SetterFragment {
        accessor: PropertyAccessorFragmentData {
            executable: ExecutableFragmentData::new(fd(name)),
            inducing_variable: None,
        },
    }
}

fn new_formal_parameter_fragment(name: Option<Name>, kind: ParameterKind, private_name: Option<Name>) -> FormalParameterFragment {
    FormalParameterFragment {
        variable: VariableFragmentData::new(fd(name)),
        parameter_kind: kind,
        private_name,
    }
}

fn set(f: &FragmentData, flag: FragmentFlags, value: bool) {
    f.flags.set(flag, value);
}

fn has(store: &ElementStore, f: FragmentId, flag: FragmentFlags) -> bool {
    store.fragment_data(f).is_some_and(|d| d.flags.has(flag))
}

/// Dart `TypeParameterElementImpl(firstFragment:)`.
pub fn new_type_parameter_element(store: &mut ElementStore, fragment: FId<TypeParameterFragment>) -> EId<TypeParameterElement> {
    let name = store.fragment(fragment).name;
    let element = store.add::<TypeParameterElement>(TypeParameterElement::new(ed(name, fragment.raw())));
    store.fragment(fragment).element.set_once(element.raw());
    element
}

/// Dart `FormalParameterFragmentImpl.initElement`.
pub fn init_formal_parameter_element(store: &mut ElementStore, first: FId<FormalParameterFragment>) -> EId<FormalParameterElement> {
    let mut chain = vec![first];
    while let Some(next) = store.fragment(*chain.last().unwrap()).next_fragment {
        chain.push(FId::from_raw(next));
    }
    let fragment = store.fragment(first);
    let data = FormalParameterElement {
        variable: VariableElementData::new(ed(fragment.name, first.raw())),
        kind: fragment.parameter_kind,
        type_: VarSlot::with(TypeId::INVALID),
        base_formal_parameter: None,
        field: VarSlot::new(),
    };
    let explicitly_covariant = fragment
        .flags
        .has(FragmentFlags::FORMAL_PARAMETER_FRAGMENT_IS_EXPLICITLY_COVARIANT);
    let tag = chain
        .iter()
        .map(|f| f.raw().tag())
        .find(|t| matches!(t, Tag::FieldFormalParameter | Tag::SuperFormalParameter))
        .unwrap_or(Tag::FormalParameter);
    let element: EId<FormalParameterElement> = match tag {
        Tag::FieldFormalParameter => store.add::<FieldFormalParameterElement>(data).upcast(),
        Tag::SuperFormalParameter => store.add::<SuperFormalParameterElement>(data).upcast(),
        _ => store.add::<FormalParameterElement>(data),
    };
    store.get(element).flags.set(
        ElementFlags::FORMAL_PARAMETER_ELEMENT_IS_COVARIANT,
        explicitly_covariant,
    );
    for f in chain {
        store.fragment(f).element.set_once(element.raw());
    }
    element
}

/// Dart `GenericFunctionTypeElementImpl(fragment)`.
fn new_generic_function_type_element(store: &mut ElementStore, fragment: FId<GenericFunctionTypeFragment>) {
    let f = store.fragment(fragment);
    let tps = f.type_params.clone();
    let ps = f.formal_params.clone();
    let type_params: Vec<_> = tps
        .iter()
        .map(|&tp| new_type_parameter_element(store, tp))
        .collect();
    let formal_params: Vec<_> = ps
        .iter()
        .map(|&p| init_formal_parameter_element(store, p))
        .collect();
    let element = store.add::<GenericFunctionTypeElement>(GenericFunctionTypeElement {
        element: ed(None, fragment.raw()),
        type_params,
        formal_params,
        return_type: VarSlot::new(),
        type_: VarSlot::new(),
    });
    store.fragment(fragment).element.set_once(element.raw());
}

// ---- FragmentBuilder ----

/// Dart `_EnclosingContext`.
struct EnclosingContext {
    fragment: FragmentId,
    formal_parameters: Vec<FId<FormalParameterFragment>>,
    type_parameters: Vec<FId<TypeParameterFragment>>,
}

/// Dart `FragmentBuilder`.
pub struct FragmentBuilder<'l, 'a> {
    core: &'l mut LinkerCore<'a>,
    lib: &'l mut LibraryBuilder,
    lib_index: usize,
    unit_index: usize,
    parsed: Arc<ParsedUnit>,
    library_fragment: FId<LibraryFragment>,
    export_directive_index: usize,
    import_directive_index: usize,
    part_directive_index: usize,
    stack: Vec<EnclosingContext>,
    declared: IndexMap<NodeId, FragmentId>,
    /// Dart `Linker.declaringFormalParameters`: field and formal fragment
    /// of a declaring parameter of a primary constructor.
    declaring_formal_parameters: IndexMap<NodeId, (FId<FieldFragment>, FId<FormalParameterFragment>)>,
}

impl<'l, 'a> FragmentBuilder<'l, 'a> {
    pub fn new(linker: &'l mut Linker<'a>, lib_index: usize, unit_index: usize) -> Self {
        let Linker { core, builders, .. } = linker;
        let lib = &mut builders[lib_index];
        let unit = &lib.units[unit_index];
        let parsed = unit.parsed.clone();
        let library_fragment = unit.fragment;
        FragmentBuilder {
            core,
            lib,
            lib_index,
            unit_index,
            parsed,
            library_fragment,
            export_directive_index: 0,
            import_directive_index: 0,
            part_directive_index: 0,
            stack: vec![EnclosingContext {
                fragment: library_fragment.raw(),
                formal_parameters: Vec::new(),
                type_parameters: Vec::new(),
            }],
            declared: IndexMap::new(),
            declaring_formal_parameters: IndexMap::new(),
        }
    }

    /// Stores the declared fragments in the linking unit.
    pub fn finish(self) {
        let declared = self.declared;
        self.lib.units[self.unit_index].declared_fragments.extend(declared);
    }

    fn name_of(&self, token: Option<TokenId>) -> Option<Name> {
        let ast = &self.parsed.ast;
        fragment_name(ast, token).map(|n| self.core.name(n))
    }

    fn store(&mut self) -> &mut ElementStore {
        &mut self.core.store
    }

    fn feature(&self, flag: ExperimentalFlag) -> bool {
        self.lib.is_enabled(flag)
    }

    /// Dart `_linker.setFragmentNode` + `node.declaredFragment`.
    fn bind(&mut self, fragment: FragmentId, node: NodeId) {
        self.core
            .fragment_nodes
            .insert(fragment, (self.lib_index, self.unit_index, node));
        self.declared.insert(node, fragment);
    }

    /// Dart `_buildMetadata`: copies the annotations into the
    /// `ConstExprs`.
    fn build_metadata(&mut self, list: NodeList<Annotation>) -> Metadata {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        let mut metadata = Metadata::default();
        for &annotation in ast.list(list) {
            metadata.annotations.push(ElementAnnotation {
                library_fragment: self.library_fragment,
                annotation_ast: self.core.const_exprs.copy(ast, annotation),
            });
        }
        metadata
    }

    fn copy_expr(&mut self, node: impl Into<NodeId>) -> ConstExprId {
        let parsed = self.parsed.clone();
        self.core.const_exprs.copy(&parsed.ast, node)
    }

    fn with_enclosing(&mut self, fragment: FragmentId, f: impl FnOnce(&mut Self)) -> EnclosingContext {
        self.stack.push(EnclosingContext {
            fragment,
            formal_parameters: Vec::new(),
            type_parameters: Vec::new(),
        });
        f(self);
        self.stack.pop().unwrap()
    }

    /// Dart `_addChildFragment`.
    fn add_child_fragment(&mut self, child: FragmentId) {
        let parent = self.stack.last().unwrap().fragment;
        let store = &mut self.core.store;
        self.lib.add_child_fragment(store, parent, child);
    }

    fn add_top_fragment(&mut self, fragment: FragmentId) {
        let store = &mut self.core.store;
        self.lib.add_top_fragment(store, self.library_fragment, fragment);
    }

    /// Dart `_EnclosingContext.addParameter`.
    fn add_parameter(&mut self, fragment: FId<FormalParameterFragment>) {
        let ctx = self.stack.last_mut().unwrap();
        let parent = ctx.fragment;
        ctx.formal_parameters.push(fragment);
        self.core.store.fragment_mut(fragment).enclosing_fragment = Some(parent);
    }

    /// Dart `_EnclosingContext.addTypeParameter`.
    fn add_type_parameter(&mut self, fragment: FId<TypeParameterFragment>) {
        let ctx = self.stack.last_mut().unwrap();
        let parent = ctx.fragment;
        ctx.type_parameters.push(fragment);
        self.core.store.fragment_mut(fragment).enclosing_fragment = Some(parent);
    }

    /// Sets the type parameters of an instance, executable or type alias
    /// fragment (the Dart setters set the enclosing fragment).
    fn set_type_parameters(&mut self, fragment: FragmentId, list: Vec<FId<TypeParameterFragment>>) {
        let store = &mut self.core.store;
        for &tp in &list {
            store.fragment_mut(tp).enclosing_fragment = Some(fragment);
        }
        set_type_params_of(store, fragment, list);
    }

    fn set_formal_parameters(&mut self, fragment: FragmentId, list: Vec<FId<FormalParameterFragment>>) {
        let store = &mut self.core.store;
        for &p in &list {
            store.fragment_mut(p).enclosing_fragment = Some(fragment);
        }
        set_formal_params_of(store, fragment, list);
    }

    // ---- directives ----

    /// Dart `buildDirectives`.
    pub fn build_directives(&mut self) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        for &directive in ast.list(ast.get(parsed.unit).directives) {
            let d = directive.raw();
            if let Some(n) = ast.cast::<ExportDirective>(d) {
                let index = self.export_directive_index;
                self.export_directive_index += 1;
                let metadata = self.build_metadata(ast.get(n).metadata);
                let unit = self.library_fragment;
                if let Some(e) = self.store().fragment_mut(unit).library_exports.get_mut(index) {
                    e.directive.metadata = metadata;
                }
            } else if let Some(n) = ast.cast::<ImportDirective>(d) {
                let index = self.import_directive_index;
                self.import_directive_index += 1;
                let metadata = self.build_metadata(ast.get(n).metadata);
                let unit = self.library_fragment;
                if let Some(e) = self.store().fragment_mut(unit).library_imports.get_mut(index) {
                    e.directive.metadata = metadata;
                }
            } else if let Some(n) = ast.cast::<PartDirective>(d) {
                let index = self.part_directive_index;
                self.part_directive_index += 1;
                let metadata = self.build_metadata(ast.get(n).metadata);
                let unit = self.library_fragment;
                if let Some(e) = self.store().fragment_mut(unit).parts.get_mut(index) {
                    e.directive.metadata = metadata;
                }
            }
        }
    }

    /// Dart `buildLibraryMetadata`. Where Dart shares the metadata object
    /// of the first directive, the annotations are copied again.
    pub fn build_library_metadata(&mut self) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        let directives = ast.list(ast.get(parsed.unit).directives);
        let library = self.lib.element;
        for &d in directives {
            if let Some(l) = ast.cast::<LibraryDirective>(d.raw()) {
                let metadata = self.build_metadata(ast.get(l).metadata);
                self.store().get_mut(library).metadata = metadata;
                return;
            }
        }
        if let Some(&first) = directives.first() {
            let f = first.raw();
            let metadata = if let Some(n) = ast.cast::<ExportDirective>(f) {
                Some(ast.get(n).metadata)
            } else if let Some(n) = ast.cast::<ImportDirective>(f) {
                Some(ast.get(n).metadata)
            } else { ast.cast::<PartDirective>(f).map(|n| ast.get(n).metadata) };
            if let Some(list) = metadata {
                let metadata = self.build_metadata(list);
                self.store().get_mut(library).metadata = metadata;
            }
        }
    }

    // ---- declarations ----

    /// Dart `buildDeclarationFragments`.
    pub fn build_declaration_fragments(&mut self) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        for &declaration in ast.list(ast.get(parsed.unit).declarations) {
            self.visit_declaration(declaration.raw());
        }
    }

    fn visit_declaration(&mut self, d: NodeId) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        if let Some(n) = ast.cast::<ClassDeclaration>(d) {
            self.visit_class_declaration(n);
        } else if let Some(n) = ast.cast::<ClassTypeAlias>(d) {
            self.visit_class_type_alias(n);
        } else if let Some(n) = ast.cast::<EnumDeclaration>(d) {
            self.visit_enum_declaration(n);
        } else if let Some(n) = ast.cast::<ExtensionDeclaration>(d) {
            self.visit_extension_declaration(n);
        } else if let Some(n) = ast.cast::<ExtensionTypeDeclaration>(d) {
            self.visit_extension_type_declaration(n);
        } else if let Some(n) = ast.cast::<FunctionDeclaration>(d) {
            self.visit_function_declaration(n);
        } else if let Some(n) = ast.cast::<FunctionTypeAlias>(d) {
            self.visit_function_type_alias(n);
        } else if let Some(n) = ast.cast::<GenericTypeAlias>(d) {
            self.visit_generic_type_alias(n);
        } else if let Some(n) = ast.cast::<MixinDeclaration>(d) {
            self.visit_mixin_declaration(n);
        } else if let Some(n) = ast.cast::<TopLevelVariableDeclaration>(d) {
            self.visit_top_level_variable_declaration(n);
        } else {
            panic!("unexpected declaration {:?}", ast.kind(d));
        }
    }

    fn visit_class_member(&mut self, m: NodeId) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        if let Some(n) = ast.cast::<ConstructorDeclaration>(m) {
            self.visit_constructor_declaration(n);
        } else if let Some(n) = ast.cast::<FieldDeclaration>(m) {
            self.visit_field_declaration(n);
        } else if let Some(n) = ast.cast::<MethodDeclaration>(m) {
            self.visit_method_declaration(n);
        } else if ast.is::<PrimaryConstructorBody>(m) {
            // Dart `visitPrimaryConstructorBody`: handled by the
            // primary constructor declaration.
        } else {
            panic!("unexpected class member {:?}", ast.kind(m));
        }
    }

    /// Dart `visitBlockClassBody` / `visitEmptyClassBody`.
    fn visit_class_body(&mut self, body: Id<ClassBody>) {
        let parsed = self.parsed.clone();
        for m in class_body_members(&parsed.ast, body) {
            self.visit_class_member(m.raw());
        }
    }

    /// `ClassNamePart.accept`: `visitNameWithTypeParameters` or
    /// `visitPrimaryConstructorDeclaration`.
    fn visit_class_name_part(&mut self, part: Id<ClassNamePart>) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        if let Some(n) = ast.cast::<NameWithTypeParameters>(part.raw()) {
            if let Some(tps) = ast.get(n).type_parameters {
                self.visit_type_parameter_list(tps);
            }
        } else if let Some(p) = ast.cast::<PrimaryConstructorDeclaration>(part.raw()) {
            self.visit_primary_constructor_declaration(p);
        }
    }

    fn visit_type_parameter_list(&mut self, list: Id<TypeParameterList>) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        for &tp in ast.list(ast.get(list).type_parameters) {
            self.visit_type_parameter(tp);
        }
    }

    /// Dart `visitTypeParameter`.
    fn visit_type_parameter(&mut self, node: Id<TypeParameter>) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        let n = ast.get(node);
        let name = self.name_of(Some(n.name));
        let mut data = TypeParameterFragment { fragment: fd(name) };
        data.fragment.metadata = self.build_metadata(n.metadata);
        let fragment = self.store().add_fragment::<TypeParameterFragment>(data);
        self.bind(fragment.raw(), node.raw());
        self.add_type_parameter(fragment);
        if let Some(bound) = n.bound {
            self.visit_type(bound);
        }
    }

    /// Type annotations: `visitNamedType`, `visitGenericFunctionType`,
    /// `visitRecordTypeAnnotation` (and its fields).
    fn visit_type(&mut self, node: Id<TypeAnnotation>) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        let t = node.raw();
        if let Some(n) = ast.cast::<NamedType>(t) {
            if let Some(args) = ast.get(n).type_arguments {
                self.visit_type_argument_list(args);
            }
        } else if let Some(g) = ast.cast::<GenericFunctionType>(t) {
            self.visit_generic_function_type(g);
        } else if let Some(r) = ast.cast::<RecordTypeAnnotation>(t) {
            let r = ast.get(r);
            for &f in ast.list(r.positional_fields) {
                self.visit_type(ast.get(f).type_);
            }
            if let Some(named) = r.named_fields {
                for &f in ast.list(ast.get(named).fields) {
                    self.visit_type(ast.get(f).type_);
                }
            }
        }
    }

    fn visit_type_argument_list(&mut self, list: Id<TypeArgumentList>) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        for &a in ast.list(ast.get(list).arguments) {
            self.visit_type(a);
        }
    }

    fn visit_named_types(&mut self, list: NodeList<NamedType>) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        for &t in ast.list(list) {
            self.visit_type(t.upcast());
        }
    }

    /// Dart `visitGenericFunctionType`.
    fn visit_generic_function_type(&mut self, node: Id<GenericFunctionType>) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        let n = ast.get(node);
        let mut data = GenericFunctionTypeFragment {
            fragment: fd(None),
            type_params: Vec::new(),
            formal_params: Vec::new(),
            is_nullable: n.question.is_some(),
        };
        // Dart `_libraryFragment.encloseElement(fragment)`.
        data.fragment.enclosing_fragment = Some(self.library_fragment.raw());
        data.fragment.first_token_offset = Some(ast.offset(node));
        let fragment = self.store().add_fragment::<GenericFunctionTypeFragment>(data);
        self.bind(fragment.raw(), node.raw());
        let holder = self.with_enclosing(fragment.raw(), |b| {
            if let Some(tps) = n.type_parameters {
                b.visit_type_parameter_list(tps);
            }
            let tps = std::mem::take(&mut b.stack.last_mut().unwrap().type_parameters);
            b.store().fragment_mut(fragment).type_params = tps.clone();
            b.stack.last_mut().unwrap().type_parameters = tps;
            b.visit_formal_parameter_list(n.parameters);
        });
        {
            let f = self.store().fragment_mut(fragment);
            f.type_params = holder.type_parameters;
            f.formal_params = holder.formal_parameters;
        }
        new_generic_function_type_element(self.store(), fragment);
        if let Some(r) = n.return_type {
            self.visit_type(r);
        }
    }

    fn visit_formal_parameter_list(&mut self, list: Id<FormalParameterList>) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        for &p in ast.list(ast.get(list).parameters) {
            let raw = p.raw();
            if let Some(r) = ast.cast::<RegularFormalParameter>(raw) {
                self.visit_regular_formal_parameter(r);
            } else if let Some(f) = ast.cast::<FieldFormalParameter>(raw) {
                self.visit_field_formal_parameter(f);
            } else if let Some(s) = ast.cast::<SuperFormalParameter>(raw) {
                self.visit_super_formal_parameter(s);
            }
        }
    }

    /// Dart `visitClassDeclaration`.
    fn visit_class_declaration(&mut self, node: Id<ClassDeclaration>) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        let n = ast.get(node);
        let name = self.name_of(Some(class_name_part_name(ast, n.name_part)));
        let mut data = ClassFragment {
            interface: InterfaceFragmentData::new(fd(name)),
        };
        let f = &data.fragment;
        set(f, FragmentFlags::CLASS_FRAGMENT_IS_ABSTRACT, n.abstract_keyword.is_some());
        set(f, FragmentFlags::FRAGMENT_IS_AUGMENTATION, n.augment_keyword.is_some());
        set(f, FragmentFlags::CLASS_FRAGMENT_IS_BASE, n.base_keyword.is_some());
        set(f, FragmentFlags::CLASS_FRAGMENT_IS_FINAL, n.final_keyword.is_some());
        set(f, FragmentFlags::CLASS_FRAGMENT_IS_INTERFACE, n.interface_keyword.is_some());
        set(f, FragmentFlags::CLASS_FRAGMENT_IS_MIXIN_CLASS, n.mixin_keyword.is_some());
        set(f, FragmentFlags::CLASS_FRAGMENT_IS_SEALED, n.sealed_keyword.is_some());
        set(f, FragmentFlags::CLASS_FRAGMENT_HAS_EXTENDS_CLAUSE, n.extends_clause.is_some());
        data.fragment.metadata = self.build_metadata(n.metadata);
        let fragment = self.store().add_fragment::<ClassFragment>(data);
        self.bind(fragment.raw(), node.raw());
        self.add_top_fragment(fragment.raw());
        let holder = self.with_enclosing(fragment.raw(), |b| {
            b.visit_class_name_part(n.name_part);
            b.visit_class_body(n.body);
        });
        self.set_type_parameters(fragment.raw(), holder.type_parameters);
        if let Some(e) = n.extends_clause {
            self.visit_type(ast.get(e).superclass.upcast());
        }
        if let Some(w) = n.with_clause {
            self.visit_named_types(ast.get(w).mixin_types);
        }
        if let Some(i) = n.implements_clause {
            self.visit_named_types(ast.get(i).interfaces);
        }
    }

    /// Dart `visitClassTypeAlias`.
    fn visit_class_type_alias(&mut self, node: Id<ClassTypeAlias>) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        let n = ast.get(node);
        let name = self.name_of(Some(n.name));
        let mut data = ClassFragment {
            interface: InterfaceFragmentData::new(fd(name)),
        };
        let f = &data.fragment;
        set(f, FragmentFlags::CLASS_FRAGMENT_IS_ABSTRACT, n.abstract_keyword.is_some());
        set(f, FragmentFlags::CLASS_FRAGMENT_IS_BASE, n.base_keyword.is_some());
        set(f, FragmentFlags::CLASS_FRAGMENT_IS_FINAL, n.final_keyword.is_some());
        set(f, FragmentFlags::CLASS_FRAGMENT_IS_INTERFACE, n.interface_keyword.is_some());
        set(f, FragmentFlags::CLASS_FRAGMENT_IS_MIXIN_APPLICATION, true);
        set(f, FragmentFlags::CLASS_FRAGMENT_IS_MIXIN_CLASS, n.mixin_keyword.is_some());
        set(f, FragmentFlags::CLASS_FRAGMENT_IS_SEALED, n.sealed_keyword.is_some());
        data.fragment.metadata = self.build_metadata(n.metadata);
        let fragment = self.store().add_fragment::<ClassFragment>(data);
        self.bind(fragment.raw(), node.raw());
        self.add_top_fragment(fragment.raw());
        let holder = self.with_enclosing(fragment.raw(), |b| {
            if let Some(tps) = n.type_parameters {
                b.visit_type_parameter_list(tps);
            }
        });
        if n.type_parameters.is_some() {
            self.set_type_parameters(fragment.raw(), holder.type_parameters);
        }
        self.visit_type(n.superclass.upcast());
        self.visit_named_types(ast.get(n.with_clause).mixin_types);
        if let Some(i) = n.implements_clause {
            self.visit_named_types(ast.get(i).interfaces);
        }
    }

    /// Dart `visitConstructorDeclaration`.
    fn visit_constructor_declaration(&mut self, node: Id<ConstructorDeclaration>) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        let n = ast.get(node);
        let name = self.name_of(n.name).unwrap_or_else(|| self.core.name("new"));
        let mut data = new_constructor_fragment(fd(Some(name)));
        let is_factory = n.factory_keyword.is_some();
        let mut is_const = n.const_keyword.is_some();
        {
            let f = &data.fragment;
            set(f, FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_ORIGIN_DECLARATION, true);
            set(f, FragmentFlags::FRAGMENT_IS_AUGMENTATION, n.augment_keyword.is_some());
            set(f, FragmentFlags::EXECUTABLE_FRAGMENT_IS_EXTERNAL, n.external_keyword.is_some());
            set(f, FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY, is_factory);
            let redirecting = n.redirected_constructor.is_some()
                || ast
                    .list(n.initializers)
                    .iter()
                    .any(|i| ast.is::<RedirectingConstructorInvocation>(i.raw()));
            set(f, FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_REDIRECTING, redirecting);
            set(f, FragmentFlags::FRAGMENT_IS_COMPLETE, constructor_is_complete(ast, node));
        }
        data.fragment.metadata = self.build_metadata(n.metadata);
        data.type_name = n
            .type_name
            .map(|t| self.core.name(ast.tokens.lexeme(ast.get(t).token)));
        let enclosing_is_enum = self.stack.last().unwrap().fragment.tag() == Tag::Enum;
        if enclosing_is_enum && !is_factory && self.feature(ExperimentalFlag::PrimaryConstructors) {
            is_const = true;
        }
        set(&data.fragment, FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST, is_const);
        if is_const || is_factory {
            let initializers: Vec<ConstExprId> = ast
                .list(n.initializers)
                .iter()
                .map(|&i| self.core.const_exprs.copy(ast, i))
                .collect();
            data.constant_initializers.set_once(initializers);
        }
        let fragment = self.store().add_fragment::<ConstructorFragment>(data);
        self.bind(fragment.raw(), node.raw());
        self.add_child_fragment(fragment.raw());
        self.build_executable_element_children(fragment.raw(), Some(n.parameters), None);
    }

    /// Dart `visitEnumDeclaration`, with the synthetic enum AST.
    fn visit_enum_declaration(&mut self, node: Id<EnumDeclaration>) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        let n = ast.get(node);
        let enum_name_text = fragment_name(ast, Some(class_name_part_name(ast, n.name_part)))
            .unwrap_or("")
            .to_string();
        let name = self.name_of(Some(class_name_part_name(ast, n.name_part)));
        let mut data = EnumFragment {
            interface: InterfaceFragmentData::new(fd(name)),
        };
        set(&data.fragment, FragmentFlags::FRAGMENT_IS_AUGMENTATION, n.augment_keyword.is_some());
        data.fragment.metadata = self.build_metadata(n.metadata);
        let fragment = self.store().add_fragment::<EnumFragment>(data);
        self.bind(fragment.raw(), node.raw());
        self.add_top_fragment(fragment.raw());
        if let Some(w) = n.with_clause {
            self.visit_named_types(ast.get(w).mixin_types);
        }
        if let Some(i) = n.implements_clause {
            self.visit_named_types(ast.get(i).interfaces);
        }
        let holder = self.with_enclosing(fragment.raw(), |b| {
            let mut values_elements: Vec<String> = Vec::new();
            let mut values_names: IndexSet<Arc<str>> = IndexSet::new();
            for constant in enum_body_constants(ast, n.body) {
                let c = ast.get(constant);
                let text = ast.tokens.lexeme(c.name).to_string();
                let mut field = new_field_fragment(b.name_of(Some(c.name)));
                {
                    let f = &field.fragment;
                    set(f, FragmentFlags::VARIABLE_FRAGMENT_HAS_IMPLICIT_TYPE, true);
                    set(f, FragmentFlags::NON_PARAMETER_VARIABLE_FRAGMENT_HAS_INITIALIZER, true);
                    set(f, FragmentFlags::FRAGMENT_IS_AUGMENTATION, c.augment_keyword.is_some());
                    set(f, FragmentFlags::VARIABLE_FRAGMENT_IS_CONST, true);
                    set(f, FragmentFlags::FIELD_FRAGMENT_IS_ENUM_CONSTANT, true);
                    set(f, FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_DECLARATION, true);
                    set(f, FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC, true);
                }
                field.fragment.metadata = b.build_metadata(c.metadata);
                let initializer = b.enum_constant_initializer(&enum_name_text, c.arguments);
                field.constant_initializer = Some(initializer);
                let field = b.store().add_fragment::<FieldFragment>(field);
                b.bind(field.raw(), constant.raw());
                b.add_child_fragment(field.raw());
                values_elements.push(text.clone());
                values_names.insert(text.into());
            }
            let mut values = new_field_fragment(Some(b.core.name("values")));
            {
                let f = &values.fragment;
                set(f, FragmentFlags::VARIABLE_FRAGMENT_IS_CONST, true);
                set(f, FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC, true);
                set(f, FragmentFlags::FIELD_FRAGMENT_IS_ORIGIN_ENUM_VALUES, true);
            }
            let (values_initializer, values_type_node) = b.enum_values_nodes(&enum_name_text, &values_elements);
            values.constant_initializer = Some(values_initializer);
            let values = b.store().add_fragment::<FieldFragment>(values);
            b.add_child_fragment(values.raw());
            b.lib.implicit_enum_nodes.insert(
                fragment,
                ImplicitEnumNodes {
                    fragment,
                    values_fragment: values,
                    values_names,
                    values_initializer,
                    values_type_node,
                },
            );
            b.visit_class_name_part(n.name_part);
            for m in enum_body_members(ast, n.body) {
                b.visit_class_member(m.raw());
            }
        });
        self.set_type_parameters(fragment.raw(), holder.type_parameters);
    }

    /// The synthetic `InstanceCreationExpression` of an enum constant
    /// (`E<typeArguments>.name(arguments)`), in the `ConstExprs`.
    fn enum_constant_initializer(&mut self, enum_name: &str, arguments: Option<Id<EnumConstantArguments>>) -> ConstExprId {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        let args = arguments.map(|a| ast.get(a));
        let type_arguments = args
            .and_then(|a| a.type_arguments)
            .map(|t| self.core.const_exprs.copy(ast, t).0);
        let argument_list = args.map(|a| self.core.const_exprs.copy(ast, a.argument_list).0);
        let constructor_name = args
            .and_then(|a| a.constructor_selector)
            .map(|s| ast.tokens.lexeme(ast.get(ast.get(s).name).token).to_string());
        let dst = &mut self.core.const_exprs.ast;
        let type_name = synthetic_string(dst, enum_name);
        let named_type = dst.add(NamedType {
            import_prefix: None,
            name: type_name,
            type_arguments: type_arguments.map(dst_cast),
            question: None,
        });
        let (period, name) = match &constructor_name {
            Some(c) => {
                let period = dst.tokens.push_synthetic(TokenType::PERIOD, 0, 0);
                let token = synthetic_string(dst, c);
                let id = dst.add(SimpleIdentifier { token });
                (Some(period), Some(id))
            }
            None => (None, None),
        };
        let constructor = dst.add(ConstructorName {
            type_: named_type,
            period,
            name,
        });
        let argument_list = match argument_list {
            Some(a) => Id::<ArgumentList>::from_raw(a),
            None => {
                let open = dst.tokens.push_synthetic(TokenType::OPEN_PAREN, 0, 0);
                let close = dst.tokens.push_synthetic(TokenType::CLOSE_PAREN, 0, 0);
                let arguments = dst.new_list(std::iter::empty());
                dst.add(ArgumentList {
                    left_parenthesis: open,
                    arguments,
                    right_parenthesis: close,
                })
            }
        };
        let creation = dst.add(InstanceCreationExpression {
            keyword: None,
            constructor_name: constructor,
            type_arguments: None,
            argument_list,
        });
        ConstExprId(creation.raw())
    }

    /// The synthetic `values` initializer `[a, b, ...]` and its type
    /// `List<E>`, in the `ConstExprs`.
    fn enum_values_nodes(&mut self, enum_name: &str, names: &[String]) -> (ConstExprId, ConstExprId) {
        let dst = &mut self.core.const_exprs.ast;
        let mut elements = Vec::new();
        for n in names {
            let token = synthetic_string(dst, n);
            elements.push(dst.add(SimpleIdentifier { token }).raw());
        }
        let open = dst.tokens.push_synthetic(TokenType::OPEN_SQUARE_BRACKET, 0, 0);
        let close = dst.tokens.push_synthetic(TokenType::CLOSE_SQUARE_BRACKET, 0, 0);
        let elements = dst.new_list(elements.into_iter().map(Id::<CollectionElement>::from_raw));
        let list = dst.add(ListLiteral {
            const_keyword: None,
            type_arguments: None,
            left_bracket: open,
            elements,
            right_bracket: close,
        });
        let list_name = synthetic_string(dst, "List");
        let enum_token = synthetic_string(dst, enum_name);
        let argument = dst.add(NamedType {
            import_prefix: None,
            name: enum_token,
            type_arguments: None,
            question: None,
        });
        let lt = dst.tokens.push_synthetic(TokenType::LT, 0, 0);
        let gt = dst.tokens.push_synthetic(TokenType::GT, 0, 0);
        let arguments = dst.new_list(std::iter::once(argument.upcast::<TypeAnnotation>()));
        let type_arguments = dst.add(TypeArgumentList {
            left_bracket: lt,
            arguments,
            right_bracket: gt,
        });
        let values_type = dst.add(NamedType {
            import_prefix: None,
            name: list_name,
            type_arguments: Some(type_arguments),
            question: None,
        });
        (ConstExprId(list.raw()), ConstExprId(values_type.raw()))
    }

    /// Dart `visitExtensionDeclaration`.
    fn visit_extension_declaration(&mut self, node: Id<ExtensionDeclaration>) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        let n = ast.get(node);
        let name = self.name_of(n.name);
        let mut data = ExtensionFragment {
            instance: InstanceFragmentData::new(fd(name)),
        };
        set(&data.fragment, FragmentFlags::FRAGMENT_IS_AUGMENTATION, n.augment_keyword.is_some());
        data.fragment.metadata = self.build_metadata(n.metadata);
        let fragment = self.store().add_fragment::<ExtensionFragment>(data);
        self.bind(fragment.raw(), node.raw());
        self.add_top_fragment(fragment.raw());
        let holder = self.with_enclosing(fragment.raw(), |b| {
            if let Some(tps) = n.type_parameters {
                b.visit_type_parameter_list(tps);
            }
            b.visit_class_body(n.body);
        });
        self.set_type_parameters(fragment.raw(), holder.type_parameters);
        if let Some(on) = n.on_clause {
            self.visit_type(ast.get(on).extended_type);
        }
    }

    /// Dart `visitExtensionTypeDeclaration`.
    fn visit_extension_type_declaration(&mut self, node: Id<ExtensionTypeDeclaration>) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        let n = ast.get(node);
        let name = self.name_of(Some(class_name_part_name(ast, n.name_part)));
        let mut data = ExtensionTypeFragment {
            interface: InterfaceFragmentData::new(fd(name)),
        };
        set(&data.fragment, FragmentFlags::FRAGMENT_IS_AUGMENTATION, n.augment_keyword.is_some());
        data.fragment.metadata = self.build_metadata(n.metadata);
        let fragment = self.store().add_fragment::<ExtensionTypeFragment>(data);
        self.bind(fragment.raw(), node.raw());
        self.add_top_fragment(fragment.raw());
        let holder = self.with_enclosing(fragment.raw(), |b| {
            b.visit_class_name_part(n.name_part);
            b.visit_class_body(n.body);
        });
        self.set_type_parameters(fragment.raw(), holder.type_parameters);
        if let Some(i) = n.implements_clause {
            self.visit_named_types(ast.get(i).interfaces);
        }
    }

    /// Dart `visitFieldDeclaration`.
    fn visit_field_declaration(&mut self, node: Id<FieldDeclaration>) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        let n = ast.get(node);
        let list = n.fields;
        let is_const = variable_list_is_const(ast, list);
        let is_final = variable_list_is_final(ast, list);
        let is_static = field_is_static(ast, node);
        for &variable in ast.list(ast.get(list).variables) {
            let v = ast.get(variable);
            let mut data = new_field_fragment(self.name_of(Some(v.name)));
            {
                let f = &data.fragment;
                set(f, FragmentFlags::NON_PARAMETER_VARIABLE_FRAGMENT_HAS_INITIALIZER, v.initializer.is_some());
                set(f, FragmentFlags::VARIABLE_FRAGMENT_IS_ABSTRACT, n.abstract_keyword.is_some());
                set(f, FragmentFlags::FRAGMENT_IS_AUGMENTATION, n.augment_keyword.is_some());
                set(f, FragmentFlags::VARIABLE_FRAGMENT_IS_CONST, is_const);
                set(f, FragmentFlags::FIELD_FRAGMENT_IS_EXPLICITLY_COVARIANT, n.covariant_keyword.is_some());
                set(f, FragmentFlags::VARIABLE_FRAGMENT_IS_EXTERNAL, n.external_keyword.is_some());
                set(f, FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL, is_final);
                set(f, FragmentFlags::VARIABLE_FRAGMENT_IS_LATE, variable_list_is_late(ast, list));
                set(f, FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_DECLARATION, true);
                set(f, FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC, is_static);
                set(f, FragmentFlags::VARIABLE_FRAGMENT_HAS_IMPLICIT_TYPE, ast.get(list).type_.is_none());
            }
            // Dart shares one metadata object for all variables.
            data.fragment.metadata = self.build_metadata(n.metadata);
            let mut is_final_instance = false;
            if let Some(initializer) = v.initializer {
                if is_const {
                    data.constant_initializer = Some(self.copy_expr(initializer));
                } else if is_final && !is_static {
                    data.constant_initializer = Some(self.copy_expr(initializer));
                    is_final_instance = true;
                }
            }
            let fragment = self.store().add_fragment::<FieldFragment>(data);
            if is_final_instance {
                self.lib.final_instance_fields.insert(fragment);
            }
            self.bind(fragment.raw(), variable.raw());
            self.add_child_fragment(fragment.raw());
        }
        if let Some(t) = ast.get(list).type_ {
            self.visit_type(t);
        }
    }

    /// The common part of the three formal parameter visits.
    fn finish_formal_parameter(
        &mut self,
        fragment: FId<FormalParameterFragment>,
        node: NodeId,
        default_value: Option<Id<Expression>>,
        has_implicit_type: bool,
        metadata: NodeList<Annotation>,
        suffix: Option<Id<FunctionTypedFormalParameterSuffix>>,
        type_: Option<Id<TypeAnnotation>>,
    ) {
        self.add_parameter(fragment);
        let initializer = default_value.map(|v| self.copy_expr(v));
        let metadata = self.build_metadata(metadata);
        {
            let f = self.store().fragment_mut(fragment);
            f.constant_initializer = initializer;
            f.fragment.metadata = metadata;
            f.flags
                .set(FragmentFlags::VARIABLE_FRAGMENT_HAS_IMPLICIT_TYPE, has_implicit_type);
            f.flags
                .set(FragmentFlags::FORMAL_PARAMETER_FRAGMENT_IS_ORIGIN_DECLARATION, true);
        }
        self.declared.insert(node, fragment.raw());
        self.build_function_typed_parameter_suffix(fragment, suffix);
        if let Some(t) = type_ {
            self.visit_type(t);
        }
    }

    /// Dart `visitRegularFormalParameter`.
    fn visit_regular_formal_parameter(&mut self, node: Id<RegularFormalParameter>) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        let n = ast.get(node);
        let declaring = self.declaring_formal_parameters.get(&node.raw()).copied();
        let fragment = match declaring {
            Some((_, formal)) => formal,
            None => {
                let name = self.name_of(n.name);
                self.store()
                    .add_fragment::<FormalParameterFragment>(new_formal_parameter_fragment(name, n.kind, None))
            }
        };
        self.core
            .fragment_nodes
            .insert(fragment.raw(), (self.lib_index, self.unit_index, node.raw()));
        {
            let is_final = n
                .const_final_or_var_keyword
                .is_some_and(|k| ast.tokens.lexeme(k) == "final");
            let f = self.core.store.fragment(fragment);
            f.flags.set(
                FragmentFlags::FORMAL_PARAMETER_FRAGMENT_IS_EXPLICITLY_COVARIANT,
                declaring.is_none() && n.covariant_keyword.is_some(),
            );
            f.flags.set(FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL, is_final);
        }
        self.finish_formal_parameter(
            fragment,
            node.raw(),
            n.default_clause.map(|d| ast.get(d).value),
            n.type_.is_none() && n.function_typed_suffix.is_none(),
            n.metadata,
            n.function_typed_suffix,
            n.type_,
        );
    }

    /// Dart `visitFieldFormalParameter`.
    fn visit_field_formal_parameter(&mut self, node: Id<FieldFormalParameter>) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        let n = ast.get(node);
        let mut name = fragment_name(ast, Some(n.name)).map(str::to_string);
        let mut private_name = None;
        if let Some(n2) = &name
            && n.kind.is_named()
            && self.feature(ExperimentalFlag::PrivateNamedParameters)
            && let Some(public) = corresponding_public_name(n2)
        {
            private_name = Some(n2.clone());
            name = Some(public);
        }
        let name = self.core.name_opt(name.as_deref());
        let private_name = self.core.name_opt(private_name.as_deref());
        let fragment: FId<FormalParameterFragment> = self
            .store()
            .add_fragment::<FieldFormalParameterFragment>(new_formal_parameter_fragment(name, n.kind, private_name))
            .upcast();
        self.core
            .fragment_nodes
            .insert(fragment.raw(), (self.lib_index, self.unit_index, node.raw()));
        self.finish_formal_parameter(
            fragment,
            node.raw(),
            n.default_clause.map(|d| ast.get(d).value),
            n.type_.is_none() && n.function_typed_suffix.is_none(),
            n.metadata,
            n.function_typed_suffix,
            n.type_,
        );
    }

    /// Dart `visitSuperFormalParameter`.
    fn visit_super_formal_parameter(&mut self, node: Id<SuperFormalParameter>) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        let n = ast.get(node);
        let name = self.name_of(Some(n.name));
        let fragment: FId<FormalParameterFragment> = self
            .store()
            .add_fragment::<SuperFormalParameterFragment>(new_formal_parameter_fragment(name, n.kind, None))
            .upcast();
        self.core
            .fragment_nodes
            .insert(fragment.raw(), (self.lib_index, self.unit_index, node.raw()));
        self.finish_formal_parameter(
            fragment,
            node.raw(),
            n.default_clause.map(|d| ast.get(d).value),
            n.type_.is_none() && n.function_typed_suffix.is_none(),
            n.metadata,
            n.function_typed_suffix,
            n.type_,
        );
    }

    /// Dart `visitFunctionDeclaration`.
    fn visit_function_declaration(&mut self, node: Id<FunctionDeclaration>) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        let n = ast.get(node);
        let name = self.name_of(Some(n.name));
        let fe = ast.get(n.function_expression);
        let body = fe.body;
        let fragment: FragmentId = if function_is_getter(ast, node) {
            let data = new_getter_fragment(name);
            set(&data.fragment, FragmentFlags::FRAGMENT_IS_AUGMENTATION, n.augment_keyword.is_some());
            set(&data.fragment, FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_DECLARATION, true);
            set(&data.fragment, FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC, true);
            let f = self.store().add_fragment::<GetterFragment>(data).raw();
            self.add_top_fragment(f);
            f
        } else if function_is_setter(ast, node) {
            let data = new_setter_fragment(name);
            set(&data.fragment, FragmentFlags::FRAGMENT_IS_AUGMENTATION, n.augment_keyword.is_some());
            set(&data.fragment, FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_DECLARATION, true);
            set(&data.fragment, FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC, true);
            let f = self.store().add_fragment::<SetterFragment>(data).raw();
            self.add_top_fragment(f);
            f
        } else {
            let data = TopLevelFunctionFragment {
                executable: ExecutableFragmentData::new(fd(name)),
            };
            set(&data.fragment, FragmentFlags::FRAGMENT_IS_AUGMENTATION, n.augment_keyword.is_some());
            set(&data.fragment, FragmentFlags::TOP_LEVEL_FUNCTION_FRAGMENT_IS_ORIGIN_DECLARATION, true);
            set(&data.fragment, FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC, true);
            let f = self.store().add_fragment::<TopLevelFunctionFragment>(data).raw();
            self.add_top_fragment(f);
            f
        };
        let metadata = self.build_metadata(n.metadata);
        {
            let f = fragment_data_mut(self.store(), fragment);
            f.flags.set(FragmentFlags::EXECUTABLE_FRAGMENT_HAS_IMPLICIT_RETURN_TYPE, n.return_type.is_none());
            f.flags.set(FragmentFlags::EXECUTABLE_FRAGMENT_IS_ASYNCHRONOUS, is_asynchronous(ast, body));
            f.flags.set(FragmentFlags::EXECUTABLE_FRAGMENT_IS_EXTERNAL, n.external_keyword.is_some());
            f.flags.set(FragmentFlags::EXECUTABLE_FRAGMENT_IS_GENERATOR, is_generator(ast, body));
            f.flags.set(FragmentFlags::FRAGMENT_IS_COMPLETE, function_is_complete(ast, node));
            f.metadata = metadata;
        }
        self.bind(fragment, node.raw());
        self.build_executable_element_children(fragment, fe.parameters, fe.type_parameters);
        if let Some(r) = n.return_type {
            self.visit_type(r);
        }
    }

    /// Dart `visitFunctionTypeAlias`.
    fn visit_function_type_alias(&mut self, node: Id<FunctionTypeAlias>) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        let n = ast.get(node);
        let name = self.name_of(Some(n.name));
        let mut data = TypeAliasFragment {
            fragment: fd(name),
            type_params: Vec::new(),
            has_self_reference: BoolSlot::new(false),
        };
        data.fragment.first_token_offset = Some(ast.offset(node));
        data.fragment.metadata = self.build_metadata(n.metadata);
        let fragment = self.store().add_fragment::<TypeAliasFragment>(data);
        self.bind(fragment.raw(), node.raw());
        self.add_top_fragment(fragment.raw());
        let holder = self.with_enclosing(fragment.raw(), |b| {
            if let Some(tps) = n.type_parameters {
                b.visit_type_parameter_list(tps);
            }
            if let Some(r) = n.return_type {
                b.visit_type(r);
            }
            b.visit_formal_parameter_list(n.parameters);
        });
        self.set_type_parameters(fragment.raw(), holder.type_parameters);
        for p in holder.formal_parameters {
            init_formal_parameter_element(self.store(), p);
        }
    }

    /// Dart `visitGenericTypeAlias`.
    fn visit_generic_type_alias(&mut self, node: Id<GenericTypeAlias>) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        let n = ast.get(node);
        let name = self.name_of(Some(n.name));
        let mut data = TypeAliasFragment {
            fragment: fd(name),
            type_params: Vec::new(),
            has_self_reference: BoolSlot::new(false),
        };
        data.fragment.first_token_offset = Some(ast.offset(node));
        set(&data.fragment, FragmentFlags::FRAGMENT_IS_AUGMENTATION, n.augment_keyword.is_some());
        data.fragment.metadata = self.build_metadata(n.metadata);
        let fragment = self.store().add_fragment::<TypeAliasFragment>(data);
        self.bind(fragment.raw(), node.raw());
        self.add_top_fragment(fragment.raw());
        let holder = self.with_enclosing(fragment.raw(), |b| {
            if let Some(tps) = n.type_parameters {
                b.visit_type_parameter_list(tps);
            }
        });
        self.set_type_parameters(fragment.raw(), holder.type_parameters);
        self.visit_type(n.type_);
    }

    /// Dart `visitMethodDeclaration`.
    fn visit_method_declaration(&mut self, node: Id<MethodDeclaration>) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        let n = ast.get(node);
        let name = self.name_of(Some(n.name));
        let is_complete = method_is_complete(ast, node);
        let is_static = method_is_static(ast, node);
        let fragment: FragmentId = if method_is_getter(ast, node) {
            let data = new_getter_fragment(name);
            set(&data.fragment, FragmentFlags::EXECUTABLE_FRAGMENT_IS_ABSTRACT, !is_complete);
            set(&data.fragment, FragmentFlags::FRAGMENT_IS_AUGMENTATION, n.augment_keyword.is_some());
            set(&data.fragment, FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_DECLARATION, true);
            set(&data.fragment, FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC, is_static);
            self.store().add_fragment::<GetterFragment>(data).raw()
        } else if method_is_setter(ast, node) {
            let data = new_setter_fragment(name);
            set(&data.fragment, FragmentFlags::EXECUTABLE_FRAGMENT_IS_ABSTRACT, !is_complete);
            set(&data.fragment, FragmentFlags::FRAGMENT_IS_AUGMENTATION, n.augment_keyword.is_some());
            set(&data.fragment, FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_DECLARATION, true);
            set(&data.fragment, FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC, is_static);
            self.store().add_fragment::<SetterFragment>(data).raw()
        } else {
            let data = MethodFragment {
                executable: ExecutableFragmentData::new(fd(name)),
            };
            set(&data.fragment, FragmentFlags::EXECUTABLE_FRAGMENT_IS_ABSTRACT, !is_complete);
            set(&data.fragment, FragmentFlags::FRAGMENT_IS_AUGMENTATION, n.augment_keyword.is_some());
            set(&data.fragment, FragmentFlags::METHOD_FRAGMENT_IS_ORIGIN_DECLARATION, true);
            set(&data.fragment, FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC, is_static);
            self.store().add_fragment::<MethodFragment>(data).raw()
        };
        self.add_child_fragment(fragment);
        let metadata = self.build_metadata(n.metadata);
        let invokes_super = invokes_super_self(ast, node);
        {
            let f = fragment_data_mut(self.store(), fragment);
            f.flags.set(FragmentFlags::EXECUTABLE_FRAGMENT_HAS_IMPLICIT_RETURN_TYPE, n.return_type.is_none());
            f.flags.set(FragmentFlags::EXECUTABLE_FRAGMENT_INVOKES_SUPER_SELF, invokes_super);
            f.flags.set(FragmentFlags::EXECUTABLE_FRAGMENT_IS_ASYNCHRONOUS, is_asynchronous(ast, n.body));
            f.flags.set(
                FragmentFlags::EXECUTABLE_FRAGMENT_IS_EXTERNAL,
                n.external_keyword.is_some() || ast.is::<NativeFunctionBody>(n.body.raw()),
            );
            f.flags.set(FragmentFlags::EXECUTABLE_FRAGMENT_IS_GENERATOR, is_generator(ast, n.body));
            f.flags.set(FragmentFlags::FRAGMENT_IS_COMPLETE, is_complete);
            f.metadata = metadata;
        }
        self.bind(fragment, node.raw());
        self.build_executable_element_children(fragment, n.parameters, n.type_parameters);
        if let Some(r) = n.return_type {
            self.visit_type(r);
        }
    }

    /// Dart `visitMixinDeclaration`.
    fn visit_mixin_declaration(&mut self, node: Id<MixinDeclaration>) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        let n = ast.get(node);
        let name = self.name_of(Some(n.name));
        let mut data = MixinFragment {
            interface: InterfaceFragmentData::new(fd(name)),
            super_invoked_names: OnceSlot::new(),
        };
        set(&data.fragment, FragmentFlags::FRAGMENT_IS_AUGMENTATION, n.augment_keyword.is_some());
        set(&data.fragment, FragmentFlags::MIXIN_FRAGMENT_IS_BASE, n.base_keyword.is_some());
        data.fragment.metadata = self.build_metadata(n.metadata);
        let fragment = self.store().add_fragment::<MixinFragment>(data);
        self.bind(fragment.raw(), node.raw());
        self.add_top_fragment(fragment.raw());
        let holder = self.with_enclosing(fragment.raw(), |b| {
            if let Some(tps) = n.type_parameters {
                b.visit_type_parameter_list(tps);
            }
            b.visit_class_body(n.body);
        });
        self.set_type_parameters(fragment.raw(), holder.type_parameters);
        if let Some(on) = n.on_clause {
            self.visit_named_types(ast.get(on).superclass_constraints);
        }
        if let Some(i) = n.implements_clause {
            self.visit_named_types(ast.get(i).interfaces);
        }
    }

    /// Dart `visitPrimaryConstructorDeclaration`.
    fn visit_primary_constructor_declaration(&mut self, node: Id<PrimaryConstructorDeclaration>) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        let n = ast.get(node);
        if let Some(tps) = n.type_parameters {
            self.visit_type_parameter_list(tps);
        }
        let name = n
            .constructor_name
            .and_then(|c| self.name_of(Some(ast.get(c).name)))
            .unwrap_or_else(|| self.core.name("new"));
        let parent = ast.parent(node).expect("parent");
        let extension_type = ast.cast::<ExtensionTypeDeclaration>(parent);
        let is_augmentation = extension_type.is_some_and(|e| ast.get(e).augment_keyword.is_some());
        let mut data = new_constructor_fragment(fd(Some(name)));
        let is_const = n.const_keyword.is_some() || ast.is::<EnumDeclaration>(parent);
        {
            let f = &data.fragment;
            set(f, FragmentFlags::FRAGMENT_IS_AUGMENTATION, is_augmentation);
            set(f, FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_ORIGIN_DECLARATION, true);
            set(f, FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST, is_const);
            set(f, FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_PRIMARY, true);
            set(f, FragmentFlags::FRAGMENT_IS_COMPLETE, true);
        }
        data.type_name = Some(self.core.name(ast.tokens.lexeme(n.type_name)));
        // The primary constructor body is a member of the class body.
        let body = ast.parent(parent).and(None::<Id<PrimaryConstructorBody>>).or_else(|| {
            let body = if let Some(c) = ast.cast::<ClassDeclaration>(parent) {
                class_body_members(ast, ast.get(c).body)
            } else if let Some(e) = ast.cast::<EnumDeclaration>(parent) {
                enum_body_members(ast, ast.get(e).body)
            } else if let Some(e) = extension_type {
                class_body_members(ast, ast.get(e).body)
            } else {
                Vec::new()
            };
            body.into_iter()
                .find_map(|m| ast.cast::<PrimaryConstructorBody>(m.raw()))
        });
        if let Some(body) = body {
            let b = ast.get(body);
            data.fragment.metadata = self.build_metadata(b.metadata);
            if is_const {
                let initializers: Vec<ConstExprId> = ast
                    .list(b.initializers)
                    .iter()
                    .map(|&i| self.core.const_exprs.copy(ast, i))
                    .collect();
                data.constant_initializers.set_once(initializers);
            }
        }
        let fragment = self.store().add_fragment::<ConstructorFragment>(data);
        self.bind(fragment.raw(), node.raw());
        self.add_child_fragment(fragment.raw());

        let formal_parameters = ast.list(ast.get(n.formal_parameters).parameters).to_vec();
        let mut is_first_representation = false;
        if extension_type.is_some() {
            is_first_representation = formal_parameters
                .first()
                .is_some_and(|p| ast.is::<RegularFormalParameter>(p.raw()));
            if !is_first_representation {
                let field = new_field_fragment(None);
                set(&field.fragment, FragmentFlags::FRAGMENT_IS_AUGMENTATION, is_augmentation);
                set(&field.fragment, FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL, true);
                set(
                    &field.fragment,
                    FragmentFlags::FIELD_FRAGMENT_IS_ORIGIN_EXTENSION_TYPE_RECOVERY_REPRESENTATION,
                    true,
                );
                let field = self.store().add_fragment::<FieldFragment>(field);
                self.add_child_fragment(field.raw());
            }
        }
        for (i, p) in formal_parameters.iter().enumerate() {
            let Some(p) = ast.cast::<RegularFormalParameter>(p.raw()) else {
                continue;
            };
            let pn = ast.get(p);
            let is_representation = i == 0 && is_first_representation;
            if pn.const_final_or_var_keyword.is_none() && !is_representation {
                continue;
            }
            let name_text = fragment_name(ast, pn.name).map(str::to_string);
            let field_name = self.core.name_opt(name_text.as_deref());
            let field = new_field_fragment(field_name);
            let is_final = pn
                .const_final_or_var_keyword
                .is_some_and(|k| ast.tokens.lexeme(k) == "final");
            {
                let f = &field.fragment;
                set(f, FragmentFlags::FRAGMENT_IS_AUGMENTATION, is_augmentation);
                set(f, FragmentFlags::FIELD_FRAGMENT_IS_EXPLICITLY_COVARIANT, pn.covariant_keyword.is_some());
                set(f, FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL, is_final || is_representation);
                set(f, FragmentFlags::FIELD_FRAGMENT_IS_ORIGIN_DECLARING_FORMAL_PARAMETER, true);
                set(
                    f,
                    FragmentFlags::VARIABLE_FRAGMENT_HAS_IMPLICIT_TYPE,
                    pn.type_.is_none() && pn.function_typed_suffix.is_none(),
                );
            }
            let field = self.store().add_fragment::<FieldFragment>(field);
            self.core
                .fragment_nodes
                .insert(field.raw(), (self.lib_index, self.unit_index, p.raw()));
            self.add_child_fragment(field.raw());
            let mut formal_name = name_text.clone();
            let mut private_name = None;
            if let Some(n2) = &name_text
                && pn.kind.is_named()
                && self.feature(ExperimentalFlag::PrivateNamedParameters)
                && let Some(public) = corresponding_public_name(n2)
            {
                private_name = Some(n2.clone());
                formal_name = Some(public);
            }
            let formal_name = self.core.name_opt(formal_name.as_deref());
            let private_name = self.core.name_opt(private_name.as_deref());
            let formal: FId<FormalParameterFragment> = self
                .store()
                .add_fragment::<FieldFormalParameterFragment>(new_formal_parameter_fragment(
                    formal_name,
                    pn.kind,
                    private_name,
                ))
                .upcast();
            self.store()
                .fragment(formal)
                .flags
                .set(FragmentFlags::FIELD_FORMAL_PARAMETER_FRAGMENT_IS_DECLARING, true);
            self.declaring_formal_parameters.insert(p.raw(), (field, formal));
            self.core.declaring_formal_parameters.push((field, formal));
        }
        self.build_executable_element_children(fragment.raw(), Some(n.formal_parameters), None);
    }

    /// Dart `visitTopLevelVariableDeclaration`.
    fn visit_top_level_variable_declaration(&mut self, node: Id<TopLevelVariableDeclaration>) {
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        let n = ast.get(node);
        let list = n.variables;
        let is_const = variable_list_is_const(ast, list);
        for &variable in ast.list(ast.get(list).variables) {
            let v = ast.get(variable);
            let mut data = new_top_level_variable_fragment(self.name_of(Some(v.name)));
            {
                let f = &data.fragment;
                set(f, FragmentFlags::NON_PARAMETER_VARIABLE_FRAGMENT_HAS_INITIALIZER, v.initializer.is_some());
                set(f, FragmentFlags::VARIABLE_FRAGMENT_IS_ABSTRACT, n.abstract_keyword.is_some());
                set(f, FragmentFlags::FRAGMENT_IS_AUGMENTATION, n.augment_keyword.is_some());
                set(f, FragmentFlags::VARIABLE_FRAGMENT_IS_CONST, is_const);
                set(f, FragmentFlags::VARIABLE_FRAGMENT_IS_EXTERNAL, n.external_keyword.is_some());
                set(f, FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL, variable_list_is_final(ast, list));
                set(f, FragmentFlags::VARIABLE_FRAGMENT_IS_LATE, variable_list_is_late(ast, list));
                set(f, FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_DECLARATION, true);
                set(f, FragmentFlags::VARIABLE_FRAGMENT_HAS_IMPLICIT_TYPE, ast.get(list).type_.is_none());
            }
            data.fragment.metadata = self.build_metadata(n.metadata);
            if is_const && let Some(initializer) = v.initializer {
                data.constant_initializer = Some(self.copy_expr(initializer));
            }
            let fragment = self.store().add_fragment::<TopLevelVariableFragment>(data);
            self.add_top_fragment(fragment.raw());
            self.bind(fragment.raw(), variable.raw());
        }
        if let Some(t) = ast.get(list).type_ {
            self.visit_type(t);
        }
    }

    /// Dart `_buildExecutableElementChildren`.
    fn build_executable_element_children(
        &mut self,
        fragment: FragmentId,
        formal_parameters: Option<Id<FormalParameterList>>,
        type_parameters: Option<Id<TypeParameterList>>,
    ) {
        let holder = self.with_enclosing(fragment, |b| {
            if let Some(list) = formal_parameters {
                b.visit_formal_parameter_list(list);
            }
            if let Some(list) = type_parameters {
                b.visit_type_parameter_list(list);
            }
        });
        if formal_parameters.is_some() {
            self.set_formal_parameters(fragment, holder.formal_parameters);
        }
        if type_parameters.is_some() {
            self.set_type_parameters(fragment, holder.type_parameters);
        }
    }

    /// Dart `_buildFunctionTypedParameterSuffix`.
    fn build_function_typed_parameter_suffix(
        &mut self,
        fragment: FId<FormalParameterFragment>,
        suffix: Option<Id<FunctionTypedFormalParameterSuffix>>,
    ) {
        let Some(suffix) = suffix else { return };
        let parsed = self.parsed.clone();
        let ast = &parsed.ast;
        let s = ast.get(suffix);
        let holder = self.with_enclosing(fragment.raw(), |b| {
            if let Some(tps) = s.type_parameters {
                b.visit_type_parameter_list(tps);
            }
            b.visit_formal_parameter_list(s.formal_parameters);
        });
        for tp in holder.type_parameters {
            new_type_parameter_element(self.store(), tp);
        }
        for p in holder.formal_parameters {
            init_formal_parameter_element(self.store(), p);
        }
    }
}

/// A synthetic string token with [text] (Dart `StringToken(TokenType.STRING,
/// text, -1)`; the offset is 0 here).
fn synthetic_string(ast: &mut Ast, text: &str) -> TokenId {
    ast.tokens
        .push_synthetic_string(TokenType::STRING, text, 0, 0, Some(0))
}

fn dst_cast(raw: NodeId) -> Id<TypeArgumentList> {
    Id::from_raw(raw)
}

/// Dart `correspondingPublicName` (token_impl.dart).
pub fn corresponding_public_name(identifier: &str) -> Option<String> {
    const RESERVED: &[&str] = &[
        "assert", "break", "case", "catch", "class", "const", "continue", "default", "do", "else",
        "enum", "extends", "false", "final", "finally", "for", "if", "in", "is", "new", "null",
        "rethrow", "return", "super", "switch", "this", "throw", "true", "try", "var", "void",
        "while", "with",
    ];
    let bytes = identifier.as_bytes();
    if bytes.first() != Some(&b'_') || bytes.len() == 1 {
        return None;
    }
    if bytes[1] == b'_' || bytes[1].is_ascii_digit() {
        return None;
    }
    let public = &identifier[1..];
    if RESERVED.contains(&public) {
        return None;
    }
    Some(public.to_string())
}

fn set_type_params_of(store: &mut ElementStore, fragment: FragmentId, list: Vec<FId<TypeParameterFragment>>) {
    let i = fragment.index();
    let f = &mut store.fragments;
    match fragment.tag() {
        Tag::Class => f.classes.get_mut(i).type_params = list,
        Tag::Enum => f.enums.get_mut(i).type_params = list,
        Tag::Mixin => f.mixins.get_mut(i).type_params = list,
        Tag::Extension => f.extensions.get_mut(i).type_params = list,
        Tag::ExtensionType => f.extension_types.get_mut(i).type_params = list,
        Tag::Method => f.methods.get_mut(i).type_params = list,
        Tag::Constructor => f.constructors.get_mut(i).type_params = list,
        Tag::Getter => f.getters.get_mut(i).type_params = list,
        Tag::Setter => f.setters.get_mut(i).type_params = list,
        Tag::TopLevelFunction => f.functions.get_mut(i).type_params = list,
        Tag::TypeAlias => f.type_aliases.get_mut(i).type_params = list,
        Tag::GenericFunctionType => f.generic_function_types.get_mut(i).type_params = list,
        t => panic!("no type parameters on {t:?}"),
    }
}

fn set_formal_params_of(store: &mut ElementStore, fragment: FragmentId, list: Vec<FId<FormalParameterFragment>>) {
    let i = fragment.index();
    let f = &mut store.fragments;
    match fragment.tag() {
        Tag::Method => f.methods.get_mut(i).formal_params = list,
        Tag::Constructor => f.constructors.get_mut(i).formal_params = list,
        Tag::Getter => f.getters.get_mut(i).formal_params = list,
        Tag::Setter => f.setters.get_mut(i).formal_params = list,
        Tag::TopLevelFunction => f.functions.get_mut(i).formal_params = list,
        Tag::GenericFunctionType => f.generic_function_types.get_mut(i).formal_params = list,
        t => panic!("no formal parameters on {t:?}"),
    }
}

// ---- ElementBuilder ----

/// Dart `ElementBuilder`.
pub struct ElementBuilder<'l, 'a> {
    core: &'l mut LinkerCore<'a>,
    lib: &'l mut LibraryBuilder,
    /// Dart `_executableElements`.
    executable_elements: Vec<EId<ExecutableElement>>,
}

impl<'l, 'a> ElementBuilder<'l, 'a> {
    pub fn new(linker: &'l mut Linker<'a>, lib_index: usize) -> Self {
        let Linker { core, builders, .. } = linker;
        ElementBuilder {
            core,
            lib: &mut builders[lib_index],
            executable_elements: Vec::new(),
        }
    }

    fn store(&mut self) -> &mut ElementStore {
        &mut self.core.store
    }

    fn library(&self) -> EId<LibraryElement> {
        self.lib.element
    }

    fn name_text(&self, name: Option<Name>) -> Option<&str> {
        name.map(|n| self.core.name_str(n))
    }

    /// Dart `buildElements`.
    pub fn build_elements(mut self) {
        self.build_top_fragments();
        self.add_extension_type_recovery_fragments();
        self.build_instance_element_members();
        self.build_formal_parameter_elements();
    }

    fn declare_top(&mut self, kind: TopLevelReferenceKind, name: Option<Name>, element: ElementId, lookup: Option<Arc<str>>) -> RefId {
        let text = name.map(|n| self.core.name_str(n).to_string());
        let r = self.lib.references.declare_top_level(kind, text.as_deref());
        self.lib.references.reference.set_element(r, element);
        self.lib.element_references.insert(element, r);
        if let Some(lookup) = lookup {
            self.lib.declare(Some(lookup), r, element);
        }
        r
    }

    fn declare_member(&mut self, container: ElementId, kind: MemberReferenceKind, name: Option<Name>, element: ElementId) {
        let text = name.map(|n| self.core.name_str(n).to_string());
        let container = self.lib.element_references[&container];
        let r = self.lib.references.declare_member(container, kind, text.as_deref());
        self.lib.references.reference.set_element(r, element);
        self.lib.element_references.insert(element, r);
    }

    fn lookup_name(&self, name: Option<Name>, setter: bool) -> Option<Arc<str>> {
        let text = self.name_text(name)?;
        Some(if setter {
            format!("{text}=").into()
        } else {
            text.into()
        })
    }

    /// Creates the type parameter elements of [fragment] (the element
    /// constructors of `element.dart`) and returns them.
    fn type_parameter_elements(&mut self, fragment: FragmentId) -> Vec<EId<TypeParameterElement>> {
        let tps: Vec<FId<TypeParameterFragment>> = match fragment.tag() {
            Tag::Class | Tag::Enum | Tag::Mixin | Tag::Extension | Tag::ExtensionType => {
                self.core.store.instance_fragment(FId::from_raw(fragment)).type_params.clone()
            }
            Tag::TypeAlias => self.core.store.fragment(FId::<TypeAliasFragment>::from_raw(fragment)).type_params.clone(),
            _ => self.core.store.executable_fragment(FId::from_raw(fragment)).type_params.clone(),
        };
        tps.into_iter()
            .map(|tp| new_type_parameter_element(&mut self.core.store, tp))
            .collect()
    }

    /// Dart `_buildTopFragments`.
    fn build_top_fragments(&mut self) {
        let top: Vec<(FId<LibraryFragment>, Vec<FragmentId>)> = self
            .lib
            .top_fragments
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();
        let mut last_fragments: IndexMap<Option<Name>, FragmentId> = IndexMap::new();
        for (unit, fragments) in top {
            for fragment in fragments {
                let name = self.core.store.fragment_data(fragment).unwrap().name;
                let last = last_fragments.get(&name).copied();
                match fragment.tag() {
                    Tag::Class => self.handle_class_fragment(unit, last, FId::from_raw(fragment)),
                    Tag::Enum => self.handle_enum_fragment(unit, last, FId::from_raw(fragment)),
                    Tag::Extension => self.handle_extension_fragment(unit, last, FId::from_raw(fragment)),
                    Tag::ExtensionType => self.handle_extension_type_fragment(unit, last, FId::from_raw(fragment)),
                    Tag::Getter => self.handle_top_level_getter_fragment(unit, last, FId::from_raw(fragment)),
                    Tag::Mixin => self.handle_mixin_fragment(unit, last, FId::from_raw(fragment)),
                    Tag::Setter => self.handle_top_level_setter_fragment(unit, last, FId::from_raw(fragment)),
                    Tag::TopLevelFunction => self.handle_top_level_function_fragment(unit, last, FId::from_raw(fragment)),
                    Tag::TopLevelVariable => self.handle_top_level_variable_fragment(unit, last, FId::from_raw(fragment)),
                    Tag::TypeAlias => self.handle_type_alias_fragment(unit, FId::from_raw(fragment)),
                    t => panic!("unexpected top fragment {t:?}"),
                }
                last_fragments.insert(name, fragment);
            }
        }
    }

    fn is_augmentation(&self, f: FragmentId) -> bool {
        has(&self.core.store, f, FragmentFlags::FRAGMENT_IS_AUGMENTATION)
    }

    /// Dart `addFragment` of instance fragments and `_linkTypeParameters`
    /// (common prefix only).
    fn add_fragment(&mut self, last: FragmentId, fragment: FragmentId) {
        let element = *self.core.store.fragment_data(last).unwrap().element.get();
        let store = &mut self.core.store;
        // Dart: the augmentation is added after the last fragment of the
        // chain.
        let mut tail = last;
        while let Some(next) = store.fragment_data(tail).unwrap().next_fragment {
            tail = next;
        }
        fragment_data_mut(store, tail).next_fragment = Some(fragment);
        fragment_data_mut(store, fragment).previous_fragment = Some(tail);
        store.fragment_data(fragment).unwrap().element.set_once(element);
    }

    fn link_type_parameters(&mut self, previous: FragmentId, current: FragmentId) {
        let store = &mut self.core.store;
        let get = |store: &ElementStore, f: FragmentId| -> Vec<FId<TypeParameterFragment>> {
            match f.tag() {
                Tag::Class | Tag::Enum | Tag::Mixin | Tag::Extension | Tag::ExtensionType => {
                    store.instance_fragment(FId::from_raw(f)).type_params.clone()
                }
                _ => store.executable_fragment(FId::from_raw(f)).type_params.clone(),
            }
        };
        let p = get(store, previous);
        let c = get(store, current);
        for (a, b) in p.iter().zip(c.iter()) {
            store.fragment_mut(*a).next_fragment = Some(b.raw());
            store.fragment_mut(*b).previous_fragment = Some(a.raw());
            if let Some(&e) = store.fragment(*a).element.try_get() {
                store.fragment(*b).element.set_once(e);
            }
        }
    }

    fn link_formal_parameters(&mut self, previous: FragmentId, current: FragmentId) {
        let store = &mut self.core.store;
        let p = store.executable_fragment(FId::from_raw(previous)).formal_params.clone();
        let c = store.executable_fragment(FId::from_raw(current)).formal_params.clone();
        for (a, b) in p.iter().zip(c.iter()) {
            store.fragment_mut(*a).next_fragment = Some(b.raw());
            store.fragment_mut(*b).previous_fragment = Some(a.raw());
        }
    }

    fn handle_class_fragment(&mut self, unit: FId<LibraryFragment>, last: Option<FragmentId>, fragment: FId<ClassFragment>) {
        self.store().fragment_mut(unit).classes.push(fragment);
        if self.is_augmentation(fragment.raw())
            && let Some(last) = last
            && last.tag() == Tag::Class
            && !has(&self.core.store, last, FragmentFlags::CLASS_FRAGMENT_IS_MIXIN_APPLICATION)
        {
            self.add_fragment(last, fragment.raw());
            self.link_type_parameters(last, fragment.raw());
            return;
        }
        let f = self.core.store.fragment(fragment);
        let name = f.name;
        let is_abstract = f.flags.has(FragmentFlags::CLASS_FRAGMENT_IS_ABSTRACT)
            || f.flags.has(FragmentFlags::CLASS_FRAGMENT_IS_SEALED);
        let is_base = f.flags.has(FragmentFlags::CLASS_FRAGMENT_IS_BASE);
        let is_final = f.flags.has(FragmentFlags::CLASS_FRAGMENT_IS_FINAL);
        let is_interface = f.flags.has(FragmentFlags::CLASS_FRAGMENT_IS_INTERFACE);
        let element = self.store().add::<ClassElement>(ClassElement {
            interface: InterfaceElementData::new(ed(name, fragment.raw())),
        });
        self.core.store.fragment(fragment).element.set_once(element.raw());
        {
            let flags = &self.core.store.get(element).flags;
            flags.set(ElementFlags::CLASS_ELEMENT_IS_ABSTRACT, is_abstract);
            flags.set(ElementFlags::CLASS_ELEMENT_IS_BASE, is_base);
            flags.set(ElementFlags::CLASS_ELEMENT_IS_FINAL, is_final);
            flags.set(ElementFlags::CLASS_ELEMENT_IS_INTERFACE, is_interface);
        }
        let tps = self.type_parameter_elements(fragment.raw());
        self.store().get_mut(element).type_params = tps;
        if self.is_augmentation(fragment.raw()) && last.is_some() {
            self.store().get_mut(element).previous_fragment_of_different_kind = last;
        }
        let library = self.library();
        self.store().get_mut(library).classes.push(element);
        let lookup = self.lookup_name(name, false);
        self.declare_top(TopLevelReferenceKind::Class, name, element.raw(), lookup);
    }

    fn handle_enum_fragment(&mut self, unit: FId<LibraryFragment>, last: Option<FragmentId>, fragment: FId<EnumFragment>) {
        self.store().fragment_mut(unit).enums.push(fragment);
        if self.is_augmentation(fragment.raw())
            && let Some(last) = last
            && last.tag() == Tag::Enum
        {
            self.add_fragment(last, fragment.raw());
            self.link_type_parameters(last, fragment.raw());
            return;
        }
        let name = self.core.store.fragment(fragment).name;
        let element = self.store().add::<EnumElement>(EnumElement {
            interface: InterfaceElementData::new(ed(name, fragment.raw())),
        });
        self.core.store.fragment(fragment).element.set_once(element.raw());
        let tps = self.type_parameter_elements(fragment.raw());
        self.store().get_mut(element).type_params = tps;
        if self.is_augmentation(fragment.raw()) && last.is_some() {
            self.store().get_mut(element).previous_fragment_of_different_kind = last;
        }
        let library = self.library();
        self.store().get_mut(library).enums.push(element);
        let lookup = self.lookup_name(name, false);
        self.declare_top(TopLevelReferenceKind::Enum, name, element.raw(), lookup);
    }

    fn handle_extension_fragment(&mut self, unit: FId<LibraryFragment>, last: Option<FragmentId>, fragment: FId<ExtensionFragment>) {
        self.store().fragment_mut(unit).extensions.push(fragment);
        if self.is_augmentation(fragment.raw())
            && let Some(last) = last
            && last.tag() == Tag::Extension
        {
            self.add_fragment(last, fragment.raw());
            self.link_type_parameters(last, fragment.raw());
            return;
        }
        let name = self.core.store.fragment(fragment).name;
        let element = self.store().add::<ExtensionElement>(ExtensionElement {
            instance: InstanceElementData::new(ed(name, fragment.raw())),
            extended_type: VarSlot::with(TypeId::INVALID),
        });
        self.core.store.fragment(fragment).element.set_once(element.raw());
        let tps = self.type_parameter_elements(fragment.raw());
        self.store().get_mut(element).type_params = tps;
        if self.is_augmentation(fragment.raw()) && last.is_some() {
            self.store().get_mut(element).previous_fragment_of_different_kind = last;
        }
        let library = self.library();
        self.store().get_mut(library).extensions.push(element);
        let lookup = self.lookup_name(name, false);
        self.declare_top(TopLevelReferenceKind::Extension, name, element.raw(), lookup);
    }

    fn handle_extension_type_fragment(&mut self, unit: FId<LibraryFragment>, last: Option<FragmentId>, fragment: FId<ExtensionTypeFragment>) {
        self.store().fragment_mut(unit).extension_types.push(fragment);
        if self.is_augmentation(fragment.raw())
            && let Some(last) = last
            && last.tag() == Tag::ExtensionType
        {
            self.add_fragment(last, fragment.raw());
            self.link_type_parameters(last, fragment.raw());
            return;
        }
        let name = self.core.store.fragment(fragment).name;
        let element = self.store().add::<ExtensionTypeElement>(ExtensionTypeElement {
            interface: InterfaceElementData::new(ed(name, fragment.raw())),
            has_representation_self_reference: BoolSlot::new(false),
            has_implements_self_reference: BoolSlot::new(false),
            type_erasure: OnceSlot::new(),
        });
        self.core.store.fragment(fragment).element.set_once(element.raw());
        let tps = self.type_parameter_elements(fragment.raw());
        self.store().get_mut(element).type_params = tps;
        if self.is_augmentation(fragment.raw()) && last.is_some() {
            self.store().get_mut(element).previous_fragment_of_different_kind = last;
        }
        let library = self.library();
        self.store().get_mut(library).extension_types.push(element);
        let lookup = self.lookup_name(name, false);
        self.declare_top(TopLevelReferenceKind::ExtensionType, name, element.raw(), lookup);
    }

    fn handle_mixin_fragment(&mut self, unit: FId<LibraryFragment>, last: Option<FragmentId>, fragment: FId<MixinFragment>) {
        self.store().fragment_mut(unit).mixins.push(fragment);
        if self.is_augmentation(fragment.raw())
            && let Some(last) = last
            && last.tag() == Tag::Mixin
        {
            self.add_fragment(last, fragment.raw());
            self.link_type_parameters(last, fragment.raw());
            return;
        }
        let name = self.core.store.fragment(fragment).name;
        let element = self.store().add::<MixinElement>(MixinElement {
            interface: InterfaceElementData::new(ed(name, fragment.raw())),
            superclass_constraints: VarSlot::with(TypeList::EMPTY),
        });
        self.core.store.fragment(fragment).element.set_once(element.raw());
        let tps = self.type_parameter_elements(fragment.raw());
        self.store().get_mut(element).type_params = tps;
        if self.is_augmentation(fragment.raw()) && last.is_some() {
            self.store().get_mut(element).previous_fragment_of_different_kind = last;
        }
        let library = self.library();
        self.store().get_mut(library).mixins.push(element);
        let lookup = self.lookup_name(name, false);
        self.declare_top(TopLevelReferenceKind::Mixin, name, element.raw(), lookup);
    }

    fn handle_top_level_function_fragment(&mut self, unit: FId<LibraryFragment>, last: Option<FragmentId>, fragment: FId<TopLevelFunctionFragment>) {
        self.store().fragment_mut(unit).functions.push(fragment);
        if let Some(last) = last
            && last.tag() == Tag::TopLevelFunction
            && self.is_augmentation(fragment.raw())
        {
            self.add_fragment(last, fragment.raw());
            self.link_type_parameters(last, fragment.raw());
            self.link_formal_parameters(last, fragment.raw());
            return;
        }
        let name = self.core.store.fragment(fragment).name;
        let element = self.store().add::<TopLevelFunctionElement>(TopLevelFunctionElement {
            executable: ExecutableElementData::new(ed(name, fragment.raw())),
        });
        self.core.store.fragment(fragment).element.set_once(element.raw());
        self.init_executable(element.upcast(), fragment.raw());
        if self.is_augmentation(fragment.raw()) && last.is_some() {
            self.store().get_mut(element).previous_fragment_of_different_kind = last;
        }
        let library = self.library();
        self.store().get_mut(library).top_level_functions.push(element);
        let lookup = self.lookup_name(name, false);
        self.declare_top(TopLevelReferenceKind::Function, name, element.raw(), lookup);
    }

    /// The common part of the executable element constructors: type
    /// parameters and `hasEnclosingTypeParameterReference`, and Dart
    /// `_executableElements.add`.
    fn init_executable(&mut self, element: EId<ExecutableElement>, fragment: FragmentId) {
        let tps = self.type_parameter_elements(fragment);
        let store = &mut self.core.store;
        store.element_data(element.raw()).unwrap().flags.set(
            ElementFlags::EXECUTABLE_ELEMENT_HAS_ENCLOSING_TYPE_PARAMETER_REFERENCE,
            true,
        );
        executable_data_mut(store, element.raw()).type_params = tps;
        self.executable_elements.push(element);
    }

    fn new_getter_element(&mut self, fragment: FId<GetterFragment>) -> EId<GetterElement> {
        let name = self.core.store.fragment(fragment).name;
        let element = self.store().add::<GetterElement>(GetterElement {
            accessor: PropertyAccessorElementData {
                executable: ExecutableElementData::new(ed(name, fragment.raw())),
                variable: VarSlot::new(),
            },
        });
        self.core.store.fragment(fragment).element.set_once(element.raw());
        self.init_executable(element.upcast(), fragment.raw());
        element
    }

    fn new_setter_element(&mut self, fragment: FId<SetterFragment>) -> EId<SetterElement> {
        let name = self.core.store.fragment(fragment).name;
        let element = self.store().add::<SetterElement>(SetterElement {
            accessor: PropertyAccessorElementData {
                executable: ExecutableElementData::new(ed(name, fragment.raw())),
                variable: VarSlot::new(),
            },
        });
        self.core.store.fragment(fragment).element.set_once(element.raw());
        self.init_executable(element.upcast(), fragment.raw());
        element
    }

    fn new_top_level_variable_element(&mut self, fragment: FId<TopLevelVariableFragment>) -> EId<TopLevelVariableElement> {
        let name = self.core.store.fragment(fragment).name;
        let element = self.store().add::<TopLevelVariableElement>(TopLevelVariableElement {
            property: PropertyInducingElementData::new(ed(name, fragment.raw())),
        });
        self.core.store.fragment(fragment).element.set_once(element.raw());
        element
    }

    fn new_field_element(&mut self, fragment: FId<FieldFragment>) -> EId<FieldElement> {
        let name = self.core.store.fragment(fragment).name;
        let element = self.store().add::<FieldElement>(FieldElement {
            property: PropertyInducingElementData::new(ed(name, fragment.raw())),
        });
        self.core.store.fragment(fragment).element.set_once(element.raw());
        self.core.store.get(element).flags.set(
            ElementFlags::FIELD_ELEMENT_HAS_ENCLOSING_TYPE_PARAMETER_REFERENCE,
            true,
        );
        element
    }

    /// Dart `_topLevelVariableElement`.
    fn top_level_variable_element(&self, fragment: Option<FragmentId>) -> Option<EId<TopLevelVariableElement>> {
        let f = fragment?;
        let store = &self.core.store;
        match f.tag() {
            Tag::TopLevelVariable => store.fragment_data(f).unwrap().element.try_get().and_then(|e| e.cast()),
            Tag::Getter | Tag::Setter => {
                let e = *store.fragment_data(f).unwrap().element.try_get()?;
                store
                    .property_accessor(EId::from_raw(e))
                    .variable
                    .get()
                    .and_then(|v| v.raw().cast())
            }
            _ => None,
        }
    }

    /// Dart `_fieldElement`.
    fn field_element(&self, fragment: Option<FragmentId>) -> Option<EId<FieldElement>> {
        let f = fragment?;
        let store = &self.core.store;
        match f.tag() {
            Tag::Field => store.fragment_data(f).unwrap().element.try_get().and_then(|e| e.cast()),
            Tag::Getter | Tag::Setter => {
                let e = *store.fragment_data(f).unwrap().element.try_get()?;
                store
                    .property_accessor(EId::from_raw(e))
                    .variable
                    .get()
                    .and_then(|v| v.raw().cast())
            }
            _ => None,
        }
    }

    /// The last fragment of the chain of [first].
    fn last_fragment(&self, first: FragmentId) -> FragmentId {
        let mut f = first;
        while let Some(next) = self.core.store.fragment_data(f).unwrap().next_fragment {
            f = next;
        }
        f
    }

    fn handle_top_level_getter_fragment(&mut self, unit: FId<LibraryFragment>, last: Option<FragmentId>, fragment: FId<GetterFragment>) {
        self.store().fragment_mut(unit).getters.push(fragment);
        let mut last_variable = self.top_level_variable_element(last);
        let last_getter = last_variable.and_then(|v| self.core.store.get(v).getter);
        if self.is_augmentation(fragment.raw())
            && let Some(g) = last_getter
        {
            let first = self.core.store.get(g).first_fragment().raw();
            let tail = self.last_fragment(first);
            self.add_fragment(tail, fragment.raw());
            return;
        }
        let name = self.core.store.fragment(fragment).name;
        let getter = self.new_getter_element(fragment);
        if self.is_augmentation(fragment.raw()) && last.is_some() {
            self.store().get_mut(getter).previous_fragment_of_different_kind = last;
        }
        let library = self.library();
        self.store().get_mut(library).getters.push(getter);
        let lookup = self.lookup_name(name, false);
        self.declare_top(TopLevelReferenceKind::Getter, name, getter.raw(), lookup);
        if let Some(v) = last_variable
            && self.core.store.get(v).getter.is_some()
        {
            last_variable = None;
        }
        let variable = match last_variable {
            Some(v) => v,
            None => {
                let data = new_top_level_variable_fragment(name);
                set(&data.fragment, FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_GETTER_SETTER, true);
                let vf = self.store().add_fragment::<TopLevelVariableFragment>(data);
                self.store().fragment_mut(vf).enclosing_fragment = Some(unit.raw());
                self.store().fragment_mut(unit).variables.push(vf);
                let v = self.new_top_level_variable_element(vf);
                self.store().get_mut(library).top_level_variables.push(v);
                self.declare_top(TopLevelReferenceKind::TopLevelVariable, name, v.raw(), None);
                v
            }
        };
        self.core.store.get(getter).variable.set(Some(variable.upcast()));
        self.store().get_mut(variable).getter = Some(getter);
    }

    fn handle_top_level_setter_fragment(&mut self, unit: FId<LibraryFragment>, last: Option<FragmentId>, fragment: FId<SetterFragment>) {
        self.store().fragment_mut(unit).setters.push(fragment);
        let mut last_variable = self.top_level_variable_element(last);
        let last_setter = last_variable.and_then(|v| self.core.store.get(v).setter);
        if self.is_augmentation(fragment.raw())
            && let Some(s) = last_setter
        {
            let first = self.core.store.get(s).first_fragment().raw();
            let tail = self.last_fragment(first);
            self.add_fragment(tail, fragment.raw());
            self.link_formal_parameters(tail, fragment.raw());
            return;
        }
        let name = self.core.store.fragment(fragment).name;
        let setter = self.new_setter_element(fragment);
        if self.is_augmentation(fragment.raw()) && last.is_some() {
            self.store().get_mut(setter).previous_fragment_of_different_kind = last;
        }
        let library = self.library();
        self.store().get_mut(library).setters.push(setter);
        let lookup = self.lookup_name(name, true);
        self.declare_top(TopLevelReferenceKind::Setter, name, setter.raw(), lookup);
        if let Some(v) = last_variable
            && self.core.store.get(v).setter.is_some()
        {
            last_variable = None;
        }
        let variable = match last_variable {
            Some(v) => v,
            None => {
                let data = new_top_level_variable_fragment(name);
                set(&data.fragment, FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_GETTER_SETTER, true);
                let vf = self.store().add_fragment::<TopLevelVariableFragment>(data);
                self.store().fragment_mut(vf).enclosing_fragment = Some(unit.raw());
                self.store().fragment_mut(unit).variables.push(vf);
                let v = self.new_top_level_variable_element(vf);
                self.store().get_mut(library).top_level_variables.push(v);
                self.declare_top(TopLevelReferenceKind::TopLevelVariable, name, v.raw(), None);
                v
            }
        };
        self.core.store.get(setter).variable.set(Some(variable.upcast()));
        self.store().get_mut(variable).setter = Some(setter);
    }

    fn handle_top_level_variable_fragment(&mut self, unit: FId<LibraryFragment>, last: Option<FragmentId>, fragment: FId<TopLevelVariableFragment>) {
        self.store().fragment_mut(unit).variables.push(fragment);
        let last_variable = self.top_level_variable_element(last);
        let name = self.core.store.fragment(fragment).name;
        let library = self.library();
        let variable = match last_variable {
            Some(v) if self.is_augmentation(fragment.raw()) => {
                let first = self.core.store.get(v).first_fragment().raw();
                let tail = self.last_fragment(first);
                self.add_fragment(tail, fragment.raw());
                v
            }
            _ => {
                let v = self.new_top_level_variable_element(fragment);
                if self.is_augmentation(fragment.raw()) && last.is_some() {
                    self.store().get_mut(v).previous_fragment_of_different_kind = last;
                }
                self.store().get_mut(library).top_level_variables.push(v);
                self.declare_top(TopLevelReferenceKind::TopLevelVariable, name, v.raw(), None);
                v
            }
        };
        let f = self.core.store.fragment(fragment);
        let is_abstract = f.flags.has(FragmentFlags::VARIABLE_FRAGMENT_IS_ABSTRACT);
        let is_external = f.flags.has(FragmentFlags::VARIABLE_FRAGMENT_IS_EXTERNAL);
        let is_augmentation = f.flags.has(FragmentFlags::FRAGMENT_IS_AUGMENTATION);
        let has_setter = variable_fragment_has_setter(&self.core.store, fragment.raw());
        // The synthetic getter.
        {
            let data = new_getter_fragment(name);
            let g = &data.fragment;
            set(g, FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE, true);
            set(g, FragmentFlags::EXECUTABLE_FRAGMENT_IS_ABSTRACT, is_abstract);
            set(g, FragmentFlags::FRAGMENT_IS_AUGMENTATION, is_augmentation);
            set(g, FragmentFlags::FRAGMENT_IS_COMPLETE, is_external || !is_abstract);
            set(g, FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC, true);
            let getter_fragment = self.store().add_fragment::<GetterFragment>(data);
            self.induce_getter(fragment.raw(), getter_fragment);
            self.store().fragment_mut(getter_fragment).enclosing_fragment = Some(unit.raw());
            self.store().fragment_mut(unit).getters.push(getter_fragment);
            match self.core.store.get(variable).getter {
                Some(g) => {
                    let first = self.core.store.get(g).first_fragment().raw();
                    let tail = self.last_fragment(first);
                    self.add_fragment(tail, getter_fragment.raw());
                }
                None => {
                    let getter = self.new_getter_element(getter_fragment);
                    self.store().get_mut(library).getters.push(getter);
                    let lookup = self.lookup_name(name, false);
                    self.declare_top(TopLevelReferenceKind::Getter, name, getter.raw(), lookup);
                    self.store().get_mut(variable).getter = Some(getter);
                    self.core.store.get(getter).variable.set(Some(variable.upcast()));
                }
            }
        }
        if has_setter {
            let data = new_setter_fragment(name);
            let s = &data.fragment;
            set(s, FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE, true);
            set(s, FragmentFlags::EXECUTABLE_FRAGMENT_IS_ABSTRACT, is_abstract);
            set(s, FragmentFlags::FRAGMENT_IS_AUGMENTATION, is_augmentation);
            set(s, FragmentFlags::FRAGMENT_IS_COMPLETE, is_external || !is_abstract);
            set(s, FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC, true);
            let setter_fragment = self.store().add_fragment::<SetterFragment>(data);
            self.induce_setter(fragment.raw(), setter_fragment);
            self.store().fragment_mut(setter_fragment).enclosing_fragment = Some(unit.raw());
            self.store().fragment_mut(unit).setters.push(setter_fragment);
            let value = self.value_parameter(false);
            self.set_formal_params(setter_fragment.raw(), vec![value]);
            match self.core.store.get(variable).setter {
                Some(s) => {
                    let first = self.core.store.get(s).first_fragment().raw();
                    let tail = self.last_fragment(first);
                    self.add_fragment(tail, setter_fragment.raw());
                    self.link_formal_parameters(tail, setter_fragment.raw());
                }
                None => {
                    let setter = self.new_setter_element(setter_fragment);
                    self.store().get_mut(library).setters.push(setter);
                    let lookup = self.lookup_name(name, true);
                    self.declare_top(TopLevelReferenceKind::Setter, name, setter.raw(), lookup);
                    self.store().get_mut(variable).setter = Some(setter);
                    self.core.store.get(setter).variable.set(Some(variable.upcast()));
                }
            }
        }
    }

    /// The synthetic `value` parameter of a setter of a variable.
    fn value_parameter(&mut self, explicitly_covariant: bool) -> FId<FormalParameterFragment> {
        let name = self.core.name("value");
        let data = new_formal_parameter_fragment(Some(name), ParameterKind::Required, None);
        data.flags.set(
            FragmentFlags::FORMAL_PARAMETER_FRAGMENT_IS_EXPLICITLY_COVARIANT,
            explicitly_covariant,
        );
        self.store().add_fragment::<FormalParameterFragment>(data)
    }

    fn set_formal_params(&mut self, fragment: FragmentId, list: Vec<FId<FormalParameterFragment>>) {
        for &p in &list {
            self.store().fragment_mut(p).enclosing_fragment = Some(fragment);
        }
        set_formal_params_of(self.store(), fragment, list);
    }

    /// Dart `inducedGetter =`.
    fn induce_getter(&mut self, variable: FragmentId, getter: FId<GetterFragment>) {
        let store = &mut self.core.store;
        match variable.tag() {
            Tag::Field => store.fragment_mut(FId::<FieldFragment>::from_raw(variable)).induced_getter = Some(getter),
            _ => store.fragment_mut(FId::<TopLevelVariableFragment>::from_raw(variable)).induced_getter = Some(getter),
        }
        store.fragment_mut(getter).inducing_variable = Some(FId::from_raw(variable));
    }

    /// Dart `inducedSetter =`.
    fn induce_setter(&mut self, variable: FragmentId, setter: FId<SetterFragment>) {
        let store = &mut self.core.store;
        match variable.tag() {
            Tag::Field => store.fragment_mut(FId::<FieldFragment>::from_raw(variable)).induced_setter = Some(setter),
            _ => store.fragment_mut(FId::<TopLevelVariableFragment>::from_raw(variable)).induced_setter = Some(setter),
        }
        store.fragment_mut(setter).inducing_variable = Some(FId::from_raw(variable));
    }

    fn handle_type_alias_fragment(&mut self, unit: FId<LibraryFragment>, fragment: FId<TypeAliasFragment>) {
        self.store().fragment_mut(unit).type_aliases.push(fragment);
        let name = self.core.store.fragment(fragment).name;
        let element = self.store().add::<TypeAliasElement>(TypeAliasElement {
            element: ed(name, fragment.raw()),
            type_params: Vec::new(),
            aliased_type: VarSlot::new(),
        });
        self.core.store.fragment(fragment).element.set_once(element.raw());
        let tps = self.type_parameter_elements(fragment.raw());
        self.store().get_mut(element).type_params = tps;
        let library = self.library();
        self.store().get_mut(library).type_aliases.push(element);
        let lookup = self.lookup_name(name, false);
        self.declare_top(TopLevelReferenceKind::TypeAlias, name, element.raw(), lookup);
    }

    /// Dart `_addExtensionTypeRecoveryFragments`.
    fn add_extension_type_recovery_fragments(&mut self) {
        let library = self.library();
        let extension_types = self.core.store.get(library).extension_types.clone();
        for et in extension_types {
            let first = self.core.store.get(et).first_fragment().raw();
            let children = self.lib.parent_child_fragments.entry(first).or_default().clone();
            let has_primary = children.iter().any(|&c| {
                c.tag() == Tag::Constructor && has(&self.core.store, c, FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_PRIMARY)
            });
            if has_primary {
                continue;
            }
            let name = self.core.store.get(et).name;
            let mut ctor = new_constructor_fragment(fd(Some(self.core.name("new"))));
            ctor.type_name = name;
            ctor.fragment.enclosing_fragment = Some(first);
            set(&ctor.fragment, FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_PRIMARY, true);
            set(&ctor.fragment, FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST, true);
            set(&ctor.fragment, FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_ORIGIN_EXTENSION_TYPE_RECOVERY, true);
            let ctor = self.store().add_fragment::<ConstructorFragment>(ctor);
            let mut field = new_field_fragment(None);
            field.fragment.enclosing_fragment = Some(first);
            set(&field.fragment, FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL, true);
            set(&field.fragment, FragmentFlags::FIELD_FRAGMENT_IS_ORIGIN_EXTENSION_TYPE_RECOVERY_REPRESENTATION, true);
            set(&field.fragment, FragmentFlags::VARIABLE_FRAGMENT_HAS_IMPLICIT_TYPE, true);
            let field = self.store().add_fragment::<FieldFragment>(field);
            let list = self.lib.parent_child_fragments.entry(first).or_default();
            list.insert(0, ctor.raw());
            list.insert(0, field.raw());
        }
    }

    /// Dart `_buildInstanceElementMembers`.
    fn build_instance_element_members(&mut self) {
        let library = self.library();
        let l = self.core.store.get(library);
        let instances: Vec<ElementId> = l
            .classes
            .iter()
            .map(|e| e.raw())
            .chain(l.enums.iter().map(|e| e.raw()))
            .chain(l.extensions.iter().map(|e| e.raw()))
            .chain(l.extension_types.iter().map(|e| e.raw()))
            .chain(l.mixins.iter().map(|e| e.raw()))
            .collect();
        let dart_core_enum = &*self.lib.uri == "dart:core";
        for instance in instances {
            let first = self.core.store.element_data(instance).unwrap().first_fragment;
            let mut fragments = vec![first];
            while let Some(next) = self.core.store.fragment_data(*fragments.last().unwrap()).unwrap().next_fragment {
                fragments.push(next);
            }
            let children: Vec<FragmentId> = fragments
                .iter()
                .flat_map(|f| self.lib.parent_child_fragments.get(f).cloned().unwrap_or_default())
                .collect();
            let mut last_instance: IndexMap<Option<Name>, FragmentId> = IndexMap::new();
            let mut last_static: IndexMap<Option<Name>, FragmentId> = IndexMap::new();
            for fragment in children {
                let data = self.core.store.fragment_data(fragment).unwrap();
                let name = data.name;
                let is_static = match fragment.tag() {
                    Tag::Constructor => true,
                    Tag::Field => data.flags.has(FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC),
                    Tag::Getter | Tag::Setter | Tag::Method => {
                        data.flags.has(FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC)
                    }
                    _ => false,
                };
                let last = if is_static {
                    last_static.get(&name).copied()
                } else {
                    last_instance.get(&name).copied()
                };
                let mut to_track = fragment;
                match fragment.tag() {
                    Tag::Field => {
                        to_track = self.handle_instance_field_fragment(instance, last, FId::from_raw(fragment)).raw();
                    }
                    Tag::Getter => self.handle_instance_getter_fragment(instance, last, FId::from_raw(fragment), dart_core_enum),
                    Tag::Setter => self.handle_instance_setter_fragment(instance, last, FId::from_raw(fragment)),
                    Tag::Method => self.handle_instance_method_fragment(instance, last, FId::from_raw(fragment)),
                    Tag::Constructor => self.handle_instance_constructor_fragment(instance, last, FId::from_raw(fragment)),
                    t => panic!("unexpected member fragment {t:?}"),
                }
                let track_name = self.core.store.fragment_data(to_track).unwrap().name;
                if is_static {
                    last_static.insert(track_name, to_track);
                } else {
                    last_instance.insert(track_name, to_track);
                }
            }
            if instance.tag() == Tag::ExtensionType {
                let i = self.core.store.instance(EId::from_raw(instance));
                let mut executables: Vec<ElementId> = i
                    .getters
                    .iter()
                    .map(|e| e.raw())
                    .chain(i.setters.iter().map(|e| e.raw()))
                    .chain(i.methods.iter().map(|e| e.raw()))
                    .collect();
                executables.extend(
                    self.core
                        .store
                        .interface(EId::from_raw(instance))
                        .constructors
                        .iter()
                        .map(|e| e.raw()),
                );
                for e in executables {
                    self.core
                        .store
                        .element_data(e)
                        .unwrap()
                        .flags
                        .set(ElementFlags::EXECUTABLE_ELEMENT_IS_EXTENSION_TYPE_MEMBER, true);
                }
            }
        }
    }

    fn handle_instance_constructor_fragment(&mut self, instance: ElementId, last: Option<FragmentId>, fragment: FId<ConstructorFragment>) {
        let enclosing = self.core.store.fragment(fragment).enclosing_fragment.unwrap();
        push_constructor_fragment(self.store(), enclosing, fragment);
        if self.is_augmentation(fragment.raw())
            && let Some(last) = last
            && last.tag() == Tag::Constructor
        {
            self.add_fragment(last, fragment.raw());
            self.link_type_parameters(last, fragment.raw());
            self.link_formal_parameters(last, fragment.raw());
            return;
        }
        let name = self.core.store.fragment(fragment).name;
        let element = self.store().add::<ConstructorElement>(ConstructorElement {
            executable: ExecutableElementData::new(ed(name, fragment.raw())),
            redirected_constructor: VarSlot::new(),
            super_constructor: VarSlot::new(),
        });
        self.core.store.fragment(fragment).element.set_once(element.raw());
        self.declare_member(instance, MemberReferenceKind::Constructor, name, element.raw());
        self.init_executable(element.upcast(), fragment.raw());
        if self.is_augmentation(fragment.raw()) && last.is_some() {
            self.store().get_mut(element).previous_fragment_of_different_kind = last;
        }
        interface_data_mut(self.store(), instance).constructors.push(element);
    }

    fn handle_instance_field_fragment(&mut self, instance: ElementId, last: Option<FragmentId>, fragment: FId<FieldFragment>) -> FId<FieldFragment> {
        let enclosing = self.core.store.fragment(fragment).enclosing_fragment.unwrap();
        let f = self.core.store.fragment(fragment);
        if f.flags.has(FragmentFlags::FIELD_FRAGMENT_IS_ORIGIN_ENUM_VALUES)
            && enclosing.tag() == Tag::Enum
            && self.core.store.fragment_data(enclosing).unwrap().previous_fragment.is_some()
        {
            // Dart: the `values` of an enum augmentation add their
            // elements to the `values` of the first fragment.
            let first = self.core.store.element_data(instance).unwrap().first_fragment;
            let augmentation = self.lib.implicit_enum_nodes.shift_remove(&FId::from_raw(enclosing));
            if let Some(first_implicit) = self.lib.implicit_enum_nodes.get_mut(&FId::from_raw(first)) {
                if let Some(a) = augmentation {
                    first_implicit.values_names.extend(a.values_names);
                }
                return first_implicit.values_fragment;
            }
            return fragment;
        }
        push_field_fragment(self.store(), enclosing, fragment);
        let last_field = self.field_element(last);
        let last_field_fragment = last_field.map(|e| {
            let first = self.core.store.get(e).first_fragment().raw();
            self.last_fragment(first)
        });
        let field = match (last_field, last_field_fragment) {
            (Some(e), Some(lf)) if self.is_augmentation(fragment.raw()) && lf.tag() == Tag::Field => {
                self.add_fragment(lf, fragment.raw());
                e
            }
            _ => {
                let name = self.core.store.fragment(fragment).name;
                let e = self.new_field_element(fragment);
                self.declare_member(instance, MemberReferenceKind::Field, name, e.raw());
                if self.is_augmentation(fragment.raw()) && last.is_some() {
                    self.store().get_mut(e).previous_fragment_of_different_kind = last;
                }
                instance_data_mut(self.store(), instance).fields.push(e);
                e
            }
        };
        let f = self.core.store.fragment(fragment);
        let name = f.name;
        let is_abstract = f.flags.has(FragmentFlags::VARIABLE_FRAGMENT_IS_ABSTRACT);
        let is_augmentation = f.flags.has(FragmentFlags::FRAGMENT_IS_AUGMENTATION);
        let is_static = f.flags.has(FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC);
        let explicitly_covariant = f.flags.has(FragmentFlags::FIELD_FRAGMENT_IS_EXPLICITLY_COVARIANT);
        let has_setter = variable_fragment_has_setter(&self.core.store, fragment.raw());
        {
            let data = new_getter_fragment(name);
            let g = &data.fragment;
            set(g, FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE, true);
            set(g, FragmentFlags::EXECUTABLE_FRAGMENT_IS_ABSTRACT, is_abstract);
            set(g, FragmentFlags::FRAGMENT_IS_AUGMENTATION, is_augmentation);
            set(g, FragmentFlags::FRAGMENT_IS_COMPLETE, !is_abstract);
            set(g, FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC, is_static);
            let getter_fragment = self.store().add_fragment::<GetterFragment>(data);
            self.induce_getter(fragment.raw(), getter_fragment);
            push_getter_fragment(self.store(), enclosing, getter_fragment);
            match self.core.store.get(field).getter {
                Some(g) => {
                    let first = self.core.store.get(g).first_fragment().raw();
                    let tail = self.last_fragment(first);
                    self.add_fragment(tail, getter_fragment.raw());
                }
                None => {
                    let getter = self.new_getter_element(getter_fragment);
                    self.declare_member(instance, MemberReferenceKind::Getter, name, getter.raw());
                    instance_data_mut(self.store(), instance).getters.push(getter);
                    self.store().get_mut(field).getter = Some(getter);
                    self.core.store.get(getter).variable.set(Some(field.upcast()));
                }
            }
        }
        if has_setter {
            let data = new_setter_fragment(name);
            let s = &data.fragment;
            set(s, FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE, true);
            set(s, FragmentFlags::EXECUTABLE_FRAGMENT_IS_ABSTRACT, is_abstract);
            set(s, FragmentFlags::FRAGMENT_IS_AUGMENTATION, is_augmentation);
            set(s, FragmentFlags::FRAGMENT_IS_COMPLETE, !is_abstract);
            set(s, FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC, is_static);
            let setter_fragment = self.store().add_fragment::<SetterFragment>(data);
            self.induce_setter(fragment.raw(), setter_fragment);
            push_setter_fragment(self.store(), enclosing, setter_fragment);
            let value = self.value_parameter(explicitly_covariant);
            self.set_formal_params(setter_fragment.raw(), vec![value]);
            match self.core.store.get(field).setter {
                Some(s) => {
                    let first = self.core.store.get(s).first_fragment().raw();
                    let tail = self.last_fragment(first);
                    self.add_fragment(tail, setter_fragment.raw());
                    self.link_formal_parameters(tail, setter_fragment.raw());
                }
                None => {
                    let setter = self.new_setter_element(setter_fragment);
                    self.declare_member(instance, MemberReferenceKind::Setter, name, setter.raw());
                    instance_data_mut(self.store(), instance).setters.push(setter);
                    self.store().get_mut(field).setter = Some(setter);
                    self.core.store.get(setter).variable.set(Some(field.upcast()));
                }
            }
        }
        fragment
    }

    fn handle_instance_getter_fragment(&mut self, instance: ElementId, last: Option<FragmentId>, fragment: FId<GetterFragment>, dart_core: bool) {
        let enclosing = self.core.store.fragment(fragment).enclosing_fragment.unwrap();
        push_getter_fragment(self.store(), enclosing, fragment);
        let mut last_field = self.field_element(last);
        let last_getter = last_field.and_then(|f| self.core.store.get(f).getter);
        if self.is_augmentation(fragment.raw())
            && let Some(g) = last_getter
        {
            let first = self.core.store.get(g).first_fragment().raw();
            let tail = self.last_fragment(first);
            self.add_fragment(tail, fragment.raw());
            return;
        }
        let name = self.core.store.fragment(fragment).name;
        let getter = self.new_getter_element(fragment);
        self.declare_member(instance, MemberReferenceKind::Getter, name, getter.raw());
        if self.is_augmentation(fragment.raw()) && last.is_some() {
            self.store().get_mut(getter).previous_fragment_of_different_kind = last;
        }
        instance_data_mut(self.store(), instance).getters.push(getter);
        // Dart `isDartCoreEnum && name == 'index'`.
        if dart_core
            && instance.tag() == Tag::Class
            && self.name_text(self.core.store.element_data(instance).unwrap().name) == Some("Enum")
            && self.name_text(name) == Some("index")
        {
            self.core
                .store
                .fragment(fragment)
                .flags
                .set(FragmentFlags::EXECUTABLE_FRAGMENT_IS_ABSTRACT, false);
        }
        if let Some(f) = last_field
            && self.core.store.get(f).getter.is_some()
        {
            last_field = None;
        }
        let field = match last_field {
            Some(f) => f,
            None => {
                let is_static = self
                    .core
                    .store
                    .fragment(fragment)
                    .flags
                    .has(FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC);
                let data = new_field_fragment(name);
                set(&data.fragment, FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_GETTER_SETTER, true);
                set(&data.fragment, FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC, is_static);
                let ff = self.store().add_fragment::<FieldFragment>(data);
                push_field_fragment(self.store(), enclosing, ff);
                let f = self.new_field_element(ff);
                self.declare_member(instance, MemberReferenceKind::Field, name, f.raw());
                instance_data_mut(self.store(), instance).fields.push(f);
                f
            }
        };
        self.core.store.get(getter).variable.set(Some(field.upcast()));
        self.store().get_mut(field).getter = Some(getter);
    }

    fn handle_instance_setter_fragment(&mut self, instance: ElementId, last: Option<FragmentId>, fragment: FId<SetterFragment>) {
        let enclosing = self.core.store.fragment(fragment).enclosing_fragment.unwrap();
        push_setter_fragment(self.store(), enclosing, fragment);
        let mut last_field = self.field_element(last);
        let last_setter = last_field.and_then(|f| self.core.store.get(f).setter);
        if self.is_augmentation(fragment.raw())
            && let Some(s) = last_setter
        {
            let first = self.core.store.get(s).first_fragment().raw();
            let tail = self.last_fragment(first);
            self.add_fragment(tail, fragment.raw());
            self.link_formal_parameters(tail, fragment.raw());
            return;
        }
        let name = self.core.store.fragment(fragment).name;
        let setter = self.new_setter_element(fragment);
        self.declare_member(instance, MemberReferenceKind::Setter, name, setter.raw());
        if self.is_augmentation(fragment.raw()) && last.is_some() {
            self.store().get_mut(setter).previous_fragment_of_different_kind = last;
        }
        instance_data_mut(self.store(), instance).setters.push(setter);
        if let Some(f) = last_field
            && self.core.store.get(f).setter.is_some()
        {
            last_field = None;
        }
        let field = match last_field {
            Some(f) => f,
            None => {
                let is_static = self
                    .core
                    .store
                    .fragment(fragment)
                    .flags
                    .has(FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC);
                let data = new_field_fragment(name);
                set(&data.fragment, FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_GETTER_SETTER, true);
                set(&data.fragment, FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC, is_static);
                let ff = self.store().add_fragment::<FieldFragment>(data);
                push_field_fragment(self.store(), enclosing, ff);
                let f = self.new_field_element(ff);
                self.declare_member(instance, MemberReferenceKind::Field, name, f.raw());
                instance_data_mut(self.store(), instance).fields.push(f);
                f
            }
        };
        self.core.store.get(setter).variable.set(Some(field.upcast()));
        self.store().get_mut(field).setter = Some(setter);
    }

    fn handle_instance_method_fragment(&mut self, instance: ElementId, last: Option<FragmentId>, fragment: FId<MethodFragment>) {
        let enclosing = self.core.store.fragment(fragment).enclosing_fragment.unwrap();
        push_method_fragment(self.store(), enclosing, fragment);
        if let Some(last) = last
            && last.tag() == Tag::Method
            && self.is_augmentation(fragment.raw())
        {
            self.add_fragment(last, fragment.raw());
            self.link_type_parameters(last, fragment.raw());
            self.link_formal_parameters(last, fragment.raw());
            return;
        }
        let name = self.core.store.fragment(fragment).name;
        let element = self.store().add::<MethodElement>(MethodElement {
            executable: ExecutableElementData::new(ed(name, fragment.raw())),
            is_operator_equal_with_parameter_type_from_object: BoolSlot::new(false),
            type_inference_error: OnceSlot::new(),
        });
        self.core.store.fragment(fragment).element.set_once(element.raw());
        self.declare_member(instance, MemberReferenceKind::Method, name, element.raw());
        self.init_executable(element.upcast(), fragment.raw());
        if self.is_augmentation(fragment.raw()) && last.is_some() {
            self.store().get_mut(element).previous_fragment_of_different_kind = last;
        }
        instance_data_mut(self.store(), instance).methods.push(element);
    }

    /// Dart `_buildFormalParameterElements`, and the cached
    /// `formalParameters` of the executable elements.
    fn build_formal_parameter_elements(&mut self) {
        let executables = std::mem::take(&mut self.executable_elements);
        for element in executables {
            let first = self.core.store.element_data(element.raw()).unwrap().first_fragment;
            let ps = self.core.store.executable_fragment(FId::from_raw(first)).formal_params.clone();
            let elements: Vec<EId<FormalParameterElement>> = ps
                .into_iter()
                .map(|p| init_formal_parameter_element(&mut self.core.store, p))
                .collect();
            executable_data_mut(&mut self.core.store, element.raw()).formal_params = elements;
        }
    }
}

/// Dart `PropertyInducingFragmentImpl.hasSetter`.
fn variable_fragment_has_setter(store: &ElementStore, f: FragmentId) -> bool {
    let d = store.fragment_data(f).unwrap();
    if d.flags.has(FragmentFlags::VARIABLE_FRAGMENT_IS_CONST) {
        return false;
    }
    let is_final = d.flags.has(FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL);
    if d.flags.has(FragmentFlags::VARIABLE_FRAGMENT_IS_LATE) {
        return !is_final
            || !d
                .flags
                .has(FragmentFlags::NON_PARAMETER_VARIABLE_FRAGMENT_HAS_INITIALIZER);
    }
    !is_final
}

fn push_constructor_fragment(store: &mut ElementStore, enclosing: FragmentId, f: FId<ConstructorFragment>) {
    let i = enclosing.index();
    let fr = &mut store.fragments;
    match enclosing.tag() {
        Tag::Class => fr.classes.get_mut(i).constructors.push(f),
        Tag::Enum => fr.enums.get_mut(i).constructors.push(f),
        Tag::Mixin => fr.mixins.get_mut(i).constructors.push(f),
        Tag::ExtensionType => fr.extension_types.get_mut(i).constructors.push(f),
        t => panic!("constructor in {t:?}"),
    }
}

fn instance_fragment_mut(store: &mut ElementStore, f: FragmentId) -> &mut InstanceFragmentData {
    let i = f.index();
    let fr = &mut store.fragments;
    match f.tag() {
        Tag::Class => &mut fr.classes.get_mut(i).interface.instance,
        Tag::Enum => &mut fr.enums.get_mut(i).interface.instance,
        Tag::Mixin => &mut fr.mixins.get_mut(i).interface.instance,
        Tag::ExtensionType => &mut fr.extension_types.get_mut(i).interface.instance,
        Tag::Extension => &mut fr.extensions.get_mut(i).instance,
        t => panic!("not an instance fragment: {t:?}"),
    }
}

fn push_field_fragment(store: &mut ElementStore, enclosing: FragmentId, f: FId<FieldFragment>) {
    instance_fragment_mut(store, enclosing).fields.push(f);
    store.fragment_mut(f).enclosing_fragment = Some(enclosing);
}

fn push_getter_fragment(store: &mut ElementStore, enclosing: FragmentId, f: FId<GetterFragment>) {
    instance_fragment_mut(store, enclosing).getters.push(f);
    store.fragment_mut(f).enclosing_fragment = Some(enclosing);
}

fn push_setter_fragment(store: &mut ElementStore, enclosing: FragmentId, f: FId<SetterFragment>) {
    instance_fragment_mut(store, enclosing).setters.push(f);
    store.fragment_mut(f).enclosing_fragment = Some(enclosing);
}

fn push_method_fragment(store: &mut ElementStore, enclosing: FragmentId, f: FId<MethodFragment>) {
    instance_fragment_mut(store, enclosing).methods.push(f);
    store.fragment_mut(f).enclosing_fragment = Some(enclosing);
}

/// `InstanceElementImpl` data, mutable.
pub fn instance_data_mut(store: &mut ElementStore, e: ElementId) -> &mut InstanceElementData {
    let i = e.index();
    let el = &mut store.elements;
    match e.tag() {
        Tag::Class => &mut el.classes.get_mut(i).interface.instance,
        Tag::Enum => &mut el.enums.get_mut(i).interface.instance,
        Tag::Mixin => &mut el.mixins.get_mut(i).interface.instance,
        Tag::ExtensionType => &mut el.extension_types.get_mut(i).interface.instance,
        Tag::Extension => &mut el.extensions.get_mut(i).instance,
        t => panic!("not an instance element: {t:?}"),
    }
}

/// `InterfaceElementImpl` data, mutable.
pub fn interface_data_mut(store: &mut ElementStore, e: ElementId) -> &mut InterfaceElementData {
    let i = e.index();
    let el = &mut store.elements;
    match e.tag() {
        Tag::Class => &mut el.classes.get_mut(i).interface,
        Tag::Enum => &mut el.enums.get_mut(i).interface,
        Tag::Mixin => &mut el.mixins.get_mut(i).interface,
        Tag::ExtensionType => &mut el.extension_types.get_mut(i).interface,
        t => panic!("not an interface element: {t:?}"),
    }
}

/// `ExecutableElementImpl` data, mutable.
pub fn executable_data_mut(store: &mut ElementStore, e: ElementId) -> &mut ExecutableElementData {
    let i = e.index();
    let el = &mut store.elements;
    match e.tag() {
        Tag::Method => &mut el.methods.get_mut(i).executable,
        Tag::Constructor => &mut el.constructors.get_mut(i).executable,
        Tag::Getter => &mut el.getters.get_mut(i).accessor.executable,
        Tag::Setter => &mut el.setters.get_mut(i).accessor.executable,
        Tag::TopLevelFunction => &mut el.functions.get_mut(i).executable,
        Tag::LocalFunction => &mut el.local_functions.get_mut(i).executable,
        t => panic!("not an executable element: {t:?}"),
    }
}
