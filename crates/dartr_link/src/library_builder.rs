// Dart source: pkg/analyzer/lib/src/summary2/library_builder.dart

//! [`LibraryBuilder`]: builds the element of one library of a cycle: the
//! library element and its units, the directives, the fragments and
//! elements (through `element_builder.rs`), the export scope, and the
//! synthetic constructors.

use std::sync::Arc;

use dartr_ast::{Id, MethodDeclaration, MixinDeclaration};

use dartr_element::*;
use dartr_parser::experimental_features::ExperimentalFeatures;
use dartr_parser::experimental_flags::ExperimentalFlag;
use indexmap::{IndexMap, IndexSet};

use crate::ast_util::{MixinSuperInvokedNamesCollector, class_body_members};
use crate::element_builder::{ElementBuilder, FragmentBuilder};
use crate::export::{Combinator, Export, ExportLocation, ExportScope};
use crate::informative_data::{InfoBuilder, InformativeDataApplier};
use crate::input::{
    LinkCombinator, LinkDirectiveUri, LinkImport, LinkLibraryInput, LinkPartUri, LinkUnitInput,
};
use crate::link::{Linker, LinkerCore, LinkingUnit};
use crate::reference::{
    BuiltInReferenceKind, LibraryReference, LibraryReferenceBuilder, MemberReferenceKind, RefId,
};

/// Dart `ImplicitEnumNodes`: the synthetic `values` field of an enum.
#[derive(Debug, Clone)]
pub struct ImplicitEnumNodes {
    pub fragment: FId<EnumFragment>,
    pub values_fragment: FId<FieldFragment>,
    pub values_names: IndexSet<Arc<str>>,
    /// The synthetic `ListLiteral` initializer in the `ConstExprs`.
    pub values_initializer: ConstExprId,
    /// The synthetic `List<E>` type annotation in the `ConstExprs`.
    pub values_type_node: ConstExprId,
}

/// Dart `LibraryBuilder`.
pub struct LibraryBuilder {
    pub uri: Arc<str>,
    pub input: LinkLibraryInput,
    pub element: EId<LibraryElement>,
    /// Dart `units`: the defining unit, then the parts (depth first).
    pub units: Vec<LinkingUnit>,
    /// Dart `implicitEnumNodes`.
    pub implicit_enum_nodes: IndexMap<FId<EnumFragment>, ImplicitEnumNodes>,
    /// Dart `_topFragments`.
    pub top_fragments: IndexMap<FId<LibraryFragment>, Vec<FragmentId>>,
    /// Dart `_parentChildFragments`.
    pub parent_child_fragments: IndexMap<FragmentId, Vec<FragmentId>>,
    /// Dart `_declaredReferences`: lookup name to reference and element.
    pub declared_references: IndexMap<Arc<str>, (RefId, ElementId)>,
    /// Dart `exportScope`.
    pub export_scope: ExportScope,
    /// Dart `references`.
    pub references: LibraryReferenceBuilder,
    /// Dart `element.reference` of the elements of this library.
    pub element_references: IndexMap<ElementId, RefId>,
    /// Dart `_nextLocalReferenceId`.
    pub next_local_reference_id: u32,
    /// Dart `finalInstanceFields`.
    pub final_instance_fields: IndexSet<FId<FieldFragment>>,
    /// Dart `element.featureSet` (the parser features of the defining unit).
    pub features: ExperimentalFeatures,
}

impl LibraryBuilder {
    pub fn is_enabled(&self, flag: ExperimentalFlag) -> bool {
        self.features.is_experiment_enabled(flag)
    }

