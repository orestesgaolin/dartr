// Dart source: pkg/analyzer/test/src/dart/element/type_constraint_gatherer_test.dart

//! Port of `type_constraint_gatherer_test.dart` (`TypeConstraintGathererTest`,
//! 38 tests). `_checkMatch` / `_checkNotMatch` are the free functions
//! [`check_match`] / [`check_not_match`].
//!
//! `test_functionType_hasTypeFormals_closure` is a Dart `@FailingTest`; here
//! it is `#[should_panic]` (it must keep failing, as in Dart).

#![allow(non_snake_case)]

use dartr_element::{EId, TypeId, TypeParameterElement};
use dartr_flow::type_analyzer_operations::TypeConstraintGenerator;
use dartr_typesystem::TypeExt;
use dartr_typesystem::test_support::*;
use dartr_typesystem::type_constraint_gatherer::TypeConstraintGatherer;

// ------------------------------------------------------------------ helpers

/// `_checkMatch(typeParameters:, P:, Q:, leftSchema:, expected:)`.
fn check_match(
    t: &TypeSystemTest,
    type_parameters: &[EId<TypeParameterElement>],
    P: TypeId,
    Q: TypeId,
    left_schema: bool,
    expected: &[&str],
) {
    let operations = t.type_system_operations();
    let mut gatherer = TypeConstraintGatherer::new(type_parameters, &operations, false, None);

    let is_match = gatherer.perform_subtype_constraint_generation_internal(P, Q, left_schema, None);
    assert!(
        is_match,
        "expected a match: {} <# {} (leftSchema: {left_schema})",
        t.display(P),
        t.display(Q)
    );

    let constraints = gatherer.compute_constraints();
    let ctx = t.ctx();
    let mut constraints_str: Vec<String> = constraints
        .iter()
        .map(|(key, value)| {
            let lower_str = t.display(value.lower.unwrap_type_schema_view());
            let upper_str = t.display(value.upper.unwrap_type_schema_view());
            let name = ctx.element_name(key.raw()).unwrap_or("null");
            format!("{lower_str} <: {name} <: {upper_str}")
        })
        .collect();

    // unorderedEquals
    let mut expected: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
    constraints_str.sort();
    expected.sort();
    assert_eq!(
        constraints_str,
        expected,
        "{} <# {} (leftSchema: {left_schema})",
        t.display(P),
        t.display(Q)
    );
}

/// `_checkNotMatch(typeParameters, P, Q, leftSchema)`.
fn check_not_match(
    t: &TypeSystemTest,
    type_parameters: &[EId<TypeParameterElement>],
    P: TypeId,
    Q: TypeId,
    left_schema: bool,
) {
    let operations = t.type_system_operations();
    let mut gatherer = TypeConstraintGatherer::new(type_parameters, &operations, false, None);

    let is_match = gatherer.perform_subtype_constraint_generation_internal(P, Q, left_schema, None);
    assert!(
        !is_match,
        "expected no match: {} <# {} (leftSchema: {left_schema})",
        t.display(P),
        t.display(Q)
    );
    assert!(gatherer.is_constraint_set_empty());
}

// -------------------------------------------------------------------- tests

/// If `P` and `Q` are identical types, then the subtype match holds under no
/// constraints.
#[test]
fn test_equal_left_right() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");

        check_match(
            &t,
            &[T],
            t.parse_type("int"),
            t.parse_type("int"),
            true,
            &["_ <: T <: _"],
        );

        check_match(
            &t,
            &[T],
            t.parse_function_type("void Function(int)"),
            t.parse_function_type("void Function(int)"),
            true,
            &["_ <: T <: _"],
        );

        check_match(
            &t,
            &[T],
            t.parse_function_type("T1 Function<T1>()"),
            t.parse_function_type("T2 Function<T2>()"),
            true,
            &["_ <: T <: _"],
        );
    });
}

