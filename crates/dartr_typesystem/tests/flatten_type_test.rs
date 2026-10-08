// Dart source: pkg/analyzer/test/src/dart/element/flatten_type_test.dart

use dartr_element::TypeId;
use dartr_typesystem::test_support::*;

fn core_async_spec(classes: &[&str]) -> LibrarySpec {
    LibrarySpec {
        imports: strs(&["dart:core", "dart:async"]),
        classes: classes.iter().map(|c| ClassSpec::new(c)).collect(),
        ..LibrarySpec::test()
    }
}

mod flatten_type_test {
    use super::*;

    fn check(t: &TypeSystemTest, ty: TypeId, expected: &str) {
        let result = t.type_system().flatten(ty);
        assert_eq!(t.display(result), expected);
    }

    #[test]
    #[ignore = "needs normalize (class hierarchy merges conflicting interfaces with normalizeInterfaceType) (unit A4)"]
    fn interface_type_conflicting_future_interfaces() {
        // Repeated generic elements in the hierarchy should not trip the
        // recursion guard, and traversal order still determines the future type.
        let mut t = TypeSystemTest::new();
        t.build_test_library(core_async_spec(&[
            "abstract class Derived<T> implements Future<T>",
            "abstract class A extends Derived<int> implements Derived<num>",
            "abstract class A1 implements Future<int>",
            "abstract class A2 extends A1 implements Future<num>",
            "abstract class B1 implements Future<num>",
            "abstract class B2 extends B1 implements Future<int>",
        ]));
        check(&t, t.parse_type("A"), "int");
        check(&t, t.parse_type("A2"), "int");
        check(&t, t.parse_type("B2"), "num");
    }

    #[test]
    #[ignore = "needs normalize (class hierarchy merges conflicting interfaces with normalizeInterfaceType) (unit A4)"]
    fn interface_type_conflicting_future_interfaces_disjoint() {
        // Neither 'String' nor 'int' is more specific than the other, meaning
        // they are completely disjoint. Conflict resolution handles this
        // deterministically via parent-first interface traversal order.
        let mut t = TypeSystemTest::new();
        t.build_test_library(core_async_spec(&[
            "abstract class A1 implements Future<int>",
            "abstract class A2 extends A1 implements Future<String>",
            "abstract class B1 implements Future<String>",
            "abstract class B2 extends B1 implements Future<int>",
        ]));
        check(&t, t.parse_type("A2"), "int");
        check(&t, t.parse_type("B2"), "String");
    }

    #[test]
    fn interface_type_implements_future() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(core_async_spec(&[
            "abstract class Derived<T> implements Future<T>",
        ]));
        check(&t, t.parse_type("Derived<dynamic>"), "dynamic");
        check(&t, t.parse_type("Derived<int>"), "int");
        check(&t, t.parse_type("Derived<Derived>"), "Derived");
        check(&t, t.parse_type("Derived<Derived<int>>"), "Derived<int>");
    }

    #[test]
    fn interface_type_recursive_hierarchy() {
        // Even though there is a loop in the class hierarchy,
        // flatten() should terminate successfully.
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: vec![
                ClassSpec::new("class A extends B"),
                ClassSpec::new("class B extends A"),
            ],
            ..LibrarySpec::test()
        });
        check(&t, t.parse_type("A"), "A");
        check(&t, t.parse_type("B"), "B");
    }

    #[test]
    fn simple_types() {
        let t = TypeSystemTest::new();
        check(&t, t.parse_type("dynamic"), "dynamic");
        check(&t, t.parse_type("int"), "int");
        check(&t, t.parse_type("int?"), "int?");

        check(&t, t.parse_type("Future<int>"), "int");
        check(&t, t.parse_type("Future<int?>"), "int?");
        check(&t, t.parse_type("Future<int>?"), "int?");
        check(&t, t.parse_type("Future<int?>?"), "int?");

        check(&t, t.parse_type("FutureOr<int>"), "int");
        check(&t, t.parse_type("FutureOr<int?>"), "int?");
        check(&t, t.parse_type("FutureOr<int>?"), "int?");
        check(&t, t.parse_type("FutureOr<int?>?"), "int?");

        check(&t, t.parse_type("Future<Future<int>>"), "Future<int>");
        check(&t, t.parse_type("Future<Future<int>?>"), "Future<int>?");
        check(&t, t.parse_type("FutureOr<Future<int>>"), "Future<int>");
        check(&t, t.parse_type("FutureOr<Future<int?>>"), "Future<int?>");
        check(&t, t.parse_type("FutureOr<Future<int>>?"), "Future<int>?");
        check(&t, t.parse_type("FutureOr<Future<int?>>?"), "Future<int?>?");
    }

    #[test]
    fn type_parameter() {
        let t = TypeSystemTest::new();
        // Bounds with future type are flattened.
        t.with_type_parameter_scope("T extends Future<int>", |scope| {
            check(&t, scope.parse_type("T"), "int");
        });
        t.with_type_parameter_scope("T extends FutureOr<int>", |scope| {
            check(&t, scope.parse_type("T"), "int");
        });

        // Nullable type parameters preserve nullability after flattening.
        t.with_type_parameter_scope("T extends Future<int>", |scope| {
            check(&t, scope.parse_type("T?"), "int?");
        });
        t.with_type_parameter_scope("T extends FutureOr<int>", |scope| {
            check(&t, scope.parse_type("T?"), "int?");
        });

        // Promoted bounds are used when they have a future type.
        t.with_type_parameter_scope("T", |scope| {
            check(&t, scope.parse_type("T & Future<int>"), "int");
            check(&t, scope.parse_type("T & FutureOr<int>"), "int");
        });

        // Without a future type, the type parameter itself is unchanged.
        t.with_type_parameter_scope("T extends int", |scope| {
            check(&t, scope.parse_type("T"), "T");
        });
        t.with_type_parameter_scope("T", |scope| {
            check(&t, scope.parse_type("T & int"), "T");
        });
    }

    #[test]
    fn unknown_inferred_type() {
        let t = TypeSystemTest::new();
        let ty = TypeId::UNKNOWN;
        // Dart: same(type)
        assert_eq!(t.type_system().flatten(ty), ty);
    }
}

