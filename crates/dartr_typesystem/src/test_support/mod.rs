// Dart source: pkg/analyzer/lib/src/test_utilities/test_library_builder.dart
// (buildLibrariesFromSpec, _LibraryBuilder, _*Declaration, _Scope,
// TypeSpecParser), pkg/analyzer/lib/src/test_utilities/mock_sdk_elements.dart
// (_sdkSpec), pkg/analyzer/test/generated/type_system_base.dart
// (AbstractTypeSystemTest, TypeParsingScope),
// pkg/analyzer/test/generated/test_analysis_context.dart (TestAnalysisContext)

//! Test support: the Rust port of the analyzer's type system test helpers.
//!
//! [`TypeSystemTest`] is `AbstractTypeSystemTest`: it builds the mock SDK
//! (`dart:core`, `dart:async` of `mock_sdk_elements.dart`) from the same
//! spec strings, fills a [`TypeProvider`], builds test libraries
//! ([`TypeSystemTest::build_test_library`]) and parses types
//! ([`TypeSystemTest::parse_type`]), so ported tests read like the Dart
//! tests:
//!
//! ```
//! use dartr_typesystem::test_support::*;
//! let mut t = TypeSystemTest::new();
//! t.build_test_library(LibrarySpec {
//!     classes: vec![ClassSpec::new("class A<T>")],
//!     ..LibrarySpec::test()
//! });
//! let ts = t.type_system();
//! assert!(ts.is_subtype_of(t.parse_type("A<int>"), t.parse_type("A<num>")));
//! assert_eq!(t.display(t.parse_type("A<int>?")), "A<int>?");
//! ```
//!
//! All elements live in one cycle store that is read through
//! `Ctx { current: Some(&store) }` (the store is not frozen, so later test
//! libraries and the type parameters of parsed function types can be
//! added).

pub mod spec_parser;

use std::sync::Arc;

use dartr_element::{
    BoolSlot, ClassElement, ClassFragment, ConstructorElement, ConstructorFragment, Ctx,
    DisplayOptions, EId, ElementData, ElementFlags, ElementId, ElementStore, ElementType,
    EnumElement, EnumFragment, ExecutableElementData, ExecutableFragmentData, ExtensionTypeElement,
    ExtensionTypeFragment, FId, FeatureSet, FieldElement, FieldFragment, FnParam,
    FormalParameterElement, FormalParameterFragment, FragmentData, FragmentFlags, FragmentId,
    Generation, InterfaceElement, InterfaceElementData, InterfaceFragmentData, LibraryElement,
    LibraryFragment, LibraryLanguageVersion, Metadata, MethodElement, MethodFragment, MixinElement,
    MixinFragment, Name, NamedType, NoopSink, Nullability, OnceSlot, PropertyInducingElementData,
    PropertyInducingFragmentData, SourceRef, Tag, TopLevelFunctionElement,
    TopLevelFunctionFragment, TypeAliasElement, TypeAliasFragment, TypeId, TypeKind, TypeList,
    TypeParameterElement, TypeParameterFragment, TypeProvider, VarSlot, VariableElementData,
    VariableFragmentData, Version, WorldSnapshot,
};
use indexmap::IndexMap;

use crate::type_ext::TypeExt;
use crate::type_system::TypeSystem;
use spec_parser::{ParsedFormalParameter, ParsedType, ParsedTypeParameter, SpecParser};

// ------------------------------------------------------------------ specs

/// `ClassSpec`.
#[derive(Clone, Debug, Default)]
pub struct ClassSpec {
    pub header: String,
    pub constructors: Vec<String>,
    pub methods: Vec<String>,
}

impl ClassSpec {
    pub fn new(header: &str) -> ClassSpec {
        ClassSpec {
            header: header.into(),
            ..ClassSpec::default()
        }
    }

    /// `constructors: [ConstructorSpec(...), ...]`.
    pub fn constructors(mut self, headers: &[&str]) -> ClassSpec {
        self.constructors = strs(headers);
        self
    }

    /// `methods: [MethodSpec(...), ...]`.
    pub fn methods(mut self, headers: &[&str]) -> ClassSpec {
        self.methods = strs(headers);
        self
    }
}

/// `EnumSpec`.
#[derive(Clone, Debug, Default)]
pub struct EnumSpec {
    pub header: String,
    pub constants: Vec<String>,
}

impl EnumSpec {
    pub fn new(header: &str) -> EnumSpec {
        EnumSpec {
            header: header.into(),
            constants: Vec::new(),
        }
    }

    pub fn constants(mut self, names: &[&str]) -> EnumSpec {
        self.constants = strs(names);
        self
    }
}

/// `LibrarySpec`. The other declaration kinds (`ExtensionTypeSpec`,
/// `TopLevelFunctionSpec`, `MixinSpec`, `TypeAliasSpec`) have only a header,
/// so their lists hold the header strings.
#[derive(Clone, Debug, Default)]
pub struct LibrarySpec {
    pub uri: String,
    pub imports: Vec<String>,
    pub classes: Vec<ClassSpec>,
    pub enums: Vec<EnumSpec>,
    pub extension_types: Vec<String>,
    pub functions: Vec<String>,
    pub mixins: Vec<String>,
    pub type_aliases: Vec<String>,
}

impl LibrarySpec {
    /// The spec of `buildTestLibrary`: `package:test/test.dart`, importing
    /// `dart:core`.
    pub fn test() -> LibrarySpec {
        LibrarySpec {
            uri: TEST_LIBRARY_URI.into(),
            imports: vec!["dart:core".into()],
            ..LibrarySpec::default()
        }
    }
}