#[test]
fn test_functionType_hasTypeFormals() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");

        check_match(
            &t,
            &[T],
            scope.parse_type("T Function<T1>(T1)"),
            t.parse_function_type("int Function<S1>(S1)"),
            false,
            &["_ <: T <: int"],
        );

        check_match(
            &t,
            &[T],
            t.parse_function_type("int Function<T1>(T1)"),
            scope.parse_type("T Function<S1>(S1)"),
            true,
            &["int <: T <: _"],
        );

        // We unified type formals, but still not match because return types.
        check_not_match(
            &t,
            &[T],
            t.parse_function_type("int Function<T1>(T1)"),
            t.parse_function_type("String Function<S1>(S1)"),
            false,
        );
    });
}

#[test]
fn test_functionType_hasTypeFormals_bounds_different_subtype() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");
        check_not_match(
            &t,
            &[T],
            scope.parse_type("T Function<T1>()"),
            t.parse_function_type("int Function<S1 extends num>()"),
            false,
        );
    });
}

#[test]
fn test_functionType_hasTypeFormals_bounds_different_top() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");
        check_match(
            &t,
            &[T],
            scope.parse_type("T Function<T1 extends void>()"),
            t.parse_function_type("int Function<S1 extends dynamic>()"),
            false,
            &["_ <: T <: int"],
        );
    });
}

#[test]
fn test_functionType_hasTypeFormals_bounds_different_unrelated() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");
        check_not_match(
            &t,
            &[T],
            scope.parse_type("T Function<T1 extends int>()"),
            t.parse_function_type("int Function<S1 extends String>()"),
            false,
        );
    });
}

#[test]
fn test_functionType_hasTypeFormals_bounds_same_leftDefault_rightDefault() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");
        check_match(
            &t,
            &[T],
            scope.parse_type("T Function<T1>()"),
            t.parse_function_type("int Function<S1>()"),
            false,
            &["_ <: T <: int"],
        );
    });
}

#[test]
fn test_functionType_hasTypeFormals_bounds_same_leftDefault_rightObjectQ() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");
        check_match(
            &t,
            &[T],
            scope.parse_type("T Function<T1>()"),
            t.parse_function_type("int Function<S1 extends Object?>()"),
            false,
            &["_ <: T <: int"],
        );
    });
}

/// Dart: `@FailingTest(reason: 'Closure of type constraints is not
/// implemented yet')`.
#[test]
#[should_panic(expected = "T Function<X>(X) <# List<Y> Function<Y>(Y)")]
fn test_functionType_hasTypeFormals_closure() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");
        check_match(
            &t,
            &[T],
            scope.parse_type("T Function<X>(X)"),
            t.parse_function_type("List<Y> Function<Y>(Y)"),
            true,
            &["_ <: T <: List<Object?>"],
        );
    });
}

#[test]
fn test_functionType_hasTypeFormals_differentCount() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");
        check_not_match(
            &t,
            &[T],
            scope.parse_type("T Function<T1>()"),
            t.parse_function_type("int Function<S1, S2>()"),
            false,
        );
    });
}

#[test]
fn test_functionType_noTypeFormals_parameters_extraOptionalLeft() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");

        check_match(
            &t,
            &[T],
            t.parse_function_type("void Function([int])"),
            t.parse_function_type("void Function()"),
            true,
            &["_ <: T <: _"],
        );

        check_match(
            &t,
            &[T],
            t.parse_function_type("void Function({int a})"),
            t.parse_function_type("void Function()"),
            true,
            &["_ <: T <: _"],
        );
    });
}

#[test]
fn test_functionType_noTypeFormals_parameters_extraRequiredLeft() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");

        check_not_match(
            &t,
            &[T],
            t.parse_function_type("void Function(int)"),
            t.parse_function_type("void Function()"),
            true,
        );

        check_not_match(
            &t,
            &[T],
            t.parse_function_type("void Function({required int a})"),
            t.parse_function_type("void Function()"),
            true,
        );
    });
}