    /// Dart `LibraryBuilder.build`.
    pub fn build(core: &mut LinkerCore<'_>, input: LinkLibraryInput) -> LibraryBuilder {
        let unit = &input.unit;
        let parsed = unit.parsed.clone();
        let ast = &parsed.ast;
        let mut name = String::new();
        let mut name_offset = -1i32;
        let mut name_length = 0u32;
        for &directive in ast.list(ast.get(parsed.unit).directives) {
            if let Some(l) = ast.cast::<dartr_ast::LibraryDirective>(directive.raw()) {
                if let Some(n) = ast.get(l).name {
                    name = crate::ast_util::dotted_name(ast, n);
                    name_offset = ast.offset(n) as i32;
                    name_length = ast.length(n);
                }
                break;
            }
        }

        let library_id: EId<LibraryElement> = EId::from_raw(ElementId::new(
            core.store.id,
            Tag::Library,
            core.store.elements.libraries.len() as u32,
        ));
        let fragment = new_library_fragment(core, library_id, unit);
        let features = parsed.feature_set;
        let feature_names = ExperimentalFlag::VALUES
            .iter()
            .filter(|f| features.is_experiment_enabled(**f))
            .map(|f| Arc::<str>::from(f.name()));
        let lv = parsed.language_version;
        let element = LibraryElement {
            element: ElementData::new(Some(core.name(&name)), fragment.raw()),
            metadata: Metadata::default(),
            documentation_comment: None,
            language_version: LibraryLanguageVersion {
                package: Version {
                    major: lv.package.0,
                    minor: lv.package.1,
                },
                override_: lv.override_.map(|(major, minor)| Version { major, minor }),
            },
            feature_set: FeatureSet::new(feature_names),
            entry_point: OnceSlot::new(),
            load_library_function: OnceSlot::new(),
            name_offset,
            name_length,
            classes: Vec::new(),
            enums: Vec::new(),
            extensions: Vec::new(),
            extension_types: Vec::new(),
            getters: Vec::new(),
            setters: Vec::new(),
            mixins: Vec::new(),
            top_level_functions: Vec::new(),
            top_level_variables: Vec::new(),
            type_aliases: Vec::new(),
            export_namespace: OnceSlot::new(),
            public_namespace: OnceSlot::new(),
            field_name_non_promotability_info: OnceSlot::new(),
        };
        let added = core.store.add::<LibraryElement>(element);
        debug_assert_eq!(added, library_id);
        if !unit.exists {
            core.store
                .get(library_id)
                .flags
                .set(ElementFlags::LIBRARY_ELEMENT_IS_SYNTHETIC, true);
        }
        core.store
            .fragment(fragment)
            .element
            .set_once(library_id.raw());

        let uri = unit.uri.clone();
        let mut references = LibraryReferenceBuilder::new(LibraryReference::new(uri.clone()));
        references
            .reference
            .set_element(RefId::LIBRARY, library_id.raw());
        // Dart `_createLoadLibraryReference`: the `loadLibrary` function is
        // created on first use (`LoadLibraryFunctionProvider`).
        references.declare_top_level(
            crate::reference::TopLevelReferenceKind::Function,
            Some("loadLibrary"),
        );

        LibraryBuilder {
            uri,
            units: vec![LinkingUnit {
                parsed: parsed.clone(),
                fragment,
                declared_fragments: IndexMap::new(),
            }],
            input,
            element: library_id,
            implicit_enum_nodes: IndexMap::new(),
            top_fragments: IndexMap::new(),
            parent_child_fragments: IndexMap::new(),
            declared_references: IndexMap::new(),
            export_scope: ExportScope::default(),
            references,
            element_references: IndexMap::new(),
            next_local_reference_id: 0,
            final_instance_fields: IndexSet::new(),
            features,
        }
    }

    /// Dart `addChildFragment`.
    pub fn add_child_fragment(
        &mut self,
        store: &mut ElementStore,
        parent: FragmentId,
        child: FragmentId,
    ) {
        crate::informative_data::fragment_data_mut(store, child).enclosing_fragment = Some(parent);
        self.parent_child_fragments
            .entry(parent)
            .or_default()
            .push(child);
    }

    /// Dart `addTopFragment`.
    pub fn add_top_fragment(
        &mut self,
        store: &mut ElementStore,
        parent: FId<LibraryFragment>,
        fragment: FragmentId,
    ) {
        crate::informative_data::fragment_data_mut(store, fragment).enclosing_fragment =
            Some(parent.raw());
        self.top_fragments.entry(parent).or_default().push(fragment);
    }

    /// Dart `declare`: records the element under its lookup name.
    pub fn declare(&mut self, lookup_name: Option<Arc<str>>, reference: RefId, element: ElementId) {
        if let Some(name) = lookup_name {
            self.declared_references.insert(name, (reference, element));
        }
    }

