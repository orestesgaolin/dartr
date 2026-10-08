// Dart source: pkg/analyzer/test/src/dart/element/display_string_test.dart

//! Port of `ElementDisplayStringTest`. The Dart tests resolve source code;
//! dartr cannot resolve yet, so each test builds the same elements:
//! through `TypeSystemTest::build_test_library` (spec strings) where the
//! spec builder supports the declaration, else with the small hand-built
//! helpers below (getters, setters, variables, extensions, local functions,
//! labels, prefixes, directives, class modifiers that the spec parser does
//! not know). The expected strings are the Dart ones.

use std::sync::Arc;

use dartr_element::*;
use dartr_typesystem::TypeExt;
use dartr_typesystem::test_support::*;

// ------------------------------------------------------------------ helpers

/// `element.displayString()`.
fn display_string(t: &TypeSystemTest, element: ElementId) -> String {
    element_display_string_with(&t.ctx(), element, DisplayOptions::default())
}

/// `element.displayString(multiline: true)`.
fn display_string_multiline(t: &TypeSystemTest, element: ElementId) -> String {
    element_display_string_with(
        &t.ctx(),
        element,
        DisplayOptions {
            multiline: true,
            prefer_type_alias: false,
        },
    )
}

/// `element.displayString(preferTypeAlias: true)`.
fn display_string_prefer_type_alias(t: &TypeSystemTest, element: ElementId) -> String {
    element_display_string_with(
        &t.ctx(),
        element,
        DisplayOptions {
            multiline: false,
            prefer_type_alias: true,
        },
    )
}

fn name(t: &TypeSystemTest, text: &str) -> Name {
    t.world.generation.names.intern(text)
}

fn test_library(t: &TypeSystemTest) -> EId<LibraryElement> {
    t.test_library.expect("no test library")
}

/// The library fragment of the test library (`findElement.libraryFragment`).
fn test_unit(t: &TypeSystemTest) -> FId<LibraryFragment> {
    t.ctx().get(test_library(t)).first_fragment()
}

/// An empty test library (`resolveTestCode` of code without declarations
/// that the spec builder knows).
fn empty_test_library(t: &mut TypeSystemTest) {
    t.build_test_library(LibrarySpec::test());
}

/// Element data of a hand-built element of the test library. The display
/// code never reads the first fragment of these elements.
fn data(t: &TypeSystemTest, element_name: Option<&str>, tag: Tag) -> ElementData {
    let library = test_library(t);
    let mut d = ElementData::new(
        element_name.map(|n| name(t, n)),
        FragmentId::new(t.store.id, tag, 0),
    );
    d.library = Some(library);
    d.enclosing = Some(library.raw());
    d
}

fn type_parameter(
    t: &TypeSystemTest,
    tp_name: &str,
    bound: Option<TypeId>,
) -> EId<TypeParameterElement> {
    let e = TypeParameterElement::new(data(t, Some(tp_name), Tag::TypeParameter));
    e.bound.set(bound);
    t.store.add(e)
}

fn formal_parameter(
    t: &TypeSystemTest,
    param_name: &str,
    kind: ParameterKind,
    ty: TypeId,
) -> EId<FormalParameterElement> {
    let e = FormalParameterElement {
        variable: VariableElementData::new(data(t, Some(param_name), Tag::FormalParameter)),
        kind,
        type_: VarSlot::with(ty),
        base_formal_parameter: None,
        field: VarSlot::new(),
    };
    t.store.add(e)
}

fn executable(
    t: &TypeSystemTest,
    element_name: &str,
    tag: Tag,
    type_params: Vec<EId<TypeParameterElement>>,
    params: Vec<EId<FormalParameterElement>>,
    return_type: TypeId,
) -> ExecutableElementData {
    let mut e = ExecutableElementData::new(data(t, Some(element_name), tag));
    e.type_params = type_params;
    e.formal_params = params;
    e.return_type.set(Some(return_type));
    e
}

/// A top-level variable (`findElement.topVar`).
fn top_var(t: &TypeSystemTest, var_name: &str, ty: TypeId) -> EId<TopLevelVariableElement> {
    let p = PropertyInducingElementData::new(data(t, Some(var_name), Tag::TopLevelVariable));
    p.type_.set(Some(ty));
    t.store.add(TopLevelVariableElement { property: p })
}

/// A getter (`findElement.topGet`).
fn getter(t: &TypeSystemTest, getter_name: &str, return_type: TypeId) -> EId<GetterElement> {
    t.store.add(GetterElement {
        accessor: PropertyAccessorElementData {
            executable: executable(t, getter_name, Tag::Getter, vec![], vec![], return_type),
            variable: VarSlot::new(),
        },
    })
}

/// A setter `set <name>(<type> value)` (`findElement.topSet` / `setter`).
fn setter(t: &TypeSystemTest, setter_name: &str, value_type: TypeId) -> EId<SetterElement> {
    let value = formal_parameter(t, "value", ParameterKind::Required, value_type);
    t.store.add(SetterElement {
        accessor: PropertyAccessorElementData {
            executable: executable(
                t,
                setter_name,
                Tag::Setter,
                vec![],
                vec![value],
                TypeId::VOID,
            ),
            variable: VarSlot::new(),
        },
    })
}

