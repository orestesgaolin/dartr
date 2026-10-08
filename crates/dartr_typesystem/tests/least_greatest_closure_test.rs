// Dart source: pkg/analyzer/test/src/dart/element/least_greatest_closure_test.dart

//! `GreatestClosureTest`.

#![allow(non_snake_case)]

use dartr_element::{EId, TypeId, TypeParameterElement};
use dartr_typesystem::test_support::*;

/// `_check(type, typeParameters:, greatest:, least:)`.
fn check(
    t: &TypeSystemTest,
    ty: TypeId,
    type_parameters: &[EId<TypeParameterElement>],
    greatest: &str,
    least: &str,
) {
    let ts = t.type_system();
    let greatest_result = ts.greatest_closure(ty, type_parameters);
    assert_eq!(t.display(greatest_result), greatest);

    let least_result = ts.least_closure(ty, type_parameters);
    assert_eq!(t.display(least_result), least);
}

#[test]
fn contravariant() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");

        check(
            &t,
            scope.parse_type("void Function(T)"),
            &[T],
            "void Function(Never)",
            "void Function(Object?)",
        );

        check(
            &t,
            scope.parse_type("void Function(T) Function()"),
            &[T],
            "void Function(Never) Function()",
            "void Function(Object?) Function()",
        );
    });
}

#[test]
fn covariant() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");

        check(
            &t,
            scope.parse_type_parameter_type("T"),
            &[T],
            "Object?",
            "Never",
        );
        check(
            &t,
            scope.parse_type_parameter_type("T?"),
            &[T],
            "Object?",
            "Never?",
        );

        check(
            &t,
            scope.parse_type("List<T>"),
            &[T],
            "List<Object?>",
            "List<Never>",
        );

        check(
            &t,
            scope.parse_type("void Function(int Function(T))"),
            &[T],
            "void Function(int Function(Object?))",
            "void Function(int Function(Never))",
        );
    });
}

#[test]
fn function() {
    // void Function<U extends T>()
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        check(
            &t,
            scope.parse_type("void Function<U extends T>()"),
            &[scope.type_parameter("T")],
            "Function",
            "Never",
        );
    });
}

#[test]
fn unrelated() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T, U", |scope| {
        let check_unchanged = |ty: TypeId, expected: &str| {
            let T = scope.type_parameter("T");
            check(&t, ty, &[T], expected, expected);
        };

        check_unchanged(t.parse_type("int"), "int");
        check_unchanged(t.parse_type("int?"), "int?");

        check_unchanged(t.parse_type("List<int>"), "List<int>");
        check_unchanged(t.parse_type("List<int>?"), "List<int>?");

        check_unchanged(t.parse_type("Object"), "Object");
        check_unchanged(t.parse_type("Object?"), "Object?");

        check_unchanged(t.parse_type("Never"), "Never");
        check_unchanged(t.parse_type("Never?"), "Never?");

        check_unchanged(t.parse_type("dynamic"), "dynamic");

        check_unchanged(t.parse_type("String Function(int)"), "String Function(int)");

        check_unchanged(scope.parse_type("U"), "U");
    });
}