/// Spec string lists: `strs(&["typedef A = int"])`.
pub fn strs(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

const TEST_LIBRARY_URI: &str = "package:test/test.dart";

/// `_sdkSpec` of `mock_sdk_elements.dart`.
fn sdk_spec() -> Vec<LibrarySpec> {
    vec![
        LibrarySpec {
            uri: "dart:core".into(),
            classes: vec![
                ClassSpec::new("class bool extends Object").constructors(&[
                    "const factory fromEnvironment(String name, {bool defaultValue})",
                ]),
                ClassSpec::new("class double extends num"),
                ClassSpec::new("class int extends num").constructors(&[
                    "const factory fromEnvironment(String name, {int defaultValue})",
                ]),
                ClassSpec::new("class num extends Object implements Comparable<num>"),
                ClassSpec::new("abstract class Comparable<T>"),
                ClassSpec::new("abstract class Function extends Object"),
                ClassSpec::new("abstract class Iterable<E> extends Object"),
                ClassSpec::new("abstract class Iterator<E> extends Object"),
                ClassSpec::new("abstract class List<E> extends Object implements Iterable<E>"),
                ClassSpec::new("abstract class Map<K, V> extends Object"),
                ClassSpec::new("class Null extends Object"),
                ClassSpec::new("class Object")
                    .methods(&["String toString()", "bool operator ==(Object other)"]),
                ClassSpec::new("abstract class Record extends Object"),
                ClassSpec::new("abstract class Set<E> extends Object implements Iterable<E>"),
                ClassSpec::new("class String extends Object")
                    .constructors(&[
                        "const factory fromEnvironment(String name, {String defaultValue})",
                    ])
                    .methods(&["String toLowerCase()", "String operator +(String other)"]),
                ClassSpec::new("abstract class Symbol extends Object")
                    .constructors(&["const factory new(String name)"]),
                ClassSpec::new("abstract class Type extends Object"),
            ],
            ..LibrarySpec::default()
        },
        LibrarySpec {
            uri: "dart:async".into(),
            imports: vec!["dart:core".into()],
            classes: vec![
                ClassSpec::new("abstract class Future<T> extends Object"),
                ClassSpec::new("class FutureOr<T> extends Object"),
            ],
            ..LibrarySpec::default()
        },
    ]
}

// ------------------------------------------------------------------ scope

/// `_Scope`: the type parameters of the enclosing declarations (innermost
/// last) and the interfaces and type aliases of the libraries.
#[derive(Clone)]
struct Scope {
    type_parameters: Vec<(String, EId<TypeParameterElement>)>,
    interfaces: Arc<IndexMap<String, EId<InterfaceElement>>>,
    type_aliases: Arc<IndexMap<String, EId<TypeAliasElement>>>,
}

impl Scope {
    /// `_Scope.forLibraries(libraries, typeParameters)`.
    fn for_libraries(
        ctx: &Ctx<'_>,
        libraries: &[EId<LibraryElement>],
        type_parameters: &[EId<TypeParameterElement>],
    ) -> Scope {
        let mut interfaces = IndexMap::new();
        let mut type_aliases = IndexMap::new();
        for &library in libraries {
            let lib = ctx.get(library);
            let mut add = |e: ElementId| {
                if let Some(name) = ctx.element_name(e) {
                    interfaces.insert(name.to_string(), EId::<InterfaceElement>::from_raw(e));
                }
            };
            lib.classes.iter().for_each(|e| add(e.raw()));
            lib.enums.iter().for_each(|e| add(e.raw()));
            lib.extension_types.iter().for_each(|e| add(e.raw()));
            lib.mixins.iter().for_each(|e| add(e.raw()));
            for &e in &lib.type_aliases {
                if let Some(name) = ctx.element_name(e.raw()) {
                    type_aliases.insert(name.to_string(), e);
                }
            }
        }
        let mut scope = Scope {
            type_parameters: Vec::new(),
            interfaces: Arc::new(interfaces),
            type_aliases: Arc::new(type_aliases),
        };
        for &tp in type_parameters {
            scope.add_type_parameter(ctx, tp);
        }
        scope
    }

    fn add_type_parameter(&mut self, ctx: &Ctx<'_>, element: EId<TypeParameterElement>) {
        let name = ctx.element_name(element.raw()).unwrap_or("").to_string();
        self.type_parameters.push((name, element));
    }

    fn lookup_type_parameter(&self, name: &str) -> Option<EId<TypeParameterElement>> {
        self.type_parameters
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|&(_, e)| e)
    }
}

// ------------------------------------------------------------------ materialize

/// `_ParsedType.materialize(scope)`.
fn materialize(ctx: &Ctx<'_>, ty: &ParsedType, scope: &Scope) -> TypeId {
    match ty {
        ParsedType::Explicit(t) => *t,
        ParsedType::Named { name, args } => {
            if let Some(element) = scope.lookup_type_parameter(name) {
                assert!(args.is_empty());
                return ctx.type_parameter_type(element, Nullability::None);
            }
            let args: Vec<TypeId> = args.iter().map(|a| materialize(ctx, a, scope)).collect();
            if let Some(&element) = scope.type_aliases.get(name) {
                return ctx.instantiate_type_alias(element, &args, Nullability::None);
            }
            let Some(&element) = scope.interfaces.get(name) else {
                panic!("Unknown type: {name}");
            };
            ctx.interface_type(element, &args, Nullability::None)
        }
        ParsedType::Nullable(inner) => {
            let inner = materialize(ctx, inner, scope);
            ctx.with_nullability(inner, Nullability::Question)
        }
        ParsedType::Promoted {
            base,
            promoted_bound,
        } => {
            let base = materialize(ctx, base, scope);
            let TypeKind::TypeParameter {
                param, nullability, ..
            } = *ctx.ty(base)
            else {
                panic!("Cannot promote a non-type-parameter type");
            };
            let bound = materialize(ctx, promoted_bound, scope);
            ctx.promoted_type_parameter_type(param, nullability, Some(bound))
        }
        ParsedType::Function {
            type_parameters,
            formal_parameters,
            return_type,
        } => {
            let mut function_scope = scope.clone();
            let type_params =
                materialize_type_parameters_standalone(ctx, type_parameters, &mut function_scope);
            let params: Vec<FnParam> = formal_parameters
                .iter()
                .map(|p| materialize_fn_param(ctx, p, &function_scope))
                .collect();
            let ret = materialize(ctx, return_type, &function_scope);
            ctx.function_type(&type_params, &params, ret, Nullability::None, None)
        }
        ParsedType::Record {
            positional_fields,
            named_fields,
        } => {
            let positional: Vec<TypeId> = positional_fields
                .iter()
                .map(|f| materialize(ctx, f, scope))
                .collect();
            let named: Vec<NamedType> = named_fields
                .iter()
                .map(|(name, f)| NamedType {
                    name: ctx.name(name),
                    ty: materialize(ctx, f, scope),
                })
                .collect();
            ctx.record_type(&positional, &named, Nullability::None, None)
        }
    }
}

/// A parameter of a parsed function type. Dart creates a standalone
/// `FormalParameterElementImpl`; the Rust function type keeps the name,
/// kind, type and covariance (no element).
fn materialize_fn_param(ctx: &Ctx<'_>, p: &ParsedFormalParameter, scope: &Scope) -> FnParam {
    FnParam {
        name: p.name.as_deref().map(|n| ctx.name(n)),
        kind: p.kind,
        ty: materialize(ctx, &p.ty, scope),
        covariant: p.is_covariant,
        element: None,
    }
}

/// `_TypeParameterDeclarations.materializeStandalone(scope)`: creates the
/// elements, adds them to [scope], then resolves the bounds.
fn materialize_type_parameters_standalone(
    ctx: &Ctx<'_>,
    parsed: &[ParsedTypeParameter],
    scope: &mut Scope,
) -> Vec<EId<TypeParameterElement>> {
    let elements: Vec<EId<TypeParameterElement>> = parsed
        .iter()
        .map(|p| ctx.new_type_parameter(Some(ctx.name(&p.name)), p.variance, None))
        .collect();
    resolve_type_parameters(ctx, parsed, &elements, scope);
    elements
}

