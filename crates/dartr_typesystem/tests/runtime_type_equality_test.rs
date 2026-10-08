// Dart source: pkg/analyzer/test/src/dart/element/runtime_type_equality_test.dart

//! `RuntimeTypeEqualityTypeTest`.

use dartr_element::TypeId;
use dartr_typesystem::test_support::*;

fn _check(t: &TypeSystemTest, t1: TypeId, t2: TypeId, expected: bool) {
    let ts = t.type_system();

    let result = ts.runtime_types_equal(t1, t2);
    if result != expected {
        panic!(
            "\nExpected {}.\nT1: {}\nT2: {}\n",
            if expected { "equal" } else { "not equal" },
            t.display(t1),
            t.display(t2)
        );
    }

    let result = ts.runtime_types_equal(t2, t1);
    if result != expected {
        panic!(
            "\nExpected {}.\nT1: {}\nT2: {}\n",
            if expected { "equal" } else { "not equal" },
            t.display(t1),
            t.display(t2)
        );
    }
}

fn _equal(t: &TypeSystemTest, t1: TypeId, t2: TypeId) {
    _check(t, t1, t2, true);
}

fn _equal2(t: &TypeSystemTest, t1: &str, t2: &str) {
    _equal(t, t.parse_type(t1), t.parse_type(t2));
}

fn _not_equal(t: &TypeSystemTest, t1: TypeId, t2: TypeId) {
    _check(t, t1, t2, false);
}

fn _not_equal2(t: &TypeSystemTest, t1: &str, t2: &str) {
    _not_equal(t, t.parse_type(t1), t.parse_type(t2));
}

#[test]
fn dynamic() {
    let t = TypeSystemTest::new();
    _equal(&t, t.parse_type("dynamic"), t.parse_type("dynamic"));
    _not_equal(&t, t.parse_type("dynamic"), t.parse_type("void"));
    _not_equal(&t, t.parse_type("dynamic"), t.parse_type("int"));

    _not_equal(&t, t.parse_type("dynamic"), t.parse_type("Never"));
    _not_equal(&t, t.parse_type("dynamic"), t.parse_type("Never?"));
}

#[test]
fn function_type_parameters() {
    let t = TypeSystemTest::new();
    let check = |t1: &str, t2: &str, expected: bool| {
        _check(
            &t,
            t.parse_function_type(t1),
            t.parse_function_type(t2),
            expected,
        );
    };

    {
        let check_required_parameter = |t1: &str, t2: &str, expected: bool| {
            check(
                &format!("void Function({t1})"),
                &format!("void Function({t2})"),
                expected,
            );
        };

        check_required_parameter("int", "int", true);
        check_required_parameter("int", "int?", false);

        check_required_parameter("int?", "int", false);
        check_required_parameter("int?", "int?", true);

        check("void Function(int a)", "void Function(int b)", true);

        check("void Function(int)", "void Function([int])", false);

        check("void Function(int)", "void Function({int a})", false);

        check(
            "void Function(int)",
            "void Function({required int a})",
            false,
        );
    }

    {
        check("void Function({int a})", "void Function({int a})", true);

        check("void Function({int a})", "void Function({bool a})", false);

        check("void Function({int a})", "void Function({int b})", false);

        check(
            "void Function({int a})",
            "void Function({required int a})",
            false,
        );
    }

    {
        check(
            "void Function({required int a})",
            "void Function({required int a})",
            true,
        );

        check(
            "void Function({required int a})",
            "void Function({required bool a})",
            false,
        );

        check(
            "void Function({required int a})",
            "void Function({required int b})",
            false,
        );

        check(
            "void Function({required int a})",
            "void Function({int a})",
            false,
        );
    }
}

#[test]
fn function_type_return_type() {
    let t = TypeSystemTest::new();
    let check = |t1: &str, t2: &str, expected: bool| {
        _check(
            &t,
            t.parse_function_type(t1),
            t.parse_function_type(t2),
            expected,
        );
    };

    check("int Function()", "int Function()", true);
    check("int Function()", "int? Function()", false);
}