#[test]
fn test_functionType_noTypeFormals_parameters_extraRight() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");
        check_not_match(
            &t,
            &[T],
            t.parse_function_type("void Function()"),
            scope.parse_type("void Function(T)"),
            true,
        );
    });
}

#[test]
fn test_functionType_noTypeFormals_parameters_leftOptionalNamed() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");

        check_match(
            &t,
            &[T],
            t.parse_function_type("void Function({int a})"),
            scope.parse_type("void Function({T a})"),
            true,
            &["_ <: T <: int"],
        );

        check_match(
            &t,
            &[T],
            scope.parse_type("void Function({T a})"),
            t.parse_function_type("void Function({int a})"),
            false,
            &["int <: T <: _"],
        );

        // int vs. String
        check_not_match(
            &t,
            &[T],
            t.parse_function_type("void Function({int a})"),
            t.parse_function_type("void Function({String a})"),
            true,
        );

        // Skip left non-required named.
        check_match(
            &t,
            &[T],
            t.parse_function_type("void Function({int a, int b, int c})"),
            scope.parse_type("void Function({T b})"),
            true,
            &["_ <: T <: int"],
        );

        // Not match if skip left required named.
        check_not_match(
            &t,
            &[T],
            t.parse_function_type("void Function({required int a, int b})"),
            scope.parse_type("void Function({T b})"),
            true,
        );

        // Not match if skip right named.
        check_not_match(
            &t,
            &[T],
            t.parse_function_type("void Function({int b})"),
            scope.parse_type("void Function({int a, T b})"),
            true,
        );
    });
}

#[test]
fn test_functionType_noTypeFormals_parameters_leftOptionalPositional() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");

        check_match(
            &t,
            &[T],
            scope.parse_type("void Function([int])"),
            scope.parse_type("void Function(T)"),
            true,
            &["_ <: T <: int"],
        );

        check_match(
            &t,
            &[T],
            scope.parse_type("void Function([T])"),
            scope.parse_type("void Function(int)"),
            false,
            &["int <: T <: _"],
        );
        check_match(
            &t,
            &[T],
            scope.parse_type("void Function([int])"),
            scope.parse_type("void Function([T])"),
            true,
            &["_ <: T <: int"],
        );

        check_match(
            &t,
            &[T],
            scope.parse_type("void Function([T])"),
            scope.parse_type("void Function([int])"),
            false,
            &["int <: T <: _"],
        );

        check_not_match(
            &t,
            &[T],
            scope.parse_type("void Function([int])"),
            scope.parse_type("void Function(String)"),
            true,
        );

        check_not_match(
            &t,
            &[T],
            scope.parse_type("void Function([int])"),
            scope.parse_type("void Function([String])"),
            true,
        );

        check_not_match(
            &t,
            &[T],
            scope.parse_type("void Function([int])"),
            scope.parse_type("void Function({int a})"),
            true,
        );

        check_not_match(
            &t,
            &[T],
            scope.parse_type("void Function([int])"),
            scope.parse_type("void Function({int a})"),
            false,
        );
    });
}

#[test]
fn test_functionType_noTypeFormals_parameters_leftRequiredPositional() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");

        check_match(
            &t,
            &[T],
            scope.parse_type("void Function(int)"),
            scope.parse_type("void Function(T)"),
            true,
            &["_ <: T <: int"],
        );

        check_match(
            &t,
            &[T],
            scope.parse_type("void Function(T)"),
            scope.parse_type("void Function(int)"),
            false,
            &["int <: T <: _"],
        );

        check_not_match(
            &t,
            &[T],
            scope.parse_type("void Function(int)"),
            scope.parse_type("void Function(String)"),
            true,
        );

        check_not_match(
            &t,
            &[T],
            scope.parse_type("void Function(int)"),
            scope.parse_type("void Function([T])"),
            true,
        );

        check_not_match(
            &t,
            &[T],
            scope.parse_type("void Function(int)"),
            scope.parse_type("void Function({T a})"),
            true,
        );
    });
}