    /// Dart `buildElements`.
    pub fn build_elements(linker: &mut Linker<'_>, index: usize) {
        let input_unit = linker.builders[index].input.unit.clone();
        let container = linker.builders[index].units[0].fragment;
        build_directives(linker, index, &input_unit, container);

        let unit_count = linker.builders[index].units.len();
        for unit_index in 0..unit_count {
            let mut fb = FragmentBuilder::new(linker, index, unit_index);
            fb.build_directives();
            fb.build_declaration_fragments();
            if unit_index == 0 {
                fb.build_library_metadata();
            }
            fb.finish();
        }

        ElementBuilder::new(linker, index).build_elements();

        for unit_index in 0..unit_count {
            let (parsed, fragment) = {
                let u = &linker.builders[index].units[unit_index];
                (u.parsed.clone(), u.fragment)
            };
            let info = InfoBuilder::build(&parsed.ast, parsed.unit, &parsed.line_info.line_starts);
            let library = linker.builders[index].element;
            InformativeDataApplier {
                store: &mut linker.core.store,
            }
            .apply(library, fragment, &info);
        }

        // Dart `_declareDartCoreDynamicNever`.
        let builder = &mut linker.builders[index];
        if &*builder.uri == "dart:core" {
            let r = builder
                .references
                .reference
                .built_in(BuiltInReferenceKind::Dynamic);
            builder
                .references
                .reference
                .set_element(r, ElementId::DYNAMIC);
            builder.declare(Some("dynamic".into()), r, ElementId::DYNAMIC);
            let r = builder
                .references
                .reference
                .built_in(BuiltInReferenceKind::Never);
            builder
                .references
                .reference
                .set_element(r, ElementId::NEVER);
            builder.declare(Some("Never".into()), r, ElementId::NEVER);
        }
    }

    /// Dart `buildInitialExportScope`.
    pub fn initial_export_scope(&self) -> ExportScope {
        let mut scope = ExportScope::default();
        for (name, (_, element)) in &self.declared_references {
            if name.starts_with('_') {
                continue;
            }
            scope.declare(name.clone(), *element);
        }
        scope
    }

    /// Dart `addExporters`.
    pub fn add_exporters(
        linker: &Linker<'_>,
        index: usize,
        scopes: &mut [ExportScope],
        exports: &mut [Vec<Export>],
    ) {
        let builder = &linker.builders[index];
        let store = &linker.core.store;
        for (fragment_index, unit) in builder.units.iter().enumerate() {
            let fragment = store.fragment(unit.fragment);
            for (export_index, export) in fragment.library_exports.iter().enumerate() {
                let DirectiveUri::Library { library, .. } = &export.directive.uri else {
                    continue;
                };
                let combinators = combinators_of(linker, &export.combinators);
                let export_ = Export {
                    exporter: index,
                    location: ExportLocation {
                        fragment_index: fragment_index as u32,
                        export_index: export_index as u32,
                    },
                    combinators,
                };
                let exported_uri = library_uri(linker, *library);
                if let Some(&b) = linker.builder_by_uri.get(exported_uri.as_ref()) {
                    exports[b].push(export_);
                } else if let Some(linked) = linker.core.deps.library(&exported_uri) {
                    for entry in &linked.export_entries {
                        export_.add_to_export_scope(scopes, entry);
                    }
                }
            }
        }
    }

    /// Dart `storeExportScope`.
    pub fn store_export_scope(linker: &mut Linker<'_>, index: usize) {
        let builder = &linker.builders[index];
        let mut namespace = Namespace::default();
        for entry in builder.export_scope.entries_by_name.values() {
            namespace
                .defined_names
                .insert(linker.core.name(&entry.name), entry.element);
        }
        let main = namespace
            .defined_names
            .get(&linker.core.name("main"))
            .copied();
        let library = linker.core.store.get(builder.element);
        library.export_namespace.set_once(Arc::new(namespace));
        let entry_point = main.and_then(|m| m.cast::<TopLevelFunctionElement>());
        library.entry_point.set_once(entry_point);
    }

