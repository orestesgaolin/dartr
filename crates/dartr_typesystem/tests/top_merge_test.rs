// Dart source: pkg/analyzer/test/src/dart/element/top_merge_test.dart

//! `TopMergeTest`. Dart `topMerge` throws where `try_top_merge` returns
//! `None`, so `_checkThrows` checks for `None`.

use dartr_element::TypeId;
use dartr_typesystem::TypeExt;
use dartr_typesystem::test_support::*;

/// `_check(T, S, expected)`: Dart `result != expected` is Dart `==`.
fn _check(t: &TypeSystemTest, ty: TypeId, s: TypeId, expected: TypeId) {
    let ts = t.type_system();
    let result = ts
        .try_top_merge(ty, s)
        .unwrap_or_else(|| panic!("topMerge({}, {}) threw", t.display(ty), t.display(s)));
    if !t.ctx().dart_eq(result, expected) {
        panic!(
            "Expected: {}, actual: {}",
            t.display(expected),
            t.display(result)
        );
    }

    let result = ts
        .try_top_merge(s, ty)
        .unwrap_or_else(|| panic!("topMerge({}, {}) threw", t.display(s), t.display(ty)));
    if !t.ctx().dart_eq(result, expected) {
        panic!(
            "Expected: {}, actual: {}",
            t.display(expected),
            t.display(result)
        );
    }
}

/// `_checkThrows(T, S)`.
fn _check_throws(t: &TypeSystemTest, ty: TypeId, s: TypeId) {
    let ts = t.type_system();
    assert!(
        ts.try_top_merge(ty, s).is_none(),
        "Expected topMerge({}, {}) to throw",
        t.display(ty),
        t.display(s)
    );
    assert!(
        ts.try_top_merge(s, ty).is_none(),
        "Expected topMerge({}, {}) to throw",
        t.display(s),
        t.display(ty)
    );
}

#[test]
fn different_structure() {
    let t = TypeSystemTest::new();
    _check_throws(&t, t.parse_type("int"), t.parse_type("void Function()"));

    t.with_type_parameter_scope("T", |scope| {
        _check_throws(&t, t.parse_type("int"), scope.parse_type("T"));
        _check_throws(&t, t.parse_type("void Function()"), scope.parse_type("T"));
    });
}

#[test]
fn dynamic() {
    let t = TypeSystemTest::new();
    // NNBD_TOP_MERGE(dynamic, dynamic) = dynamic
    _check(
        &t,
        t.parse_type("dynamic"),
        t.parse_type("dynamic"),
        t.parse_type("dynamic"),
    );
}

#[test]
fn function_formal_parameters_different_count() {
    let t = TypeSystemTest::new();
    _check_throws(
        &t,
        t.parse_function_type("void Function()"),
        t.parse_function_type("void Function(int)"),
    );
}

#[test]
fn function_parameters_covariant() {
    let t = TypeSystemTest::new();
    _check(
        &t,
        t.parse_function_type("void Function(covariant Object?)"),
        t.parse_function_type("void Function(dynamic)"),
        t.parse_function_type("void Function(covariant Object?)"),
    );

    _check(
        &t,
        t.parse_function_type("void Function(covariant int)"),
        t.parse_function_type("void Function(num)"),
        t.parse_function_type("void Function(covariant num)"),
    );
}

#[test]
fn function_parameters_kind_optional_positional_optional_positional() {
    let t = TypeSystemTest::new();
    _check(
        &t,
        t.parse_function_type("void Function([int])"),
        t.parse_function_type("void Function([int])"),
        t.parse_function_type("void Function([int])"),
    );
}

#[test]
fn function_parameters_kind_required_named_optional_named() {
    let t = TypeSystemTest::new();
    _check_throws(
        &t,
        t.parse_function_type("void Function({required int a})"),
        t.parse_function_type("void Function({int a})"),
    );
}

#[test]
fn function_parameters_kind_required_positional_optional_positional() {
    let t = TypeSystemTest::new();
    _check_throws(
        &t,
        t.parse_function_type("void Function(int)"),
        t.parse_function_type("void Function([int])"),
    );

    _check_throws(
        &t,
        t.parse_function_type("void Function({int a})"),
        t.parse_function_type("void Function({int b})"),
    );
}

#[test]
fn function_parameters_mismatch() {
    let t = TypeSystemTest::new();
    _check(
        &t,
        t.parse_function_type("void Function(int a)"),
        t.parse_function_type("void Function(int b)"),
        t.parse_function_type("void Function(int a)"),
    );
}

#[test]
fn function_parameters_type() {
    let t = TypeSystemTest::new();
    _check(
        &t,
        t.parse_function_type("void Function(Object? a)"),
        t.parse_function_type("void Function(dynamic a)"),
        t.parse_function_type("void Function(Object? a)"),
    );
}

#[test]
fn function_return_type() {
    let t = TypeSystemTest::new();
    _check(
        &t,
        t.parse_function_type("void Function()"),
        t.parse_function_type("Object? Function()"),
        t.parse_function_type("Object? Function()"),
    );
}

#[test]
fn function_type_parameters_bounds_merge() {
    let t = TypeSystemTest::new();
    _check(
        &t,
        t.parse_function_type("T Function<T extends dynamic>()"),
        t.parse_function_type("T Function<T extends Object?>()"),
        t.parse_function_type("T Function<T extends Object?>()"),
    );
}

