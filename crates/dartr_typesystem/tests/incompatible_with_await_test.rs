// Dart source: pkg/analyzer/test/src/dart/element/incompatible_with_await_test.dart

use dartr_element::TypeId;
use dartr_typesystem::test_support::*;

fn is_incompatible(t: &TypeSystemTest, ty: TypeId) {
    assert!(t.type_system().is_incompatible_with_await(ty));
}

fn is_not_incompatible(t: &TypeSystemTest, ty: TypeId) {
    assert!(!t.type_system().is_incompatible_with_await(ty));
}

fn extension_types(t: &mut TypeSystemTest, specs: &[&str]) {
    t.build_test_library(LibrarySpec {
        imports: strs(&["dart:core", "dart:async"]),
        extension_types: strs(specs),
        ..LibrarySpec::test()
    });
}

#[test]
fn class_int() {
    let t = TypeSystemTest::new();
    is_not_incompatible(&t, t.parse_type("int"));
    is_not_incompatible(&t, t.parse_type("int?"));
}

#[test]
fn extension_type_implements_future() {
    let mut t = TypeSystemTest::new();
    extension_types(
        &mut t,
        &["extension type A(Future<int> it) implements Future<int>"],
    );
    is_not_incompatible(&t, t.parse_interface_type("A"));
}

#[test]
fn extension_type_not_implements_future() {
    let mut t = TypeSystemTest::new();
    extension_types(&mut t, &["extension type A(Future<int> it)"]);
    is_incompatible(&t, t.parse_interface_type("A"));
}

#[test]
fn future_int() {
    let t = TypeSystemTest::new();
    is_not_incompatible(&t, t.parse_type("Future<int>"));
}

#[test]
fn future_or_int() {
    let t = TypeSystemTest::new();
    is_not_incompatible(&t, t.parse_type("FutureOr<int>"));
}

#[test]
fn type_parameter_bound_extension_type_implements_future() {
    let mut t = TypeSystemTest::new();
    extension_types(
        &mut t,
        &["extension type A(Future<int> it) implements Future<int>"],
    );
    t.with_type_parameter_scope("T extends A", |scope| {
        is_not_incompatible(&t, scope.parse_type("T"));
    });
}

#[test]
fn type_parameter_bound_extension_type_not_implements_future() {
    let mut t = TypeSystemTest::new();
    extension_types(&mut t, &["extension type A(Future<int> it)"]);
    t.with_type_parameter_scope("T extends A", |scope| {
        is_incompatible(&t, scope.parse_type("T"));
    });
}

#[test]
fn type_parameter_bound_num_none() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T extends num", |scope| {
        is_not_incompatible(&t, scope.parse_type("T"));
    });
}

#[test]
fn type_parameter_promoted_bound_extension_type_implements_future() {
    // Incompatible with `await`, used as a bound.
    // Does not matter, `T` is promoted to not incompatible.
    let mut t = TypeSystemTest::new();
    extension_types(
        &mut t,
        &[
            "extension type N(Future<int> it)",
            "extension type F(Future<int> it) implements Future<int>",
        ],
    );
    t.with_type_parameter_scope("T extends N", |scope| {
        is_not_incompatible(&t, scope.parse_type("T & F"));
    });
}

#[test]
fn type_parameter_promoted_bound_extension_type_not_implements_future() {
    let mut t = TypeSystemTest::new();
    extension_types(&mut t, &["extension type A(Future<int> it)"]);
    t.with_type_parameter_scope("T", |scope| {
        is_incompatible(&t, scope.parse_type("T & A"));
    });
}

#[test]
fn type_parameter_promoted_bound_int_none() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        is_not_incompatible(&t, scope.parse_type("T & int"));
    });
}