    /// Dart `buildClassSyntheticConstructors`.
    pub fn build_class_synthetic_constructors(linker: &mut Linker<'_>, index: usize) {
        let library = linker.builders[index].element;
        let classes = linker.core.store.get(library).classes.clone();
        for class in classes {
            let first = linker.core.store.get(class).first_fragment();
            let store = &linker.core.store;
            if store
                .fragment(first)
                .flags
                .has(FragmentFlags::CLASS_FRAGMENT_IS_MIXIN_APPLICATION)
            {
                continue;
            }
            if !store.get(class).constructors.is_empty() {
                continue;
            }
            let class_name = store.get(class).name;
            let new = linker.core.name("new");
            let mut data = crate::element_builder::new_constructor_fragment(FragmentData::new(
                Some(new),
                None,
            ));
            data.type_name = class_name;
            data.fragment.enclosing_fragment = Some(first.raw());
            data.flags.set(
                FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_ORIGIN_IMPLICIT_DEFAULT,
                true,
            );
            let fragment = linker.core.store.add_fragment::<ConstructorFragment>(data);
            linker.core.store.fragment_mut(first).constructors = vec![fragment];
            let element = new_constructor_element(&mut linker.core, fragment, new);
            let builder = &mut linker.builders[index];
            let container = builder.element_references[&class.raw()];
            let r = builder.references.reference.declare_member(
                container,
                MemberReferenceKind::Constructor,
                "new",
            );
            builder.references.reference.set_element(r, element.raw());
            builder.element_references.insert(element.raw(), r);
            linker.core.store.get_mut(class).constructors = vec![element];
        }
    }

    /// Dart `buildEnumSyntheticConstructors`.
    pub fn build_enum_synthetic_constructors(linker: &mut Linker<'_>, index: usize) {
        let library = linker.builders[index].element;
        let enums = linker.core.store.get(library).enums.clone();
        let new = linker.core.name("new");
        for enum_ in enums {
            let store = &linker.core.store;
            let has_constructor = store.get(enum_).constructors.iter().any(|&c| {
                let c = store.get(c);
                let factory = store
                    .fragment(c.first_fragment())
                    .flags
                    .has(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY);
                !factory || c.name == Some(new)
            });
            if has_constructor {
                continue;
            }
            let first = store.get(enum_).first_fragment();
            let mut data = crate::element_builder::new_constructor_fragment(FragmentData::new(
                Some(new),
                None,
            ));
            data.type_name = store.get(enum_).name;
            data.fragment.enclosing_fragment = Some(first.raw());
            data.flags.set(
                FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_ORIGIN_IMPLICIT_DEFAULT,
                true,
            );
            data.flags
                .set(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST, true);
            let fragment = linker.core.store.add_fragment::<ConstructorFragment>(data);
            linker
                .core
                .store
                .fragment_mut(first)
                .constructors
                .push(fragment);
            let element = new_constructor_element(&mut linker.core, fragment, new);
            let builder = &mut linker.builders[index];
            let container = builder.element_references[&enum_.raw()];
            let r = builder.references.reference.declare_member(
                container,
                MemberReferenceKind::Constructor,
                "new",
            );
            builder.references.reference.set_element(r, element.raw());
            builder.element_references.insert(element.raw(), r);
            linker.core.store.get_mut(enum_).constructors.push(element);
        }
    }

    /// Dart `replaceConstFieldsIfNoConstConstructor`.
    pub fn replace_const_fields_if_no_const_constructor(linker: &mut Linker<'_>, index: usize) {
        let fields: Vec<FId<FieldFragment>> = linker.builders[index]
            .final_instance_fields
            .iter()
            .copied()
            .collect();
        let mut cache: IndexMap<ElementId, bool> = IndexMap::new();
        for field in fields {
            let store = &linker.core.store;
            let Some(enclosing) = store.fragment(field).enclosing_fragment else {
                continue;
            };
            let Some(&element) = store.fragment_data(enclosing).unwrap().element.try_get() else {
                continue;
            };
            let has_const = match element.cast::<InterfaceElement>() {
                Some(i) => *cache.entry(element).or_insert_with(|| {
                    store.interface(i).constructors.iter().any(|&c| {
                        store
                            .fragment(store.get(c).first_fragment())
                            .flags
                            .has(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST)
                    })
                }),
                None => false,
            };
            if !has_const {
                linker.core.store.fragment_mut(field).constant_initializer = None;
            }
        }
    }

