// Dart source: pkg/analyzer/test/src/dart/element/nullable_test.dart

//! `IsNonNullableTest`, `IsNullableTest`, `IsPotentiallyNonNullableTest`,
//! `IsPotentiallyNullableTest`, `IsStrictlyNonNullableTest`,
//! `PromoteToNonNullTest`.

use dartr_element::{EId, Nullability, TypeId, TypeKind, TypeParameterElement};
use dartr_typesystem::TypeExt;
use dartr_typesystem::test_support::*;

mod is_non_nullable_test {
    use super::*;

    fn is_not_non_nullable(t: &TypeSystemTest, ty: TypeId) {
        assert!(
            !t.type_system().is_non_nullable(ty),
            "is_non_nullable: {}",
            t.display(ty)
        );
    }

    fn is_non_nullable(t: &TypeSystemTest, ty: TypeId) {
        assert!(
            t.type_system().is_non_nullable(ty),
            "is_non_nullable: {}",
            t.display(ty)
        );
    }

    #[test]
    fn dynamic() {
        let t = TypeSystemTest::new();
        is_not_non_nullable(&t, t.parse_type("dynamic"));
    }

    #[test]
    fn function() {
        let t = TypeSystemTest::new();
        is_non_nullable(&t, t.parse_type("void Function()"));

        is_not_non_nullable(&t, t.parse_type("void Function()?"));
    }

    #[test]
    fn function_class() {
        let t = TypeSystemTest::new();
        is_non_nullable(&t, t.parse_type("Function"));
        is_not_non_nullable(&t, t.parse_type("Function?"));
    }

    #[test]
    fn future_or_none_argument() {
        let t = TypeSystemTest::new();
        is_non_nullable(&t, t.parse_type("FutureOr<int>"));

        is_not_non_nullable(&t, t.parse_type("FutureOr<int>?"));
    }

    #[test]
    fn future_or_question_argument() {
        let t = TypeSystemTest::new();
        is_not_non_nullable(&t, t.parse_type("FutureOr<int?>"));

        is_not_non_nullable(&t, t.parse_type("FutureOr<int?>?"));
    }

    #[test]
    fn interface() {
        let t = TypeSystemTest::new();
        is_non_nullable(&t, t.parse_type("int"));
        is_not_non_nullable(&t, t.parse_type("int?"));
    }

    #[test]
    fn interface_extension_type2() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            extension_types: strs(&[
                "extension type A(int it)",
                "extension type B(int it) implements int",
            ]),
            ..LibrarySpec::test()
        });
        is_not_non_nullable(&t, t.parse_interface_type("A"));
        is_non_nullable(&t, t.parse_interface_type("B"));
    }

    #[test]
    fn invalid_type() {
        let t = TypeSystemTest::new();
        is_not_non_nullable(&t, t.parse_type("InvalidType"));
    }

    #[test]
    fn never() {
        let t = TypeSystemTest::new();
        is_non_nullable(&t, t.parse_type("Never"));
        is_not_non_nullable(&t, t.parse_type("Never?"));
    }

    #[test]
    fn null() {
        let t = TypeSystemTest::new();
        is_not_non_nullable(&t, t.parse_type("Null"));
    }

    #[test]
    fn type_parameter_bound_none() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends int", |scope| {
            is_non_nullable(&t, scope.parse_type("T"));
            is_not_non_nullable(&t, scope.parse_type("T?"));
        });
    }

    #[test]
    fn type_parameter_bound_question() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends int?", |scope| {
            is_not_non_nullable(&t, scope.parse_type("T"));
            is_not_non_nullable(&t, scope.parse_type("T?"));
        });
    }

    #[test]
    fn type_parameter_promoted_bound_none() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_non_nullable(&t, scope.parse_type("T & int"));
            is_non_nullable(&t, scope.parse_type("(T & int)?"));
        });
    }

    #[test]
    fn type_parameter_promoted_bound_question() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_not_non_nullable(&t, scope.parse_type("T & int?"));
            is_not_non_nullable(&t, scope.parse_type("(T & int?)?"));
        });
    }

    #[test]
    fn void() {
        let t = TypeSystemTest::new();
        is_not_non_nullable(&t, t.parse_type("void"));
    }
}

mod is_nullable_test {
    use super::*;

