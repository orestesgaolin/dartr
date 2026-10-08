// Dart source: pkg/analyzer/test/src/dart/resolution/context_collection_resolution.dart
// (PubPackageResolutionTest: resolve the test file) and
// pkg/analyzer/test/src/summary/elements_base.dart (buildLibrary), as far
// as the inheritance tests need them.

//! A test-only "declaration builder": it parses Dart source with the real
//! parser (`dartr_ast_builder::parse_string`) and builds the declared
//! classes, mixins and extension types with their members, so that the
//! ported inheritance tests keep the Dart source of the original tests.
//!
//! It is not a linker (unit B1–B5): class headers go through the
//! `test_support` spec builder, member types are parsed from the source
//! text of their type annotations, and there is no inference. A member
//! without an explicit type (except the `void` return type of a setter)
//! panics with [`IMPLICIT_TYPE`], so tests that need top-level or override
//! inference are found and listed as waiting for the linker.

#![allow(dead_code)]

use dartr_ast::{
    Ast, BlockClassBody, ClassDeclaration, ClassMember, ClassTypeAlias, CompilationUnitMember,
    EmptyFunctionBody, ExtensionTypeDeclaration, FieldDeclaration, FormalParameterList, Id,
    ImplementsClause, MethodDeclaration, MixinDeclaration, NameWithTypeParameters,
    PrimaryConstructorDeclaration, RegularFormalParameter, TypeParameterList, WithClause,
};
use dartr_element::{
    BoolSlot, ClassElement, Ctx, DisplayOptions, EId, ElemRef, ElementData, ElementFlags,
    ElementId, ExecutableElementData, ExecutableFragmentData, ExtensionTypeElement, FieldElement,
    FieldFragment, FormalParameterElement, FormalParameterFragment, FragmentData, FragmentFlags,
    FragmentId, GetterElement, GetterFragment, InterfaceElement, LibraryElement, MethodElement,
    MethodFragment, MixinElement, OnceSlot, ParameterKind, PropertyAccessorElementData,
    PropertyAccessorFragmentData, PropertyInducingElement, PropertyInducingElementData,
    PropertyInducingFragmentData, SetterElement, SetterFragment, Tag, TypeId, TypeParameterElement,
    TypeParameterFragment, VarSlot, VariableElementData, VariableFragmentData,
};
use dartr_typesystem::TypeExt;
use dartr_typesystem::inheritance_manager3::{InheritanceManager3, Name};
use dartr_typesystem::member;
use dartr_typesystem::test_support::{
    ClassSpec, LibrarySpec, TypeParsingScope, TypeSystemTest, strs,
};

/// The panic message for a member without an explicit type.
pub const IMPLICIT_TYPE: &str = "implicit type: needs the linker (inference)";

/// The URI of the test library (`package:test/test.dart`).
pub const TEST_URI: &str = "package:test/test.dart";

/// The extra library with the `dart:core` classes that the mock SDK of
/// `test_support` does not have.
const MOCK_CORE_EXTRA: &str = "dart:_core_extra";

/// A built test world: the mock SDK, the libraries of the test sources, and
/// the last library as the test library.
pub struct SourceTest {
    backend: Backend,
    pub library: EId<LibraryElement>,
}

/// How the elements of a [`SourceTest`] are built.
enum Backend {
    /// The mock SDK of `test_support` and the declaration builder of this
    /// module (no inference).
    Mock(Box<TypeSystemTest>),
    /// The real linker (`dartr_link` through `dartr_driver`) with the SDK of
    /// the `dart` on PATH: override inference and covariance are done.
    Linked(Box<LinkedWorld>),
}

/// The world of [`SourceTest::linked`].
struct LinkedWorld {
    driver: dartr_driver::driver::Driver,
    tp: dartr_element::TypeProvider,
    features: dartr_element::FeatureSet,
    sink: dartr_element::NoopSink,
    /// The folder of the test files, removed on drop.
    dir: std::path::PathBuf,
}

impl Drop for LinkedWorld {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A member to create, collected from the AST.
struct MemberDecl {
    kind: MemberKind,
    name: String,
    is_static: bool,
    is_abstract: bool,
    /// Source text of the return type (getters, methods) or the field type.
    return_type: Option<String>,
    /// Source text of the type parameters (`<T extends num>`).
    type_parameters: Vec<(String, Option<String>)>,
    parameters: Vec<ParamDecl>,
    /// Fields: final or const (no setter).
    is_final: bool,
    is_covariant: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MemberKind {
    Method,
    Getter,
    Setter,
    Field,
}

struct ParamDecl {
    name: Option<String>,
    ty: Option<String>,
    kind: ParameterKind,
    is_covariant: bool,
}

/// A class-like declaration of a source library.
struct TypeDecl {
    name: String,
    members: Vec<MemberDecl>,
    is_extension_type: bool,
}

impl SourceTest {
    /// Builds the test library [source] (`package:test/test.dart`).
    pub fn new(source: &str) -> SourceTest {
        SourceTest::with_files(&[(TEST_URI, source)])
    }

    /// Builds the libraries [files] (URI, source), in order; the last one is
    /// the test library. `import 'a.dart';` is resolved relative to
    /// `package:test/`.
    pub fn with_files(files: &[(&str, &str)]) -> SourceTest {
        let mut t = TypeSystemTest::new();
        let extra = t.build_libraries(&[LibrarySpec {
            uri: MOCK_CORE_EXTRA.into(),
            imports: strs(&["dart:core"]),
            classes: vec![ClassSpec::new("abstract class Invocation")],
            ..LibrarySpec::default()
        }])[MOCK_CORE_EXTRA];
        add_object_members(&mut t, extra);

        let mut library = None;
        for (uri, source) in files {
            library = Some(build_source_library(&mut t, uri, source, extra));
        }
        let library = library.expect("at least one file");
        t.test_library = Some(library);
        SourceTest {
            backend: Backend::Mock(Box::new(t)),
            library,
        }
    }

    /// Links the test library [source] (`package:test/test.dart`) with the
    /// real linker and the SDK of the `dart` on PATH. For the tests that
    /// need inference (implicit types, inherited covariance).
    pub fn linked(source: &str) -> SourceTest {
        SourceTest::linked_files(&[(TEST_URI, source)])
    }