/// A local function (`findElement.localFunction`).
fn local_function(
    t: &TypeSystemTest,
    function_name: &str,
    type_params: Vec<EId<TypeParameterElement>>,
    params: Vec<EId<FormalParameterElement>>,
    return_type: TypeId,
) -> EId<LocalFunctionElement> {
    t.store.add(LocalFunctionElement {
        executable: executable(
            t,
            function_name,
            Tag::LocalFunction,
            type_params,
            params,
            return_type,
        ),
    })
}

/// An extension `extension <name> on <extended type>`.
fn extension(
    t: &TypeSystemTest,
    extension_name: Option<&str>,
    extended_type: TypeId,
) -> EId<ExtensionElement> {
    t.store.add(ExtensionElement {
        instance: InstanceElementData::new(data(t, extension_name, Tag::Extension)),
        extended_type: VarSlot::with(extended_type),
    })
}

/// `findElement.parameter(name)` in the top-level function [function].
fn parameter(t: &TypeSystemTest, function: &str, param_name: &str) -> EId<FormalParameterElement> {
    let ctx = t.ctx();
    ctx.get(t.top_level_function(function))
        .formal_params
        .iter()
        .copied()
        .find(|&p| ctx.element_name(p.raw()) == Some(param_name))
        .unwrap_or_else(|| panic!("no parameter {param_name}"))
}

/// `findElement.typeParameter(name)` of the top-level function [function].
fn function_type_parameter(t: &TypeSystemTest, function: &str) -> EId<TypeParameterElement> {
    t.ctx().get(t.top_level_function(function)).type_params[0]
}

/// Sets class modifiers that the spec parser does not know (`base`,
/// `final`, `interface`, `mixin class`).
fn set_class_modifiers(
    t: &TypeSystemTest,
    class: EId<ClassElement>,
    element_flags: ElementFlags,
    fragment_flags: FragmentFlags,
) {
    let ctx = t.ctx();
    let data = ctx.get(class);
    data.flags.set(element_flags, true);
    ctx.fragment(data.first_fragment())
        .flags
        .set(fragment_flags, true);
}

/// The URI of a directive whose target does not exist: Dart still has a
/// `DirectiveUriWithSourceImpl` (a `Source` for the missing file).
fn uri_with_source(relative: &str, absolute: &str) -> DirectiveUri {
    DirectiveUri::Source {
        relative_uri_string: Arc::from(relative),
        relative_uri: Arc::from(absolute),
        source: SourceRef {
            path: Arc::from(format!("/home/test/lib/{relative}")),
            uri: Arc::from(absolute),
        },
    }
}

fn directive(t: &TypeSystemTest, uri: DirectiveUri) -> ElementDirective {
    ElementDirective {
        library_fragment: test_unit(t),
        uri,
        metadata: Metadata::default(),
    }
}

fn library_import(
    t: &TypeSystemTest,
    uri: DirectiveUri,
    prefix: Option<FId<PrefixFragment>>,
) -> LibraryImport {
    LibraryImport {
        directive: directive(t, uri),
        is_synthetic: false,
        combinators: vec![],
        import_keyword_offset: 0,
        prefix,
        namespace: OnceSlot::new(),
    }
}

/// `import '<uri>' as <prefix>;` for each URI: one prefix element with one
/// fragment, used by every import.
fn prefix_with_imports(
    t: &mut TypeSystemTest,
    prefix_name: &str,
    uris: &[&str],
) -> EId<PrefixElement> {
    let unit = test_unit(t);
    let mut fragment = FragmentData::new(Some(name(t, prefix_name)), Some(0));
    fragment.enclosing_fragment = Some(unit.raw());
    let prefix_fragment = t.store.add_fragment::<PrefixFragment>(PrefixFragment {
        fragment,
        offset: 0,
        is_deferred: false,
    });
    let mut element = ElementData::new(Some(name(t, prefix_name)), prefix_fragment.raw());
    element.library = Some(test_library(t));
    element.enclosing = Some(test_library(t).raw());
    let prefix: EId<PrefixElement> = t.store.add(PrefixElement {
        element,
        local_id: name(t, prefix_name),
        last_fragment: prefix_fragment,
    });
    t.store
        .fragment(prefix_fragment)
        .element
        .set_once(prefix.raw());
    let imports: Vec<LibraryImport> = uris
        .iter()
        .map(|uri| {
            let absolute = format!("package:test/{uri}");
            library_import(t, uri_with_source(uri, &absolute), Some(prefix_fragment))
        })
        .collect();
    t.store.fragment_mut(unit).library_imports.extend(imports);
    prefix
}

// ------------------------------------------------------------------ tests

#[test]
fn class() {
    // class A {}
    // abstract class B<T> extends A {}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        classes: vec![
            ClassSpec::new("class A"),
            ClassSpec::new("abstract class B<T> extends A"),
        ],
        ..LibrarySpec::test()
    });
    let b = t.class_element("B");
    assert_eq!(display_string(&t, b.raw()), "abstract class B<T> extends A");
}