    /// Dart `resolveConstructorFieldFormals`.
    pub fn resolve_constructor_field_formals(linker: &mut Linker<'_>, index: usize) {
        let library = linker.builders[index].element;
        let store = &linker.core.store;
        let l = store.get(library);
        let interfaces: Vec<EId<InterfaceElement>> = l
            .classes
            .iter()
            .map(|c| c.upcast())
            .chain(l.enums.iter().map(|e| e.upcast()))
            .chain(l.extension_types.iter().map(|e| e.upcast()))
            .chain(l.mixins.iter().map(|m| m.upcast()))
            .collect();
        for interface in interfaces {
            if let Some(class) = interface.raw().cast::<ClassElement>()
                && store
                    .fragment(store.get(class).first_fragment())
                    .flags
                    .has(FragmentFlags::CLASS_FRAGMENT_IS_MIXIN_APPLICATION)
            {
                continue;
            }
            let i = store.interface(interface);
            for &constructor in &i.constructors {
                for &parameter in &store.get(constructor).formal_params {
                    if parameter.raw().tag() != Tag::FieldFormalParameter {
                        continue;
                    }
                    let p = store.get(parameter);
                    let fragment = store.fragment(p.first_fragment());
                    let name = fragment.private_name.or(p.name);
                    let name_text = name.map(|n| linker.core.name_str(n)).unwrap_or("");
                    let field = i.fields.iter().copied().find(|&f| {
                        store.get(f).name.map(|n| linker.core.name_str(n)) == Some(name_text)
                            || (name_text.is_empty() && store.get(f).name.is_none())
                    });
                    p.field.set(field);
                }
            }
        }
    }

    /// Dart `collectMixinSuperInvokedNames`.
    pub fn collect_mixin_super_invoked_names(linker: &mut Linker<'_>, index: usize) {
        let builder = &linker.builders[index];
        for unit in &builder.units {
            let ast = &unit.parsed.ast;
            for &declaration in ast.list(ast.get(unit.parsed.unit).declarations) {
                let Some(mixin) = ast.cast::<MixinDeclaration>(declaration.raw()) else {
                    continue;
                };
                let mut names = IndexSet::new();
                {
                    let mut collector = MixinSuperInvokedNamesCollector { names: &mut names };
                    for member in class_body_members(ast, ast.get(mixin).body) {
                        if let Some(m) = ast.cast::<MethodDeclaration>(member.raw()) {
                            let m: Id<MethodDeclaration> = m;
                            ast.accept(ast.get(m).body, &mut collector);
                        }
                    }
                }
                let mut sorted: Vec<String> = names.into_iter().collect();
                sorted.sort_by(|a, b| crate::dump::compare_utf16(a, b));
                let Some(&fragment) = unit.declared_fragments.get(&mixin.raw()) else {
                    continue;
                };
                let names: Vec<Name> = sorted.iter().map(|n| linker.core.name(n)).collect();
                linker
                    .core
                    .store
                    .fragment(FId::<MixinFragment>::from_raw(fragment))
                    .super_invoked_names
                    .set_once(names);
            }
        }
    }
}

/// The URI of a library element (of this cycle or of a linked cycle).
pub fn library_uri(linker: &Linker<'_>, library: EId<LibraryElement>) -> Arc<str> {
    let store = if library.store() == linker.core.store.id {
        &linker.core.store
    } else {
        linker
            .core
            .world
            .store(library.store())
            .expect("library store")
    };
    store
        .fragment(store.get(library).first_fragment())
        .source
        .uri
        .clone()
}

/// Dart `NamespaceCombinatorListExtension.build`.
pub fn combinators_of(linker: &Linker<'_>, combinators: &[NamespaceCombinator]) -> Vec<Combinator> {
    combinators
        .iter()
        .map(|c| match c {
            NamespaceCombinator::Show { shown_names, .. } => Combinator {
                is_show: true,
                names: shown_names
                    .iter()
                    .map(|n| Arc::from(linker.core.name_str(*n)))
                    .collect(),
            },
            NamespaceCombinator::Hide { hidden_names, .. } => Combinator {
                is_show: false,
                names: hidden_names
                    .iter()
                    .map(|n| Arc::from(linker.core.name_str(*n)))
                    .collect(),
            },
        })
        .collect()
}

/// A new `LibraryFragmentImpl` for [unit] (Dart constructor +
/// `isOriginNotExistingFile` + `setCodeRange(0, length)`).
fn new_library_fragment(
    core: &mut LinkerCore<'_>,
    library: EId<LibraryElement>,
    unit: &LinkUnitInput,
) -> FId<LibraryFragment> {
    let parsed = &unit.parsed;
    let mut data = FragmentData::new(None, None);
    data.first_token_offset = Some(0);
    data.code_offset = Some(0);
    data.code_length = Some(parsed.ast.length(parsed.unit));
    if !unit.exists {
        data.flags.set(
            FragmentFlags::LIBRARY_FRAGMENT_IS_ORIGIN_NOT_EXISTING_FILE,
            true,
        );
    }
    let mut fragment = LibraryFragment::new(
        data,
        SourceRef {
            path: unit.path.clone(),
            uri: unit.uri.clone(),
        },
        library,
    );
    fragment.line_starts = parsed.line_info.line_starts.as_slice().into();
    core.store.add_fragment::<LibraryFragment>(fragment)
}