#[test]
fn test_functionType_noTypeFormals_returnType() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");

        check_match(
            &t,
            &[T],
            scope.parse_type("T Function()"),
            t.parse_function_type("int Function()"),
            false,
            &["_ <: T <: int"],
        );

        check_not_match(
            &t,
            &[T],
            t.parse_function_type("String Function()"),
            t.parse_function_type("int Function()"),
            false,
        );
    });
}

/// If `P` is `C<M0, ..., Mk>` and `Q` is `C<N0, ..., Nk>`, then the match
/// holds under constraints `C0 + ... + Ck`:
///   If `Mi` is a subtype match for `Ni` with respect to L under
///   constraints `Ci`.
#[test]
fn test_interfaceType_same() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");

        check_match(
            &t,
            &[T],
            scope.parse_type("List<T>"),
            t.parse_type("List<num>"),
            false,
            &["_ <: T <: num"],
        );
        check_match(
            &t,
            &[T],
            t.parse_type("List<int>"),
            scope.parse_type("List<T>"),
            true,
            &["int <: T <: _"],
        );

        check_not_match(
            &t,
            &[T],
            t.parse_type("List<int>"),
            t.parse_type("List<String>"),
            false,
        );

        check_match(
            &t,
            &[T],
            scope.parse_type("Map<int, List<T>>"),
            t.parse_type("Map<num, List<String>>"),
            false,
            &["_ <: T <: String"],
        );
        check_match(
            &t,
            &[T],
            t.parse_type("Map<int, List<String>>"),
            scope.parse_type("Map<num, List<T>>"),
            true,
            &["String <: T <: _"],
        );

        check_not_match(
            &t,
            &[T],
            scope.parse_type("Map<T, List<int>>"),
            t.parse_type("Map<num, List<String>>"),
            false,
        );
    });
}

/// If `P` is `C0<M0, ..., Mk>` and `Q` is `C1<N0, ..., Nj>` then the match
/// holds with respect to `L` under constraints `C`:
///   If `C1<B0, ..., Bj>` is a superinterface of `C0<M0, ..., Mk>` and
///   `C1<B0, ..., Bj>` is a subtype match for `C1<N0, ..., Nj>` with
///   respect to `L` under constraints `C`.
#[test]
fn test_interfaceType_superInterface() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");

        check_match(
            &t,
            &[T],
            scope.parse_type("List<T>"),
            t.parse_type("Iterable<num>"),
            false,
            &["_ <: T <: num"],
        );
        check_match(
            &t,
            &[T],
            t.parse_type("List<int>"),
            scope.parse_type("Iterable<T>"),
            true,
            &["int <: T <: _"],
        );

        check_not_match(
            &t,
            &[T],
            t.parse_type("List<int>"),
            t.parse_type("Iterable<String>"),
            true,
        );
    });
}

#[test]
fn test_interfaceType_topMerge() {
    let mut t = TypeSystemTest::new();
    let mut classes: Vec<ClassSpec> = Vec::new();

    let mut add_classes =
        |index: usize, extends_type_argument: &str, implements_type_argument: &str| {
            classes.extend([
                ClassSpec::new(&format!("class A{index}<T>")),
                ClassSpec::new(&format!("class B{index}<T> extends A{index}<T>")),
                ClassSpec::new(&format!(
                    "class C{index} extends A{index}<{extends_type_argument}> \
                 implements B{index}<{implements_type_argument}>"
                )),
            ]);
        };

    add_classes(0, "Object?", "dynamic");
    add_classes(1, "dynamic", "Object?");
    add_classes(2, "void", "Object?");
    add_classes(3, "Object?", "void");

    t.build_test_library(LibrarySpec {
        classes,
        ..LibrarySpec::test()
    });

    let check_match_index = |index: usize, expected_constraint: &str| {
        t.with_type_parameter_scope("T", |scope| {
            let T = scope.type_parameter("T");
            check_match(
                &t,
                &[T],
                t.parse_type(&format!("C{index}")),
                scope.parse_type(&format!("A{index}<T>")),
                true,
                &[expected_constraint],
            );
        });
    };

    check_match_index(0, "Object? <: T <: _");
    check_match_index(1, "Object? <: T <: _");
    check_match_index(2, "Object? <: T <: _");
    check_match_index(3, "Object? <: T <: _");
}