#[test]
fn extension_named() {
    // extension StringExtension on String {}
    let mut t = TypeSystemTest::new();
    empty_test_library(&mut t);
    let element = extension(&t, Some("StringExtension"), t.parse_type("String"));
    assert_eq!(
        display_string(&t, element.raw()),
        "extension StringExtension on String"
    );
}

#[test]
fn extension_unnamed() {
    // extension on String {}
    let mut t = TypeSystemTest::new();
    empty_test_library(&mut t);
    let element = extension(&t, None, t.parse_type("String"));
    assert_eq!(display_string(&t, element.raw()), "extension on String");
}

#[test]
fn extension_type() {
    // extension type MyString<T>(String it) implements String {}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        extension_types: strs(&["extension type MyString<T>(String it) implements String"]),
        ..LibrarySpec::test()
    });
    let element = t.extension_type_element("MyString");
    assert_eq!(
        display_string(&t, element.raw()),
        "extension type MyString<T>(String it) implements String"
    );
}

#[test]
#[ignore = "needs the default value code (` = 'a'`), which the element model does not keep"]
fn long_method() {
    // abstract class A {
    //   String? longMethodName(String? aaa, [String? bbb = 'a', String? ccc]);
    // }
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        classes: vec![
            ClassSpec::new("abstract class A")
                .methods(&["String? longMethodName(String? aaa, [String? bbb, String? ccc])"]),
        ],
        ..LibrarySpec::test()
    });
    let method_element = t.method(t.class_element("A").upcast(), "longMethodName");
    let single_line = display_string(&t, method_element.raw());
    assert_eq!(
        single_line,
        "String? longMethodName(String? aaa, [String? bbb = 'a', String? ccc])"
    );

    let multi_line = display_string_multiline(&t, method_element.raw());
    assert_eq!(
        multi_line,
        "String? longMethodName(\n  String? aaa, [\n  String? bbb = 'a',\n  String? ccc,\n])"
    );
}

#[test]
fn long_method_function_type() {
    // abstract class A {
    //   String? longMethodName(
    //     String? aaa,
    //     [String? Function(String?, String?, String?) bbb,
    //     String? ccc]
    //   );
    // }
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        classes: vec![
            ClassSpec::new("abstract class A").methods(&["String? longMethodName(String? aaa, \
             [String? Function(String?, String?, String?) bbb, String? ccc])"]),
        ],
        ..LibrarySpec::test()
    });
    let method_element = t.method(t.class_element("A").upcast(), "longMethodName");
    let single_line = display_string(&t, method_element.raw());
    assert_eq!(
        single_line,
        "String? longMethodName(String? aaa, \
         [String? Function(String?, String?, String?) bbb, String? ccc])"
    );

    let multi_line = display_string_multiline(&t, method_element.raw());
    assert_eq!(
        multi_line,
        "String? longMethodName(\n  String? aaa, [\n  \
         String? Function(String?, String?, String?) bbb,\n  String? ccc,\n])"
    );
}

#[test]
fn maybe_write_type_alias() {
    // typedef A = int;
    // A f() { throw 0; }
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        type_aliases: strs(&["typedef A = int"]),
        functions: strs(&["A f()"]),
        ..LibrarySpec::test()
    });
    let element = t.top_level_function("f");
    assert_eq!(display_string_prefer_type_alias(&t, element.raw()), "A f()");
}

#[test]
fn maybe_write_type_alias_nullability_non_nullable_aliased_type() {
    // typedef A = int;
    // A f1() { throw 0; }
    // A? f2() { throw 0; }
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        type_aliases: strs(&["typedef A = int"]),
        functions: strs(&["A f1()", "A? f2()"]),
        ..LibrarySpec::test()
    });
    let f1 = t.top_level_function("f1");
    assert_eq!(display_string_prefer_type_alias(&t, f1.raw()), "A f1()");

    let f2 = t.top_level_function("f2");
    assert_eq!(display_string_prefer_type_alias(&t, f2.raw()), "A? f2()");
}

#[test]
fn maybe_write_type_alias_nullability_nullable_aliased_type() {
    // typedef A = int?;
    // A f1() { throw 0; }
    // A? f2() { throw 0; }
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        type_aliases: strs(&["typedef A = int?"]),
        functions: strs(&["A f1()", "A? f2()"]),
        ..LibrarySpec::test()
    });
    let f1 = t.top_level_function("f1");
    assert_eq!(display_string_prefer_type_alias(&t, f1.raw()), "A f1()");

    // The resolver instantiates `A?` with the `?` suffix, so the alias gets
    // the suffix even though the aliased type `int?` is already nullable.
    // The spec builder (Dart and Rust) writes `A?` as
    // `A.withNullability(question)`, which returns the same `int?` with the
    // alias `A` without suffix; so set the resolver's return type here.
    let f2 = t.top_level_function("f2");
    let ctx = t.ctx();
    let a_question =
        ctx.instantiate_type_alias(t.type_alias_element("A"), &[], Nullability::Question);
    ctx.get(f2).return_type.set(Some(a_question));
    assert_eq!(display_string_prefer_type_alias(&t, f2.raw()), "A? f2()");
}