/// `_TypeParameterDeclarations.resolve(scope)`.
fn resolve_type_parameters(
    ctx: &Ctx<'_>,
    parsed: &[ParsedTypeParameter],
    elements: &[EId<TypeParameterElement>],
    scope: &mut Scope,
) {
    for &e in elements {
        scope.add_type_parameter(ctx, e);
    }
    for (p, &e) in parsed.iter().zip(elements) {
        if let Some(bound) = &p.bound {
            let bound = materialize(ctx, bound, scope);
            ctx.get(e).bound.set(Some(bound));
        }
    }
}

// ------------------------------------------------------------------ declarations

type TypeParameters = (Vec<ParsedTypeParameter>, Vec<EId<TypeParameterElement>>);

/// The elements that phase 1 created and the parsed data that phase 2
/// resolves.
enum Declaration {
    Class {
        element: EId<ClassElement>,
        type_parameters: TypeParameters,
        supertype: ParsedType,
        mixins: Vec<ParsedType>,
        interfaces: Vec<ParsedType>,
        executables: Vec<Executable>,
    },
    Enum {
        element: EId<EnumElement>,
        mixins: Vec<ParsedType>,
        interfaces: Vec<ParsedType>,
    },
    ExtensionType {
        element: EId<ExtensionTypeElement>,
        field: EId<FieldElement>,
        type_parameters: TypeParameters,
        representation_type: ParsedType,
        interfaces: Vec<ParsedType>,
    },
    Mixin {
        element: EId<MixinElement>,
        type_parameters: TypeParameters,
        constraints: Vec<ParsedType>,
        interfaces: Vec<ParsedType>,
    },
    TypeAlias {
        element: EId<TypeAliasElement>,
        type_parameters: TypeParameters,
        aliased_type: ParsedType,
    },
    Function(Executable),
}

/// A method, constructor or top-level function to resolve.
struct Executable {
    element: ElementId,
    type_parameters: TypeParameters,
    formal_parameters: Vec<(ParsedFormalParameter, EId<FormalParameterElement>)>,
    return_type: Option<ParsedType>,
}

type FormalParameters = (
    Vec<FId<FormalParameterFragment>>,
    Vec<(ParsedFormalParameter, EId<FormalParameterElement>)>,
);

/// Phase 1 (`createElement`): needs `&mut ElementStore` to fill the child
/// lists.
struct Creator<'s> {
    store: &'s mut ElementStore,
    generation: &'s Generation,
    library: EId<LibraryElement>,
    unit: FId<LibraryFragment>,
}