#[test]
fn function_type_parameters_bounds_mismatch() {
    let t = TypeSystemTest::new();
    _check_throws(
        &t,
        t.parse_function_type("T Function<T extends int>()"),
        t.parse_function_type("T Function<T>()"),
    );
}

#[test]
fn function_type_parameters_different_count() {
    let t = TypeSystemTest::new();
    _check_throws(
        &t,
        t.parse_function_type("void Function()"),
        t.parse_function_type("void Function<T, S>()"),
    );
}

#[test]
fn interface() {
    let t = TypeSystemTest::new();
    _check(
        &t,
        t.parse_type("List<dynamic>"),
        t.parse_type("List<Object?>"),
        t.parse_type("List<Object?>"),
    );

    _check(
        &t,
        t.parse_type("List<void>"),
        t.parse_type("List<Object?>"),
        t.parse_type("List<Object?>"),
    );

    _check_throws(&t, t.parse_type("Iterable<int>"), t.parse_type("List<int>"));
}

#[test]
fn invalid() {
    let t = TypeSystemTest::new();
    _check(
        &t,
        t.parse_type("InvalidType"),
        t.parse_type("int"),
        t.parse_type("InvalidType"),
    );
    _check(
        &t,
        t.parse_type("int"),
        t.parse_type("InvalidType"),
        t.parse_type("InvalidType"),
    );
}

#[test]
fn never() {
    let t = TypeSystemTest::new();
    _check(
        &t,
        t.parse_type("Never"),
        t.parse_type("Never"),
        t.parse_type("Never"),
    );
}

#[test]
fn nullability() {
    let t = TypeSystemTest::new();
    // NNBD_TOP_MERGE(T?, S?) = NNBD_TOP_MERGE(T, S)?
    _check(
        &t,
        t.parse_type("int?"),
        t.parse_type("int?"),
        t.parse_type("int?"),
    );
}

#[test]
fn nullability_mismatch() {
    let t = TypeSystemTest::new();
    _check_throws(&t, t.parse_type("int?"), t.parse_type("int"));
}

#[test]
fn object_question() {
    let t = TypeSystemTest::new();
    // NNBD_TOP_MERGE(Object?, Object?) = Object?
    _check(
        &t,
        t.parse_type("Object?"),
        t.parse_type("Object?"),
        t.parse_type("Object?"),
    );

    // NNBD_TOP_MERGE(Object?, void) = Object?
    // NNBD_TOP_MERGE(void, Object?) = Object?
    _check(
        &t,
        t.parse_type("Object?"),
        t.parse_type("void"),
        t.parse_type("Object?"),
    );

    // NNBD_TOP_MERGE(Object?, dynamic) = Object?
    // NNBD_TOP_MERGE(dynamic, Object?) = Object?
    _check(
        &t,
        t.parse_type("Object?"),
        t.parse_type("dynamic"),
        t.parse_type("Object?"),
    );
}

#[test]
fn record() {
    let t = TypeSystemTest::new();
    _check(
        &t,
        t.parse_record_type("(dynamic,)"),
        t.parse_record_type("(Object?,)"),
        t.parse_record_type("(Object?,)"),
    );

    _check(
        &t,
        t.parse_record_type("(void,)"),
        t.parse_record_type("(Object?,)"),
        t.parse_record_type("(Object?,)"),
    );

    _check(
        &t,
        t.parse_record_type("({dynamic f})"),
        t.parse_record_type("({Object? f})"),
        t.parse_record_type("({Object? f})"),
    );

    _check(
        &t,
        t.parse_record_type("(dynamic, {void f})"),
        t.parse_record_type("(Object?, {Object? f})"),
        t.parse_record_type("(Object?, {Object? f})"),
    );
}

#[test]
fn record_named_fields_different_count() {
    let t = TypeSystemTest::new();
    _check_throws(
        &t,
        t.parse_record_type("({int a})"),
        t.parse_record_type("({int a, int b})"),
    );
}

#[test]
fn record_named_fields_different_names() {
    let t = TypeSystemTest::new();
    _check_throws(
        &t,
        t.parse_record_type("({int a})"),
        t.parse_record_type("({int b})"),
    );
}

#[test]
fn record_positional_fields_different_count() {
    let t = TypeSystemTest::new();
    _check_throws(
        &t,
        t.parse_record_type("(int,)"),
        t.parse_record_type("(int, int)"),
    );
}

#[test]
fn type_parameter() {
    let t = TypeSystemTest::new();
    t.with_type_parameter_scope("T", |scope1| {
        _check(
            &t,
            scope1.parse_type("T"),
            scope1.parse_type("T"),
            scope1.parse_type("T"),
        );

        t.with_type_parameter_scope("T", |scope2| {
            _check_throws(&t, scope1.parse_type("T"), scope2.parse_type("T"));
            _check_throws(&t, scope2.parse_type("T"), scope1.parse_type("T"));
        });
    });
}

#[test]
fn void() {
    let t = TypeSystemTest::new();
    // NNBD_TOP_MERGE(void, void) = void
    _check(
        &t,
        t.parse_type("void"),
        t.parse_type("void"),
        t.parse_type("void"),
    );

    // NNBD_TOP_MERGE(void, dynamic) = Object?
    // NNBD_TOP_MERGE(dynamic, void) = Object?
    _check(
        &t,
        t.parse_type("void"),
        t.parse_type("dynamic"),
        t.parse_type("Object?"),
    );
}
