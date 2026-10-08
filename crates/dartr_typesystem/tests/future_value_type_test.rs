// Dart source: pkg/analyzer/test/src/dart/element/future_value_type_test.dart

use dartr_element::TypeId;
use dartr_typesystem::test_support::*;

fn check(t: &TypeSystemTest, ty: TypeId, expected: &str) {
    let result = t.type_system().future_value_type(ty);
    assert_eq!(t.display(result), expected);
}

/// futureValueType(`dynamic`) = `dynamic`.
#[test]
fn dynamic() {
    let t = TypeSystemTest::new();
    check(&t, t.parse_type("dynamic"), "dynamic");
}

/// futureValueType(Future<`S`>) = `S`, for all `S`.
#[test]
fn future() {
    let t = TypeSystemTest::new();
    check(&t, t.parse_type("Future<int>"), "int");
    check(&t, t.parse_type("Future<int?>"), "int?");

    check(&t, t.parse_type("Future<dynamic>"), "dynamic");
    check(&t, t.parse_type("Future<void>"), "void");

    check(&t, t.parse_type("Future<Never>"), "Never");
    check(&t, t.parse_type("Future<Never?>"), "Never?");

    check(&t, t.parse_type("Future<Object>"), "Object");
    check(&t, t.parse_type("Future<Object?>"), "Object?");
}

/// futureValueType(FutureOr<`S`>) = `S`, for all `S`.
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

/// Otherwise, for all `S`, futureValueType(`S`) = `Object?`.
#[test]
fn other() {
    let t = TypeSystemTest::new();
    check(&t, t.parse_type("Object"), "Object?");
    check(&t, t.parse_type("int"), "Object?");
}

/// futureValueType(`S?`) = futureValueType(`S`), for all `S`.
#[test]
fn suffix_question() {
    let t = TypeSystemTest::new();
    check(&t, t.parse_type("int?"), "Object?");

    check(&t, t.parse_type("Future<int>?"), "int");
    check(&t, t.parse_type("Future<int?>?"), "int?");

    check(&t, t.parse_type("FutureOr<int>?"), "int");
    check(&t, t.parse_type("FutureOr<int?>?"), "int?");

    check(&t, t.parse_type("Future<Object>?"), "Object");
    check(&t, t.parse_type("Future<Object?>?"), "Object?");

    check(&t, t.parse_type("Future<dynamic>?"), "dynamic");
    check(&t, t.parse_type("Future<void>?"), "void");
}

/// futureValueType(`void`) = `void`.
#[test]
fn void() {
    let t = TypeSystemTest::new();
    check(&t, t.parse_type("void"), "void");
}
