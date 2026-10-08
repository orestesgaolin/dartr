// Dart source: pkg/analyzer/test/src/dart/element/always_exhaustive_test.dart

use dartr_element::TypeId;
use dartr_typesystem::test_support::*;

fn is_always_exhaustive(t: &TypeSystemTest, ty: TypeId) {
    assert!(
        t.type_system().is_always_exhaustive(ty),
        "{}",
        t.display(ty)
    );
}

fn is_not_always_exhaustive(t: &TypeSystemTest, ty: TypeId) {
    assert!(
        !t.type_system().is_always_exhaustive(ty),
        "{}",
        t.display(ty)
    );
}

#[test]
fn class_bool() {
    let t = TypeSystemTest::new();
    is_always_exhaustive(&t, t.parse_type("bool"));
    is_always_exhaustive(&t, t.parse_type("bool?"));
}

#[test]
fn class_int() {
    let t = TypeSystemTest::new();
    is_not_always_exhaustive(&t, t.parse_type("int"));
    is_not_always_exhaustive(&t, t.parse_type("int?"));
}

/// Dart `test_class_Null`.
#[test]
fn class_null() {
    let t = TypeSystemTest::new();
    is_always_exhaustive(&t, t.parse_type("Null"));
}

#[test]
fn class_sealed() {
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        classes: vec![ClassSpec::new("sealed class A")],
        ..LibrarySpec::test()
    });
    is_always_exhaustive(&t, t.parse_type("A"));
    is_always_exhaustive(&t, t.parse_type("A?"));
}

#[test]
fn enum_() {
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        enums: vec![EnumSpec::new("enum E")],
        ..LibrarySpec::test()
    });
    is_always_exhaustive(&t, t.parse_type("E"));
    is_always_exhaustive(&t, t.parse_type("E?"));
}

#[test]
fn extension_type() {
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        extension_types: strs(&[
            "extension type A(bool it)",
            "extension type B(bool? it)",
            "extension type C(int it)",
        ]),
        ..LibrarySpec::test()
    });
    is_always_exhaustive(&t, t.parse_type("A"));
    is_always_exhaustive(&t, t.parse_type("B"));
    is_not_always_exhaustive(&t, t.parse_type("C"));
}

#[test]
fn future_or() {
    let t = TypeSystemTest::new();
    is_always_exhaustive(&t, t.parse_type("FutureOr<bool>"));
    is_always_exhaustive(&t, t.parse_type("FutureOr<bool>?"));

    is_always_exhaustive(&t, t.parse_type("FutureOr<bool?>"));
    is_always_exhaustive(&t, t.parse_type("FutureOr<bool?>?"));

    is_not_always_exhaustive(&t, t.parse_type("FutureOr<int>"));
    is_not_always_exhaustive(&t, t.parse_type("FutureOr<int>?"));
}

#[test]
fn record_type() {
    let t = TypeSystemTest::new();
    is_always_exhaustive(&t, t.parse_type("(bool,)"));

    is_always_exhaustive(&t, t.parse_type("({bool f0})"));

    is_not_always_exhaustive(&t, t.parse_type("(int,)"));

    is_not_always_exhaustive(&t, t.parse_type("(bool, int)"));

    is_not_always_exhaustive(&t, t.parse_type("({int f0})"));

    is_not_always_exhaustive(&t, t.parse_type("({bool f0, int f1})"));
}

#[test]
fn type_parameter() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T extends bool", |scope| {
        is_always_exhaustive(&t, scope.parse_type("T"));
    });

    t.with_type_parameter_scope("T extends num", |scope| {
        is_not_always_exhaustive(&t, scope.parse_type("T"));
    });

    t.with_type_parameter_scope("T", |scope| {
        is_always_exhaustive(&t, scope.parse_type("T & bool"));
        is_not_always_exhaustive(&t, scope.parse_type("T & int"));
    });
}