/// If `P` is `FutureOr<P0>` the match holds under constraint set `C1 + C2`:
///   If `Future<P0>` is a subtype match for `Q` under constraint set `C1`.
///   And if `P0` is a subtype match for `Q` under constraint set `C2`.
#[test]
fn test_left_futureOr() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");

        check_match(
            &t,
            &[T],
            scope.parse_type("FutureOr<T>"),
            t.parse_type("FutureOr<int>"),
            false,
            &["_ <: T <: int"],
        );

        // This is 'T <: int' and 'T <: Future<int>'.
        check_match(
            &t,
            &[T],
            scope.parse_type("FutureOr<T>"),
            t.parse_type("Future<int>"),
            false,
            &["_ <: T <: Never"],
        );

        check_not_match(
            &t,
            &[T],
            scope.parse_type("FutureOr<T>"),
            t.parse_type("int"),
            false,
        );
    });
}

/// If `P` is `Never` then the match holds under no constraints.
#[test]
fn test_left_never() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");
        check_match(
            &t,
            &[T],
            t.parse_type("Never"),
            t.parse_type("int"),
            false,
            &["_ <: T <: _"],
        );
    });
}

/// If `P` is `Null`, then the match holds under no constraints:
///  Only if `Q` is nullable.
#[test]
fn test_left_null() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");

        check_not_match(&t, &[T], t.parse_type("Null"), t.parse_type("int"), true);

        check_match(
            &t,
            &[T],
            t.parse_type("Null"),
            scope.parse_type("T"),
            true,
            &["Null <: T <: _"],
        );

        check_match(
            &t,
            &[T],
            t.parse_type("Null"),
            scope.parse_type("FutureOr<T>"),
            true,
            &["Null <: T <: _"],
        );

        let match_no_constraints = |Q: TypeId| {
            check_match(&t, &[T], t.parse_type("Null"), Q, true, &["_ <: T <: _"]);
        };

        match_no_constraints(scope.parse_type("List<T>?"));
        match_no_constraints(t.parse_type("String?"));
        match_no_constraints(t.parse_type("void"));
        match_no_constraints(t.parse_type("dynamic"));
        match_no_constraints(t.parse_type("Object?"));
        match_no_constraints(t.parse_type("Null"));
        match_no_constraints(t.parse_function_type("void Function()?"));
    });
}

/// If `P` is `P0?` the match holds under constraint set `C1 + C2`:
///   If `P0` is a subtype match for `Q` under constraint set `C1`.
///   And if `Null` is a subtype match for `Q` under constraint set `C2`.
#[test]
fn test_left_suffixQuestion() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");

        // TODO(scheglov): any better test case?
        check_match(
            &t,
            &[T],
            t.parse_type("num?"),
            t.parse_type("dynamic"),
            true,
            &["_ <: T <: _"],
        );

        check_not_match(&t, &[T], scope.parse_type("T?"), t.parse_type("int"), true);
    });
}

/// If `Q` is `Q0?` the match holds under constraint set `C`:
///   Or if `P` is `dynamic` or `void` and `Object` is a subtype match
///   for `Q0` under constraint set `C`.
#[test]
fn test_left_top_right_nullable() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("U extends Object", |scope| {
        let U = scope.type_parameter("U");
        let U_question = scope.parse_type("U?");

        check_match(
            &t,
            &[U],
            t.parse_type("dynamic"),
            U_question,
            false,
            &["Object <: U <: _"],
        );
        check_match(
            &t,
            &[U],
            t.parse_type("void"),
            U_question,
            false,
            &["Object <: U <: _"],
        );
    });
}