impl Creator<'_> {
    fn name(&self, text: &str) -> Name {
        self.generation.names.intern(text)
    }

    fn element_data(
        &self,
        name: Option<&str>,
        first_fragment: FragmentId,
        enclosing: ElementId,
    ) -> ElementData {
        let mut data = ElementData::new(name.map(|n| self.name(n)), first_fragment);
        data.library = Some(self.library);
        data.enclosing = Some(enclosing);
        data
    }

    fn fragment_data(&self, name: Option<&str>, enclosing: FragmentId) -> FragmentData {
        let mut f = FragmentData::new(name.map(|n| self.name(n)), Some(0));
        f.enclosing_fragment = Some(enclosing);
        f
    }

    /// `_TypeParameterDeclarations.createFragments` + `createElements`.
    fn type_parameters(
        &mut self,
        parsed: &[ParsedTypeParameter],
        enclosing: ElementId,
        enclosing_fragment: FragmentId,
    ) -> (
        Vec<FId<TypeParameterFragment>>,
        Vec<EId<TypeParameterElement>>,
    ) {
        let mut fragments = Vec::new();
        let mut elements = Vec::new();
        for p in parsed {
            let fragment =
                self.store
                    .add_fragment::<TypeParameterFragment>(TypeParameterFragment {
                        fragment: self.fragment_data(Some(&p.name), enclosing_fragment),
                    });
            let mut element = TypeParameterElement::new(self.element_data(
                Some(&p.name),
                fragment.raw(),
                enclosing,
            ));
            element.variance = p.variance;
            let id: EId<TypeParameterElement> = self.store.add(element);
            self.store.fragment(fragment).element.set_once(id.raw());
            fragments.push(fragment);
            elements.push(id);
        }
        (fragments, elements)
    }

    /// `_FormalParameterDeclarations.createFragments` + `createElements`.
    fn formal_parameters(
        &mut self,
        parsed: &[ParsedFormalParameter],
        enclosing: ElementId,
        enclosing_fragment: FragmentId,
    ) -> FormalParameters {
        let mut fragments = Vec::new();
        let mut elements = Vec::new();
        for p in parsed {
            let fd = self.fragment_data(p.name.as_deref(), enclosing_fragment);
            fd.flags.set(
                FragmentFlags::FORMAL_PARAMETER_FRAGMENT_IS_EXPLICITLY_COVARIANT,
                p.is_covariant,
            );
            let fragment =
                self.store
                    .add_fragment::<FormalParameterFragment>(FormalParameterFragment {
                        variable: VariableFragmentData::new(fd),
                        parameter_kind: p.kind,
                        private_name: None,
                    });
            let data = self.element_data(p.name.as_deref(), fragment.raw(), enclosing);
            data.flags.set(
                ElementFlags::FORMAL_PARAMETER_ELEMENT_IS_COVARIANT,
                p.is_covariant,
            );
            let id: EId<FormalParameterElement> = self.store.add(FormalParameterElement {
                variable: VariableElementData::new(data),
                kind: p.kind,
                type_: VarSlot::with(TypeId::INVALID),
                base_formal_parameter: None,
                field: VarSlot::new(),
            });
            self.store.fragment(fragment).element.set_once(id.raw());
            fragments.push(fragment);
            elements.push((p.clone(), id));
        }
        (fragments, elements)
    }

    fn class(&mut self, spec: &ClassSpec) -> Declaration {
        let header = SpecParser::parse_class_header(&spec.header);
        let fd = self.fragment_data(Some(&header.name), self.unit.raw());
        fd.flags.set(
            FragmentFlags::CLASS_FRAGMENT_IS_ABSTRACT,
            header.is_abstract,
        );
        fd.flags
            .set(FragmentFlags::CLASS_FRAGMENT_IS_SEALED, header.is_sealed);
        let fragment = self.store.add_fragment::<ClassFragment>(ClassFragment {
            interface: InterfaceFragmentData::new(fd),
        });
        let data = self.element_data(Some(&header.name), fragment.raw(), self.library.raw());
        data.flags
            .set(ElementFlags::CLASS_ELEMENT_IS_ABSTRACT, header.is_abstract);
        let element: EId<ClassElement> = self.store.add(ClassElement {
            interface: InterfaceElementData::new(data),
        });
        self.store
            .fragment(fragment)
            .element
            .set_once(element.raw());

        let (tp_fragments, tp_elements) =
            self.type_parameters(&header.type_parameters, element.raw(), fragment.raw());
        self.store.fragment_mut(fragment).type_params = tp_fragments;
        self.store.get_mut(element).type_params = tp_elements.clone();

        let mut executables = Vec::new();
        for c in &spec.constructors {
            executables.push(self.constructor(element, fragment, c));
        }
        for m in &spec.methods {
            executables.push(self.method(element, fragment, m));
        }

        self.store.get_mut(self.library).classes.push(element);
        self.store.fragment_mut(self.unit).classes.push(fragment);
        Declaration::Class {
            element,
            type_parameters: (header.type_parameters, tp_elements),
            supertype: header.supertype,
            mixins: header.mixins,
            interfaces: header.interfaces,
            executables,
        }
    }

    fn constructor(
        &mut self,
        class: EId<ClassElement>,
        class_fragment: FId<ClassFragment>,
        spec: &str,
    ) -> Executable {
        let header = SpecParser::parse_constructor_header(spec);
        let fd = self.fragment_data(Some(&header.name), class_fragment.raw());
        fd.flags.set(
            FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_ORIGIN_DECLARATION,
            true,
        );
        fd.flags.set(
            FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST,
            header.is_const,
        );
        fd.flags.set(
            FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY,
            header.is_factory,
        );
        let fragment = self
            .store
            .add_fragment::<ConstructorFragment>(ConstructorFragment {
                executable: ExecutableFragmentData::new(fd),
                constant_initializers: OnceSlot::new(),
                new_keyword_offset: None,
                factory_keyword_offset: None,
                type_name: None,
                type_name_offset: None,
                period_offset: None,
                name_end: None,
                this_keyword_offset: None,
            });
        let data = self.element_data(Some(&header.name), fragment.raw(), class.raw());
        let element: EId<ConstructorElement> = self.store.add(ConstructorElement {
            executable: ExecutableElementData::new(data),
            redirected_constructor: VarSlot::new(),
            super_constructor: VarSlot::new(),
        });
        self.store
            .fragment(fragment)
            .element
            .set_once(element.raw());
        let (p_fragments, params) =
            self.formal_parameters(&header.formal_parameters, element.raw(), fragment.raw());
        self.store.fragment_mut(fragment).formal_params = p_fragments;
        self.store.get_mut(element).formal_params = params.iter().map(|(_, e)| *e).collect();
        self.store.get_mut(class).constructors.push(element);
        self.store
            .fragment_mut(class_fragment)
            .constructors
            .push(fragment);
        Executable {
            element: element.raw(),
            type_parameters: (Vec::new(), Vec::new()),
            formal_parameters: params,
            return_type: None,
        }
    }

    fn method(
        &mut self,
        class: EId<ClassElement>,
        class_fragment: FId<ClassFragment>,
        spec: &str,
    ) -> Executable {
        let header = SpecParser::parse_method_header(spec);
        let fd = self.fragment_data(Some(&header.name), class_fragment.raw());
        let fragment = self.store.add_fragment::<MethodFragment>(MethodFragment {
            executable: ExecutableFragmentData::new(fd),
        });
        let data = self.element_data(Some(&header.name), fragment.raw(), class.raw());
        let element: EId<MethodElement> = self.store.add(MethodElement {
            executable: ExecutableElementData::new(data),
            is_operator_equal_with_parameter_type_from_object: BoolSlot::new(false),
            type_inference_error: OnceSlot::new(),
        });
        self.store
            .fragment(fragment)
            .element
            .set_once(element.raw());
        let (tp_fragments, tps) =
            self.type_parameters(&header.type_parameters, element.raw(), fragment.raw());
        let (p_fragments, params) =
            self.formal_parameters(&header.formal_parameters, element.raw(), fragment.raw());
        {
            let f = self.store.fragment_mut(fragment);
            f.type_params = tp_fragments;
            f.formal_params = p_fragments;
        }
        {
            let e = self.store.get_mut(element);
            e.type_params = tps.clone();
            e.formal_params = params.iter().map(|(_, e)| *e).collect();
        }
        self.store.get_mut(class).methods.push(element);
        self.store
            .fragment_mut(class_fragment)
            .methods
            .push(fragment);
        Executable {
            element: element.raw(),
            type_parameters: (header.type_parameters, tps),
            formal_parameters: params,
            return_type: Some(header.return_type),
        }
    }

    fn top_level_function(&mut self, spec: &str) -> Declaration {
        let header = SpecParser::parse_top_level_function_header(spec);
        let fd = self.fragment_data(Some(&header.name), self.unit.raw());
        let fragment =
            self.store
                .add_fragment::<TopLevelFunctionFragment>(TopLevelFunctionFragment {
                    executable: ExecutableFragmentData::new(fd),
                });
        let data = self.element_data(Some(&header.name), fragment.raw(), self.library.raw());
        let element: EId<TopLevelFunctionElement> = self.store.add(TopLevelFunctionElement {
            executable: ExecutableElementData::new(data),
        });
        self.store
            .fragment(fragment)
            .element
            .set_once(element.raw());
        let (tp_fragments, tps) =
            self.type_parameters(&header.type_parameters, element.raw(), fragment.raw());
        let (p_fragments, params) =
            self.formal_parameters(&header.formal_parameters, element.raw(), fragment.raw());
        {
            let f = self.store.fragment_mut(fragment);
            f.type_params = tp_fragments;
            f.formal_params = p_fragments;
        }
        {
            let e = self.store.get_mut(element);
            e.type_params = tps.clone();
            e.formal_params = params.iter().map(|(_, e)| *e).collect();
        }
        self.store
            .get_mut(self.library)
            .top_level_functions
            .push(element);
        self.store.fragment_mut(self.unit).functions.push(fragment);
        Declaration::Function(Executable {
            element: element.raw(),
            type_parameters: (header.type_parameters, tps),
            formal_parameters: params,
            return_type: Some(header.return_type),
        })
    }

    fn field(
        &mut self,
        name: &str,
        enclosing: ElementId,
        enclosing_fragment: FragmentId,
    ) -> (EId<FieldElement>, FId<FieldFragment>) {
        let fd = self.fragment_data(Some(name), enclosing_fragment);
        let fragment = self.store.add_fragment::<FieldFragment>(FieldFragment {
            property: PropertyInducingFragmentData {
                variable: VariableFragmentData::new(fd),
                induced_getter: None,
                induced_setter: None,
            },
            inherits_covariant: BoolSlot::new(false),
        });
        let data = self.element_data(Some(name), fragment.raw(), enclosing);
        let element: EId<FieldElement> = self.store.add(FieldElement {
            property: PropertyInducingElementData::new(data),
        });
        self.store
            .fragment(fragment)
            .element
            .set_once(element.raw());
        (element, fragment)
    }

    fn enum_(&mut self, spec: &EnumSpec) -> Declaration {
        let header = SpecParser::parse_enum_header(&spec.header);
        let fd = self.fragment_data(Some(&header.name), self.unit.raw());
        let fragment = self.store.add_fragment::<EnumFragment>(EnumFragment {
            interface: InterfaceFragmentData::new(fd),
        });
        let data = self.element_data(Some(&header.name), fragment.raw(), self.library.raw());
        let element: EId<EnumElement> = self.store.add(EnumElement {
            interface: InterfaceElementData::new(data),
        });
        self.store
            .fragment(fragment)
            .element
            .set_once(element.raw());
        for name in &spec.constants {
            let (field, field_fragment) = self.field(name, element.raw(), fragment.raw());
            let ff = self.store.fragment(field_fragment);
            ff.flags
                .set(FragmentFlags::FIELD_FRAGMENT_IS_ENUM_CONSTANT, true);
            ff.flags.set(
                FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_DECLARATION,
                true,
            );
            self.store.get_mut(element).fields.push(field);
            self.store
                .fragment_mut(fragment)
                .fields
                .push(field_fragment);
        }
        self.store.get_mut(self.library).enums.push(element);
        self.store.fragment_mut(self.unit).enums.push(fragment);
        Declaration::Enum {
            element,
            mixins: header.mixins,
            interfaces: header.interfaces,
        }
    }

    fn extension_type(&mut self, spec: &str) -> Declaration {
        let header = SpecParser::parse_extension_type_header(spec);
        let fd = self.fragment_data(Some(&header.name), self.unit.raw());
        let fragment = self
            .store
            .add_fragment::<ExtensionTypeFragment>(ExtensionTypeFragment {
                interface: InterfaceFragmentData::new(fd),
            });
        let data = self.element_data(Some(&header.name), fragment.raw(), self.library.raw());
        let element: EId<ExtensionTypeElement> = self.store.add(ExtensionTypeElement {
            interface: InterfaceElementData::new(data),
            has_representation_self_reference: BoolSlot::new(false),
            has_implements_self_reference: BoolSlot::new(false),
            type_erasure: OnceSlot::new(),
        });
        self.store
            .fragment(fragment)
            .element
            .set_once(element.raw());
        let (tp_fragments, tps) =
            self.type_parameters(&header.type_parameters, element.raw(), fragment.raw());
        self.store.fragment_mut(fragment).type_params = tp_fragments;
        self.store.get_mut(element).type_params = tps.clone();
        let (field, field_fragment) =
            self.field(&header.representation_name, element.raw(), fragment.raw());
        self.store.fragment(field_fragment).flags.set(
            FragmentFlags::FIELD_FRAGMENT_IS_ORIGIN_DECLARING_FORMAL_PARAMETER,
            true,
        );
        self.store.get_mut(element).fields.push(field);
        self.store
            .fragment_mut(fragment)
            .fields
            .push(field_fragment);
        self.store
            .get_mut(self.library)
            .extension_types
            .push(element);
        self.store
            .fragment_mut(self.unit)
            .extension_types
            .push(fragment);
        Declaration::ExtensionType {
            element,
            field,
            type_parameters: (header.type_parameters, tps),
            representation_type: header.representation_type,
            interfaces: header.interfaces,
        }
    }

    fn mixin(&mut self, spec: &str) -> Declaration {
        let header = SpecParser::parse_mixin_header(spec);
        let fd = self.fragment_data(Some(&header.name), self.unit.raw());
        let fragment = self.store.add_fragment::<MixinFragment>(MixinFragment {
            interface: InterfaceFragmentData::new(fd),
            super_invoked_names: OnceSlot::new(),
        });
        let data = self.element_data(Some(&header.name), fragment.raw(), self.library.raw());
        let element: EId<MixinElement> = self.store.add(MixinElement {
            interface: InterfaceElementData::new(data),
            superclass_constraints: VarSlot::new(),
        });
        self.store
            .fragment(fragment)
            .element
            .set_once(element.raw());
        let (tp_fragments, tps) =
            self.type_parameters(&header.type_parameters, element.raw(), fragment.raw());
        self.store.fragment_mut(fragment).type_params = tp_fragments;
        self.store.get_mut(element).type_params = tps.clone();
        self.store.get_mut(self.library).mixins.push(element);
        self.store.fragment_mut(self.unit).mixins.push(fragment);
        Declaration::Mixin {
            element,
            type_parameters: (header.type_parameters, tps),
            constraints: header.constraints,
            interfaces: header.interfaces,
        }
    }

    fn type_alias(&mut self, spec: &str) -> Declaration {
        let header = SpecParser::parse_type_alias_header(spec);
        let fd = self.fragment_data(Some(&header.name), self.unit.raw());
        let fragment = self
            .store
            .add_fragment::<TypeAliasFragment>(TypeAliasFragment {
                fragment: fd,
                type_params: Vec::new(),
                has_self_reference: BoolSlot::new(false),
            });
        let data = self.element_data(Some(&header.name), fragment.raw(), self.library.raw());
        let element: EId<TypeAliasElement> = self.store.add(TypeAliasElement {
            element: data,
            type_params: Vec::new(),
            aliased_type: VarSlot::new(),
        });
        self.store
            .fragment(fragment)
            .element
            .set_once(element.raw());
        let (tp_fragments, tps) =
            self.type_parameters(&header.type_parameters, element.raw(), fragment.raw());
        self.store.fragment_mut(fragment).type_params = tp_fragments;
        self.store.get_mut(element).type_params = tps.clone();
        self.store.get_mut(self.library).type_aliases.push(element);
        self.store
            .fragment_mut(self.unit)
            .type_aliases
            .push(fragment);
        Declaration::TypeAlias {
            element,
            type_parameters: (header.type_parameters, tps),
            aliased_type: header.aliased_type,
        }
    }
}