/// Dart `ConstructorElementImpl(...)` for a fragment without type
/// parameters (synthetic constructors).
pub fn new_constructor_element(
    core: &mut LinkerCore<'_>,
    fragment: FId<ConstructorFragment>,
    name: Name,
) -> EId<ConstructorElement> {
    let element = core.store.add::<ConstructorElement>(ConstructorElement {
        executable: ExecutableElementData::new(ElementData::new(Some(name), fragment.raw())),
        redirected_constructor: VarSlot::new(),
        super_constructor: VarSlot::new(),
    });
    core.store
        .fragment(fragment)
        .element
        .set_once(element.raw());
    core.store.get(element).flags.set(
        ElementFlags::EXECUTABLE_ELEMENT_HAS_ENCLOSING_TYPE_PARAMETER_REFERENCE,
        true,
    );
    element
}

/// Dart `LibraryBuilder._buildDirectives`.
fn build_directives(
    linker: &mut Linker<'_>,
    index: usize,
    kind: &LinkUnitInput,
    container: FId<LibraryFragment>,
) {
    let exports: Vec<LibraryExport> = kind
        .exports
        .iter()
        .map(|state| LibraryExport {
            directive: ElementDirective {
                library_fragment: container,
                uri: build_directive_uri(linker, &state.uri),
                metadata: Metadata::default(),
            },
            combinators: build_namespace_combinators(&linker.core, &state.combinators),
            export_keyword_offset: state.export_keyword_offset as i32,
        })
        .collect();
    linker.core.store.fragment_mut(container).library_exports = exports;

    let mut imports = Vec::new();
    for state in &kind.imports {
        imports.push(build_library_import(linker, index, container, state));
    }
    linker.core.store.fragment_mut(container).library_imports = imports;
    let prefixes: Vec<EId<PrefixElement>> = linker
        .core
        .store
        .fragment(container)
        .library_import_prefixes_by_id
        .values()
        .copied()
        .collect();
    linker
        .core
        .store
        .fragment_mut(container)
        .library_import_prefixes = prefixes;

    let mut parts = Vec::new();
    for state in &kind.parts {
        let uri = match &state.uri {
            LinkPartUri::Unit {
                relative_uri_string,
                relative_uri,
                unit,
            } => {
                let library = linker.builders[index].element;
                let fragment = new_library_fragment(&mut linker.core, library, unit);
                linker
                    .core
                    .store
                    .fragment(fragment)
                    .element
                    .set_once(library.raw());
                linker.builders[index].units.push(LinkingUnit {
                    parsed: unit.parsed.clone(),
                    fragment,
                    declared_fragments: IndexMap::new(),
                });
                build_directives(linker, index, unit, fragment);
                DirectiveUri::Unit {
                    relative_uri_string: relative_uri_string.clone(),
                    relative_uri: relative_uri.clone(),
                    library_fragment: fragment,
                }
            }
            LinkPartUri::Other(uri) => build_directive_uri(linker, uri),
        };
        parts.push(PartInclude {
            directive: ElementDirective {
                library_fragment: container,
                uri,
                metadata: Metadata::default(),
            },
            part_keyword_offset: state.part_keyword_offset as i32,
        });
    }
    linker.core.store.fragment_mut(container).parts = parts;
}

/// Dart `_buildLibraryImport`.
fn build_library_import(
    linker: &mut Linker<'_>,
    index: usize,
    container: FId<LibraryFragment>,
    state: &LinkImport,
) -> LibraryImport {
    let prefix = state.prefix.as_ref().map(|p| {
        let name = linker.core.name_opt(p.name.as_deref());
        let mut data = PrefixFragment {
            fragment: FragmentData::new(name, p.name.as_ref().map(|_| p.name_offset)),
            offset: p.name_offset,
            is_deferred: p.is_deferred,
        };
        data.fragment.enclosing_fragment = Some(container.raw());
        let fragment = linker.core.store.add_fragment::<PrefixFragment>(data);
        let id = match &p.name {
            Some(n) => n.to_string(),
            None => {
                let b = &mut linker.builders[index];
                let id = format!("#{}", b.next_local_reference_id);
                b.next_local_reference_id += 1;
                id
            }
        };
        bind_library_import_prefix_element(&mut linker.core, container, &id, fragment);
        fragment
    });
    LibraryImport {
        directive: ElementDirective {
            library_fragment: container,
            uri: build_directive_uri(linker, &state.uri),
            metadata: Metadata::default(),
        },
        is_synthetic: state.is_synthetic,
        combinators: build_namespace_combinators(&linker.core, &state.combinators),
        import_keyword_offset: state.import_keyword_offset,
        prefix,
        namespace: OnceSlot::new(),
    }
}