/// If `P` is a type variable `X` in `L`, then the match holds:
///   Under constraint `_ <: X <: Q`.
#[test]
fn test_left_typeParameter2() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");

        let check = |right: TypeId, expected: &str| {
            check_match(&t, &[T], scope.parse_type("T"), right, false, &[expected]);
        };

        check(t.parse_type("num"), "_ <: T <: num");
        check(t.parse_type("num?"), "_ <: T <: num?");
    });
}

/// If `P` is a type variable `X` with bound `B` (or a promoted type variable
/// `X & B`), the match holds with constraint set `C`:
///   If `B` is a subtype match for `Q` with constraint set `C`.
/// Note: we have already eliminated the case that `X` is a variable in `L`.
#[test]
fn test_left_typeParameterOther() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");

        scope.with_type_parameter_scope("U extends int", |scope| {
            check_match(
                &t,
                &[T],
                scope.parse_type("U"),
                t.parse_type("num"),
                false,
                &["_ <: T <: _"],
            );
        });

        scope.with_type_parameter_scope("U", |scope| {
            check_match(
                &t,
                &[T],
                scope.parse_type("U & int"),
                t.parse_type("num"),
                false,
                &["_ <: T <: _"],
            );

            check_not_match(&t, &[T], scope.parse_type("U"), t.parse_type("num"), false);
        });
    });
}

/// If `P` is `_` then the match holds with no constraints.
#[test]
fn test_left_unknown() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");
        check_match(
            &t,
            &[T],
            t.parse_type("UnknownInferredType"),
            t.parse_type("num"),
            true,
            &["_ <: T <: _"],
        );
    });
}

#[test]
fn test_recordType_differentShape() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");

        check_not_match(
            &t,
            &[T],
            scope.parse_type("(T, int)"),
            scope.parse_type("(int,)"),
            true,
        );

        check_not_match(
            &t,
            &[T],
            scope.parse_type("(T,)"),
            scope.parse_type("(int, int)"),
            true,
        );

        check_not_match(
            &t,
            &[T],
            scope.parse_type("({T f1})"),
            scope.parse_type("({int f2})"),
            true,
        );

        check_not_match(
            &t,
            &[T],
            scope.parse_type("({T f1, int f2})"),
            scope.parse_type("({int f1})"),
            true,
        );

        check_not_match(
            &t,
            &[T],
            scope.parse_type("({T f1})"),
            scope.parse_type("({int f1, int f2})"),
            true,
        );

        check_not_match(
            &t,
            &[T],
            scope.parse_type("(int, {T f2})"),
            scope.parse_type("({int f1, int f2})"),
            true,
        );
    });
}

#[test]
fn test_recordType_recordClass() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");
        check_match(
            &t,
            &[T],
            scope.parse_type("(T,)"),
            t.parse_type("Record"),
            true,
            &["_ <: T <: _"],
        );
    });
}

#[test]
fn test_recordType_sameShape_named() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");

        check_match(
            &t,
            &[T],
            scope.parse_type("({T f1})"),
            scope.parse_type("({int f1})"),
            true,
            &["_ <: T <: int"],
        );

        check_match(
            &t,
            &[T],
            scope.parse_type("({int f1})"),
            scope.parse_type("({T f1})"),
            false,
            &["int <: T <: _"],
        );

        check_not_match(
            &t,
            &[T],
            scope.parse_type("({int f1})"),
            scope.parse_type("({String f1})"),
            false,
        );

        check_match(
            &t,
            &[T],
            scope.parse_type("({int f1, T f2})"),
            scope.parse_type("({num f1, String f2})"),
            true,
            &["_ <: T <: String"],
        );

        check_match(
            &t,
            &[T],
            scope.parse_type("({int f1, String f2})"),
            scope.parse_type("({num f1, T f2})"),
            false,
            &["String <: T <: _"],
        );

        check_not_match(
            &t,
            &[T],
            scope.parse_type("(T, int, {T f1, int f2})"),
            scope.parse_type("({int f1, String f2})"),
            true,
        );
    });
}

