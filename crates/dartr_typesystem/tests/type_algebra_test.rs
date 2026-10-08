// Dart source: pkg/analyzer/test/src/dart/element/type_algebra_test.dart

//! Port of `type_algebra_test.dart`.
//!
//! Dart `same(a, b)` on types is `identical`, which is `TypeId ==` here.
//! Dart `same` on `Substitution` objects compares object identity; the Rust
//! `MapSubstitution` is a value, so these tests compare with `==` (the same
//! map in the same order).

#![allow(non_snake_case)]

use dartr_element::{Ctx, EId, Nullability, TypeId, TypeKind, TypeParameterElement};
use dartr_typesystem::test_support::*;
use dartr_typesystem::{MapSubstitution, TypeExt};
use indexmap::IndexMap;

/// Returns a type where all occurrences of the given type parameters have been
/// replaced with the corresponding types.
///
/// This will copy only the sub-terms of [type] that contain substituted
/// variables; all other [DartType] objects will be reused.
///
/// In particular, if no type parameters were substituted, this is guaranteed
/// to return the [type] instance (not a copy), so the caller may use
/// [identical] to efficiently check if a distinct type was created.
fn substitute(
    ctx: &Ctx<'_>,
    t: TypeId,
    substitution: &[(EId<TypeParameterElement>, TypeId)],
) -> TypeId {
    if substitution.is_empty() {
        return t;
    }
    let map: IndexMap<_, _> = substitution.iter().copied().collect();
    MapSubstitution::from_map(map).substitute_type(ctx, t)
}

// ------------------------------------------------------------------ _Base

/// `_Base._typeStr`.
fn type_str(t: &TypeSystemTest, ty: TypeId) -> String {
    let ctx = t.ctx();
    let mut result = t.display(ty);

    if let Some(alias) = ctx.type_alias(ty) {
        let alias = *ctx.alias(alias);
        result += &format!(" via {}", ctx.element_name(alias.element.raw()).unwrap());
        let type_argument_str_list: Vec<String> = ctx
            .list(alias.args)
            .iter()
            .map(|&a| type_str(t, a))
            .collect();
        if !type_argument_str_list.is_empty() {
            result += &format!("<{}>", type_argument_str_list.join(", "));
        }
    }

    result
}

/// `_Base.assertType`.
fn assert_type(t: &TypeSystemTest, ty: TypeId, expected: &str) {
    let type_str = type_str(t, ty);
    assert_eq!(type_str, expected);
}

/// `_Base._assertSubstitution`.
fn assert_substitution(
    t: &TypeSystemTest,
    ty: TypeId,
    substitution: &[(EId<TypeParameterElement>, TypeId)],
    expected: &str,
) {
    let result = substitute(&t.ctx(), ty, substitution);
    assert_type(t, result, expected);
    // Dart: isNot(same(type)). A changed type is a different structure, so
    // it has a different TypeId.
    assert_ne!(result, ty);
}

/// `SubstituteTest._assertIdenticalType`.
fn assert_identical_type(
    t: &TypeSystemTest,
    ty: TypeId,
    substitution: &[(EId<TypeParameterElement>, TypeId)],
) {
    let result = substitute(&t.ctx(), ty, substitution);
    // Dart: same(type)
    assert_eq!(result, ty);
}

fn map(pairs: &[(EId<TypeParameterElement>, TypeId)]) -> MapSubstitution {
    MapSubstitution::from_map(pairs.iter().copied().collect())
}

// ------------------------------------------------------------------ tests

mod map_substitution_test {
    use super::*;

    #[test]
    fn and_then_empty_and_then_not_empty() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            let ctx = t.ctx();
            let T = scope.type_parameter("T");
            let after = map(&[(T, scope.parse_type("int"))]);

