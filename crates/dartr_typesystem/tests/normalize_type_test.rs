// Dart source: pkg/analyzer/test/src/dart/element/normalize_type_test.dart

//! `NormalizeTypeTest`.

use dartr_element::{Nullability, TypeId, TypeKind};
use dartr_typesystem::TypeExt;
use dartr_typesystem::test_support::*;

fn _assert_nullability(t: &TypeSystemTest, ty: TypeId, expected: Nullability) {
    if t.ctx().nullability_suffix(ty) != expected {
        panic!("Expected {expected:?} in {}", t.display(ty));
    }
}

fn _assert_nullability_question(t: &TypeSystemTest, ty: TypeId) {
    _assert_nullability(t, ty, Nullability::Question);
}

/// `_check(T, expected)`: Dart `expect(result, expected)` is Dart `==`.
fn _check(t: &TypeSystemTest, ty: TypeId, expected: TypeId) {
    let result = t.type_system().normalize(ty);
    assert!(
        t.ctx().dart_eq(result, expected),
        "\nexpected: {}\nactual: {}\n",
        t.display(expected),
        t.display(result)
    );
    _check_formal_parameters_is_covariant(t, result, expected);
}

fn _check_formal_parameters_is_covariant(t: &TypeSystemTest, t1: TypeId, t2: TypeId) {
    let ctx = t.ctx();
    if let (TypeKind::Function(f1), TypeKind::Function(f2)) = (*ctx.ty(t1), *ctx.ty(t2)) {
        let parameters1 = ctx.list(f1.params);
        let parameters2 = ctx.list(f2.params);
        assert_eq!(parameters1.len(), parameters2.len());
        for (parameter1, parameter2) in parameters1.iter().zip(parameters2) {
            if parameter1.covariant != parameter2.covariant {
                panic!(
                    "\nparameter1 isCovariant: {}\nparameter2 isCovariant: {}\nT1: {}\nT2: {}\n",
                    parameter1.covariant,
                    parameter2.covariant,
                    t.display(t1),
                    t.display(t2)
                );
            }
            _check_formal_parameters_is_covariant(t, parameter1.ty, parameter2.ty);
        }
    }
}

#[test]
fn function_type_parameter() {
    let t = TypeSystemTest::new();
    _check(
        &t,
        t.parse_type("void Function(FutureOr<Object>)"),
        t.parse_type("void Function(Object)"),
    );

    _check(
        &t,
        t.parse_type("void Function({FutureOr<Object> a})"),
        t.parse_type("void Function({Object a})"),
    );

    _check(
        &t,
        t.parse_type("void Function({required FutureOr<Object> a})"),
        t.parse_type("void Function({required Object a})"),
    );

    _check(
        &t,
        t.parse_type("void Function([FutureOr<Object>])"),
        t.parse_type("void Function([Object])"),
    );
}

#[test]
fn function_type_parameter_covariant() {
    let t = TypeSystemTest::new();
    _check(
        &t,
        t.parse_type("void Function(covariant FutureOr<Object>)"),
        t.parse_type("void Function(covariant Object)"),
    );
}

#[test]
fn function_type_parameter_type_parameter() {
    let t = TypeSystemTest::new();
    _check(
        &t,
        t.parse_type("void Function<T extends Never>(T)"),
        t.parse_type("void Function<T2 extends Never>(Never)"),
    );

    _check(
        &t,
        t.parse_type("void Function<T extends Iterable<FutureOr<dynamic>>>(T)"),
        t.parse_type("void Function<T2 extends Iterable<dynamic>>(T2)"),
    );
}

#[test]
fn function_type_return_type() {
    let t = TypeSystemTest::new();
    _check(
        &t,
        t.parse_type("FutureOr<Object> Function()"),
        t.parse_type("Object Function()"),
    );

    _check(
        &t,
        t.parse_type("int Function()"),
        t.parse_type("int Function()"),
    );
}

#[test]
fn function_type_type_parameter_bound_normalized() {
    let t = TypeSystemTest::new();
    _check(
        &t,
        t.parse_type("void Function<T extends FutureOr<Object>>()"),
        t.parse_type("void Function<T extends Object>()"),
    );
}

#[test]
fn function_type_type_parameter_bound_unchanged() {
    let t = TypeSystemTest::new();
    _check(
        &t,
        t.parse_type("int Function<T extends num>()"),
        t.parse_type("int Function<T extends num>()"),
    );
}

#[test]
fn function_type_type_parameter_fresh() {
    let t = TypeSystemTest::new();
    _check(
        &t,
        t.parse_type("T Function<T>(T)"),
        t.parse_type("U Function<U>(U)"),
    );
}

#[test]
fn function_type_type_parameter_fresh_bound() {
    let t = TypeSystemTest::new();
    _check(
        &t,
        t.parse_type("T Function<T, S extends T>(T, S)"),
        t.parse_type("U Function<U, V extends U>(U, V)"),
    );
}