#[test]
fn test_recordType_sameShape_positional() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");

        check_match(
            &t,
            &[T],
            scope.parse_type("(T,)"),
            scope.parse_type("(num,)"),
            true,
            &["_ <: T <: num"],
        );

        check_match(
            &t,
            &[T],
            scope.parse_type("(int,)"),
            scope.parse_type("(T,)"),
            false,
            &["int <: T <: _"],
        );

        check_not_match(
            &t,
            &[T],
            scope.parse_type("(int,)"),
            scope.parse_type("(String,)"),
            false,
        );

        check_match(
            &t,
            &[T],
            scope.parse_type("(int, T)"),
            scope.parse_type("(num, String)"),
            true,
            &["_ <: T <: String"],
        );

        check_match(
            &t,
            &[T],
            scope.parse_type("(int, String)"),
            scope.parse_type("(num, T)"),
            false,
            &["String <: T <: _"],
        );

        check_not_match(
            &t,
            &[T],
            scope.parse_type("(T, int)"),
            scope.parse_type("(num, String)"),
            true,
        );
    });
}

#[test]
fn test_right_functionClass() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");
        check_match(
            &t,
            &[T],
            t.parse_function_type("void Function()"),
            t.parse_type("Function"),
            true,
            &["_ <: T <: _"],
        );
    });
}

/// If `Q` is `FutureOr<Q0>` the match holds under constraint set `C`:
#[test]
fn test_right_futureOr() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");

        // If `P` is `FutureOr<P0>` and `P0` is a subtype match for `Q0` under
        // constraint set `C`.
        check_match(
            &t,
            &[T],
            scope.parse_type("FutureOr<T>"),
            t.parse_type("FutureOr<num>"),
            false,
            &["_ <: T <: num"],
        );
        check_match(
            &t,
            &[T],
            t.parse_type("FutureOr<num>"),
            scope.parse_type("FutureOr<T>"),
            true,
            &["num <: T <: _"],
        );
        check_not_match(
            &t,
            &[T],
            t.parse_type("FutureOr<String>"),
            t.parse_type("FutureOr<int>"),
            true,
        );

        // Or if `P` is a subtype match for `Future<Q0>` under non-empty
        // constraint set `C`.
        check_match(
            &t,
            &[T],
            scope.parse_type("Future<T>"),
            t.parse_type("FutureOr<num>"),
            false,
            &["_ <: T <: num"],
        );
        check_match(
            &t,
            &[T],
            t.parse_type("Future<int>"),
            scope.parse_type("FutureOr<T>"),
            true,
            &["int <: T <: _"],
        );
        check_match(
            &t,
            &[T],
            t.parse_type("Future<int>"),
            t.parse_type("FutureOr<Object>"),
            true,
            &["_ <: T <: _"],
        );
        check_not_match(
            &t,
            &[T],
            t.parse_type("Future<String>"),
            t.parse_type("FutureOr<int>"),
            true,
        );

        // Or if `P` is a subtype match for `Q0` under constraint set `C`.
        check_match(
            &t,
            &[T],
            scope.parse_type("List<T>"),
            t.parse_type("FutureOr<List<int>>"),
            false,
            &["_ <: T <: int"],
        );
        check_match(
            &t,
            &[T],
            t.parse_type("Never"),
            scope.parse_type("FutureOr<T>"),
            true,
            &["Never <: T <: _"],
        );

        // Or if `P` is a subtype match for `Future<Q0>` under empty
        // constraint set `C`.
        check_match(
            &t,
            &[T],
            t.parse_type("Future<int>"),
            t.parse_type("FutureOr<num>"),
            false,
            &["_ <: T <: _"],
        );

        // Otherwise.
        check_not_match(
            &t,
            &[T],
            scope.parse_type("List<T>"),
            t.parse_type("FutureOr<int>"),
            false,
        );
    });
}