    /// [`Self::linked`] for the libraries [files] (`package:test/...` URI,
    /// source); the last one is the test library.
    pub fn linked_files(files: &[(&str, &str)]) -> SourceTest {
        use dartr_driver::driver::Driver;
        use dartr_driver::file_state::{FileConfig, FileSystemState, SourceFactory};
        use dartr_project::package_config::{Package, Packages};
        use dartr_project::{DartSdk, Workspace};
        use std::sync::atomic::{AtomicUsize, Ordering};

        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "dartr_typesystem_test_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let lib = dir.join("lib");
        std::fs::create_dir_all(&lib).unwrap();
        let root = dartr_project::paths::normalize(dir.to_str().unwrap());
        let lib_path = dartr_project::paths::normalize(lib.to_str().unwrap());
        let mut paths = Vec::new();
        for (uri, source) in files {
            let relative = uri
                .strip_prefix("package:test/")
                .unwrap_or_else(|| panic!("not a package:test/ URI: {uri}"));
            let path = lib.join(relative);
            std::fs::write(&path, source).unwrap();
            paths.push(dartr_project::paths::normalize(path.to_str().unwrap()));
        }

        let sdk = dartr_project::sdk::find_sdk_path()
            .map(|p| DartSdk::new(&p))
            .expect("a Dart SDK (dart on PATH)");
        let version = sdk
            .language_version()
            .map(|v| (v.major, v.minor))
            .unwrap_or((3, 13));
        let packages = Packages::new(vec![Package {
            name: "test".to_string(),
            root: root.clone(),
            lib: lib_path,
            language_version: None,
        }]);
        let source_factory = SourceFactory {
            workspace: Workspace::basic(packages, &root),
            sdk: Some(sdk),
        };
        let config_for = Box::new(move |_: &str, _: &str| FileConfig {
            package_language_version: version,
            experiments: Vec::new(),
        });
        let generation = std::sync::Arc::new(dartr_element::Generation::new(0));
        let mut driver = Driver::new(FileSystemState::new(source_factory, config_for), generation);
        let file_ids: Vec<_> = paths.iter().map(|p| driver.fs.get_file_for_path(p)).collect();
        driver.fs.discover();
        driver.link_libraries(&file_ids);
        let test_uri = driver.fs.file(*file_ids.last().unwrap()).uri_str.clone();
        assert_eq!(&*test_uri, TEST_URI);
        let library = *driver
            .state
            .world
            .libraries
            .get(&*test_uri)
            .expect("the test library is linked");
        let tp = dartr_link::types_builder::world_type_provider(&driver.state.world);
        SourceTest {
            backend: Backend::Linked(Box::new(LinkedWorld {
                driver,
                tp,
                features: dartr_element::FeatureSet::default(),
                sink: dartr_element::NoopSink,
                dir,
            })),
            library,
        }
    }

    pub fn ctx(&self) -> Ctx<'_> {
        match &self.backend {
            Backend::Mock(t) => t.ctx(),
            Backend::Linked(w) => Ctx {
                world: &w.driver.state.world,
                current: None,
                local: None,
                tp: &w.tp,
                features: &w.features,
                req: &w.sink,
            },
        }
    }

    /// `InheritanceManager3()`.
    pub fn manager(&self) -> InheritanceManager3<'_> {
        InheritanceManager3::new(self.ctx())
    }

    /// `Name(null, name)`.
    pub fn name(&self, name: &str) -> Name {
        Name::new(&self.ctx(), None, name)
    }

    /// `findElement.classOrMixin(name)` (also enums and extension types)
    /// in the test library.
    pub fn class_or_mixin(&self, name: &str) -> EId<InterfaceElement> {
        find_interface(&self.ctx(), self.library, name)
            .unwrap_or_else(|| panic!("no class or mixin {name}"))
    }

    /// The interface element [name] of the library [uri].
    pub fn interface_in(&self, uri: &str, name: &str) -> EId<InterfaceElement> {
        let library = self.ctx().library_by_uri(uri);
        let library = library
            .or_else(|| {
                let ctx = self.ctx();
                // Libraries of the test store are not in the world map.
                let Backend::Mock(t) = &self.backend else {
                    return None;
                };
                let store = &t.store;
                store
                    .elements
                    .libraries
                    .iter()
                    .map(|(i, _)| {
                        EId::<LibraryElement>::from_raw(ElementId::new(store.id, Tag::Library, i))
                    })
                    .find(|&l| ctx.library_uri(l) == uri)
            })
            .unwrap_or_else(|| panic!("no library {uri}"));
        find_interface(&self.ctx(), library, name).unwrap_or_else(|| panic!("no {name} in {uri}"))
    }

    /// `findElement.typeParameter(name)`: a type parameter of a class-like
    /// declaration of the test library.
    pub fn type_parameter(&self, name: &str) -> EId<TypeParameterElement> {
        let ctx = self.ctx();
        let l = ctx.get(self.library);
        l.classes
            .iter()
            .map(|e| e.raw())
            .chain(l.mixins.iter().map(|e| e.raw()))
            .chain(l.extension_types.iter().map(|e| e.raw()))
            .flat_map(|e| ctx.interface(EId::from_raw(e)).type_params.clone())
            .find(|&p| ctx.element_name(p.raw()) == Some(name))
            .unwrap_or_else(|| panic!("no type parameter {name}"))
    }

    /// `findElement.method(name, of: className)`.
    pub fn method(&self, class: &str, name: &str) -> ElemRef {
        let ctx = self.ctx();
        let element = self.class_or_mixin(class);
        let m = ctx
            .interface(element)
            .methods
            .iter()
            .find(|m| ctx.element_name(m.raw()) == Some(name))
            .unwrap_or_else(|| panic!("no method {class}.{name}"));
        ElemRef::Base(m.raw())
    }

    /// `findElement.getter(name, of: className)`.
    pub fn getter(&self, class: &str, name: &str) -> ElemRef {
        let ctx = self.ctx();
        let element = self.class_or_mixin(class);
        let m = ctx
            .interface(element)
            .getters
            .iter()
            .find(|m| ctx.element_name(m.raw()) == Some(name))
            .unwrap_or_else(|| panic!("no getter {class}.{name}"));
        ElemRef::Base(m.raw())
    }

    /// `findElement.setter(name, of: className)`.
    pub fn setter(&self, class: &str, name: &str) -> ElemRef {
        let ctx = self.ctx();
        let element = self.class_or_mixin(class);
        let m = ctx
            .interface(element)
            .setters
            .iter()
            .find(|m| ctx.element_name(m.raw()) == Some(name))
            .unwrap_or_else(|| panic!("no setter {class}.{name}"));
        ElemRef::Base(m.raw())
    }

    /// `_assertExecutable(element, expected)`. Stricter than Dart: a
    /// missing element with a non-null [expected] fails (Dart only checks
    /// `isNull` then).
    pub fn assert_executable(&self, element: Option<ElemRef>, expected: Option<&str>) {
        let ctx = self.ctx();
        match (element, expected) {
            (Some(element), Some(expected)) => {
                assert_eq!(self.executable_line(element), expected);
                let enclosing = member::enclosing_element(&ctx, element);
                if member::is_getter(&ctx, element) || member::is_setter(&ctx, element) {
                    let variable = member::variable(&ctx, element).expect("variable");
                    // Dart: same(enclosingElement)
                    assert_eq!(member::enclosing_element(&ctx, variable), enclosing);
                    assert_eq!(member::name(&ctx, variable), member::name(&ctx, element));
                    let expected_type = if member::is_getter(&ctx, element) {
                        member::return_type(&ctx, element)
                    } else {
                        member::type_(&ctx, member::formal_parameters(&ctx, element)[0])
                    };
                    // Dart: ==
                    assert!(
                        ctx.dart_eq(member::type_(&ctx, variable), expected_type),
                        "variable type {} != {}",
                        self.display(member::type_(&ctx, variable)),
                        self.display(expected_type)
                    );
                }
            }
            (None, None) => {}
            (element, expected) => panic!(
                "expected {expected:?}, got {:?}",
                element.map(|e| self.executable_line(e))
            ),
        }
    }

    /// `_assertGetInherited(className:, name:, expected:)`.
    pub fn assert_get_inherited(&self, class_name: &str, name: &str, expected: Option<&str>) {
        let member = self
            .manager()
            .get_inherited(self.class_or_mixin(class_name), self.name(name));
        self.assert_executable(member, expected);
    }