    fn is_not_nullable(t: &TypeSystemTest, ty: TypeId) {
        assert!(
            !t.type_system().is_nullable(ty),
            "is_nullable: {}",
            t.display(ty)
        );
    }

    fn is_nullable(t: &TypeSystemTest, ty: TypeId) {
        assert!(
            t.type_system().is_nullable(ty),
            "is_nullable: {}",
            t.display(ty)
        );
    }

    #[test]
    fn dynamic() {
        let t = TypeSystemTest::new();
        is_nullable(&t, t.parse_type("dynamic"));
    }

    #[test]
    fn function() {
        let t = TypeSystemTest::new();
        is_not_nullable(&t, t.parse_type("void Function()"));

        is_nullable(&t, t.parse_type("void Function()?"));
    }

    #[test]
    fn function_class() {
        let t = TypeSystemTest::new();
        is_not_nullable(&t, t.parse_type("Function"));
        is_nullable(&t, t.parse_type("Function?"));
    }

    #[test]
    fn future_or_none_argument() {
        let t = TypeSystemTest::new();
        is_not_nullable(&t, t.parse_type("FutureOr<int>"));

        is_nullable(&t, t.parse_type("FutureOr<int>?"));
    }

    #[test]
    fn future_or_question_argument() {
        let t = TypeSystemTest::new();
        is_nullable(&t, t.parse_type("FutureOr<int?>"));

        is_nullable(&t, t.parse_type("FutureOr<int?>?"));
    }

    #[test]
    fn interface() {
        let t = TypeSystemTest::new();
        is_not_nullable(&t, t.parse_type("int"));
        is_nullable(&t, t.parse_type("int?"));
    }

    #[test]
    fn interface_extension_type2() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            extension_types: strs(&[
                "extension type A(int it)",
                "extension type B(int it) implements int",
            ]),
            ..LibrarySpec::test()
        });
        is_not_nullable(&t, t.parse_interface_type("A"));
        is_not_nullable(&t, t.parse_interface_type("B"));
        is_nullable(&t, t.parse_interface_type("B?"));
    }

    #[test]
    fn invalid_type() {
        let t = TypeSystemTest::new();
        is_nullable(&t, t.parse_type("InvalidType"));
    }

    #[test]
    fn never() {
        let t = TypeSystemTest::new();
        is_not_nullable(&t, t.parse_type("Never"));
        is_nullable(&t, t.parse_type("Never?"));
    }

    #[test]
    fn null() {
        let t = TypeSystemTest::new();
        is_nullable(&t, t.parse_type("Null"));
    }

    #[test]
    fn type_parameter_bound_none() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends int", |scope| {
            is_not_nullable(&t, scope.parse_type("T"));
            is_nullable(&t, scope.parse_type("T?"));
        });
    }

    #[test]
    fn type_parameter_bound_question_none() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends int?", |scope| {
            is_not_nullable(&t, scope.parse_type("T"));
            is_nullable(&t, scope.parse_type("T?"));
        });
    }

    #[test]
    fn type_parameter_promoted_bound_none() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_not_nullable(&t, scope.parse_type("T & int"));
            is_not_nullable(&t, scope.parse_type("(T & int)?"));
        });
    }

    #[test]
    fn type_parameter_promoted_bound_question() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_nullable(&t, scope.parse_type("T & int?"));
            is_nullable(&t, scope.parse_type("(T & int?)?"));
        });
    }

    #[test]
    fn void() {
        let t = TypeSystemTest::new();
        is_nullable(&t, t.parse_type("void"));
    }
}

mod is_potentially_non_nullable_test {
    use super::*;

    fn is_not_potentially_non_nullable(t: &TypeSystemTest, ty: TypeId) {
        assert!(
            !t.type_system().is_potentially_non_nullable(ty),
            "is_potentially_non_nullable: {}",
            t.display(ty)
        );
    }

    fn is_potentially_non_nullable(t: &TypeSystemTest, ty: TypeId) {
        assert!(
            t.type_system().is_potentially_non_nullable(ty),
            "is_potentially_non_nullable: {}",
            t.display(ty)
        );
    }

    #[test]
    fn dynamic() {
        let t = TypeSystemTest::new();
        is_not_potentially_non_nullable(&t, t.parse_type("dynamic"));
    }

    #[test]
    fn future_or() {
        let t = TypeSystemTest::new();
        is_potentially_non_nullable(&t, t.parse_type("FutureOr<int>"));

        is_not_potentially_non_nullable(&t, t.parse_type("FutureOr<int?>"));
    }