/// Dart `LibraryFragmentImpl.bindLibraryImportPrefixElement`.
fn bind_library_import_prefix_element(
    core: &mut LinkerCore<'_>,
    unit: FId<LibraryFragment>,
    id: &str,
    fragment: FId<PrefixFragment>,
) {
    let local_id = core.name(id);
    let existing = core
        .store
        .fragment(unit)
        .library_import_prefixes_by_id
        .get(&local_id)
        .copied();
    let element = match existing {
        None => {
            let name = core.store.fragment(fragment).name;
            let element = core.store.add::<PrefixElement>(PrefixElement {
                element: ElementData::new(name, fragment.raw()),
                local_id,
                last_fragment: fragment,
            });
            core.store
                .fragment_mut(unit)
                .library_import_prefixes_by_id
                .insert(local_id, element);
            element
        }
        Some(element) => {
            // Dart `PrefixElementImpl.addFragment`.
            let last = core.store.get(element).last_fragment;
            core.store.fragment_mut(last).next_fragment = Some(fragment.raw());
            core.store.fragment_mut(fragment).previous_fragment = Some(last.raw());
            core.store.get_mut(element).last_fragment = fragment;
            element
        }
    };
    core.store
        .fragment(fragment)
        .element
        .set_once(element.raw());
}

/// The `DirectiveUri*Impl` of a selected directive URI.
fn build_directive_uri(linker: &Linker<'_>, uri: &LinkDirectiveUri) -> DirectiveUri {
    match uri {
        LinkDirectiveUri::None => DirectiveUri::None,
        LinkDirectiveUri::RelativeUriString {
            relative_uri_string,
        } => DirectiveUri::RelativeUriString {
            relative_uri_string: relative_uri_string.clone(),
        },
        LinkDirectiveUri::RelativeUri {
            relative_uri_string,
            relative_uri,
        } => DirectiveUri::RelativeUri {
            relative_uri_string: relative_uri_string.clone(),
            relative_uri: relative_uri.clone(),
        },
        LinkDirectiveUri::Source {
            relative_uri_string,
            relative_uri,
            path,
            uri,
        } => DirectiveUri::Source {
            relative_uri_string: relative_uri_string.clone(),
            relative_uri: relative_uri.clone(),
            source: SourceRef {
                path: path.clone(),
                uri: uri.clone(),
            },
        },
        LinkDirectiveUri::Library {
            relative_uri_string,
            relative_uri,
            library_uri,
        } => match linker.library_of_uri(library_uri) {
            Some(library) => {
                let store = if library.store() == linker.core.store.id {
                    &linker.core.store
                } else {
                    linker
                        .core
                        .world
                        .store(library.store())
                        .expect("library store")
                };
                let source = store
                    .fragment(store.get(library).first_fragment())
                    .source
                    .clone();
                DirectiveUri::Library {
                    relative_uri_string: relative_uri_string.clone(),
                    relative_uri: relative_uri.clone(),
                    source,
                    library,
                }
            }
            None => DirectiveUri::RelativeUri {
                relative_uri_string: relative_uri_string.clone(),
                relative_uri: relative_uri.clone(),
            },
        },
    }
}

/// Dart `_buildCombinators`.
fn build_namespace_combinators(
    core: &LinkerCore<'_>,
    combinators: &[LinkCombinator],
) -> Vec<NamespaceCombinator> {
    combinators
        .iter()
        .map(|c| {
            let names: Vec<Name> = c.names.iter().map(|n| core.name(n)).collect();
            if c.is_show {
                NamespaceCombinator::Show {
                    shown_names: names,
                    offset: c.keyword_offset,
                    end: c.end_offset as i32,
                }
            } else {
                NamespaceCombinator::Hide {
                    hidden_names: names,
                    offset: c.keyword_offset,
                    end: c.end_offset as i32,
                }
            }
        })
        .collect()
}