    /// `_assertGetMember(className:, name:, expected:, concrete:, forSuper:)`.
    pub fn assert_get_member(
        &self,
        class_name: &str,
        name: &str,
        expected: Option<&str>,
        concrete: bool,
        for_super: bool,
    ) {
        let member = self.manager().get_member_with(
            self.class_or_mixin(class_name),
            self.name(name),
            dartr_typesystem::inheritance_manager3::GetMemberOptions {
                concrete,
                for_super,
                ..Default::default()
            },
        );
        self.assert_executable(member, expected);
    }

    /// `_assertGetMember2`: [`Self::assert_get_member`] without and with
    /// `concrete`.
    pub fn assert_get_member2(&self, class_name: &str, name: &str, expected: Option<&str>) {
        self.assert_get_member(class_name, name, expected, false, false);
        self.assert_get_member(class_name, name, expected, true, false);
    }

    /// `_assertGetOverridden4(className:, name:, expected:)`.
    pub fn assert_get_overridden4(&self, class_name: &str, name: &str, expected: Option<&str>) {
        let members = self
            .manager()
            .get_overridden(self.class_or_mixin(class_name), self.name(name));
        assert_eq!(
            executable_list_text(self, members.as_deref()).as_deref(),
            expected
        );
    }

    /// `_assertInheritedConcreteMap(result, className, expected)`.
    pub fn assert_inherited_concrete_map(&self, class_name: &str, expected: &str) {
        let map = self
            .manager()
            .get_inherited_concrete_map(self.class_or_mixin(class_name));
        assert_eq!(name_to_executable_map_text(self, &map), expected);
    }

    /// `_assertInheritedMap(result, className, expected)`.
    pub fn assert_inherited_map(&self, class_name: &str, expected: &str) {
        let map = self
            .manager()
            .get_inherited_map(self.class_or_mixin(class_name));
        assert_eq!(name_to_executable_map_text(self, map), expected);
    }

    /// `assertInterfaceText(element, expected)` of the `_Base2` tests.
    pub fn assert_interface_text(
        &self,
        element: EId<InterfaceElement>,
        configuration: InterfacePrinterConfiguration,
        expected: &str,
    ) {
        let actual = interface_text(self.ctx(), element, configuration);
        if actual != expected {
            panic!("--- actual ---\n{actual}--- expected ---\n{expected}");
        }
    }

    /// `type.getDisplayString()`.
    pub fn display(&self, t: TypeId) -> String {
        dartr_element::type_display_string_with(&self.ctx(), t, DisplayOptions::default())
    }

    /// `'${element.enclosingElement?.name}.${element.lookupName}: ${typeString(element.type)}'`
    /// (the line format of `_assertExecutable` and `_assertNameToExecutableMap`).
    pub fn executable_line(&self, element: ElemRef) -> String {
        let ctx = self.ctx();
        let enclosing = member::enclosing_element(&ctx, element)
            .and_then(|e| ctx.element_name(e))
            .unwrap_or("null");
        let lookup_name = member::lookup_name(&ctx, element).unwrap_or_default();
        let ty = self.display(member::type_(&ctx, element));
        format!("{enclosing}.{lookup_name}: {ty}")
    }
}

fn find_interface(
    ctx: &Ctx<'_>,
    library: EId<LibraryElement>,
    name: &str,
) -> Option<EId<InterfaceElement>> {
    let l = ctx.get(library);
    l.classes
        .iter()
        .map(|e| e.raw())
        .chain(l.mixins.iter().map(|e| e.raw()))
        .chain(l.enums.iter().map(|e| e.raw()))
        .chain(l.extension_types.iter().map(|e| e.raw()))
        .find(|&e| ctx.element_name(e) == Some(name))
        .map(EId::from_raw)
}

/// Adds the members of `Object` that the mock SDK does not declare
/// (`hashCode`, `runtimeType`, `noSuchMethod`), as in `dart:core`.
fn add_object_members(t: &mut TypeSystemTest, extra: EId<LibraryElement>) {
    let object = t.tp.object_element().upcast::<InterfaceElement>();
    let decls = vec![
        MemberDecl::getter("hashCode", "int"),
        MemberDecl::getter("runtimeType", "Type"),
        MemberDecl {
            parameters: vec![ParamDecl {
                name: Some("invocation".into()),
                ty: Some("Invocation".into()),
                kind: ParameterKind::Required,
                is_covariant: false,
            }],
            ..MemberDecl::method("noSuchMethod", "dynamic")
        },
    ];
    let core = t.core_library;
    let libraries = vec![t.core_library, t.async_library, extra];
    add_members(t, object, core, &decls, &libraries, false);
}

impl MemberDecl {
    fn getter(name: &str, ty: &str) -> MemberDecl {
        MemberDecl {
            kind: MemberKind::Getter,
            name: name.into(),
            is_static: false,
            is_abstract: false,
            return_type: Some(ty.into()),
            type_parameters: Vec::new(),
            parameters: Vec::new(),
            is_final: false,
            is_covariant: false,
        }
    }

    fn method(name: &str, ty: &str) -> MemberDecl {
        MemberDecl {
            kind: MemberKind::Method,
            ..MemberDecl::getter(name, ty)
        }
    }
}

// ------------------------------------------------------------------ AST walk

fn text(source: &str, ast: &Ast, id: impl Into<dartr_ast::NodeId> + Copy) -> String {
    source[ast.offset(id) as usize..ast.end(id) as usize].to_string()
}

fn token(ast: &Ast, t: dartr_syntax::TokenId) -> String {
    ast.tokens.lexeme(t).to_string()
}

fn type_parameters_text(source: &str, ast: &Ast, list: Option<Id<TypeParameterList>>) -> String {
    list.map(|l| text(source, ast, l)).unwrap_or_default()
}

fn type_parameter_decls(
    source: &str,
    ast: &Ast,
    list: Option<Id<TypeParameterList>>,
) -> Vec<(String, Option<String>)> {
    let Some(list) = list else {
        return Vec::new();
    };
    ast.list(ast[list].type_parameters)
        .iter()
        .map(|&p| {
            let p = &ast[p];
            (token(ast, p.name), p.bound.map(|b| text(source, ast, b)))
        })
        .collect()
}

fn with_text(source: &str, ast: &Ast, clause: Option<Id<WithClause>>) -> String {
    match clause {
        Some(c) => {
            let types: Vec<String> = ast
                .list(ast[c].mixin_types)
                .iter()
                .map(|&t| text(source, ast, t))
                .collect();
            format!(" with {}", types.join(", "))
        }
        None => String::new(),
    }
}

fn implements_text(source: &str, ast: &Ast, clause: Option<Id<ImplementsClause>>) -> String {
    match clause {
        Some(c) => {
            let types: Vec<String> = ast
                .list(ast[c].interfaces)
                .iter()
                .map(|&t| text(source, ast, t))
                .collect();
            format!(" implements {}", types.join(", "))
        }
        None => String::new(),
    }
}

fn parameters(source: &str, ast: &Ast, list: Option<Id<FormalParameterList>>) -> Vec<ParamDecl> {
    let Some(list) = list else {
        return Vec::new();
    };
    ast.list(ast[list].parameters)
        .iter()
        .map(|&p| {
            let p = ast
                .cast::<RegularFormalParameter>(p)
                .unwrap_or_else(|| panic!("unsupported parameter: {}", text(source, ast, p)));
            let p = &ast[p];
            assert!(
                p.function_typed_suffix.is_none(),
                "function-typed parameters are not supported"
            );
            ParamDecl {
                name: p.name.map(|n| token(ast, n)),
                ty: p.type_.map(|t| text(source, ast, t)),
                kind: p.kind,
                is_covariant: p.covariant_keyword.is_some(),
            }
        })
        .collect()
}

