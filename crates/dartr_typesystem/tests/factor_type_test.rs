// Dart source: pkg/analyzer/test/src/dart/element/factor_type_test.dart

use dartr_element::TypeId;
use dartr_typesystem::test_support::*;

fn check(t: &TypeSystemTest, ty: TypeId, s: TypeId, expected_str: &str) {
    let result = t.type_system().factor(ty, s);
    let result_str = t.display(result);
    assert_eq!(result_str, expected_str);
}

#[test]
fn dynamic() {
    let t = TypeSystemTest::new();
    check(&t, t.parse_type("dynamic"), t.parse_type("int"), "dynamic");
}

#[test]
fn future_or() {
    let t = TypeSystemTest::new();
    let p = |s| t.parse_type(s);
    check(&t, p("FutureOr<int>"), p("int"), "Future<int>");
    check(&t, p("FutureOr<int>"), p("Future<int>"), "int");

    check(&t, p("FutureOr<int?>"), p("int"), "FutureOr<int?>");
    check(&t, p("FutureOr<int?>"), p("Future<int>"), "FutureOr<int?>");
    check(&t, p("FutureOr<int?>"), p("int?"), "Future<int?>");
    check(&t, p("FutureOr<int?>"), p("Future<int?>"), "int?");

    check(&t, p("FutureOr<int>"), p("num"), "Future<int>");
    check(&t, p("FutureOr<int>"), p("Future<num>"), "int");
}

#[test]
fn object() {
    let t = TypeSystemTest::new();
    let p = |s| t.parse_type(s);
    check(&t, p("Object"), p("Object"), "Never");
    check(&t, p("Object"), p("Object?"), "Never");

    check(&t, p("Object"), p("int"), "Object");
    check(&t, p("Object"), p("int?"), "Object");

    check(&t, p("Object?"), p("Object"), "Never?");
    check(&t, p("Object?"), p("Object?"), "Never");

    check(&t, p("Object?"), p("int"), "Object?");
    check(&t, p("Object?"), p("int?"), "Object");
}

#[test]
fn subtype() {
    let t = TypeSystemTest::new();
    let p = |s| t.parse_type(s);
    check(&t, p("int"), p("int"), "Never");
    check(&t, p("int"), p("int?"), "Never");

    check(&t, p("int?"), p("int"), "Never?");
    check(&t, p("int?"), p("int?"), "Never");

    check(&t, p("int"), p("num"), "Never");
    check(&t, p("int"), p("num?"), "Never");

    check(&t, p("int?"), p("num"), "Never?");
    check(&t, p("int?"), p("num?"), "Never");

    check(&t, p("int"), p("Null"), "int");
    check(&t, p("int?"), p("Null"), "int");

    check(&t, p("int"), p("String"), "int");
    check(&t, p("int?"), p("String"), "int?");

    check(&t, p("int"), p("String?"), "int");
    check(&t, p("int?"), p("String?"), "int");
}

#[test]
fn void() {
    let t = TypeSystemTest::new();
    check(&t, t.parse_type("void"), t.parse_type("int"), "void");
}