            let combined = MapSubstitution::empty().and_then(&ctx, &after);
            // Dart: same(Substitution.empty). Rust substitutions are values.
            assert_eq!(combined, MapSubstitution::empty());
        });
    }

    #[test]
    fn and_then_not_empty_and_then_empty() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            let ctx = t.ctx();
            let T = scope.type_parameter("T");
            let inner = map(&[(T, scope.parse_type("int"))]);

            let combined = inner.and_then(&ctx, &MapSubstitution::empty());
            // Dart: same(inner). Rust substitutions are values.
            assert_eq!(combined, inner);
        });
    }

    #[test]
    fn and_then_not_empty_and_then_not_empty() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T, G", |scope| {
            let ctx = t.ctx();
            let T = scope.type_parameter("T");
            let G = scope.type_parameter("G");
            let inner = map(&[(T, scope.parse_type("List<G>"))]);
            let after = map(&[(G, t.parse_type("String"))]);

            let combined = inner.and_then(&ctx, &after);
            let result = combined.substitute_type(&ctx, scope.parse_type("T"));
            assert_type(&t, result, "List<String>");
        });
    }
}

mod substitute_empty_test {
    use super::*;

    #[test]
    fn interface() {
        let mut t = TypeSystemTest::new();
        // class A<T> {}
        t.build_test_library(LibrarySpec {
            classes: vec![ClassSpec::new("class A<T>")],
            ..LibrarySpec::test()
        });

        let ty = t.parse_type("A<int>");

        let result = MapSubstitution::empty().substitute_type(&t.ctx(), ty);
        // Dart: same(type)
        assert_eq!(result, ty);
    }
}

mod substitute_from_interface_type_test {
    use super::*;

    #[test]
    fn method_return_type() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: vec![ClassSpec::new("class A<T>").methods(&["List<T> foo()"])],
            ..LibrarySpec::test()
        });
        let ctx = t.ctx();

        let substitution =
            MapSubstitution::from_interface_type(&ctx, t.parse_interface_type("A<int>"));

        let foo = t.method(t.class_element("A").upcast(), "foo");
        let ty = ctx.executable(foo.upcast()).return_type.get().unwrap();
        assert_type(&t, ty, "List<T>");

        let result = substitution.substitute_type(&ctx, ty);
        assert_type(&t, result, "List<int>");
    }
}

mod substitute_from_pairs_test {
    use super::*;

    #[test]
    fn method_return_type() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: vec![ClassSpec::new("class A<T, U>").methods(&["Map<T, U> foo()"])],
            ..LibrarySpec::test()
        });
        let ctx = t.ctx();
        let A = t.class_element("A");
        let T = ctx.interface_type_parameters(A.upcast())[0];
        let U = ctx.interface_type_parameters(A.upcast())[1];

        let foo = t.method(A.upcast(), "foo");
        let ty = ctx.executable(foo.upcast()).return_type.get().unwrap();
        assert_type(&t, ty, "Map<T, U>");

        let result =
            MapSubstitution::from_pairs(&[T, U], &[t.parse_type("int"), t.parse_type("double")])
                .substitute_type(&ctx, ty);
        assert_type(&t, result, "Map<int, double>");
    }
}

mod substitute_test {
    use super::*;