fn members(source: &str, ast: &Ast, members: &[Id<ClassMember>]) -> Vec<MemberDecl> {
    let mut result = Vec::new();
    for &m in members {
        if let Some(m) = ast.cast::<MethodDeclaration>(m) {
            let m = &ast[m];
            let kind = match m.property_keyword.map(|k| token(ast, k)).as_deref() {
                Some("get") => MemberKind::Getter,
                Some("set") => MemberKind::Setter,
                _ => MemberKind::Method,
            };
            let is_static = m
                .modifier_keyword
                .is_some_and(|k| token(ast, k) == "static");
            let is_abstract = ast.is::<EmptyFunctionBody>(m.body) && m.external_keyword.is_none();
            result.push(MemberDecl {
                kind,
                name: token(ast, m.name),
                is_static,
                is_abstract,
                return_type: m.return_type.map(|t| text(source, ast, t)),
                type_parameters: type_parameter_decls(source, ast, m.type_parameters),
                parameters: parameters(source, ast, m.parameters),
                is_final: false,
                is_covariant: false,
            });
        } else if let Some(f) = ast.cast::<FieldDeclaration>(m) {
            let f = &ast[f];
            let list = &ast[f.fields];
            let keyword = list.keyword.map(|k| token(ast, k));
            for &v in ast.list(list.variables) {
                result.push(MemberDecl {
                    kind: MemberKind::Field,
                    name: token(ast, ast[v].name),
                    is_static: f.static_keyword.is_some(),
                    is_abstract: f.abstract_keyword.is_some(),
                    return_type: list.type_.map(|t| text(source, ast, t)),
                    type_parameters: Vec::new(),
                    parameters: Vec::new(),
                    is_final: matches!(keyword.as_deref(), Some("final" | "const")),
                    is_covariant: f.covariant_keyword.is_some(),
                });
            }
        }
        // Constructors are not needed by the inheritance tests.
    }
    result
}

fn body_members(source: &str, ast: &Ast, body: Id<dartr_ast::ClassBody>) -> Vec<MemberDecl> {
    match ast.cast::<BlockClassBody>(body) {
        Some(b) => members(source, ast, ast.list(ast[b].members)),
        None => Vec::new(),
    }
}

/// Builds one library from [source].
fn build_source_library(
    t: &mut TypeSystemTest,
    uri: &str,
    source: &str,
    extra: EId<LibraryElement>,
) -> EId<LibraryElement> {
    let parsed = dartr_ast_builder::parse_string(source, uri);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse diagnostics: {:?}",
        parsed.diagnostics
    );
    let ast = &parsed.ast;
    let unit = &ast[parsed.unit];

    let mut spec = LibrarySpec {
        uri: uri.into(),
        imports: strs(&["dart:core", "dart:async", MOCK_CORE_EXTRA]),
        ..LibrarySpec::default()
    };
    for &d in ast.list(unit.directives) {
        if let Some(i) = ast.cast::<dartr_ast::ImportDirective>(d) {
            let uri_text = text(source, ast, ast[i].uri);
            let relative = uri_text.trim_matches(|c| c == '\'' || c == '"');
            let resolved = if relative.contains(':') {
                relative.to_string()
            } else {
                format!("package:test/{relative}")
            };
            spec.imports.push(resolved);
        }
    }

    let mut decls = Vec::new();
    for &d in ast.list(unit.declarations) {
        if let Some(a) = ast.cast::<dartr_ast::GenericTypeAlias>(d) {
            let a = &ast[a];
            spec.type_aliases.push(format!(
                "typedef {}{} = {}",
                token(ast, a.name),
                type_parameters_text(source, ast, a.type_parameters),
                text(source, ast, a.type_)
            ));
            continue;
        }
        decls.push(declaration(source, ast, d, &mut spec));
    }

    let library = t.build_libraries(&[spec])[uri];

    let mut libraries = vec![t.core_library, t.async_library, extra];
    for import in imports_of(t, library) {
        libraries.push(import);
    }
    libraries.push(library);
    for decl in &decls {
        let element = find_interface(&t.ctx(), library, &decl.name).unwrap();
        add_members(
            t,
            element,
            library,
            &decl.members,
            &libraries,
            decl.is_extension_type,
        );
    }
    library
}

/// The libraries built before [library] in the test store (the imports are
/// resolved by the spec builder; this is their scope for member types).
fn imports_of(t: &TypeSystemTest, library: EId<LibraryElement>) -> Vec<EId<LibraryElement>> {
    let store = &t.store;
    store
        .elements
        .libraries
        .iter()
        .map(|(i, _)| EId::<LibraryElement>::from_raw(ElementId::new(store.id, Tag::Library, i)))
        .filter(|&l| l != library && t.ctx().library_uri(l).starts_with("package:"))
        .collect()
}

fn declaration(
    source: &str,
    ast: &Ast,
    d: Id<CompilationUnitMember>,
    spec: &mut LibrarySpec,
) -> TypeDecl {
    if let Some(c) = ast.cast::<ClassDeclaration>(d) {
        let c = &ast[c];
        let name_part = ast
            .cast::<NameWithTypeParameters>(c.name_part)
            .expect("class name");
        let name_part = &ast[name_part];
        let name = token(ast, name_part.type_name);
        let mut header = String::new();
        if c.abstract_keyword.is_some() {
            header.push_str("abstract ");
        }
        header.push_str(&format!(
            "class {name}{}",
            type_parameters_text(source, ast, name_part.type_parameters)
        ));
        if let Some(e) = c.extends_clause {
            header.push_str(&format!(
                " extends {}",
                text(source, ast, ast[e].superclass)
            ));
        }
        header.push_str(&with_text(source, ast, c.with_clause));
        header.push_str(&implements_text(source, ast, c.implements_clause));
        spec.classes.push(ClassSpec::new(&header));
        return TypeDecl {
            name,
            members: body_members(source, ast, c.body),
            is_extension_type: false,
        };
    }
    if let Some(c) = ast.cast::<ClassTypeAlias>(d) {
        let c = &ast[c];
        let name = token(ast, c.name);
        let mut header = String::new();
        if c.abstract_keyword.is_some() {
            header.push_str("abstract ");
        }
        header.push_str(&format!(
            "class {name}{} extends {}",
            type_parameters_text(source, ast, c.type_parameters),
            text(source, ast, c.superclass)
        ));
        header.push_str(&with_text(source, ast, Some(c.with_clause)));
        header.push_str(&implements_text(source, ast, c.implements_clause));
        spec.classes.push(ClassSpec::new(&header));
        return TypeDecl {
            name,
            members: Vec::new(),
            is_extension_type: false,
        };
    }
    if let Some(m) = ast.cast::<MixinDeclaration>(d) {
        let m = &ast[m];
        let name = token(ast, m.name);
        let mut header = format!(
            "mixin {name}{}",
            type_parameters_text(source, ast, m.type_parameters)
        );
        if let Some(on) = m.on_clause {
            let types: Vec<String> = ast
                .list(ast[on].superclass_constraints)
                .iter()
                .map(|&t| text(source, ast, t))
                .collect();
            header.push_str(&format!(" on {}", types.join(", ")));
        }
        header.push_str(&implements_text(source, ast, m.implements_clause));
        spec.mixins.push(header);
        return TypeDecl {
            name,
            members: body_members(source, ast, m.body),
            is_extension_type: false,
        };
    }
    if let Some(e) = ast.cast::<ExtensionTypeDeclaration>(d) {
        let e = &ast[e];
        let primary = ast
            .cast::<PrimaryConstructorDeclaration>(e.name_part)
            .expect("primary constructor");
        let primary = &ast[primary];
        let name = token(ast, primary.type_name);
        let params = parameters(source, ast, Some(primary.formal_parameters));
        assert_eq!(params.len(), 1, "one representation field");
        let header = format!(
            "extension type {name}{}({} {}){}",
            type_parameters_text(source, ast, primary.type_parameters),
            params[0].ty.as_deref().expect(IMPLICIT_TYPE),
            params[0].name.as_deref().unwrap(),
            implements_text(source, ast, e.implements_clause)
        );
        spec.extension_types.push(header);
        return TypeDecl {
            name,
            members: body_members(source, ast, e.body),
            is_extension_type: true,
        };
    }
    panic!("unsupported declaration: {}", text(source, ast, d));
}