#[test]
fn maybe_write_type_alias_type_arguments() {
    // typedef A<T> = List<T>;
    // A<int> f() { throw 0; }
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        type_aliases: strs(&["typedef A<T> = List<T>"]),
        functions: strs(&["A<int> f()"]),
        ..LibrarySpec::test()
    });
    let element = t.top_level_function("f");
    assert_eq!(
        display_string_prefer_type_alias(&t, element.raw()),
        "A<int> f()"
    );
}

#[test]
fn property_getter() {
    // String get a => '';
    let mut t = TypeSystemTest::new();
    empty_test_library(&mut t);
    let element = getter(&t, "a", t.parse_type("String"));
    assert_eq!(display_string(&t, element.raw()), "String get a");
}

#[test]
fn property_setter() {
    // set a(String value) {}
    let mut t = TypeSystemTest::new();
    empty_test_library(&mut t);
    let element = setter(&t, "a", t.parse_type("String"));
    assert_eq!(display_string(&t, element.raw()), "set a(String value)");
}

#[test]
fn short_method() {
    // abstract class A {
    //   String? m(String? a, [String? b]);
    // }
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        classes: vec![
            ClassSpec::new("abstract class A").methods(&["String? m(String? a, [String? b])"]),
        ],
        ..LibrarySpec::test()
    });
    let element = t.method(t.class_element("A").upcast(), "m");
    let single_line = display_string(&t, element.raw());
    assert_eq!(single_line, "String? m(String? a, [String? b])");

    let multi_line = display_string_multiline(&t, element.raw());
    // The signature is short enough that it remains on one line even for
    // multiline: true.
    assert_eq!(multi_line, "String? m(String? a, [String? b])");
}

#[test]
fn write_class_element_base() {
    // base class A {}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        classes: vec![ClassSpec::new("class A")],
        ..LibrarySpec::test()
    });
    let element = t.class_element("A");
    set_class_modifiers(
        &t,
        element,
        ElementFlags::CLASS_ELEMENT_IS_BASE,
        FragmentFlags::CLASS_FRAGMENT_IS_BASE,
    );
    assert_eq!(display_string(&t, element.raw()), "base class A");
}

#[test]
fn write_class_element_extends() {
    // class B {}
    // class A extends B {}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        classes: vec![
            ClassSpec::new("class B"),
            ClassSpec::new("class A extends B"),
        ],
        ..LibrarySpec::test()
    });
    let element = t.class_element("A");
    assert_eq!(display_string(&t, element.raw()), "class A extends B");
}

#[test]
fn write_class_element_final() {
    // final class A {}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        classes: vec![ClassSpec::new("class A")],
        ..LibrarySpec::test()
    });
    let element = t.class_element("A");
    set_class_modifiers(
        &t,
        element,
        ElementFlags::CLASS_ELEMENT_IS_FINAL,
        FragmentFlags::CLASS_FRAGMENT_IS_FINAL,
    );
    assert_eq!(display_string(&t, element.raw()), "final class A");
}

#[test]
fn write_class_element_implements() {
    // class B {}
    // class A implements B {}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        classes: vec![
            ClassSpec::new("class B"),
            ClassSpec::new("class A implements B"),
        ],
        ..LibrarySpec::test()
    });
    let element = t.class_element("A");
    assert_eq!(display_string(&t, element.raw()), "class A implements B");
}

#[test]
fn write_class_element_interface() {
    // interface class A {}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        classes: vec![ClassSpec::new("class A")],
        ..LibrarySpec::test()
    });
    let element = t.class_element("A");
    set_class_modifiers(
        &t,
        element,
        ElementFlags::CLASS_ELEMENT_IS_INTERFACE,
        FragmentFlags::CLASS_FRAGMENT_IS_INTERFACE,
    );
    assert_eq!(display_string(&t, element.raw()), "interface class A");
}

#[test]
fn write_class_element_mixin() {
    // mixin class A {}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        classes: vec![ClassSpec::new("class A")],
        ..LibrarySpec::test()
    });
    let element = t.class_element("A");
    set_class_modifiers(
        &t,
        element,
        ElementFlags::EMPTY,
        FragmentFlags::CLASS_FRAGMENT_IS_MIXIN_CLASS,
    );
    assert_eq!(display_string(&t, element.raw()), "mixin class A");
}

#[test]
fn write_class_element_sealed() {
    // sealed class A {}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        classes: vec![ClassSpec::new("sealed class A")],
        ..LibrarySpec::test()
    });
    let element = t.class_element("A");
    assert_eq!(display_string(&t, element.raw()), "sealed class A");
}

#[test]
fn write_class_element_super_interfaces() {
    // class E {}
    // mixin W {}
    // class I {}
    // class A extends E with W implements I {}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        classes: vec![
            ClassSpec::new("class E"),
            ClassSpec::new("class I"),
            ClassSpec::new("class A extends E with W implements I"),
        ],
        mixins: strs(&["mixin W"]),
        ..LibrarySpec::test()
    });
    let element = t.class_element("A");
    assert_eq!(
        display_string(&t, element.raw()),
        "class A extends E with W implements I"
    );
}

