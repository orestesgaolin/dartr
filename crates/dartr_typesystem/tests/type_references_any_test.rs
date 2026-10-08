// Dart source: pkg/analyzer/test/src/dart/element/type_references_any_test.dart

use dartr_element::TypeId;
use dartr_typesystem::TypeExt;
use dartr_typesystem::test_support::*;

#[test]
fn false_() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let tp = scope.type_parameter("T");

        let check_false = |ty: TypeId| {
            let actual = t.ctx().references_any(ty, &[tp]);
            assert!(!actual, "{}", t.display(ty));
        };

        check_false(t.parse_type("dynamic"));
        check_false(t.parse_type("int"));
        check_false(t.parse_type("Never"));
        check_false(t.parse_type("void"));
        check_false(t.parse_type("List<int>"));
    });
}

#[test]
fn true_() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let tp = scope.type_parameter("T");

        let check_true = |ty: TypeId| {
            let actual = t.ctx().references_any(ty, &[tp]);
            assert!(actual, "{}", t.display(ty));
        };

        check_true(scope.parse_type("T"));
        check_true(scope.parse_type("List<T>"));
        check_true(scope.parse_type("Map<T, int>"));
        check_true(scope.parse_type("Map<int, T>"));
        check_true(scope.parse_type("T Function()"));
        check_true(scope.parse_type("void Function(T)"));
        check_true(scope.parse_type("void Function<U extends T>()"));
    });
}