// ------------------------------------------------------------------ elements

/// Creates the members [decls] of [element] (phase 1, `&mut` store), then
/// resolves their types (phase 2).
fn add_members(
    t: &mut TypeSystemTest,
    element: EId<InterfaceElement>,
    library: EId<LibraryElement>,
    decls: &[MemberDecl],
    libraries: &[EId<LibraryElement>],
    is_extension_type: bool,
) {
    let generation = t.world.generation.clone();
    let names = &generation.names;
    let class_fragment: FragmentId = t.ctx().element_data(element.raw()).unwrap().first_fragment;
    let class_type_parameters = t.ctx().interface(element).type_params.clone();

    // Extension types: the getter of the representation field.
    let mut decls: Vec<&MemberDecl> = decls.iter().collect();
    let representation;
    if is_extension_type {
        let field = t.ctx().interface(element).fields[0];
        let field_name = t.ctx().element_name(field.raw()).unwrap().to_string();
        representation = MemberDecl::getter(&field_name, "<representation>");
        decls.insert(0, &representation);
    }

    struct Pending {
        executable: ElementId,
        decl_index: usize,
        type_parameters: Vec<EId<TypeParameterElement>>,
        parameters: Vec<EId<FormalParameterElement>>,
        field: Option<EId<FieldElement>>,
    }
    let mut pending: Vec<Pending> = Vec::new();
    // Synthetic fields of explicit getters and setters, by name.
    let mut fields: Vec<(String, EId<FieldElement>)> = Vec::new();

    let element_data = |name: &str, fragment: FragmentId| {
        let mut data = ElementData::new(Some(names.intern(name)), fragment);
        data.library = Some(library);
        data.enclosing = Some(element.raw());
        if is_extension_type {
            data.flags.set(
                ElementFlags::EXECUTABLE_ELEMENT_IS_EXTENSION_TYPE_MEMBER,
                true,
            );
        }
        data
    };
    let fragment_data = |name: &str| {
        let mut f = FragmentData::new(Some(names.intern(name)), Some(0));
        f.enclosing_fragment = Some(class_fragment);
        f
    };

    for (index, decl) in decls.iter().enumerate() {
        let store = &mut t.store;
        match decl.kind {
            MemberKind::Field => {
                // An explicit field with a synthetic getter and setter.
                let fd = fragment_data(&decl.name);
                fd.flags.set(
                    FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_DECLARATION,
                    true,
                );
                fd.flags
                    .set(FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC, decl.is_static);
                fd.flags.set(
                    FragmentFlags::VARIABLE_FRAGMENT_IS_ABSTRACT,
                    decl.is_abstract,
                );
                fd.flags.set(
                    FragmentFlags::FIELD_FRAGMENT_IS_EXPLICITLY_COVARIANT,
                    decl.is_covariant,
                );
                let fragment = store.add_fragment::<FieldFragment>(FieldFragment {
                    property: PropertyInducingFragmentData {
                        variable: VariableFragmentData::new(fd),
                        induced_getter: None,
                        induced_setter: None,
                    },
                    inherits_covariant: BoolSlot::new(false),
                });
                let mut data = ElementData::new(Some(names.intern(&decl.name)), fragment.raw());
                data.library = Some(library);
                data.enclosing = Some(element.raw());
                let field: EId<FieldElement> = store.add(FieldElement {
                    property: PropertyInducingElementData::new(data),
                });
                store.fragment(fragment).element.set_once(field.raw());
                push_field(store, element, field);

                let getter = add_accessor(
                    store,
                    element,
                    MemberKind::Getter,
                    &decl.name,
                    &[],
                    decl,
                    &element_data,
                    &fragment_data,
                    true,
                );
                store.get_mut(field).getter = Some(getter.0.cast().unwrap());
                pending.push(Pending {
                    executable: getter.0,
                    decl_index: index,
                    type_parameters: Vec::new(),
                    parameters: Vec::new(),
                    field: Some(field),
                });
                if !decl.is_final {
                    let value = ParamDecl {
                        name: Some("value".into()),
                        ty: decl.return_type.clone(),
                        kind: ParameterKind::Required,
                        is_covariant: decl.is_covariant,
                    };
                    let setter = add_accessor(
                        store,
                        element,
                        MemberKind::Setter,
                        &decl.name,
                        std::slice::from_ref(&value),
                        decl,
                        &element_data,
                        &fragment_data,
                        true,
                    );
                    store.get_mut(field).setter = Some(setter.0.cast().unwrap());
                    pending.push(Pending {
                        executable: setter.0,
                        decl_index: index,
                        type_parameters: Vec::new(),
                        parameters: setter.1,
                        field: None,
                    });
                }
            }
            MemberKind::Getter | MemberKind::Setter => {
                let is_representation = is_extension_type && index == 0;
                let accessor = add_accessor(
                    store,
                    element,
                    decl.kind,
                    &decl.name,
                    &decl.parameters,
                    decl,
                    &element_data,
                    &fragment_data,
                    is_representation,
                );
                // The variable: the representation field, or a synthetic
                // field per name.
                let field = if is_representation {
                    let field = store.get(element_fields_owner(element)).fields[0];
                    store.get_mut(field).getter = Some(accessor.0.cast().unwrap());
                    field
                } else if let Some((_, f)) = fields.iter().find(|(n, _)| *n == decl.name) {
                    *f
                } else {
                    let fd = fragment_data(&decl.name);
                    fd.flags.set(
                        FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_GETTER_SETTER,
                        true,
                    );
                    fd.flags
                        .set(FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC, decl.is_static);
                    let fragment = store.add_fragment::<FieldFragment>(FieldFragment {
                        property: PropertyInducingFragmentData {
                            variable: VariableFragmentData::new(fd),
                            induced_getter: None,
                            induced_setter: None,
                        },
                        inherits_covariant: BoolSlot::new(false),
                    });
                    let mut data = ElementData::new(Some(names.intern(&decl.name)), fragment.raw());
                    data.library = Some(library);
                    data.enclosing = Some(element.raw());
                    let field: EId<FieldElement> = store.add(FieldElement {
                        property: PropertyInducingElementData::new(data),
                    });
                    store.fragment(fragment).element.set_once(field.raw());
                    push_field(store, element, field);
                    fields.push((decl.name.clone(), field));
                    field
                };
                if !is_representation {
                    if decl.kind == MemberKind::Getter {
                        store.get_mut(field).getter = Some(accessor.0.cast().unwrap());
                    } else {
                        store.get_mut(field).setter = Some(accessor.0.cast().unwrap());
                    }
                }
                pending.push(Pending {
                    executable: accessor.0,
                    decl_index: index,
                    type_parameters: Vec::new(),
                    parameters: accessor.1,
                    field: if is_representation { None } else { Some(field) },
                });
            }
            MemberKind::Method => {
                let fd = fragment_data(&decl.name);
                fd.flags
                    .set(FragmentFlags::METHOD_FRAGMENT_IS_ORIGIN_DECLARATION, true);
                fd.flags
                    .set(FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC, decl.is_static);
                fd.flags.set(
                    FragmentFlags::EXECUTABLE_FRAGMENT_IS_ABSTRACT,
                    decl.is_abstract,
                );
                let mut ef = ExecutableFragmentData::new(fd);
                let mut tps = Vec::new();
                for (name, _) in &decl.type_parameters {
                    let f = store.add_fragment::<TypeParameterFragment>(TypeParameterFragment {
                        fragment: FragmentData::new(Some(names.intern(name)), Some(0)),
                    });
                    let mut data = ElementData::new(Some(names.intern(name)), f.raw());
                    data.library = Some(library);
                    let tp: EId<TypeParameterElement> = store.add(TypeParameterElement::new(data));
                    store.fragment(f).element.set_once(tp.raw());
                    ef.type_params.push(f);
                    tps.push(tp);
                }
                let (p_fragments, params) = add_parameters(store, &decl.parameters, library, names);
                ef.formal_params = p_fragments;
                let fragment =
                    store.add_fragment::<MethodFragment>(MethodFragment { executable: ef });
                let mut executable =
                    ExecutableElementData::new(element_data(&decl.name, fragment.raw()));
                executable.type_params = tps.clone();
                executable.formal_params = params.clone();
                let method: EId<MethodElement> = store.add(MethodElement {
                    executable,
                    is_operator_equal_with_parameter_type_from_object: BoolSlot::new(false),
                    type_inference_error: OnceSlot::new(),
                });
                store.fragment(fragment).element.set_once(method.raw());
                push_method(store, element, method);
                pending.push(Pending {
                    executable: method.raw(),
                    decl_index: index,
                    type_parameters: tps,
                    parameters: params,
                    field: None,
                });
            }
        }
    }

    // Phase 2: types.
    let t: &TypeSystemTest = t;
    let ctx = t.ctx();
    for p in &pending {
        let decl = decls[p.decl_index];
        let mut type_parameters = class_type_parameters.clone();
        type_parameters.extend(p.type_parameters.iter().copied());
        let scope = TypeParsingScope {
            test: t,
            libraries: libraries.to_vec(),
            type_parameters,
        };
        for (i, (_, bound)) in decl.type_parameters.iter().enumerate() {
            if let Some(bound) = bound {
                ctx.get(p.type_parameters[i])
                    .bound
                    .set(Some(scope.parse_type(bound)));
            }
        }
        let is_representation = is_extension_type && p.decl_index == 0;
        let executable_kind = p.executable.tag();
        let return_type = if is_representation {
            let field = ctx.interface(element).fields[0];
            ctx.get(field).type_.get().unwrap()
        } else if executable_kind == Tag::Setter && decl.kind == MemberKind::Setter {
            match &decl.return_type {
                Some(t) => scope.parse_type(t),
                None => TypeId::VOID,
            }
        } else if executable_kind == Tag::Setter {
            // The synthetic setter of a field.
            TypeId::VOID
        } else {
            scope.parse_type(decl.return_type.as_deref().expect(IMPLICIT_TYPE))
        };
        let param_decls: Vec<ParamDecl> =
            if executable_kind == Tag::Setter && decl.kind == MemberKind::Field {
                vec![ParamDecl {
                    name: Some("value".into()),
                    ty: decl.return_type.clone(),
                    kind: ParameterKind::Required,
                    is_covariant: decl.is_covariant,
                }]
            } else {
                decl.parameters
                    .iter()
                    .map(|p| ParamDecl {
                        name: p.name.clone(),
                        ty: p.ty.clone(),
                        kind: p.kind,
                        is_covariant: p.is_covariant,
                    })
                    .collect()
            };
        for (i, &param) in p.parameters.iter().enumerate() {
            let ty = scope.parse_type(param_decls[i].ty.as_deref().expect(IMPLICIT_TYPE));
            ctx.get(param).type_.set(Some(ty));
        }
        let executable = ctx.executable(p.executable.cast().unwrap());
        executable.return_type.set(Some(return_type));
        if let Some(accessor) = p
            .executable
            .cast::<dartr_element::PropertyAccessorElement>()
        {
            let variable = match p.field {
                Some(field) => field,
                None if is_representation => ctx.interface(element).fields[0],
                None => {
                    // The synthetic setter of an explicit field: the field
                    // whose setter it is.
                    let data = ctx.interface(element);
                    *data
                        .fields
                        .iter()
                        .find(|&&f| {
                            ctx.property_inducing(f.upcast()).setter.map(|s| s.raw())
                                == Some(p.executable)
                        })
                        .expect("the field of a setter")
                }
            };
            ctx.property_accessor(accessor)
                .variable
                .set(Some(variable.upcast::<PropertyInducingElement>()));
        }
        if let Some(field) = p.field {
            let field_type = if executable_kind == Tag::Getter {
                return_type
            } else {
                ctx.get(p.parameters[0]).type_.get().unwrap()
            };
            if ctx.get(field).type_.get().is_none() {
                ctx.get(field).type_.set(Some(field_type));
            }
        }
    }
}