fn materialize_list(ctx: &Ctx<'_>, types: &[ParsedType], scope: &Scope) -> TypeList {
    let list: Vec<TypeId> = types.iter().map(|t| materialize(ctx, t, scope)).collect();
    ctx.intern_list(&list)
}

/// Phase 2: `resolve(scope)` of each declaration.
fn resolve_declaration(ctx: &Ctx<'_>, declaration: &Declaration, library_scope: &Scope) {
    match declaration {
        Declaration::Class {
            element,
            type_parameters,
            supertype,
            mixins,
            interfaces,
            executables,
        } => {
            let mut scope = library_scope.clone();
            resolve_type_parameters(ctx, &type_parameters.0, &type_parameters.1, &mut scope);
            let data = ctx.get(*element);
            data.supertype
                .set(Some(materialize(ctx, supertype, &scope)));
            data.mixins.set(Some(materialize_list(ctx, mixins, &scope)));
            data.interfaces
                .set(Some(materialize_list(ctx, interfaces, &scope)));
            for e in executables {
                resolve_executable(ctx, e, &scope);
            }
        }
        Declaration::Enum {
            element,
            mixins,
            interfaces,
        } => {
            let data = ctx.get(*element);
            data.mixins
                .set(Some(materialize_list(ctx, mixins, library_scope)));
            data.interfaces
                .set(Some(materialize_list(ctx, interfaces, library_scope)));
        }
        Declaration::ExtensionType {
            element,
            field,
            type_parameters,
            representation_type,
            interfaces,
        } => {
            let mut scope = library_scope.clone();
            resolve_type_parameters(ctx, &type_parameters.0, &type_parameters.1, &mut scope);
            let representation_type = materialize(ctx, representation_type, &scope);
            let data = ctx.get(*element);
            data.type_erasure.set_once(representation_type);
            ctx.get(*field).type_.set(Some(representation_type));
            data.interfaces
                .set(Some(materialize_list(ctx, interfaces, &scope)));
        }
        Declaration::Mixin {
            element,
            type_parameters,
            constraints,
            interfaces,
        } => {
            let mut scope = library_scope.clone();
            resolve_type_parameters(ctx, &type_parameters.0, &type_parameters.1, &mut scope);
            let data = ctx.get(*element);
            data.superclass_constraints
                .set(Some(materialize_list(ctx, constraints, &scope)));
            data.interfaces
                .set(Some(materialize_list(ctx, interfaces, &scope)));
        }
        Declaration::TypeAlias {
            element,
            type_parameters,
            aliased_type,
        } => {
            let mut scope = library_scope.clone();
            resolve_type_parameters(ctx, &type_parameters.0, &type_parameters.1, &mut scope);
            ctx.get(*element)
                .aliased_type
                .set(Some(materialize(ctx, aliased_type, &scope)));
        }
        Declaration::Function(e) => resolve_executable(ctx, e, library_scope),
    }
}