/// If `Q` is `Object`, then the match holds under no constraints:
///  Only if `P` is non-nullable.
#[test]
fn test_right_object() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");

        check_match(
            &t,
            &[T],
            t.parse_type("int"),
            t.parse_type("Object"),
            false,
            &["_ <: T <: _"],
        );
        check_not_match(
            &t,
            &[T],
            t.parse_type("int?"),
            t.parse_type("Object"),
            false,
        );

        check_not_match(
            &t,
            &[T],
            t.parse_type("dynamic"),
            t.parse_type("Object"),
            false,
        );

        scope.with_type_parameter_scope("U extends num?", |scope| {
            check_not_match(
                &t,
                &[T],
                scope.parse_type("U"),
                t.parse_type("Object"),
                false,
            );
        });
    });
}

/// If `Q` is `Q0?` the match holds under constraint set `C`:
#[test]
fn test_right_suffixQuestion() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");
        let T_question = scope.parse_type("T?");

        // If `P` is `P0?` and `P0` is a subtype match for `Q0` under
        // constraint set `C`.
        check_match(
            &t,
            &[T],
            T_question,
            t.parse_type("num?"),
            false,
            &["_ <: T <: num"],
        );
        check_match(
            &t,
            &[T],
            t.parse_type("int?"),
            T_question,
            true,
            &["int <: T <: _"],
        );

        // Or if `P` is a subtype match for `Q0` under non-empty
        // constraint set `C`.
        check_match(
            &t,
            &[T],
            t.parse_type("int"),
            T_question,
            false,
            &["int <: T <: _"],
        );

        // Or if `P` is a subtype match for `Null` under constraint set `C`.
        check_match(
            &t,
            &[T],
            t.parse_type("Null"),
            t.parse_type("int?"),
            true,
            &["_ <: T <: _"],
        );

        // Or if `P` is a subtype match for `Q0` under empty
        // constraint set `C`.
        check_match(
            &t,
            &[T],
            t.parse_type("int"),
            t.parse_type("int?"),
            true,
            &["_ <: T <: _"],
        );

        check_not_match(&t, &[T], t.parse_type("int"), t.parse_type("String?"), true);
        check_not_match(
            &t,
            &[T],
            t.parse_type("int?"),
            t.parse_type("String?"),
            true,
        );
    });
}

/// If `Q` is `dynamic`, `Object?`, or `void` then the match holds under no
/// constraints.
#[test]
fn test_right_top() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");
        check_match(
            &t,
            &[T],
            t.parse_type("int"),
            t.parse_type("dynamic"),
            false,
            &["_ <: T <: _"],
        );
        check_match(
            &t,
            &[T],
            t.parse_type("int"),
            t.parse_type("Object?"),
            false,
            &["_ <: T <: _"],
        );
        check_match(
            &t,
            &[T],
            t.parse_type("int"),
            t.parse_type("void"),
            false,
            &["_ <: T <: _"],
        );
    });
}

/// If `Q` is a type variable `X` in `L`, then the match holds:
///   Under constraint `P <: X <: _`.
#[test]
fn test_right_typeParameter2() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");

        let check = |left: TypeId, expected: &str| {
            check_match(&t, &[T], left, scope.parse_type("T"), true, &[expected]);
        };

        check(t.parse_type("num"), "num <: T <: _");
        check(t.parse_type("num?"), "num? <: T <: _");
    });
}

/// If `Q` is `_` then the match holds with no constraints.
#[test]
fn test_right_unknown() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        let T = scope.type_parameter("T");
        check_match(
            &t,
            &[T],
            t.parse_type("num"),
            t.parse_type("UnknownInferredType"),
            true,
            &["_ <: T <: _"],
        );
        check_match(
            &t,
            &[T],
            t.parse_type("num"),
            t.parse_type("UnknownInferredType"),
            true,
            &["_ <: T <: _"],
        );
    });
}