fn element_fields_owner(element: EId<InterfaceElement>) -> EId<ExtensionTypeElement> {
    element.raw().cast().expect("an extension type")
}

#[allow(clippy::too_many_arguments)]
fn add_accessor(
    store: &mut dartr_element::ElementStore,
    element: EId<InterfaceElement>,
    kind: MemberKind,
    name: &str,
    params: &[ParamDecl],
    decl: &MemberDecl,
    element_data: &dyn Fn(&str, FragmentId) -> ElementData,
    fragment_data: &dyn Fn(&str) -> FragmentData,
    is_origin_variable: bool,
) -> (ElementId, Vec<EId<FormalParameterElement>>) {
    let library = element_data(name, FragmentId::new(store.id, Tag::Getter, 0))
        .library
        .unwrap();
    let fd = fragment_data(name);
    fd.flags.set(
        if is_origin_variable {
            FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE
        } else {
            FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_DECLARATION
        },
        true,
    );
    fd.flags
        .set(FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC, decl.is_static);
    fd.flags.set(
        FragmentFlags::EXECUTABLE_FRAGMENT_IS_ABSTRACT,
        decl.is_abstract,
    );
    let mut ef = ExecutableFragmentData::new(fd);
    let (p_fragments, p_elements) =
        add_parameters_named(store, params, library, |n| fragment_data(n).name.unwrap());
    ef.formal_params = p_fragments;
    let accessor = PropertyAccessorFragmentData {
        executable: ef,
        inducing_variable: None,
    };
    let fragment = match kind {
        MemberKind::Getter => store
            .add_fragment::<GetterFragment>(GetterFragment { accessor })
            .raw(),
        _ => store
            .add_fragment::<SetterFragment>(SetterFragment { accessor })
            .raw(),
    };
    let mut executable = ExecutableElementData::new(element_data(name, fragment));
    executable.formal_params = p_elements.clone();
    let accessor = PropertyAccessorElementData {
        executable,
        variable: VarSlot::new(),
    };
    let id = match kind {
        MemberKind::Getter => {
            let g: EId<GetterElement> = store.add(GetterElement { accessor });
            push_getter(store, element, g);
            g.raw()
        }
        _ => {
            let s: EId<SetterElement> = store.add(SetterElement { accessor });
            push_setter(store, element, s);
            s.raw()
        }
    };
    store.fragment_data(fragment).unwrap().element.set_once(id);
    (id, p_elements)
}