#[test]
fn function_type_type_parameters() {
    let t = TypeSystemTest::new();
    {
        _check(
            &t,
            t.parse_type("void Function<T extends num>()"),
            t.parse_type("void Function()"),
            false,
        );
    }

    {
        _check(
            &t,
            t.parse_type("void Function<T extends num>()"),
            t.parse_type("void Function<U>()"),
            false,
        );
    }

    {
        _check(
            &t,
            t.parse_type("T Function<T>(T)"),
            t.parse_type("U Function<U>(U)"),
            true,
        );
    }
}

#[test]
fn interface_type() {
    let t = TypeSystemTest::new();
    _not_equal(&t, t.parse_type("int"), t.parse_type("bool"));

    _equal(&t, t.parse_type("int"), t.parse_type("int"));
    _not_equal(&t, t.parse_type("int"), t.parse_type("int?"));

    _not_equal(&t, t.parse_type("int?"), t.parse_type("int"));
    _equal(&t, t.parse_type("int?"), t.parse_type("int?"));
}

#[test]
fn interface_type_type_arguments() {
    let t = TypeSystemTest::new();
    _not_equal2(&t, "List<int>", "List<bool>");

    _equal2(&t, "List<int>", "List<int>");
    _not_equal2(&t, "List<int>", "List<int?>");

    _not_equal2(&t, "List<int?>", "List<int>");
    _equal2(&t, "List<int?>", "List<int?>");
}

#[test]
fn never() {
    let t = TypeSystemTest::new();
    _equal(&t, t.parse_type("Never"), t.parse_type("Never"));
    _not_equal(&t, t.parse_type("Never"), t.parse_type("Never?"));
    _not_equal(&t, t.parse_type("Never"), t.parse_type("int"));

    _not_equal(&t, t.parse_type("Never?"), t.parse_type("Never"));
    _equal(&t, t.parse_type("Never?"), t.parse_type("Never?"));
    _not_equal(&t, t.parse_type("Never?"), t.parse_type("int"));
    _equal(&t, t.parse_type("Never?"), t.parse_type("Null"));
}

#[test]
fn norm() {
    let t = TypeSystemTest::new();
    _equal(&t, t.parse_type("FutureOr<Object>"), t.parse_type("Object"));
    _equal(
        &t,
        t.parse_type("FutureOr<Never>"),
        t.parse_type("Future<Never>"),
    );
    _equal(&t, t.parse_type("Never?"), t.parse_type("Null"));
}

#[test]
fn record_type_and_not() {
    let t = TypeSystemTest::new();
    _not_equal2(&t, "(int,)", "dynamic");
    _not_equal2(&t, "(int,)", "int");
    _not_equal2(&t, "(int,)", "void");
}

#[test]
fn record_type_different_shape() {
    let t = TypeSystemTest::new();
    _not_equal2(&t, "(int,)", "(int, int)");
    _not_equal2(&t, "(int,)", "({int f1})");
    _not_equal2(&t, "({int f1})", "({int f2})");
    _not_equal2(&t, "({int f1})", "({int f1, int f2})");
}

#[test]
fn record_type_same_shape_named() {
    let t = TypeSystemTest::new();
    _equal2(&t, "({int f1})", "({int f1})");
    _not_equal2(&t, "({int f1})", "({int? f1})");

    _not_equal2(&t, "({int f1})", "({double f1})");
}

#[test]
fn record_type_same_shape_positional() {
    let t = TypeSystemTest::new();
    _equal2(&t, "(int,)", "(int,)");
    _not_equal2(&t, "(int,)", "(int?,)");

    _not_equal2(&t, "(int,)", "(double,)");
}

#[test]
fn void() {
    let t = TypeSystemTest::new();
    _equal(&t, t.parse_type("void"), t.parse_type("void"));
    _not_equal(&t, t.parse_type("void"), t.parse_type("dynamic"));
    _not_equal(&t, t.parse_type("void"), t.parse_type("int"));

    _not_equal(&t, t.parse_type("void"), t.parse_type("Never"));
    _not_equal(&t, t.parse_type("void"), t.parse_type("Never?"));
}