/// `NORM(FutureOr<T>)`
/// * let S be NORM(T)
#[test]
fn future_or() {
    let t = TypeSystemTest::new();
    // * if S is a top type then S
    _check(
        &t,
        t.parse_type("FutureOr<dynamic>"),
        t.parse_type("dynamic"),
    );
    _check(
        &t,
        t.parse_type("FutureOr<InvalidType>"),
        t.parse_type("InvalidType"),
    );
    _check(&t, t.parse_type("FutureOr<void>"), t.parse_type("void"));
    _check(
        &t,
        t.parse_type("FutureOr<Object?>"),
        t.parse_type("Object?"),
    );

    // * if S is Object then S
    _check(&t, t.parse_type("FutureOr<Object>"), t.parse_type("Object"));

    // * if S is Never then Future<Never>
    _check(
        &t,
        t.parse_type("FutureOr<Never>"),
        t.parse_type("Future<Never>"),
    );

    // * if S is Null then Future<Null>?
    _check(
        &t,
        t.parse_type("FutureOr<Null>"),
        t.parse_type("Future<Null>?"),
    );

    // * else FutureOr<S>
    _check(
        &t,
        t.parse_type("FutureOr<int>"),
        t.parse_type("FutureOr<int>"),
    );
}

#[test]
fn interface_type() {
    let t = TypeSystemTest::new();
    _check(&t, t.parse_type("List<int>"), t.parse_type("List<int>"));

    _check(
        &t,
        t.parse_type("List<FutureOr<Object>>"),
        t.parse_type("List<Object>"),
    );
}

#[test]
fn primitive() {
    let t = TypeSystemTest::new();
    _check(&t, t.parse_type("dynamic"), t.parse_type("dynamic"));
    _check(&t, t.parse_type("Never"), t.parse_type("Never"));
    _check(&t, t.parse_type("void"), t.parse_type("void"));
    _check(&t, t.parse_type("int"), t.parse_type("int"));
}

/// NORM(T?)
/// * let S be NORM(T)
#[test]
fn question() {
    let t = TypeSystemTest::new();
    let check = |ty: TypeId, expected: TypeId| {
        _assert_nullability_question(&t, ty);
        _check(&t, ty, expected);
    };

    // * if S is a top type then S
    check(t.parse_type("FutureOr<dynamic>?"), t.parse_type("dynamic"));
    check(t.parse_type("FutureOr<void>?"), t.parse_type("void"));
    check(t.parse_type("FutureOr<Object?>?"), t.parse_type("Object?"));

    // * if S is Never then Null
    check(t.parse_type("Never?"), t.parse_type("Null"));

    // * if S is Never* then Null
    // Analyzer: impossible, we have only one suffix

    // * if S is Null then Null
    // Analyzer: impossible; `Null?` is always represented as `Null`.

    // * if S is FutureOr<R> and R is nullable then S
    check(
        t.parse_type("FutureOr<int?>?"),
        t.parse_type("FutureOr<int?>"),
    );

    // * if S is FutureOr<R>* and R is nullable then FutureOr<R>
    // Analyzer: impossible, we have only one suffix

    // * if S is R? then R?
    // * if S is R* then R?
    // * else S?
    check(t.parse_type("int?"), t.parse_type("int?"));
    check(t.parse_type("Object?"), t.parse_type("Object?"));
    check(t.parse_type("FutureOr<Object>?"), t.parse_type("Object?"));
}

#[test]
fn record_type() {
    let t = TypeSystemTest::new();
    _check(
        &t,
        t.parse_record_type("(int,)"),
        t.parse_record_type("(int,)"),
    );

    _check(
        &t,
        t.parse_record_type("(FutureOr<Object>,)"),
        t.parse_record_type("(Object,)"),
    );

    _check(
        &t,
        t.parse_record_type("({FutureOr<Object> foo})"),
        t.parse_record_type("({Object foo})"),
    );
}

/// NORM(X & T)
/// * let S be NORM(T)
#[test]
fn type_parameter_bound() {
    let t = TypeSystemTest::new();
    // * if S is Never then Never
    t.with_type_parameter_scope("T extends Never", |scope| {
        _check(&t, scope.parse_type("T"), t.parse_type("Never"));
    });

    // * else X
    t.with_type_parameter_scope("T", |scope| {
        _check(&t, scope.parse_type("T"), scope.parse_type("T"));
    });

    // * else X
    t.with_type_parameter_scope("T extends FutureOr<Object>", |scope| {
        _check(&t, scope.parse_type("T"), scope.parse_type("T"));
    });
}

#[test]
fn type_parameter_bound_recursive() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T extends Iterable<T>", |scope| {
        _check(&t, scope.parse_type("T"), scope.parse_type("T"));
    });
}

#[test]
fn type_parameter_promoted() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope| {
        // * if S is Never then Never
        _check(&t, scope.parse_type("T & Never"), t.parse_type("Never"));

        // * if S is a top type then X
        _check(&t, scope.parse_type("T & Object?"), scope.parse_type("T"));
        _check(
            &t,
            scope.parse_type("T & FutureOr<Object>?"),
            scope.parse_type("T"),
        );

        // * if S is X then X
        _check(&t, scope.parse_type("T & T"), scope.parse_type("T"));

        // else X & S
        _check(
            &t,
            scope.parse_type("T & FutureOr<Never>"),
            scope.parse_type("T & Future<Never>"),
        );
    });

    // * if S is Object and NORM(B) is Object where B is the bound of X then X
    t.with_type_parameter_scope("T extends Object", |scope| {
        _check(
            &t,
            scope.parse_type("T & FutureOr<Object>"),
            scope.parse_type("T"),
        );
    });
}