#[test]
fn write_class_element_type_parameters() {
    // class A<T, S extends num>{}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        classes: vec![ClassSpec::new("class A<T, S extends num>")],
        ..LibrarySpec::test()
    });
    let element = t.class_element("A");
    assert_eq!(
        display_string(&t, element.raw()),
        "class A<T, S extends num>"
    );
}

#[test]
fn write_class_element_with() {
    // mixin B {}
    // class A with B {}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        classes: vec![ClassSpec::new("class A with B")],
        mixins: strs(&["mixin B"]),
        ..LibrarySpec::test()
    });
    let element = t.class_element("A");
    assert_eq!(display_string(&t, element.raw()), "class A with B");
}

#[test]
fn write_constructor_element_explicit_named() {
    // final class A {
    //   A.named();
    // }
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        classes: vec![ClassSpec::new("class A").constructors(&["named()"])],
        ..LibrarySpec::test()
    });
    let class = t.class_element("A");
    let element = t.constructor(class.upcast(), "named");
    assert_eq!(display_string(&t, element.raw()), "A.named()");
}

#[test]
fn write_constructor_element_explicit_unnamed() {
    // final class A {
    //   A();
    // }
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        classes: vec![ClassSpec::new("class A").constructors(&["new()"])],
        ..LibrarySpec::test()
    });
    let class = t.class_element("A");
    let element = t.constructor(class.upcast(), "new");
    assert_eq!(display_string(&t, element.raw()), "A()");
}

#[test]
fn write_constructor_element_formal_parameters() {
    // final class A {
    //   A(int a, bool b, {String? c});
    // }
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        classes: vec![ClassSpec::new("class A").constructors(&["new(int a, bool b, {String? c})"])],
        ..LibrarySpec::test()
    });
    let class = t.class_element("A");
    let element = t.constructor(class.upcast(), "new");
    assert_eq!(
        display_string(&t, element.raw()),
        "A(int a, bool b, {String? c})"
    );
}

#[test]
fn write_constructor_element_synthetic() {
    // final class A {}
    // The linker adds the synthetic unnamed constructor `A()`; the spec
    // builder does not, so it is declared as `new()` (same element data:
    // name `new`, no parameters).
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        classes: vec![ClassSpec::new("class A").constructors(&["new()"])],
        ..LibrarySpec::test()
    });
    let class = t.class_element("A");
    let element = t.constructor(class.upcast(), "new");
    assert_eq!(display_string(&t, element.raw()), "A()");
}

#[test]
fn write_directive_uri() {
    // import 'src/f.dart';
    let mut t = TypeSystemTest::new();
    empty_test_library(&mut t);
    let import = library_import(
        &t,
        uri_with_source("src/f.dart", "package:test/src/f.dart"),
        None,
    );
    assert_eq!(
        library_import_display_string(&import),
        "import package:test/src/f.dart"
    );
}

#[test]
fn write_dynamic_element() {
    let t = TypeSystemTest::new();
    let element = ElementId::DYNAMIC;
    assert_eq!(display_string(&t, element), "dynamic");
}

#[test]
fn write_dynamic_type() {
    // void f(x) {}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        functions: strs(&["void f(dynamic x)"]),
        ..LibrarySpec::test()
    });
    let element = parameter(&t, "f", "x");
    assert_eq!(display_string(&t, element.raw()), "dynamic x");
}

#[test]
fn write_enum_element() {
    // enum E {a, b}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        enums: vec![EnumSpec::new("enum E").constants(&["a", "b"])],
        ..LibrarySpec::test()
    });
    let element = t.enum_element("E");
    assert_eq!(display_string(&t, element.raw()), "enum E");
}

#[test]
fn write_enum_element_implements() {
    // class A {}
    // enum E implements A {a, b}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        classes: vec![ClassSpec::new("class A")],
        enums: vec![EnumSpec::new("enum E implements A").constants(&["a", "b"])],
        ..LibrarySpec::test()
    });
    let element = t.enum_element("E");
    assert_eq!(display_string(&t, element.raw()), "enum E implements A");
}

#[test]
fn write_enum_element_mixin() {
    // mixin M {}
    // enum E with M {a, b}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        mixins: strs(&["mixin M"]),
        enums: vec![EnumSpec::new("enum E with M").constants(&["a", "b"])],
        ..LibrarySpec::test()
    });
    let element = t.enum_element("E");
    assert_eq!(display_string(&t, element.raw()), "enum E with M");
}

#[test]
fn write_enum_element_super_interfaces() {
    // mixin M {}
    // class C {}
    // enum E with M implements C {a, b}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        classes: vec![ClassSpec::new("class C")],
        mixins: strs(&["mixin M"]),
        enums: vec![EnumSpec::new("enum E with M implements C").constants(&["a", "b"])],
        ..LibrarySpec::test()
    });
    let element = t.enum_element("E");
    assert_eq!(
        display_string(&t, element.raw()),
        "enum E with M implements C"
    );
}