mod future_type_test {
    use super::*;

    fn check(t: &TypeSystemTest, ty: TypeId, expected: Option<&str>) {
        let result = t.type_system().future_type(ty);
        match result {
            None => assert_eq!(expected, None),
            Some(result) => assert_eq!(Some(t.display(result).as_str()), expected),
        }
    }

    #[test]
    fn interface_type_implements_future() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(core_async_spec(&["class A implements Future<int>"]));
        check(&t, t.parse_type("A"), Some("Future<int>"));
        check(&t, t.parse_type("A?"), None);
    }

    #[test]
    fn simple_types() {
        let t = TypeSystemTest::new();
        check(&t, t.parse_type("dynamic"), None);
        check(&t, t.parse_type("void Function()"), None);

        check(&t, t.parse_type("Object"), None);
        check(&t, t.parse_type("Object?"), None);

        check(&t, t.parse_type("int"), None);
        check(&t, t.parse_type("int?"), None);

        check(&t, t.parse_type("List<int>"), None);
        check(&t, t.parse_type("List<int?>"), None);

        check(&t, t.parse_type("List<int>?"), None);
        check(&t, t.parse_type("List<int?>?"), None);

        check(&t, t.parse_type("Future<int>"), Some("Future<int>"));
        check(&t, t.parse_type("Future<int?>"), Some("Future<int?>"));

        check(&t, t.parse_type("Future<int>?"), Some("Future<int>?"));
        check(&t, t.parse_type("Future<int?>?"), Some("Future<int?>?"));

        check(&t, t.parse_type("FutureOr<int>"), Some("FutureOr<int>"));
        check(&t, t.parse_type("FutureOr<int?>"), Some("FutureOr<int?>"));

        check(&t, t.parse_type("FutureOr<int>?"), Some("FutureOr<int>?"));
        check(&t, t.parse_type("FutureOr<int?>?"), Some("FutureOr<int?>?"));

        check(
            &t,
            t.parse_type("Future<Future<int>>"),
            Some("Future<Future<int>>"),
        );
        check(
            &t,
            t.parse_type("Future<FutureOr<int>>"),
            Some("Future<FutureOr<int>>"),
        );
        check(
            &t,
            t.parse_type("FutureOr<Future<int>>"),
            Some("FutureOr<Future<int>>"),
        );
        check(
            &t,
            t.parse_type("FutureOr<FutureOr<int>>"),
            Some("FutureOr<FutureOr<int>>"),
        );
    }

    #[test]
    fn type_parameter() {
        let t = TypeSystemTest::new();
        // Bounds with future type are returned as the future type.
        t.with_type_parameter_scope("T extends Future<int>", |scope| {
            check(&t, scope.parse_type("T"), Some("Future<int>"));
        });
        t.with_type_parameter_scope("T extends FutureOr<int>", |scope| {
            check(&t, scope.parse_type("T"), Some("FutureOr<int>"));
        });

        // Promoted bounds are used when they have a future type.
        t.with_type_parameter_scope("T", |scope| {
            check(&t, scope.parse_type("T & Future<int>"), Some("Future<int>"));
            check(
                &t,
                scope.parse_type("T & FutureOr<int>"),
                Some("FutureOr<int>"),
            );
        });

        // Without a future type, there is no future type result.
        t.with_type_parameter_scope("T extends int", |scope| {
            check(&t, scope.parse_type("T"), None);
        });
        t.with_type_parameter_scope("T", |scope| {
            check(&t, scope.parse_type("T & int"), None);
        });
    }

    #[test]
    fn unknown_inferred_type() {
        let t = TypeSystemTest::new();
        check(&t, TypeId::UNKNOWN, None);
    }
}

mod union_free_type_test {
    use super::*;

    fn check(t: &TypeSystemTest, ty: TypeId, expected: &str) {
        let result = t.type_system().union_free_type(ty);
        assert_eq!(t.display(result), expected);
    }

    #[test]
    fn simple_types() {
        let t = TypeSystemTest::new();
        check(&t, t.parse_type("Future<int>?"), "Future<int>");
        check(&t, t.parse_type("FutureOr<int>"), "int");
        check(&t, t.parse_type("FutureOr<FutureOr<int?>?>?"), "int");
        check(&t, t.parse_type("int?"), "int");
        check(&t, t.parse_type("int"), "int");
    }

    #[test]
    fn unknown_inferred_type() {
        let t = TypeSystemTest::new();
        let ty = TypeId::UNKNOWN;
        // Dart: same(type)
        assert_eq!(t.type_system().union_free_type(ty), ty);
    }
}
