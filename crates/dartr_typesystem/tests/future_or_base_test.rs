// Dart source: pkg/analyzer/test/src/dart/element/future_or_base_test.dart

use dartr_element::TypeId;
use dartr_typesystem::test_support::*;

fn check(t: &TypeSystemTest, ty: TypeId, expected: &str) {
    let result = t.type_system().future_or_base(ty);
    assert_eq!(t.display(result), expected);
}

#[test]
fn dynamic() {
    let t = TypeSystemTest::new();
    check(&t, t.parse_type("dynamic"), "dynamic");
}

#[test]
fn future_or() {
    let t = TypeSystemTest::new();
    check(&t, t.parse_type("FutureOr<int>"), "int");
    check(&t, t.parse_type("FutureOr<int?>"), "int?");

    check(&t, t.parse_type("FutureOr<dynamic>"), "dynamic");
    check(&t, t.parse_type("FutureOr<void>"), "void");

    check(&t, t.parse_type("FutureOr<Never>"), "Never");
    check(&t, t.parse_type("FutureOr<Never?>"), "Never?");

    check(&t, t.parse_type("FutureOr<Object>"), "Object");
    check(&t, t.parse_type("FutureOr<Object?>"), "Object?");
}

#[test]
fn other() {
    let t = TypeSystemTest::new();
    check(&t, t.parse_type("int"), "int");
    check(&t, t.parse_type("int?"), "int?");

    check(&t, t.parse_type("Object"), "Object");
    check(&t, t.parse_type("Object?"), "Object?");
}

/// futureValueType(`void`) = `void`.
#[test]
fn void() {
    let t = TypeSystemTest::new();
    check(&t, t.parse_type("void"), "void");
}