    #[test]
    fn bottom() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            let T = scope.type_parameter("T");
            // typeProvider.bottomType
            assert_identical_type(&t, TypeId::NEVER, &[(T, scope.parse_type("int"))]);
        });
    }

    #[test]
    fn dynamic() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            let T = scope.type_parameter("T");
            assert_identical_type(&t, TypeId::DYNAMIC, &[(T, scope.parse_type("int"))]);
        });
    }

    #[test]
    fn function_from_alias_has_ref() {
        let mut t = TypeSystemTest::new();
        // typedef Alias<T> = void Function();
        t.build_test_library(LibrarySpec {
            type_aliases: strs(&["typedef Alias<T> = void Function()"]),
            ..LibrarySpec::test()
        });

        t.with_type_parameter_scope("U", |scope| {
            let U = scope.type_parameter("U");
            let ty = scope.parse_type("Alias<U>");
            assert_type(&t, ty, "void Function() via Alias<U>");
            assert_substitution(
                &t,
                ty,
                &[(U, t.parse_type("int"))],
                "void Function() via Alias<int>",
            );
        });
    }

    #[test]
    fn function_from_alias_no_ref() {
        let mut t = TypeSystemTest::new();
        // typedef Alias<T> = void Function();
        t.build_test_library(LibrarySpec {
            type_aliases: strs(&["typedef Alias<T> = void Function()"]),
            ..LibrarySpec::test()
        });

        let ty = t.parse_type("Alias<double>");
        assert_type(&t, ty, "void Function() via Alias<double>");

        t.with_type_parameter_scope("U", |scope| {
            let U = scope.type_parameter("U");
            assert_identical_type(&t, ty, &[(U, scope.parse_type("int"))]);
        });
    }

    #[test]
    fn function_from_alias_no_type_parameters() {
        let mut t = TypeSystemTest::new();
        // typedef Alias<T> = void Function();
        t.build_test_library(LibrarySpec {
            type_aliases: strs(&["typedef Alias<T> = void Function()"]),
            ..LibrarySpec::test()
        });

        let ty = t.parse_type("Alias<int>");
        assert_type(&t, ty, "void Function() via Alias<int>");

        t.with_type_parameter_scope("U", |scope| {
            let U = scope.type_parameter("U");
            assert_identical_type(&t, ty, &[(U, scope.parse_type("int"))]);
        });
    }

    #[test]
    fn function_no_substitutions() {
        let t = TypeSystemTest::new();
        let ty = t.parse_type("bool Function(int)");

        t.with_type_parameter_scope("T", |scope| {
            let T = scope.type_parameter("T");
            assert_identical_type(&t, ty, &[(T, scope.parse_type("int"))]);
        });
    }

    #[test]
    fn function_parameters_return_type() {
        let t = TypeSystemTest::new();
        // typedef F<T, U> = T Function(U u, bool);
        t.with_type_parameter_scope("T, U", |scope| {
            let T = scope.type_parameter("T");
            let U = scope.type_parameter("U");
            let ty = scope.parse_type("T Function(U, bool)");

            assert_type(&t, ty, "T Function(U, bool)");
            assert_substitution(&t, ty, &[(T, t.parse_type("int"))], "int Function(U, bool)");
            assert_substitution(
                &t,
                ty,
                &[(T, t.parse_type("int")), (U, t.parse_type("double"))],
                "int Function(double, bool)",
            );
        });
    }

    #[test]
    fn function_type_formals() {
        let t = TypeSystemTest::new();
        // typedef F<T> = T Function<U extends T>(U);
        t.with_type_parameter_scope("T", |scope| {
            let T = scope.type_parameter("T");
            let ty = scope.parse_type("T Function<U extends T>(U)");

            assert_type(&t, ty, "T Function<U extends T>(U)");
            assert_substitution(
                &t,
                ty,
                &[(T, t.parse_type("int"))],
                "int Function<U extends int>(U)",
            );
        });
    }

    #[test]
    fn function_type_formals_bounds() {
        let mut t = TypeSystemTest::new();
        // class Triple<X, Y, Z> {}
        // typedef F<V> = bool Function<T extends Triple<T, U, V>, U>();
        t.build_test_library(LibrarySpec {
            classes: vec![ClassSpec::new("class Triple<X, Y, Z>")],
            ..LibrarySpec::test()
        });

        t.with_type_parameter_scope("V", |scope| {
            let ctx = t.ctx();
            let V = scope.type_parameter("V");
            let ty = scope.parse_type("bool Function<T extends Triple<T, U, V>, U>()");

            assert_type(&t, ty, "bool Function<T extends Triple<T, U, V>, U>()");

            let result = substitute(&ctx, ty, &[(V, t.parse_type("int"))]);
            assert_type(
                &t,
                result,
                "bool Function<T extends Triple<T, U, int>, U>()",
            );
            let TypeKind::Function(f) = *ctx.ty(result) else {
                panic!("not a function type");
            };
            let T2 = ctx.list(f.type_params)[0];
            let U2 = ctx.list(f.type_params)[1];
            let T2_bound = ctx.type_parameter_bound(T2).unwrap();
            let T2_bound_args = ctx.type_arguments(T2_bound);
            let element = |a: TypeId| match *ctx.ty(a) {
                TypeKind::TypeParameter { param, .. } => param,
                _ => panic!("not a type parameter type"),
            };
            // Dart: same(T2), same(U2)
            assert_eq!(element(T2_bound_args[0]), T2);
            assert_eq!(element(T2_bound_args[1]), U2);
        });
    }

    #[test]
    fn interface_arguments() {
        let mut t = TypeSystemTest::new();
        // class A<T> {}
        t.build_test_library(LibrarySpec {
            classes: vec![ClassSpec::new("class A<T>")],
            ..LibrarySpec::test()
        });

        t.with_type_parameter_scope("U", |scope| {
            let U = scope.type_parameter("U");
            let ty = scope.parse_type("A<U>");

            assert_type(&t, ty, "A<U>");
            assert_substitution(&t, ty, &[(U, t.parse_type("int"))], "A<int>");
        });
    }

    #[test]
    fn interface_arguments_deep() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: vec![ClassSpec::new("class A<T>")],
            ..LibrarySpec::test()
        });

        t.with_type_parameter_scope("U", |scope| {
            let U = scope.type_parameter("U");
            let ty = scope.parse_type("A<List<U>>");
            assert_type(&t, ty, "A<List<U>>");

            assert_substitution(&t, ty, &[(U, t.parse_type("int"))], "A<List<int>>");
        });
    }

    #[test]
    fn interface_no_arguments() {
        let mut t = TypeSystemTest::new();
        // class A {}
        t.build_test_library(LibrarySpec {
            classes: vec![ClassSpec::new("class A")],
            ..LibrarySpec::test()
        });

        let ty = t.parse_interface_type("A");
        t.with_type_parameter_scope("T", |scope| {
            let T = scope.type_parameter("T");
            assert_identical_type(&t, ty, &[(T, scope.parse_type("int"))]);
        });
    }

    #[test]
    fn interface_no_arguments_in_arguments() {
        let mut t = TypeSystemTest::new();
        // class A<T> {}
        t.build_test_library(LibrarySpec {
            classes: vec![ClassSpec::new("class A<T>")],
            ..LibrarySpec::test()
        });

        let ty = t.parse_interface_type("A<int>");

        t.with_type_parameter_scope("U", |scope| {
            let U = scope.type_parameter("U");
            assert_identical_type(&t, ty, &[(U, scope.parse_type("double"))]);
        });
    }

    #[test]
    fn interface_no_type_parameters_from_alias_has_ref() {
        let mut t = TypeSystemTest::new();
        // class A {}
        t.build_test_library(LibrarySpec {
            classes: vec![ClassSpec::new("class A")],
            type_aliases: strs(&["typedef Alias<T> = A"]),
            ..LibrarySpec::test()
        });

        t.with_type_parameter_scope("U", |scope| {
            let U = scope.type_parameter("U");
            let ty = scope.parse_type("Alias<U>");
            assert_type(&t, ty, "A via Alias<U>");
            assert_substitution(&t, ty, &[(U, t.parse_type("int"))], "A via Alias<int>");
        });
    }

    #[test]
    fn interface_no_type_parameters_from_alias_no_ref() {
        let mut t = TypeSystemTest::new();
        // class A {}
        t.build_test_library(LibrarySpec {
            classes: vec![ClassSpec::new("class A")],
            type_aliases: strs(&["typedef Alias<T> = A"]),
            ..LibrarySpec::test()
        });

        let ty = t.parse_type("Alias<double>");
        assert_type(&t, ty, "A via Alias<double>");

        t.with_type_parameter_scope("U", |scope| {
            let U = scope.type_parameter("U");
            assert_identical_type(&t, ty, &[(U, scope.parse_type("int"))]);
        });
    }

    #[test]
    fn interface_no_type_parameters_from_alias_no_type_parameters() {
        let mut t = TypeSystemTest::new();
        // class A {}
        t.build_test_library(LibrarySpec {
            classes: vec![ClassSpec::new("class A")],
            type_aliases: strs(&["typedef Alias = A"]),
            ..LibrarySpec::test()
        });

        let ty = t.parse_type("Alias");
        assert_type(&t, ty, "A via Alias");

        t.with_type_parameter_scope("T", |scope| {
            let T = scope.type_parameter("T");
            assert_identical_type(&t, ty, &[(T, scope.parse_type("int"))]);
        });
    }

    #[test]
    fn invalid() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            let T = scope.type_parameter("T");
            assert_identical_type(&t, TypeId::INVALID, &[(T, scope.parse_type("int"))]);
        });
    }

    #[test]
    fn record_does_not_use_type_parameter2() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            let T = scope.type_parameter("T");

            let ty = t.parse_record_type("(int,)");
            assert_type(&t, ty, "(int,)");

            assert_identical_type(&t, ty, &[(T, scope.parse_type("int"))]);
        });
    }

    #[test]
    fn record_from_alias() {
        let mut t = TypeSystemTest::new();
        // typedef Alias<T> = (int, String);
        t.build_test_library(LibrarySpec {
            type_aliases: strs(&["typedef Alias<T> = (int, String)"]),
            ..LibrarySpec::test()
        });

        t.with_type_parameter_scope("U", |scope| {
            let U = scope.type_parameter("U");
            let ty = scope.parse_type("Alias<U>");
            assert_type(&t, ty, "(int, String) via Alias<U>");
            assert_substitution(
                &t,
                ty,
                &[(U, t.parse_type("int"))],
                "(int, String) via Alias<int>",
            );
        });
    }

    #[test]
    fn record_from_alias2() {
        let mut t = TypeSystemTest::new();
        // typedef Alias<T> = (T, List<T>);
        t.build_test_library(LibrarySpec {
            type_aliases: strs(&["typedef Alias<T> = (T, List<T>)"]),
            ..LibrarySpec::test()
        });

        let ty = t.parse_type("Alias<int>");
        assert_type(&t, ty, "(int, List<int>) via Alias<int>");
    }

    #[test]
    fn record_named() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            let T = scope.type_parameter("T");
            let ty = scope.parse_type("({T f1, List<T> f2})");

            assert_type(&t, ty, "({T f1, List<T> f2})");
            assert_substitution(
                &t,
                ty,
                &[(T, t.parse_type("int"))],
                "({int f1, List<int> f2})",
            );
        });
    }

    #[test]
    fn record_positional() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            let T = scope.type_parameter("T");
            let ty = scope.parse_type("(T, List<T>)");

            assert_type(&t, ty, "(T, List<T>)");
            assert_substitution(&t, ty, &[(T, t.parse_type("int"))], "(int, List<int>)");
        });
    }

    #[test]
    fn type_parameter_nullability() {
        let t = TypeSystemTest::new();
        let ts = t.type_system();
        t.with_type_parameter_scope("T", |scope| {
            let T = scope.type_parameter("T");

            let check = |ty: &str, type_argument: TypeId, expected_type: TypeId| {
                let result =
                    map(&[(T, type_argument)]).substitute_type(&t.ctx(), scope.parse_type(ty));
                // Dart: ==
                assert!(
                    ts.dart_eq(result, expected_type),
                    "{} != {}",
                    t.display(result),
                    t.display(expected_type)
                );
            };

            check("T", t.parse_type("int"), t.parse_type("int"));
            check("T", t.parse_type("int?"), t.parse_type("int?"));

            check("T?", t.parse_type("int"), t.parse_type("int?"));
            check("T?", t.parse_type("int?"), t.parse_type("int?"));
        });
    }

    #[test]
    fn unknown_inferred_type() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            let T = scope.type_parameter("T");
            assert_identical_type(&t, TypeId::UNKNOWN, &[(T, scope.parse_type("int"))]);
        });
    }

    #[test]
    fn void() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            let T = scope.type_parameter("T");
            assert_identical_type(&t, TypeId::VOID, &[(T, scope.parse_type("int"))]);
        });
    }

    #[test]
    fn void_empty_map() {
        let t = TypeSystemTest::new();
        assert_identical_type(&t, t.parse_type("int"), &[]);
    }
}