fn add_parameters(
    store: &mut dartr_element::ElementStore,
    params: &[ParamDecl],
    library: EId<LibraryElement>,
    names: &dartr_element::NamePool,
) -> (
    Vec<dartr_element::FId<FormalParameterFragment>>,
    Vec<EId<FormalParameterElement>>,
) {
    add_parameters_named(store, params, library, |n| names.intern(n))
}

fn add_parameters_named(
    store: &mut dartr_element::ElementStore,
    params: &[ParamDecl],
    library: EId<LibraryElement>,
    intern: impl Fn(&str) -> dartr_element::Name,
) -> (
    Vec<dartr_element::FId<FormalParameterFragment>>,
    Vec<EId<FormalParameterElement>>,
) {
    let mut fragments = Vec::new();
    let mut elements = Vec::new();
    for p in params {
        let name = p.name.as_deref().map(&intern);
        let fd = FragmentData::new(name, Some(0));
        fd.flags.set(
            FragmentFlags::FORMAL_PARAMETER_FRAGMENT_IS_EXPLICITLY_COVARIANT,
            p.is_covariant,
        );
        let fragment = store.add_fragment::<FormalParameterFragment>(FormalParameterFragment {
            variable: VariableFragmentData::new(fd),
            parameter_kind: p.kind,
            private_name: None,
        });
        let mut data = ElementData::new(name, fragment.raw());
        data.library = Some(library);
        data.flags.set(
            ElementFlags::FORMAL_PARAMETER_ELEMENT_IS_COVARIANT,
            p.is_covariant,
        );
        let element: EId<FormalParameterElement> = store.add(FormalParameterElement {
            variable: VariableElementData::new(data),
            kind: p.kind,
            type_: VarSlot::new(),
            base_formal_parameter: None,
            field: VarSlot::new(),
        });
        store.fragment(fragment).element.set_once(element.raw());
        fragments.push(fragment);
        elements.push(element);
    }
    (fragments, elements)
}

macro_rules! push_member {
    ($fn:ident, $field:ident, $ty:ty) => {
        fn $fn(
            store: &mut dartr_element::ElementStore,
            element: EId<InterfaceElement>,
            m: EId<$ty>,
        ) {
            let raw = element.raw();
            match raw.tag() {
                Tag::Class => store
                    .get_mut(EId::<ClassElement>::from_raw(raw))
                    .$field
                    .push(m),
                Tag::Mixin => store
                    .get_mut(EId::<MixinElement>::from_raw(raw))
                    .$field
                    .push(m),
                Tag::ExtensionType => store
                    .get_mut(EId::<ExtensionTypeElement>::from_raw(raw))
                    .$field
                    .push(m),
                Tag::Enum => store
                    .get_mut(EId::<dartr_element::EnumElement>::from_raw(raw))
                    .$field
                    .push(m),
                _ => unreachable!(),
            }
        }
    };
}
push_member!(push_method, methods, MethodElement);
push_member!(push_getter, getters, GetterElement);
push_member!(push_setter, setters, SetterElement);
push_member!(push_field, fields, FieldElement);

/// `element.variable` of an accessor (for `_assertExecutable`).
pub fn accessor_variable(ctx: &Ctx<'_>, accessor: ElemRef) -> Option<ElemRef> {
    member::variable(ctx, accessor)
}

// ------------------------------------------------------------------ printing

/// `ElementPrinter._referenceToString` of a declaration of a library:
/// `<testLibrary>::@class::A::@method::foo`.
pub fn reference_string(ctx: &Ctx<'_>, element: ElementId) -> String {
    let data = ctx.element_data(element).expect("an element with data");
    let kind = match element.tag() {
        Tag::Class => "@class",
        Tag::Mixin => "@mixin",
        Tag::Enum => "@enum",
        Tag::ExtensionType => "@extensionType",
        Tag::Method => "@method",
        Tag::Getter => "@getter",
        Tag::Setter => "@setter",
        Tag::Field => "@field",
        Tag::Constructor => "@constructor",
        tag => panic!("no reference for {tag:?}"),
    };
    let name = ctx.element_name(element).unwrap_or("");
    let container = match data.enclosing {
        Some(e) if e.tag() != Tag::Library => reference_string(ctx, e),
        _ => {
            let uri = ctx.library_uri(data.library.unwrap());
            if uri == TEST_URI {
                "<testLibrary>".to_string()
            } else {
                uri.to_string()
            }
        }
    };
    format!("{container}::{kind}::{name}")
}

/// A small port of `TreeStringSink` + `ElementPrinter.writeElement2`.
pub struct Printer<'a> {
    pub ctx: Ctx<'a>,
    pub out: String,
    pub indent: usize,
}

impl<'a> Printer<'a> {
    pub fn new(ctx: Ctx<'a>) -> Self {
        Printer {
            ctx,
            out: String::new(),
            indent: 0,
        }
    }

    pub fn writeln_with_indent(&mut self, line: &str) {
        for _ in 0..self.indent {
            self.out.push_str("  ");
        }
        self.out.push_str(line);
        self.out.push('\n');
    }

    pub fn with_indent(&mut self, f: impl FnOnce(&mut Self)) {
        self.indent += 1;
        f(self);
        self.indent -= 1;
    }

    /// `writeElement2(element)` after the indent / prefix was written.
    fn element_lines(&self, element: Option<ElemRef>) -> Vec<(usize, String)> {
        let ctx = &self.ctx;
        match element {
            None => vec![(0, "<null>".into())],
            Some(ElemRef::Base(e)) => vec![(0, reference_string(ctx, e))],
            Some(e @ ElemRef::Member(_)) => {
                let base = member::base_element(ctx, e);
                let class = match base.tag() {
                    Tag::Method => "SubstitutedMethodElementImpl",
                    Tag::Getter => "SubstitutedGetterElementImpl",
                    Tag::Setter => "SubstitutedSetterElementImpl",
                    Tag::Field => "SubstitutedFieldElementImpl",
                    Tag::Constructor => "SubstitutedConstructorElementImpl",
                    _ => "SubstitutedFormalParameterElementImpl",
                };
                let mut lines = vec![(0, class.to_string())];
                lines.push((1, format!("baseElement: {}", reference_string(ctx, base))));
                let substitution = member::substitution(ctx, e);
                if !substitution.is_empty() {
                    let entries: Vec<String> = substitution
                        .map
                        .iter()
                        .map(|(&k, &v)| {
                            format!(
                                "{}: {}",
                                ctx.element_name(k.raw()).unwrap_or(""),
                                dartr_element::type_display_string_with(
                                    ctx,
                                    v,
                                    DisplayOptions::default()
                                )
                            )
                        })
                        .collect();
                    lines.push((1, format!("substitution: {{{}}}", entries.join(", "))));
                }
                lines
            }
        }
    }