    #[test]
    fn interface() {
        let t = TypeSystemTest::new();
        is_potentially_non_nullable(&t, t.parse_type("int"));
        is_not_potentially_non_nullable(&t, t.parse_type("int?"));
    }

    #[test]
    fn interface_extension_type2() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            extension_types: strs(&[
                "extension type A(int it)",
                "extension type B(int it) implements int",
            ]),
            ..LibrarySpec::test()
        });
        is_potentially_non_nullable(&t, t.parse_interface_type("A"));
        is_potentially_non_nullable(&t, t.parse_interface_type("B"));
    }

    #[test]
    fn invalid_type() {
        let t = TypeSystemTest::new();
        is_not_potentially_non_nullable(&t, t.parse_type("InvalidType"));
    }

    #[test]
    fn never() {
        let t = TypeSystemTest::new();
        is_potentially_non_nullable(&t, t.parse_type("Never"));
    }

    #[test]
    fn null() {
        let t = TypeSystemTest::new();
        is_not_potentially_non_nullable(&t, t.parse_type("Null"));
    }

    #[test]
    fn void() {
        let t = TypeSystemTest::new();
        is_not_potentially_non_nullable(&t, t.parse_type("void"));
    }
}

mod is_potentially_nullable_test {
    use super::*;

    fn is_not_potentially_nullable(t: &TypeSystemTest, ty: TypeId) {
        assert!(
            !t.type_system().is_potentially_nullable(ty),
            "is_potentially_nullable: {}",
            t.display(ty)
        );
    }

    fn is_potentially_nullable(t: &TypeSystemTest, ty: TypeId) {
        assert!(
            t.type_system().is_potentially_nullable(ty),
            "is_potentially_nullable: {}",
            t.display(ty)
        );
    }

    #[test]
    fn dynamic() {
        let t = TypeSystemTest::new();
        is_potentially_nullable(&t, t.parse_type("dynamic"));
    }

    #[test]
    fn future_or() {
        let t = TypeSystemTest::new();
        is_not_potentially_nullable(&t, t.parse_type("FutureOr<int>"));

        is_potentially_nullable(&t, t.parse_type("FutureOr<int?>"));
    }

    #[test]
    fn interface() {
        let t = TypeSystemTest::new();
        is_not_potentially_nullable(&t, t.parse_type("int"));
        is_potentially_nullable(&t, t.parse_type("int?"));
    }

    #[test]
    fn interface_extension_type2() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            extension_types: strs(&[
                "extension type A(int it)",
                "extension type B(int it) implements int",
            ]),
            ..LibrarySpec::test()
        });
        is_potentially_nullable(&t, t.parse_interface_type("A?"));
        is_potentially_nullable(&t, t.parse_interface_type("A"));
        is_not_potentially_nullable(&t, t.parse_interface_type("B"));
    }

    #[test]
    fn invalid_type() {
        let t = TypeSystemTest::new();
        is_potentially_nullable(&t, t.parse_type("InvalidType"));
    }

    #[test]
    fn never() {
        let t = TypeSystemTest::new();
        is_not_potentially_nullable(&t, t.parse_type("Never"));
    }

    #[test]
    fn null() {
        let t = TypeSystemTest::new();
        is_potentially_nullable(&t, t.parse_type("Null"));
    }

    #[test]
    fn void() {
        let t = TypeSystemTest::new();
        is_potentially_nullable(&t, t.parse_type("void"));
    }
}

mod is_strictly_non_nullable_test {
    use super::*;

    fn is_not_strictly_non_nullable(t: &TypeSystemTest, ty: TypeId) {
        assert!(
            !t.type_system().is_strictly_non_nullable(ty),
            "is_strictly_non_nullable: {}",
            t.display(ty)
        );
    }

    fn is_strictly_non_nullable(t: &TypeSystemTest, ty: TypeId) {
        assert!(
            t.type_system().is_strictly_non_nullable(ty),
            "is_strictly_non_nullable: {}",
            t.display(ty)
        );
    }

    #[test]
    fn dynamic() {
        let t = TypeSystemTest::new();
        is_not_strictly_non_nullable(&t, t.parse_type("dynamic"));
    }

    #[test]
    fn function() {
        let t = TypeSystemTest::new();
        is_strictly_non_nullable(&t, t.parse_type("void Function()"));

        is_not_strictly_non_nullable(&t, t.parse_type("void Function()?"));
    }