mod substitute_with_nullability_test {
    use super::*;

    #[test]
    fn interface_none() {
        let mut t = TypeSystemTest::new();
        // class A<T> {}
        t.build_test_library(LibrarySpec {
            classes: vec![ClassSpec::new("class A<T>")],
            ..LibrarySpec::test()
        });

        t.with_type_parameter_scope("U", |scope| {
            let U = scope.type_parameter("U");
            let ty = scope.parse_type("A<U>");
            assert_substitution(&t, ty, &[(U, t.parse_type("int"))], "A<int>");
        });
    }

    #[test]
    fn interface_question() {
        let mut t = TypeSystemTest::new();
        // class A<T> {}
        t.build_test_library(LibrarySpec {
            classes: vec![ClassSpec::new("class A<T>")],
            ..LibrarySpec::test()
        });

        t.with_type_parameter_scope("U", |scope| {
            let U = scope.type_parameter("U");
            let ty = scope.parse_type("A<U>?");
            assert_substitution(&t, ty, &[(U, t.parse_type("int"))], "A<int>?");
        });
    }

    /// The alias nullability suffix of [ty], or `None` without alias.
    fn alias_suffix(t: &TypeSystemTest, ty: TypeId) -> Option<Nullability> {
        let ctx = t.ctx();
        ctx.type_alias(ty).map(|a| ctx.alias(a).nullability)
    }