fn resolve_executable(ctx: &Ctx<'_>, e: &Executable, scope: &Scope) {
    let mut scope = scope.clone();
    resolve_type_parameters(ctx, &e.type_parameters.0, &e.type_parameters.1, &mut scope);
    for (parsed, element) in &e.formal_parameters {
        ctx.get(*element)
            .type_
            .set(Some(materialize(ctx, &parsed.ty, &scope)));
    }
    if let Some(return_type) = &e.return_type {
        let executable = ctx.executable(e.element.cast().unwrap());
        executable
            .return_type
            .set(Some(materialize(ctx, return_type, &scope)));
    }
}

// ------------------------------------------------------------------ test world

/// `AbstractTypeSystemTest` (with `TestAnalysisContext`): the mock SDK, an
/// optional test library, and the type parsing helpers.
pub struct TypeSystemTest {
    pub world: WorldSnapshot,
    pub store: ElementStore,
    pub tp: TypeProvider,
    pub features: FeatureSet,
    pub core_library: EId<LibraryElement>,
    pub async_library: EId<LibraryElement>,
    pub test_library: Option<EId<LibraryElement>>,
    built: IndexMap<String, EId<LibraryElement>>,
}

impl Default for TypeSystemTest {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeSystemTest {
    /// `setUp()`: builds the mock SDK and the type provider.
    pub fn new() -> TypeSystemTest {
        let generation = Arc::new(Generation::new(0));
        let store = generation.new_cycle_store();
        let world = WorldSnapshot::new(generation);
        let placeholder = EId::from_raw(ElementId::DYNAMIC);
        let mut t = TypeSystemTest {
            world,
            store,
            tp: TypeProvider::default(),
            features: FeatureSet::default(),
            core_library: placeholder,
            async_library: placeholder,
            test_library: None,
            built: IndexMap::new(),
        };
        let libraries = t.build_libraries(&sdk_spec());
        t.core_library = libraries["dart:core"];
        t.async_library = libraries["dart:async"];
        t.fill_type_provider();
        t
    }

    /// The lookup context of the tests.
    pub fn ctx(&self) -> Ctx<'_> {
        Ctx {
            world: &self.world,
            current: Some(&self.store),
            local: None,
            tp: &self.tp,
            features: &self.features,
            req: &NoopSink,
        }
    }