#[test]
fn write_enum_element_type_parameters() {
    // enum E<T> {a, b}
    // The enum spec has no type parameters: add `T` by hand.
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        enums: vec![EnumSpec::new("enum E").constants(&["a", "b"])],
        ..LibrarySpec::test()
    });
    let element = t.enum_element("E");
    let tp = type_parameter(&t, "T", None);
    t.store.get_mut(element).type_params = vec![tp];
    assert_eq!(display_string(&t, element.raw()), "enum E<T>");
}

#[test]
fn write_formal_parameter_element_is_named() {
    // void f({required int? a}){}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        functions: strs(&["void f({required int? a})"]),
        ..LibrarySpec::test()
    });
    let element = parameter(&t, "f", "a");
    assert_eq!(display_string(&t, element.raw()), "{required int? a}");
}

#[test]
fn write_formal_parameter_element_is_optional_positional() {
    // void f([int? a]){}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        functions: strs(&["void f([int? a])"]),
        ..LibrarySpec::test()
    });
    let element = parameter(&t, "f", "a");
    assert_eq!(display_string(&t, element.raw()), "[int? a]");
}

#[test]
fn write_generic_function_type_element() {
    // void Function(int a)? f;
    let mut t = TypeSystemTest::new();
    empty_test_library(&mut t);
    let a = formal_parameter(&t, "a", ParameterKind::Required, t.parse_type("int"));
    let element: EId<GenericFunctionTypeElement> = t.store.add(GenericFunctionTypeElement {
        element: data(&t, None, Tag::GenericFunctionType),
        type_params: vec![],
        formal_params: vec![a],
        return_type: VarSlot::with(TypeId::VOID),
        type_: VarSlot::new(),
    });
    assert_eq!(display_string(&t, element.raw()), "void Function(int a)");
}

#[test]
fn write_invalid_type() {
    // nonexistentType a;
    // The resolver gives the unresolved type `InvalidType`.
    let mut t = TypeSystemTest::new();
    empty_test_library(&mut t);
    let element = top_var(&t, "a", TypeId::INVALID);
    assert_eq!(display_string(&t, element.raw()), "InvalidType a");
}

#[test]
fn write_label_element() {
    // void f() {
    //   f: 0;
    // }
    let mut t = TypeSystemTest::new();
    empty_test_library(&mut t);
    let element: EId<LabelElement> = t.store.add(LabelElement {
        element: data(&t, Some("f"), Tag::Label),
    });
    assert_eq!(display_string(&t, element.raw()), "f");
}

#[test]
fn write_library_element() {
    // library f;
    let mut t = TypeSystemTest::new();
    empty_test_library(&mut t);
    let element = test_library(&t);
    assert_eq!(
        display_string(&t, element.raw()),
        "library package:test/test.dart"
    );
}

#[test]
fn write_library_export() {
    // export 'src/f.dart';
    let mut t = TypeSystemTest::new();
    empty_test_library(&mut t);
    let export = LibraryExport {
        directive: directive(&t, uri_with_source("src/f.dart", "package:test/src/f.dart")),
        combinators: vec![],
        export_keyword_offset: 0,
    };
    assert_eq!(
        library_export_display_string(&export),
        "export package:test/src/f.dart"
    );
}

#[test]
fn write_library_import() {
    // import 'src/f.dart';
    let mut t = TypeSystemTest::new();
    empty_test_library(&mut t);
    let import = library_import(
        &t,
        uri_with_source("src/f.dart", "package:test/src/f.dart"),
        None,
    );
    assert_eq!(
        library_import_display_string(&import),
        "import package:test/src/f.dart"
    );
}

#[test]
fn write_local_function_element() {
    // void f() {
    //   void g() {}
    // }
    let mut t = TypeSystemTest::new();
    empty_test_library(&mut t);
    let element = local_function(&t, "g", vec![], vec![], TypeId::VOID);
    assert_eq!(display_string(&t, element.raw()), "void g()");
}

#[test]
fn write_local_function_element_formal_parameters() {
    // void f() {
    //   void g(int a, bool b, {String? c}) {}
    // }
    let mut t = TypeSystemTest::new();
    empty_test_library(&mut t);
    let a = formal_parameter(&t, "a", ParameterKind::Required, t.parse_type("int"));
    let b = formal_parameter(&t, "b", ParameterKind::Required, t.parse_type("bool"));
    let c = formal_parameter(&t, "c", ParameterKind::Named, t.parse_type("String?"));
    let element = local_function(&t, "g", vec![], vec![a, b, c], TypeId::VOID);
    assert_eq!(
        display_string(&t, element.raw()),
        "void g(int a, bool b, {String? c})"
    );
}

