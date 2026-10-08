// Dart source: pkg/analyzer/test/src/dart/element/resolve_to_bound_test.dart

use dartr_element::TypeId;
use dartr_typesystem::test_support::*;

/// `expect('$result', expectedStr)`: Dart `TypeImpl.toString()` is the
/// display string.
fn check(t: &TypeSystemTest, ty: TypeId, expected_str: &str) {
    let result = t.type_system().resolve_to_bound(ty);
    assert_eq!(t.display(result), expected_str);
}

#[test]
fn dynamic() {
    let t = TypeSystemTest::new();
    check(&t, t.parse_type("dynamic"), "dynamic");
}

#[test]
fn function_type() {
    let t = TypeSystemTest::new();
    check(&t, t.parse_type("void Function()"), "void Function()");
}

#[test]
fn interface_type() {
    let t = TypeSystemTest::new();
    check(&t, t.parse_type("int"), "int");
    check(&t, t.parse_type("int?"), "int?");
}

#[test]
fn type_parameter_bound() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T extends int", |scope| {
        check(&t, scope.parse_type("T"), "int");
    });

    t.with_type_parameter_scope("T extends int?", |scope| {
        check(&t, scope.parse_type("T"), "int?");
    });
}

#[test]
fn type_parameter_bound_function_type() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T extends void Function()", |scope| {
        check(&t, scope.parse_type("T"), "void Function()");
    });
}

#[test]
fn type_parameter_bound_nested_no_bound() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T, U extends T", |scope| {
        check(&t, scope.parse_type("U"), "Object?");
    });
}

#[test]
fn type_parameter_bound_nested_none() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T extends int, U extends T", |scope| {
        check(&t, scope.parse_type("U"), "int");
    });
}

#[test]
fn type_parameter_bound_nested_none_outer_nullable() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T extends int, U extends T?", |scope| {
        check(&t, scope.parse_type("U"), "int?");
    });
}

#[test]
fn type_parameter_bound_nested_question() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T extends int?, U extends T", |scope| {
        check(&t, scope.parse_type("U"), "int?");
    });
}

#[test]
fn type_parameter_bound_nullable_inner() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T extends int?", |scope| {
        check(&t, scope.parse_type("T"), "int?");
    });
}

#[test]
fn type_parameter_bound_nullable_inner_outer() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T extends int?", |scope| {
        check(&t, scope.parse_type("T?"), "int?");
    });
}

#[test]
fn type_parameter_bound_nullable_outer() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T extends int", |scope| {
        check(&t, scope.parse_type("T?"), "int?");
    });
}

#[test]
fn type_parameter_no_bound() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        check(&t, scope.parse_type("T"), "Object?");
    });
}

#[test]
fn type_parameter_promoted_bound() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T extends num", |scope| {
        check(&t, scope.parse_type("T & int"), "int");
    });

    t.with_type_parameter_scope("T extends num?", |scope| {
        check(&t, scope.parse_type("T & int?"), "int?");
    });
}

#[test]
fn void() {
    let t = TypeSystemTest::new();
    check(&t, t.parse_type("void"), "void");
}