    /// `typeSystem`.
    pub fn type_system(&self) -> TypeSystem<'_> {
        TypeSystem::new(self.ctx())
    }

    /// `typeProvider`.
    pub fn type_provider(&self) -> &TypeProvider {
        &self.tp
    }

    /// `type.getDisplayString()`.
    pub fn display(&self, t: TypeId) -> String {
        dartr_element::type_display_string_with(&self.ctx(), t, DisplayOptions::default())
    }

    /// `type.getDisplayString(preferTypeAlias: true)`.
    pub fn display_alias(&self, t: TypeId) -> String {
        dartr_element::type_display_string_with(
            &self.ctx(),
            t,
            DisplayOptions {
                prefer_type_alias: true,
                ..DisplayOptions::default()
            },
        )
    }

    /// `buildLibrariesFromSpec` / `buildLibraries(specs)`: phase 1 creates
    /// the elements of all [specs], phase 2 resolves their types.
    pub fn build_libraries(
        &mut self,
        specs: &[LibrarySpec],
    ) -> IndexMap<String, EId<LibraryElement>> {
        let generation = self.world.generation.clone();
        let mut created = Vec::new();
        for spec in specs {
            let (library, unit) = self.add_library(spec);
            let mut creator = Creator {
                store: &mut self.store,
                generation: &generation,
                library,
                unit,
            };
            let mut declarations = Vec::new();
            for c in &spec.classes {
                declarations.push(creator.class(c));
            }
            for m in &spec.mixins {
                declarations.push(creator.mixin(m));
            }
            for e in &spec.enums {
                declarations.push(creator.enum_(e));
            }
            for e in &spec.extension_types {
                declarations.push(creator.extension_type(e));
            }
            for a in &spec.type_aliases {
                declarations.push(creator.type_alias(a));
            }
            for f in &spec.functions {
                declarations.push(creator.top_level_function(f));
            }
            self.built.insert(spec.uri.clone(), library);
            created.push((spec, library, declarations));
        }

        let ctx = self.ctx();
        for (spec, library, declarations) in &created {
            // `_scopeFor(spec)`: imported libraries, then the library itself.
            let mut libraries = Vec::new();
            for import in &spec.imports {
                if let Some(&l) = self.built.get(import) {
                    libraries.push(l);
                }
            }
            libraries.push(*library);
            let scope = Scope::for_libraries(&ctx, &libraries, &[]);
            // Dart order: type aliases, extension types, mixins, enums,
            // classes, functions.
            let order = |d: &Declaration| match d {
                Declaration::TypeAlias { .. } => 0,
                Declaration::ExtensionType { .. } => 1,
                Declaration::Mixin { .. } => 2,
                Declaration::Enum { .. } => 3,
                Declaration::Class { .. } => 4,
                Declaration::Function(_) => 5,
            };
            let mut sorted: Vec<&Declaration> = declarations.iter().collect();
            sorted.sort_by_key(|d| order(d));
            for d in sorted {
                resolve_declaration(&ctx, d, &scope);
            }
        }
        created
            .iter()
            .map(|(spec, library, _)| (spec.uri.clone(), *library))
            .collect()
    }

    /// `buildTestLibrary(...)`: builds the library of [spec] (use
    /// `..LibrarySpec::test()`) and makes it the test library.
    pub fn build_test_library(&mut self, spec: LibrarySpec) -> EId<LibraryElement> {
        let uri = spec.uri.clone();
        let library = self.build_libraries(&[spec])[&uri];
        self.test_library = Some(library);
        library
    }

    fn add_library(&mut self, spec: &LibrarySpec) -> (EId<LibraryElement>, FId<LibraryFragment>) {
        let names = &self.world.generation.names;
        let index = self.store.fragments.units.len() as u32;
        let unit =
            FId::<LibraryFragment>::from_raw(FragmentId::new(self.store.id, Tag::Library, index));
        // `LibraryElementImpl(..., libraryUriStr.replaceAll(':', '.'), ...)`.
        let name = spec.uri.replace(':', ".");
        let element = ElementData::new(Some(names.intern(&name)), unit.raw());
        let library: EId<LibraryElement> = self.store.add(LibraryElement {
            element,
            metadata: Metadata::default(),
            documentation_comment: None,
            language_version: LibraryLanguageVersion {
                package: Version {
                    major: 3,
                    minor: 13,
                },
                override_: None,
            },
            feature_set: FeatureSet::default(),
            entry_point: OnceSlot::new(),
            load_library_function: OnceSlot::new(),
            name_offset: 0,
            name_length: 0,
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
        });
        self.store.get_mut(library).element.library = Some(library);
        let added = self
            .store
            .add_fragment::<LibraryFragment>(LibraryFragment::new(
                FragmentData::new(None, None),
                SourceRef {
                    path: Arc::from(test_uri_path(&spec.uri)),
                    uri: Arc::from(spec.uri.as_str()),
                },
                library,
            ));
        assert_eq!(added, unit);
        self.store.fragment(unit).element.set_once(library.raw());
        (library, unit)
    }

    /// `TestAnalysisContext`: `TypeProviderImpl(coreLibrary, asyncLibrary)`
    /// with every slot that the mock SDK can fill.
    fn fill_type_provider(&mut self) {
        let ctx = self.ctx();
        let class = |library: EId<LibraryElement>, name: &str| -> Option<EId<ClassElement>> {
            ctx.get(library)
                .classes
                .iter()
                .copied()
                .find(|&c| ctx.element_name(c.raw()) == Some(name))
        };
        let tp = &self.tp;
        let core = self.core_library;
        let async_ = self.async_library;
        tp.core_library.set_once(core);
        tp.async_library.set_once(async_);
        tp.enum_element.set_once(class(core, "Enum"));
        let set = |slot: &OnceSlot<EId<ClassElement>>, library, name| {
            if let Some(c) = class(library, name) {
                slot.set_once(c);
            }
        };
        set(&tp.bool_element, core, "bool");
        set(&tp.deprecated_element, core, "Deprecated");
        set(&tp.double_element, core, "double");
        set(&tp.function_element, core, "Function");
        set(&tp.future_element, async_, "Future");
        set(&tp.future_or_element, async_, "FutureOr");
        set(&tp.int_element, core, "int");
        set(&tp.iterable_element, core, "Iterable");
        set(&tp.list_element, core, "List");
        set(&tp.map_element, core, "Map");
        set(&tp.null_element, core, "Null");
        set(&tp.num_element, core, "num");
        set(&tp.object_element, core, "Object");
        set(&tp.record_element, core, "Record");
        set(&tp.set_element, core, "Set");
        set(&tp.stack_trace_element, core, "StackTrace");
        set(&tp.stream_element, async_, "Stream");
        set(&tp.string_element, core, "String");
        set(&tp.symbol_element, core, "Symbol");
        set(&tp.type_element, core, "Type");

        let instantiate = |e: EId<ClassElement>, args: &[TypeId], n: Nullability| {
            ctx.interface_type(e.upcast(), args, n)
        };
        let none = Nullability::None;
        let question = Nullability::Question;
        let object = instantiate(tp.object_element(), &[], none);
        let null = instantiate(tp.null_element(), &[], none);
        tp.enum_type.set_once(None);
        tp.bool_type
            .set_once(instantiate(tp.bool_element(), &[], none));
        tp.double_type
            .set_once(instantiate(tp.double_element(), &[], none));
        tp.double_type_question
            .set_once(instantiate(tp.double_element(), &[], question));
        tp.function_type
            .set_once(instantiate(tp.function_element(), &[], none));
        tp.future_dynamic_type
            .set_once(instantiate(tp.future_element(), &[TypeId::DYNAMIC], none));
        tp.future_null_type
            .set_once(instantiate(tp.future_element(), &[null], none));
        tp.future_or_null_type
            .set_once(instantiate(tp.future_or_element(), &[null], none));
        tp.int_type
            .set_once(instantiate(tp.int_element(), &[], none));
        tp.int_type_question
            .set_once(instantiate(tp.int_element(), &[], question));
        tp.iterable_dynamic_type.set_once(instantiate(
            tp.iterable_element(),
            &[TypeId::DYNAMIC],
            none,
        ));
        tp.iterable_object_type
            .set_once(instantiate(tp.iterable_element(), &[object], none));
        tp.map_object_object_type
            .set_once(instantiate(tp.map_element(), &[object, object], none));
        tp.null_type.set_once(null);
        tp.num_type
            .set_once(instantiate(tp.num_element(), &[], none));
        tp.num_type_question
            .set_once(instantiate(tp.num_element(), &[], question));
        tp.object_type.set_once(object);
        tp.object_question_type
            .set_once(instantiate(tp.object_element(), &[], question));
        tp.record_type
            .set_once(instantiate(tp.record_element(), &[], none));
        tp.string_type
            .set_once(instantiate(tp.string_element(), &[], none));
        tp.symbol_type
            .set_once(instantiate(tp.symbol_element(), &[], none));
        tp.type_type
            .set_once(instantiate(tp.type_element(), &[], none));
    }

    // ---------------------------------------------------------- lookups

    fn find<T: ElementType + ?Sized>(
        &self,
        list: impl Fn(&LibraryElement) -> Vec<ElementId>,
        name: &str,
    ) -> EId<T> {
        let ctx = self.ctx();
        let library = ctx.get(self.test_library.expect("no test library"));
        list(library)
            .into_iter()
            .find(|&e| ctx.element_name(e) == Some(name))
            .and_then(|e| e.cast::<T>())
            .unwrap_or_else(|| panic!("no element {name}"))
    }

    /// `classElement(name)`.
    pub fn class_element(&self, name: &str) -> EId<ClassElement> {
        self.find(|l| l.classes.iter().map(|e| e.raw()).collect(), name)
    }

    /// `enumElement(name)`.
    pub fn enum_element(&self, name: &str) -> EId<EnumElement> {
        self.find(|l| l.enums.iter().map(|e| e.raw()).collect(), name)
    }

    /// `extensionTypeElement(name)`.
    pub fn extension_type_element(&self, name: &str) -> EId<ExtensionTypeElement> {
        self.find(
            |l| l.extension_types.iter().map(|e| e.raw()).collect(),
            name,
        )
    }

    /// `mixinElement(name)`.
    pub fn mixin_element(&self, name: &str) -> EId<MixinElement> {
        self.find(|l| l.mixins.iter().map(|e| e.raw()).collect(), name)
    }

    /// `typeAliasElement(name)`.
    pub fn type_alias_element(&self, name: &str) -> EId<TypeAliasElement> {
        self.find(|l| l.type_aliases.iter().map(|e| e.raw()).collect(), name)
    }

    /// `testLibrary.getTopLevelFunction(name)`.
    pub fn top_level_function(&self, name: &str) -> EId<TopLevelFunctionElement> {
        self.find(
            |l| l.top_level_functions.iter().map(|e| e.raw()).collect(),
            name,
        )
    }

    /// `element.getMethod(name)` of an interface element.
    pub fn method(&self, element: EId<InterfaceElement>, name: &str) -> EId<MethodElement> {
        let ctx = self.ctx();
        ctx.interface(element)
            .methods
            .iter()
            .copied()
            .find(|&m| ctx.element_name(m.raw()) == Some(name))
            .unwrap_or_else(|| panic!("no method {name}"))
    }

    /// `element.getNamedConstructor(name)`.
    pub fn constructor(
        &self,
        element: EId<InterfaceElement>,
        name: &str,
    ) -> EId<ConstructorElement> {
        let ctx = self.ctx();
        ctx.interface(element)
            .constructors
            .iter()
            .copied()
            .find(|&m| ctx.element_name(m.raw()) == Some(name))
            .unwrap_or_else(|| panic!("no constructor {name}"))
    }

    // ---------------------------------------------------------- parsing

    /// The libraries of `_typeParsingScope`: core, async, and the test
    /// library when it exists.
    pub fn default_libraries(&self) -> Vec<EId<LibraryElement>> {
        let mut libraries = vec![self.core_library, self.async_library];
        if let Some(l) = self.test_library {
            libraries.push(l);
        }
        libraries
    }

    /// `_typeParsingScope()`.
    pub fn scope(&self) -> TypeParsingScope<'_> {
        TypeParsingScope {
            test: self,
            libraries: self.default_libraries(),
            type_parameters: Vec::new(),
        }
    }

    /// `parseType(input)`.
    pub fn parse_type(&self, input: &str) -> TypeId {
        self.scope().parse_type(input)
    }

    /// `parseFunctionType(input)`.
    pub fn parse_function_type(&self, input: &str) -> TypeId {
        self.scope().parse_function_type(input)
    }

    /// `parseInterfaceType(input)`.
    pub fn parse_interface_type(&self, input: &str) -> TypeId {
        self.scope().parse_interface_type(input)
    }

    /// `parseRecordType(input)`.
    pub fn parse_record_type(&self, input: &str) -> TypeId {
        self.scope().parse_record_type(input)
    }

    /// `parseTypeParameterType(input)`.
    pub fn parse_type_parameter_type(&self, input: &str) -> TypeId {
        self.scope().parse_type_parameter_type(input)
    }

    /// `withTypeParameterScope(spec, operation)`.
    pub fn with_type_parameter_scope<R>(
        &self,
        spec: &str,
        operation: impl FnOnce(&TypeParsingScope<'_>) -> R,
    ) -> R {
        self.scope().with_type_parameter_scope(spec, operation)
    }
}