#[test]
fn write_local_function_element_type_parameters() {
    // void f() {
    //   void g<T, S extends num>() {}
    // }
    let mut t = TypeSystemTest::new();
    empty_test_library(&mut t);
    let tp_t = type_parameter(&t, "T", None);
    let tp_s = type_parameter(&t, "S", Some(t.parse_type("num")));
    let element = local_function(&t, "g", vec![tp_t, tp_s], vec![], TypeId::VOID);
    assert_eq!(
        display_string(&t, element.raw()),
        "void g<T, S extends num>()"
    );
}

#[test]
fn write_mixin_element() {
    // mixin M {}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        mixins: strs(&["mixin M"]),
        ..LibrarySpec::test()
    });
    let element = t.mixin_element("M");
    assert_eq!(display_string(&t, element.raw()), "mixin M on Object");
}

#[test]
fn write_mixin_element_base() {
    // base mixin M {}
    // The mixin spec has no `base`: set the fragment flag by hand.
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        mixins: strs(&["mixin M"]),
        ..LibrarySpec::test()
    });
    let element = t.mixin_element("M");
    let ctx = t.ctx();
    ctx.fragment(ctx.get(element).first_fragment())
        .flags
        .set(FragmentFlags::MIXIN_FRAGMENT_IS_BASE, true);
    assert_eq!(display_string(&t, element.raw()), "base mixin M on Object");
}

#[test]
fn write_mixin_element_implements() {
    // class A{}
    // mixin M implements A {}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        classes: vec![ClassSpec::new("class A")],
        mixins: strs(&["mixin M implements A"]),
        ..LibrarySpec::test()
    });
    let element = t.mixin_element("M");
    assert_eq!(
        display_string(&t, element.raw()),
        "mixin M on Object implements A"
    );
}

#[test]
fn write_mixin_element_type_parameters() {
    // mixin M<T, S extends num> {}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        mixins: strs(&["mixin M<T, S extends num>"]),
        ..LibrarySpec::test()
    });
    let element = t.mixin_element("M");
    assert_eq!(
        display_string(&t, element.raw()),
        "mixin M<T, S extends num> on Object"
    );
}

#[test]
fn write_never_element() {
    let t = TypeSystemTest::new();
    let element = ElementId::NEVER;
    assert_eq!(display_string(&t, element), "Never");
}

#[test]
fn write_never_type() {
    // Never a;
    let mut t = TypeSystemTest::new();
    empty_test_library(&mut t);
    let element = top_var(&t, "a", t.parse_type("Never"));
    assert_eq!(display_string(&t, element.raw()), "Never a");
}

#[test]
fn write_part_include() {
    // part 'src/f.dart';
    let mut t = TypeSystemTest::new();
    empty_test_library(&mut t);
    let element = PartInclude {
        directive: directive(&t, uri_with_source("src/f.dart", "package:test/src/f.dart")),
        part_keyword_offset: 0,
    };
    assert_eq!(
        part_include_display_string(&element),
        "part package:test/src/f.dart"
    );
}

#[test]
fn write_prefix_element_multiple_imports() {
    // import 'src/f.dart' as a;
    // import 'src/bar.dart' as a;
    let mut t = TypeSystemTest::new();
    empty_test_library(&mut t);
    let prefix = prefix_with_imports(&mut t, "a", &["src/f.dart", "src/bar.dart"]);
    assert_eq!(
        display_string(&t, prefix.raw()),
        "import 'src/f.dart' as a;\nimport 'src/bar.dart' as a;"
    );
}

#[test]
fn write_prefix_element_single_import() {
    // import 'src/f.dart' as a;
    let mut t = TypeSystemTest::new();
    empty_test_library(&mut t);
    let prefix = prefix_with_imports(&mut t, "a", &["src/f.dart"]);
    assert_eq!(
        display_string(&t, prefix.raw()),
        "import 'src/f.dart' as a;"
    );
}

#[test]
fn write_record_type_named() {
    // typedef A = ({int a, String b});
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        type_aliases: strs(&["typedef A = ({int a, String b})"]),
        ..LibrarySpec::test()
    });
    let type_alias = t.type_alias_element("A");
    assert_eq!(
        display_string(&t, type_alias.raw()),
        "typedef A = ({int a, String b})"
    );
}

#[test]
fn write_record_type_nullable() {
    // typedef A = (int, String)?;
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        type_aliases: strs(&["typedef A = (int, String)?"]),
        ..LibrarySpec::test()
    });
    let type_alias = t.type_alias_element("A");
    assert_eq!(
        display_string(&t, type_alias.raw()),
        "typedef A = (int, String)?"
    );
}

#[test]
fn write_record_type_positional() {
    // typedef A = (int, String);
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        type_aliases: strs(&["typedef A = (int, String)"]),
        ..LibrarySpec::test()
    });
    let type_alias = t.type_alias_element("A");
    assert_eq!(
        display_string(&t, type_alias.raw()),
        "typedef A = (int, String)"
    );
}

#[test]
fn write_record_type_positional_and_named() {
    // typedef A = (int, String, {bool flag});
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        type_aliases: strs(&["typedef A = (int, String, {bool flag})"]),
        ..LibrarySpec::test()
    });
    let type_alias = t.type_alias_element("A");
    assert_eq!(
        display_string(&t, type_alias.raw()),
        "typedef A = (int, String, {bool flag})"
    );
}

