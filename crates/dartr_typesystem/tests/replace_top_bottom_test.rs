// Dart source: pkg/analyzer/test/src/dart/element/replace_top_bottom_test.dart

//! `ReplaceTopBottomTest`.

use dartr_element::TypeId;
use dartr_typesystem::test_support::*;

/// `_check(type, expectedStr)`.
fn check(t: &TypeSystemTest, ty: TypeId, expected_str: &str) {
    let result = t.type_system().replace_top_and_bottom(ty);
    // Dart: '$result' (TypeImpl.toString is getDisplayString()).
    assert_eq!(t.display(result), expected_str);
}

#[test]
fn contravariant_bottom() {
    let t = TypeSystemTest::new();
    // Not contravariant.
    check(&t, t.parse_type("Never"), "Never");

    check(
        &t,
        t.parse_function_type("int Function(Never)"),
        "int Function(Object?)",
    );

    t.with_type_parameter_scope("T extends Never", |scope| {
        check(
            &t,
            scope.parse_type("int Function(T)"),
            "int Function(Object?)",
        );
    });
}

#[test]
fn not_contravariant_covariant_top() {
    let t = TypeSystemTest::new();
    check(&t, t.parse_type("Object?"), "Never");
    check(&t, t.parse_type("dynamic"), "Never");
    check(&t, t.parse_type("void"), "Never");

    check(&t, t.parse_type("List<Object?>"), "List<Never>");
    check(&t, t.parse_type("List<dynamic>"), "List<Never>");
    check(&t, t.parse_type("List<void>"), "List<Never>");

    check(&t, t.parse_type("FutureOr<Object?>"), "Never");
    check(&t, t.parse_type("FutureOr<dynamic>"), "Never");
    check(&t, t.parse_type("FutureOr<void>"), "Never");
    check(&t, t.parse_type("FutureOr<FutureOr<void>>"), "Never");

    check(
        &t,
        t.parse_type("int Function(int Function(Object?))"),
        "int Function(int Function(Never))",
    );

    check(&t, t.parse_type("int"), "int");
    check(&t, t.parse_type("int?"), "int?");

    check(&t, t.parse_type("List<int>"), "List<int>");
    check(&t, t.parse_type("List<int?>"), "List<int?>");
    check(&t, t.parse_type("List<int>?"), "List<int>?");
    check(&t, t.parse_type("List<int?>?"), "List<int?>?");
}

#[test]
fn not_contravariant_invariant() {
    let mut t = TypeSystemTest::new();
    // typedef F<T> = T Function(T);
    t.build_test_library(LibrarySpec {
        type_aliases: strs(&["typedef F<inout T> = T Function(T)"]),
        ..LibrarySpec::test()
    });

    check(&t, t.parse_type("F<dynamic>"), "Never Function(Never)");
}