    fn check_with_nullability_updates_alias(alias_spec: &str) {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            type_aliases: strs(&[alias_spec]),
            ..LibrarySpec::test()
        });
        let ctx = t.ctx();
        let alias = t.type_alias_element("A");
        let ty = ctx.instantiate_type_alias(alias, &[], Nullability::Question);

        let result = ctx.with_nullability(ty, Nullability::None);
        assert_eq!(alias_suffix(&t, result), Some(Nullability::None));
        assert_eq!(t.display_alias(result), "A");
    }

    #[test]
    fn with_nullability_updates_alias_function() {
        check_with_nullability_updates_alias("typedef A = void Function()");
    }

    #[test]
    fn with_nullability_updates_alias_interface() {
        check_with_nullability_updates_alias("typedef A = int");
    }

    #[test]
    fn with_nullability_updates_alias_record() {
        check_with_nullability_updates_alias("typedef A = (int,)");
    }

    #[test]
    fn with_nullability_updates_alias_type_parameter() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            type_aliases: strs(&["typedef A<T> = T"]),
            ..LibrarySpec::test()
        });
        let alias = t.type_alias_element("A");

        t.with_type_parameter_scope("U", |scope| {
            let ctx = t.ctx();
            let ty =
                ctx.instantiate_type_alias(alias, &[scope.parse_type("U")], Nullability::Question);

            let result = ctx.with_nullability(ty, Nullability::None);
            assert!(ctx.type_alias(result).is_some());
            assert_eq!(alias_suffix(&t, result), Some(Nullability::None));
        });
    }
}