    #[test]
    fn function_class() {
        let t = TypeSystemTest::new();
        is_strictly_non_nullable(&t, t.parse_type("Function"));
        is_not_strictly_non_nullable(&t, t.parse_type("Function?"));
    }

    #[test]
    fn future_or_none_argument() {
        let t = TypeSystemTest::new();
        is_strictly_non_nullable(&t, t.parse_type("FutureOr<int>"));

        is_not_strictly_non_nullable(&t, t.parse_type("FutureOr<int>?"));
    }

    #[test]
    fn future_or_question_argument() {
        let t = TypeSystemTest::new();
        is_not_strictly_non_nullable(&t, t.parse_type("FutureOr<int?>"));

        is_not_strictly_non_nullable(&t, t.parse_type("FutureOr<int?>?"));
    }

    #[test]
    fn interface() {
        let t = TypeSystemTest::new();
        is_strictly_non_nullable(&t, t.parse_type("int"));
        is_not_strictly_non_nullable(&t, t.parse_type("int?"));
    }

    #[test]
    fn interface_extension_type2() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            extension_types: strs(&[
                "extension type A(int it)",
                "extension type B(int it) implements int",
            ]),
            ..LibrarySpec::test()
        });
        is_not_strictly_non_nullable(&t, t.parse_interface_type("A"));
        is_strictly_non_nullable(&t, t.parse_interface_type("B"));
    }

    #[test]
    fn invalid_type() {
        let t = TypeSystemTest::new();
        is_not_strictly_non_nullable(&t, t.parse_type("InvalidType"));
    }

    #[test]
    fn never() {
        let t = TypeSystemTest::new();
        is_strictly_non_nullable(&t, t.parse_type("Never"));
        is_not_strictly_non_nullable(&t, t.parse_type("Never?"));
    }

    #[test]
    fn null() {
        let t = TypeSystemTest::new();
        is_not_strictly_non_nullable(&t, t.parse_type("Null"));
    }

    #[test]
    fn type_parameter_bound_none() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends int", |scope| {
            is_strictly_non_nullable(&t, scope.parse_type("T"));
            is_not_strictly_non_nullable(&t, scope.parse_type("T?"));
        });
    }

    #[test]
    fn type_parameter_bound_question() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends int?", |scope| {
            is_not_strictly_non_nullable(&t, scope.parse_type("T"));
            is_not_strictly_non_nullable(&t, scope.parse_type("T?"));
        });
    }

    #[test]
    fn void() {
        let t = TypeSystemTest::new();
        is_not_strictly_non_nullable(&t, t.parse_type("void"));
    }
}

mod promote_to_non_null_test {
    use super::*;

    /// `_check(type, expected)`.
    fn check(t: &TypeSystemTest, ty: TypeId, expected: TypeId) {
        let result = t.type_system().promote_to_non_null(ty);
        // Dart: `expect(result, expected)` uses `==`.
        assert!(
            t.ctx().dart_eq(result, expected),
            "{} -> {}, expected {}",
            t.display(ty),
            t.display(result),
            t.display(expected)
        );
    }

    /// `_checkTypeParameter(type, element:, promotedBound:)`.
    fn check_type_parameter(
        t: &TypeSystemTest,
        ty: TypeId,
        element: EId<TypeParameterElement>,
        promoted_bound: Option<TypeId>,
    ) {
        let actual = t.type_system().promote_to_non_null(ty);
        let TypeKind::TypeParameter {
            param,
            nullability,
            promoted_bound: actual_promoted_bound,
            ..
        } = *t.ctx().ty(actual)
        else {
            panic!("not a TypeParameterTypeImpl: {}", t.display(actual));
        };
        // Dart: `same(element)`; elements are ids, so `==` is identity.
        assert_eq!(param, element);
        // Dart: `expect(actual.promotedBound, promotedBound)` uses `==`.
        match (actual_promoted_bound, promoted_bound) {
            (None, None) => {}
            (Some(a), Some(e)) => assert!(
                t.ctx().dart_eq(a, e),
                "promotedBound: {}, expected {}",
                t.display(a),
                t.display(e)
            ),
            (a, e) => panic!("promotedBound: {a:?}, expected {e:?}"),
        }
        assert_eq!(nullability, Nullability::None);
    }