    /// `writeNamedElement2(name, element)`.
    pub fn write_named_element(&mut self, name: &str, element: Option<ElemRef>) {
        let lines = self.element_lines(element);
        for (i, (extra, line)) in lines.into_iter().enumerate() {
            if i == 0 {
                self.writeln_with_indent(&format!("{name}: {line}"));
            } else {
                self.indent += extra;
                self.writeln_with_indent(&line);
                self.indent -= extra;
            }
        }
    }

    /// `writeElement2(element)` on its own line.
    pub fn write_element(&mut self, element: Option<ElemRef>) {
        let lines = self.element_lines(element);
        for (extra, line) in lines {
            self.indent += extra;
            self.writeln_with_indent(&line);
            self.indent -= extra;
        }
    }

    /// `writeElementList2(name, elements)`.
    pub fn write_element_list(&mut self, name: &str, elements: &[ElemRef]) {
        if elements.is_empty() {
            return;
        }
        self.writeln_with_indent(name);
        self.with_indent(|p| {
            for &e in elements {
                p.write_element(Some(e));
            }
        });
    }
}

/// `_InstancePrinterConfiguration`.
#[derive(Clone, Copy, Default)]
pub struct InterfacePrinterConfiguration {
    pub with_object_members: bool,
    pub without_identical_implemented: bool,
}

/// `_InheritanceManager3Base2._interfaceText(element)` with
/// `_InterfacePrinter`.
pub fn interface_text(
    ctx: Ctx<'_>,
    element: EId<InterfaceElement>,
    configuration: InterfacePrinterConfiguration,
) -> String {
    use dartr_typesystem::inheritance_manager3::{Conflict, NameListMap, NameMap};

    let inheritance = InheritanceManager3::new(ctx);
    let interface = inheritance.get_interface(element);
    // Should not throw.
    inheritance.get_inherited_concrete_map(element);
    // Ensure that `inheritedMap` field is initialized.
    let inherited_map = inheritance.get_inherited_map(element).clone();

    let should_write =
        |e: ElemRef| configuration.with_object_members || !member::is_object_member(&ctx, e);
    let sort_key = |n: &Name| {
        format!(
            "{} {}",
            n.text(&ctx),
            n.library_uri
                .map(|l| ctx.library_uri(l).to_string())
                .unwrap_or_else(|| "null".into())
        )
    };

    let mut p = Printer::new(ctx);
    let write_name_to_map = |p: &mut Printer<'_>, name: &str, map: &NameMap| {
        if !map.values().any(|&e| should_write(e)) {
            return;
        }
        p.writeln_with_indent(name);
        let mut entries: Vec<(&Name, &ElemRef)> = map.iter().collect();
        entries.sort_by_key(|(n, _)| sort_key(n));
        p.with_indent(|p| {
            for (n, &e) in entries {
                if should_write(e) {
                    p.write_named_element(n.text(&ctx), Some(e));
                }
            }
        });
    };
    let write_name_to_list_map = |p: &mut Printer<'_>, name: &str, map: &NameListMap| {
        if !map.values().flatten().any(|&e| should_write(e)) {
            return;
        }
        p.writeln_with_indent(name);
        let mut entries: Vec<(&Name, &Vec<ElemRef>)> = map.iter().collect();
        entries.sort_by_key(|(n, _)| sort_key(n));
        p.with_indent(|p| {
            for (n, list) in entries {
                let list: Vec<ElemRef> =
                    list.iter().copied().filter(|&e| should_write(e)).collect();
                p.write_element_list(n.text(&ctx), &list);
            }
        });
    };

    write_name_to_map(&mut p, "map", &interface.map);
    write_name_to_map(&mut p, "declared", &interface.declared);
    if configuration.without_identical_implemented {
        assert_eq!(interface.implemented, interface.map, "implemented is map");
    } else {
        write_name_to_map(&mut p, "implemented", &interface.implemented);
    }
    write_name_to_list_map(&mut p, "overridden", &interface.overridden);
    write_name_to_list_map(&mut p, "redeclared", &interface.redeclared);
    if !interface.super_implemented.is_empty() {
        p.writeln_with_indent("superImplemented");
        p.with_indent(|p| {
            for (index, map) in interface.super_implemented.iter().enumerate() {
                write_name_to_map(p, &index.to_string(), map);
            }
        });
    }
    write_name_to_map(&mut p, "inheritedMap", &inherited_map);
    if !interface.conflicts.is_empty() {
        p.writeln_with_indent("conflicts");
        p.with_indent(|p| {
            for conflict in &interface.conflicts {
                match conflict {
                    Conflict::Candidates { candidates, .. } => {
                        p.write_element_list("CandidatesConflict", candidates);
                    }
                    Conflict::GetterMethod { getter, method, .. } => {
                        p.writeln_with_indent("GetterMethodConflict");
                        p.with_indent(|p| {
                            p.write_named_element("getter", Some(*getter));
                            p.write_named_element("method", Some(*method));
                        });
                    }
                    Conflict::HasNonExtensionAndExtensionMember {
                        non_extension,
                        extension,
                        ..
                    } => {
                        p.writeln_with_indent("HasNonExtensionAndExtensionMemberConflict");
                        p.with_indent(|p| {
                            p.write_element_list("nonExtension", non_extension);
                            p.write_element_list("extension", extension);
                        });
                    }
                    Conflict::NotUniqueExtensionMember { candidates, .. } => {
                        p.write_element_list("NotUniqueExtensionMemberConflict", candidates);
                    }
                    other => panic!("Not implemented: {}", other.kind_name()),
                }
            }
        });
    }
    p.out
}

/// `_InheritanceManager3Base2._assertGetOverridden` (the list format of
/// `_assertExecutableList` with `<null>`).
pub fn get_overridden_text(
    test: &SourceTest,
    element: EId<InterfaceElement>,
    name: &str,
) -> String {
    let overridden = test.manager().get_overridden(element, test.name(name));
    match overridden {
        None => "<null>\n".into(),
        Some(list) => {
            let mut lines: Vec<String> = list
                .iter()
                .map(|&e| format!("{}\n", test.executable_line(e)))
                .collect();
            lines.sort();
            lines.concat()
        }
    }
}

/// `_InheritanceManager3Base._assertExecutableList`: `None` for `null`.
pub fn executable_list_text(test: &SourceTest, elements: Option<&[ElemRef]>) -> Option<String> {
    let elements = elements?;
    let mut lines: Vec<String> = elements
        .iter()
        .map(|&e| format!("{}\n", test.executable_line(e)))
        .collect();
    lines.sort();
    Some(lines.concat())
}

/// `_InheritanceManager3Base._assertNameToExecutableMap`.
pub fn name_to_executable_map_text(
    test: &SourceTest,
    map: &dartr_typesystem::inheritance_manager3::NameMap,
) -> String {
    let ctx = test.ctx();
    let mut lines = Vec::new();
    for &element in map.values() {
        let enclosing = member::enclosing_element(&ctx, element).and_then(|e| ctx.element_name(e));
        if enclosing == Some("Object") {
            continue;
        }
        lines.push(test.executable_line(element));
    }
    lines.sort();
    if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    }
}