/// The file path of a test library URI (`_TestUriResolver`).
fn test_uri_path(uri: &str) -> String {
    if let Some(name) = uri.strip_prefix("dart:") {
        return format!("/sdk/{name}/{name}.dart");
    }
    if let Some(rest) = uri.strip_prefix("package:") {
        let (package, path) = rest.split_once('/').unwrap_or((rest, ""));
        return format!("/home/{package}/lib/{path}");
    }
    uri.to_string()
}

/// `TypeParsingScope`: parses type specs against fixed libraries and type
/// parameters.
pub struct TypeParsingScope<'w> {
    pub test: &'w TypeSystemTest,
    pub libraries: Vec<EId<LibraryElement>>,
    pub type_parameters: Vec<EId<TypeParameterElement>>,
}

impl TypeParsingScope<'_> {
    fn spec_scope(&self) -> Scope {
        Scope::for_libraries(&self.test.ctx(), &self.libraries, &self.type_parameters)
    }

    /// `parseType(input)`.
    pub fn parse_type(&self, input: &str) -> TypeId {
        let parsed = SpecParser::parse_type(input);
        materialize(&self.test.ctx(), &parsed, &self.spec_scope())
    }

    fn parse_kind(&self, input: &str, ok: fn(&TypeKind) -> bool, kind: &str) -> TypeId {
        let t = self.parse_type(input);
        assert!(
            ok(self.test.ctx().ty(t)),
            "Expected {kind} for \"{input}\", got: {}",
            self.test.display(t)
        );
        t
    }

    /// `parseFunctionType(input)`.
    pub fn parse_function_type(&self, input: &str) -> TypeId {
        self.parse_kind(
            input,
            |k| matches!(k, TypeKind::Function(_)),
            "FunctionTypeImpl",
        )
    }

    /// `parseInterfaceType(input)`.
    pub fn parse_interface_type(&self, input: &str) -> TypeId {
        self.parse_kind(
            input,
            |k| matches!(k, TypeKind::Interface { .. }),
            "InterfaceTypeImpl",
        )
    }

    /// `parseRecordType(input)`.
    pub fn parse_record_type(&self, input: &str) -> TypeId {
        self.parse_kind(
            input,
            |k| matches!(k, TypeKind::Record { .. }),
            "RecordTypeImpl",
        )
    }

    /// `parseTypeParameterType(input)`.
    pub fn parse_type_parameter_type(&self, input: &str) -> TypeId {
        self.parse_kind(
            input,
            |k| matches!(k, TypeKind::TypeParameter { .. }),
            "TypeParameterTypeImpl",
        )
    }

    /// `typeParameter(name)`: the innermost type parameter named [name].
    pub fn type_parameter(&self, name: &str) -> EId<TypeParameterElement> {
        let ctx = self.test.ctx();
        self.type_parameters
            .iter()
            .rev()
            .copied()
            .find(|&e| ctx.element_name(e.raw()) == Some(name))
            .unwrap_or_else(|| panic!("Unknown type parameter: {name}"))
    }

    /// `withTypeParameterScope(spec, operation)`: new standalone type
    /// parameters (`TypeSpecParser.parseTypeParameters`) in a child scope.
    pub fn with_type_parameter_scope<R>(
        &self,
        spec: &str,
        operation: impl FnOnce(&TypeParsingScope<'_>) -> R,
    ) -> R {
        let ctx = self.test.ctx();
        let parsed = SpecParser::parse_type_parameters(spec);
        let mut scope = self.spec_scope();
        let new_type_parameters = materialize_type_parameters_standalone(&ctx, &parsed, &mut scope);
        let mut type_parameters = self.type_parameters.clone();
        type_parameters.extend(new_type_parameters);
        operation(&TypeParsingScope {
            test: self.test,
            libraries: self.libraries.clone(),
            type_parameters,
        })
    }
}