    #[test]
    fn dynamic() {
        let t = TypeSystemTest::new();
        check(&t, t.parse_type("dynamic"), t.parse_type("dynamic"));
    }

    #[test]
    fn function_type() {
        let t = TypeSystemTest::new();
        // NonNull(T0 Function(...)) = T0 Function(...)
        check(
            &t,
            t.parse_type("void Function()?"),
            t.parse_type("void Function()"),
        );
    }

    #[test]
    fn future_or_question() {
        let t = TypeSystemTest::new();
        // NonNull(FutureOr<T>) = FutureOr<T>
        check(
            &t,
            t.parse_type("FutureOr<String?>?"),
            t.parse_type("FutureOr<String?>"),
        );
    }

    #[test]
    fn interface_type() {
        let t = TypeSystemTest::new();
        check(&t, t.parse_type("int"), t.parse_type("int"));
        check(&t, t.parse_type("int?"), t.parse_type("int"));

        // NonNull(C<T1, ... , Tn>) = C<T1, ... , Tn>
        check(&t, t.parse_type("List<int?>?"), t.parse_type("List<int?>"));
    }

    #[test]
    fn interface_type_function() {
        let t = TypeSystemTest::new();
        check(&t, t.parse_type("Function?"), t.parse_type("Function"));
    }

    #[test]
    fn invalid_type() {
        let t = TypeSystemTest::new();
        check(&t, t.parse_type("InvalidType"), t.parse_type("InvalidType"));
    }

    #[test]
    fn never() {
        let t = TypeSystemTest::new();
        check(&t, t.parse_type("Never"), t.parse_type("Never"));
        check(&t, t.parse_type("Never?"), t.parse_type("Never"));
    }

    #[test]
    fn null() {
        let t = TypeSystemTest::new();
        check(&t, t.parse_type("Null"), t.parse_type("Never"));
    }

    #[test]
    fn type_parameter_bound_dynamic() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends dynamic", |scope| {
            let element = scope.type_parameter("T");
            check_type_parameter(&t, scope.parse_type_parameter_type("T"), element, None);
        });
    }

    #[test]
    fn type_parameter_bound_invalid_type() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends InvalidType", |scope| {
            let element = scope.type_parameter("T");
            check_type_parameter(&t, scope.parse_type_parameter_type("T"), element, None);
        });
    }

    #[test]
    fn type_parameter_bound_none() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends int", |scope| {
            let element = scope.type_parameter("T");
            check_type_parameter(&t, scope.parse_type_parameter_type("T"), element, None);
            check_type_parameter(&t, scope.parse_type_parameter_type("T?"), element, None);
        });
    }

    #[test]
    fn type_parameter_bound_null() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            let element = scope.type_parameter("T");
            check_type_parameter(
                &t,
                scope.parse_type_parameter_type("T"),
                element,
                Some(t.parse_type("Object")),
            );
        });
    }

    #[test]
    fn type_parameter_bound_question() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends int?", |scope| {
            let element = scope.type_parameter("T");
            check_type_parameter(
                &t,
                scope.parse_type_parameter_type("T"),
                element,
                Some(t.parse_type("int")),
            );
            check_type_parameter(
                &t,
                scope.parse_type_parameter_type("T?"),
                element,
                Some(t.parse_type("int")),
            );
        });
    }

    #[test]
    fn type_parameter_promoted_bound_none() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends num?", |scope| {
            let element = scope.type_parameter("T");
            check_type_parameter(
                &t,
                scope.parse_type_parameter_type("T & int"),
                element,
                Some(t.parse_type("int")),
            );
            check_type_parameter(
                &t,
                scope.parse_type_parameter_type("(T & int)?"),
                element,
                Some(t.parse_type("int")),
            );
        });
    }

    #[test]
    fn type_parameter_promoted_bound_question() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends num?", |scope| {
            let element = scope.type_parameter("T");
            check_type_parameter(
                &t,
                scope.parse_type_parameter_type("T & int?"),
                element,
                Some(t.parse_type("int")),
            );
            check_type_parameter(
                &t,
                scope.parse_type_parameter_type("(T & int?)?"),
                element,
                Some(t.parse_type("int")),
            );
        });
    }

    #[test]
    fn void() {
        let t = TypeSystemTest::new();
        check(&t, t.parse_type("void"), t.parse_type("void"));
    }
}