#[test]
fn write_record_type_single_positional() {
    // typedef A = (int,);
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        type_aliases: strs(&["typedef A = (int,)"]),
        ..LibrarySpec::test()
    });
    let type_alias = t.type_alias_element("A");
    assert_eq!(display_string(&t, type_alias.raw()), "typedef A = (int,)");
}

#[test]
fn write_setter_element() {
    // class A {
    //   set f(int value) {}
    // }
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        classes: vec![ClassSpec::new("class A")],
        ..LibrarySpec::test()
    });
    let setter = setter(&t, "f", t.parse_type("int"));
    assert_eq!(display_string(&t, setter.raw()), "set f(int value)");
}

#[test]
fn write_top_level_function_element() {
    // int f() => 0;
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        functions: strs(&["int f()"]),
        ..LibrarySpec::test()
    });
    let function = t.top_level_function("f");
    assert_eq!(display_string(&t, function.raw()), "int f()");
}

#[test]
fn write_top_level_function_element_formal_parameters() {
    // void f(int x, String y) {}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        functions: strs(&["void f(int x, String y)"]),
        ..LibrarySpec::test()
    });
    let function = t.top_level_function("f");
    assert_eq!(
        display_string(&t, function.raw()),
        "void f(int x, String y)"
    );
}

#[test]
fn write_top_level_function_element_type_parameters() {
    // void f<T, S extends num>() {}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        functions: strs(&["void f<T, S extends num>()"]),
        ..LibrarySpec::test()
    });
    let function = t.top_level_function("f");
    assert_eq!(
        display_string(&t, function.raw()),
        "void f<T, S extends num>()"
    );
}

#[test]
fn write_type_alias_element_with_aliased_element() {
    // typedef A = int;
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        type_aliases: strs(&["typedef A = int"]),
        ..LibrarySpec::test()
    });
    let type_alias = t.type_alias_element("A");
    assert_eq!(display_string(&t, type_alias.raw()), "typedef A = int");
}

#[test]
fn write_type_alias_element_with_aliased_element_type_parameters() {
    // typedef A<T> = List<T>;
    // The linker computes the variance of the type parameters of a type
    // alias (`out` here); the spec builder does not, so it is set by hand.
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        type_aliases: strs(&["typedef A<T> = List<T>"]),
        ..LibrarySpec::test()
    });
    let type_alias = t.type_alias_element("A");
    let tp = t.ctx().get(type_alias).type_params[0];
    t.store.get_mut(tp).variance = Some(Variance::Covariant);
    assert_eq!(
        display_string(&t, type_alias.raw()),
        "typedef A<out T> = List<T>"
    );
}

#[test]
fn write_type_arguments() {
    // Map<String, double> a = {'A': 1.5};
    let mut t = TypeSystemTest::new();
    empty_test_library(&mut t);
    let element = top_var(&t, "a", t.parse_type("Map<String, double>"));
    assert_eq!(display_string(&t, element.raw()), "Map<String, double> a");
}

#[test]
fn write_type_parameter_element() {
    // void f<T extends num>() {}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        functions: strs(&["void f<T extends num>()"]),
        ..LibrarySpec::test()
    });
    let element = function_type_parameter(&t, "f");
    assert_eq!(display_string(&t, element.raw()), "T extends num");
}

#[test]
fn write_type_parameter_element_covariant() {
    // class A<in T> {}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        classes: vec![ClassSpec::new("class A<in T>")],
        ..LibrarySpec::test()
    });
    let element_a = t.ctx().get(t.class_element("A")).type_params[0];
    assert_eq!(display_string(&t, element_a.raw()), "in T");
}

#[test]
fn write_type_parameter_type() {
    // void f<T>(T t) {}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        functions: strs(&["void f<T>(T t)"]),
        ..LibrarySpec::test()
    });
    let type_alias = parameter(&t, "f", "t");
    assert_eq!(display_string(&t, type_alias.raw()), "T t");
}

#[test]
fn write_type_parameter_type_promoted_bound() {
    // void f<T extends num>(T t) {
    //   if (t is int) {
    //     t;
    //   }
    // }
    // The static type of `t;` is the promoted type `T & int` (flow
    // analysis); it is built by hand.
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        functions: strs(&["void f<T extends num>(T t)"]),
        ..LibrarySpec::test()
    });
    let tp = function_type_parameter(&t, "f");
    let ty = t.ctx().intern(TypeKind::TypeParameter {
        param: tp,
        nullability: Nullability::None,
        promoted_bound: Some(t.parse_type("int")),
        alias: None,
    });
    assert_eq!(t.display(ty), "T & int");
}

#[test]
fn write_types() {
    // class A {}
    // class B {}
    // class C implements A, B {}
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        classes: vec![
            ClassSpec::new("class A"),
            ClassSpec::new("class B"),
            ClassSpec::new("class C implements A, B"),
        ],
        ..LibrarySpec::test()
    });
    let element = t.class_element("C");
    assert_eq!(display_string(&t, element.raw()), "class C implements A, B");
}
