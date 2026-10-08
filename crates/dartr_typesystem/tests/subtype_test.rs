// Dart source: pkg/analyzer/test/src/dart/element/subtype_test.dart

//! Port of `subtype_test.dart`: `TypeSystem::is_subtype_of`
//! (`SubtypeTest`, `SubtypingCompoundTest`).

use dartr_element::TypeId;
use dartr_typesystem::test_support::*;

mod subtype_test {
    use super::*;

    /// `isNotSubtype(T0, T1)`.
    fn is_not_subtype(t: &TypeSystemTest, t0: TypeId, t1: TypeId) {
        assert!(
            !t.type_system().is_subtype_of(t0, t1),
            "Expected not subtype: {} <: {}",
            t.display(t0),
            t.display(t1)
        );
    }

    /// `isNotSubtype2(strT0, strT1)`.
    fn is_not_subtype2(t: &TypeSystemTest, str_t0: &str, str_t1: &str) {
        is_not_subtype(t, t.parse_type(str_t0), t.parse_type(str_t1));
    }

    /// `isSubtype(T0, T1)`.
    fn is_subtype(t: &TypeSystemTest, t0: TypeId, t1: TypeId) {
        assert!(
            t.type_system().is_subtype_of(t0, t1),
            "Expected subtype: {} <: {}",
            t.display(t0),
            t.display(t1)
        );
    }

    /// `isSubtype2(strT0, strT1)`.
    fn is_subtype2(t: &TypeSystemTest, str_t0: &str, str_t1: &str) {
        is_subtype(t, t.parse_type(str_t0), t.parse_type(str_t1));
    }

    #[test]
    fn extension_type_implements_not_nullable() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            extension_types: strs(&["extension type A(int it) implements int"]),
            ..LibrarySpec::test()
        });
        let ty = t.parse_interface_type("A");

        is_subtype(&t, ty, t.parse_type("Object?"));
        is_subtype(&t, ty, t.parse_type("Object"));
        is_subtype(&t, ty, t.parse_type("int"));
        is_subtype(&t, ty, t.parse_type("num"));
        is_subtype(&t, t.parse_type("Never"), ty);
    }

    #[test]
    fn extension_type_no_implemented_interfaces() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            extension_types: strs(&["extension type A(int it)"]),
            ..LibrarySpec::test()
        });
        let ty = t.parse_interface_type("A");

        is_subtype(&t, ty, t.parse_type("Object?"));
        is_not_subtype(&t, ty, t.parse_type("Object"));
        is_not_subtype(&t, ty, t.parse_type("int"));
    }

    #[test]
    fn extension_type_superinterfaces() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: vec![ClassSpec::new("class A"), ClassSpec::new("class B")],
            extension_types: strs(&["extension type X(int it) implements A"]),
            ..LibrarySpec::test()
        });
        let ty = t.parse_interface_type("X");

        is_subtype(&t, ty, t.parse_interface_type("A"));
        is_not_subtype(&t, ty, t.parse_interface_type("B"));
    }

    #[test]
    fn extension_type_type_arguments() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            extension_types: strs(&["extension type A<T>(int it)"]),
            ..LibrarySpec::test()
        });
        let a_object = t.parse_interface_type("A<Object>");
        let a_int = t.parse_interface_type("A<int>");
        let a_num = t.parse_interface_type("A<num>");

        is_subtype(&t, a_int, a_num);
        is_subtype(&t, a_int, a_object);
        is_not_subtype(&t, a_num, a_int);
    }

    #[test]
    fn function_type_01() {
        let t = TypeSystemTest::new();
        is_not_subtype2(
            &t,
            "E0 Function<E0>(E0, num)",
            "E1 Function<E1 extends num>(E1, E1)",
        );
    }

    #[test]
    fn function_type_02() {
        let t = TypeSystemTest::new();
        is_not_subtype2(
            &t,
            "int Function<E0 extends num>(E0)",
            "int Function<E1 extends int>(E1)",
        );
    }

    #[test]
    fn function_type_03() {
        let t = TypeSystemTest::new();
        is_not_subtype2(
            &t,
            "E0 Function<E0 extends num>(E0)",
            "E1 Function<E1 extends int>(E1)",
        );
    }

    #[test]
    fn function_type_04() {
        let t = TypeSystemTest::new();
        is_not_subtype2(
            &t,
            "E0 Function<E0 extends num>(int)",
            "E1 Function<E1 extends int>(int)",
        );
    }

    #[test]
    fn function_type_05() {
        let t = TypeSystemTest::new();
        is_subtype2(
            &t,
            "E0 Function<E0 extends num>(E0)",
            "num Function<E1 extends num>(E1)",
        );
    }

    #[test]
    fn function_type_06() {
        let t = TypeSystemTest::new();
        is_subtype2(
            &t,
            "E0 Function<E0 extends int>(E0)",
            "num Function<E1 extends int>(E1)",
        );
    }

    #[test]
    fn function_type_07() {
        let t = TypeSystemTest::new();
        is_subtype2(
            &t,
            "E0 Function<E0 extends int>(E0)",
            "int Function<E1 extends int>(E1)",
        );
    }

    #[test]
    fn function_type_08() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "int Function<E0>(int)", "int Function(int)");
    }

    #[test]
    fn function_type_09() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "int Function<E0, F0>(int)", "int Function<E1>(int)");
    }

    #[test]
    fn function_type_10() {
        let t = TypeSystemTest::new();
        is_subtype2(
            &t,
            "E0 Function<E0 extends List<E0>>(E0)",
            "E1 Function<E1 extends List<E1>>(E1)",
        );
    }

    #[test]
    fn function_type_11() {
        let t = TypeSystemTest::new();
        is_not_subtype2(
            &t,
            "E0 Function<E0 extends Iterable<E0>>(E0)",
            "E1 Function<E1 extends List<E1>>(E1)",
        );
    }

    #[test]
    fn function_type_12() {
        let t = TypeSystemTest::new();
        is_not_subtype2(
            &t,
            "E0 Function<E0>(E0, List<Object>)",
            "E1 Function<E1 extends List<E1>>(E1, E1)",
        );
    }

    #[test]
    fn function_type_13() {
        let t = TypeSystemTest::new();
        is_not_subtype2(
            &t,
            "List<E0> Function<E0>(E0, List<Object>)",
            "E1 Function<E1 extends List<E1>>(E1, E1)",
        );
    }

    #[test]
    fn function_type_14() {
        let t = TypeSystemTest::new();
        is_not_subtype2(
            &t,
            "int Function<E0>(E0, List<Object>)",
            "E1 Function<E1 extends List<E1>>(E1, E1)",
        );
    }

    #[test]
    fn function_type_15() {
        let t = TypeSystemTest::new();
        is_not_subtype2(
            &t,
            "E0 Function<E0>(E0, List<Object>)",
            "void Function<E1 extends List<E1>>(E1, E1)",
        );
    }

    #[test]
    fn function_type_16() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "int Function()", "Function");
    }

    #[test]
    fn function_type_17() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "Function", "int Function()");
    }

    #[test]
    fn function_type_18() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "dynamic Function()", "dynamic Function()");
    }

    #[test]
    fn function_type_19() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "dynamic Function()", "void Function()");
    }

    #[test]
    fn function_type_20() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "void Function()", "dynamic Function()");
    }

    #[test]
    fn function_type_21() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "int Function()", "void Function()");
    }

    #[test]
    fn function_type_22() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "void Function()", "int Function()");
    }

    #[test]
    fn function_type_23() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "void Function()", "void Function()");
    }

    #[test]
    fn function_type_24() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "int Function()", "int Function()");
    }

    #[test]
    fn function_type_25() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "int Function()", "Object Function()");
    }

    #[test]
    fn function_type_26() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "int Function()", "double Function()");
    }

    #[test]
    fn function_type_27() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "int Function()", "void Function(int)");
    }

    #[test]
    fn function_type_28() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "void Function()", "int Function(int)");
    }

    #[test]
    fn function_type_29() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "void Function()", "void Function(int)");
    }

    #[test]
    fn function_type_30() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "int Function(int)", "int Function(int)");
    }

    #[test]
    fn function_type_31() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "int Function(Object)", "Object Function(int)");
    }

    #[test]
    fn function_type_32() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "int Function(int)", "int Function(double)");
    }

    #[test]
    fn function_type_33() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "int Function()", "int Function(int)");
    }

    #[test]
    fn function_type_34() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "int Function(int)", "int Function(int, int)");
    }

    #[test]
    fn function_type_35() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "int Function(int, int)", "int Function(int)");
    }

    #[test]
    fn function_type_36() {
        let t = TypeSystemTest::new();
        is_not_subtype2(
            &t,
            "void Function(void Function())",
            "void Function(void Function(int))",
        );

        is_not_subtype2(
            &t,
            "void Function(void Function(int))",
            "void Function(void Function())",
        );
    }

    #[test]
    fn function_type_37() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "void Function([int])", "void Function()");
    }

    #[test]
    fn function_type_38() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "void Function([int])", "void Function(int)");
    }

    #[test]
    fn function_type_39() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "void Function(int)", "void Function([int])");
    }

    #[test]
    fn function_type_40() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "void Function([int])", "void Function([int])");
    }

    #[test]
    fn function_type_41() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "void Function([Object])", "void Function([int])");
    }

    #[test]
    fn function_type_42() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "void Function([int])", "void Function([Object])");
    }

    #[test]
    fn function_type_43() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "void Function(int, [int])", "void Function(int)");
    }

    #[test]
    fn function_type_44() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "void Function(int, [int])", "void Function(int, [int])");
    }

    #[test]
    fn function_type_45() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "void Function([int, int])", "void Function(int)");
    }

    #[test]
    fn function_type_46() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "void Function([int, int])", "void Function(int, [int])");
    }

    #[test]
    fn function_type_47() {
        let t = TypeSystemTest::new();
        is_not_subtype2(
            &t,
            "void Function([int, int])",
            "void Function(int, [int, int])",
        );
    }

    #[test]
    fn function_type_48() {
        let t = TypeSystemTest::new();
        is_subtype2(
            &t,
            "void Function([int, int, int])",
            "void Function(int, [int, int])",
        );
    }

    #[test]
    fn function_type_49() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "void Function([int])", "void Function(double)");
    }

    #[test]
    fn function_type_50() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "void Function([int])", "void Function([int, int])");
    }

    #[test]
    fn function_type_51() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "void Function([int, int])", "void Function([int])");
    }

    #[test]
    fn function_type_52() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "void Function([Object, int])", "void Function([int])");
    }

    #[test]
    fn function_type_53() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "void Function({int a})", "void Function()");
    }

    #[test]
    fn function_type_54() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "void Function({int a})", "void Function(int)");
    }

    #[test]
    fn function_type_55() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "void Function(int)", "void Function({int a})");
    }

    #[test]
    fn function_type_56() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "void Function({int a})", "void Function({int a})");
    }

    #[test]
    fn function_type_57() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "void Function({int a})", "void Function({int b})");
    }

    #[test]
    fn function_type_58() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "void Function({Object a})", "void Function({int a})");
    }

    #[test]
    fn function_type_59() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "void Function({int a})", "void Function({Object a})");
    }

    #[test]
    fn function_type_60() {
        let t = TypeSystemTest::new();
        is_subtype2(
            &t,
            "void Function(int, {int a})",
            "void Function(int, {int a})",
        );
    }

    #[test]
    fn function_type_61() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "void Function({int a})", "void Function({double a})");
    }

    #[test]
    fn function_type_62() {
        let t = TypeSystemTest::new();
        is_not_subtype2(
            &t,
            "void Function({int a})",
            "void Function({int a, int b})",
        );
    }

    #[test]
    fn function_type_63() {
        let t = TypeSystemTest::new();
        is_subtype2(
            &t,
            "void Function({int a, int b})",
            "void Function({int a})",
        );
    }

    #[test]
    fn function_type_64() {
        let t = TypeSystemTest::new();
        is_subtype2(
            &t,
            "void Function({int a, int b, int c})",
            "void Function({int a, int c})",
        );
    }

    #[test]
    fn function_type_66() {
        let t = TypeSystemTest::new();
        is_subtype2(
            &t,
            "void Function({int a, int b, int c})",
            "void Function({int b, int c})",
        );
    }

    #[test]
    fn function_type_68() {
        let t = TypeSystemTest::new();
        is_subtype2(
            &t,
            "void Function({int a, int b, int c})",
            "void Function({int c})",
        );
    }

    #[test]
    fn function_type_70() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "num Function(int)", "Object");
    }

    #[test]
    fn function_type_71() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "num Function(int)", "Object");
    }

    #[test]
    fn function_type_72() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "num Function(int)?", "Object");
    }

    #[test]
    fn function_type_73() {
        let t = TypeSystemTest::new();
        is_subtype2(
            &t,
            "void Function<E0 extends Object>()",
            "void Function<E1 extends FutureOr<Object>>()",
        );
    }

    #[test]
    fn function_type_74() {
        let t = TypeSystemTest::new();
        // Note, the order `R extends T`, then `T` is important.
        // We test that all type parameters replaced at once, not as we go.
        is_subtype2(
            &t,
            "void Function<R extends T, T>()",
            "void Function<R extends T, T>()",
        );
    }

    #[test]
    fn function_type_generic_nested() {
        let t = TypeSystemTest::new();
        is_subtype2(
            &t,
            "E0 Function(E0) Function<E0>(E0)",
            "F1 Function(F1) Function<F1>(F1)",
        );

        is_subtype2(
            &t,
            "E0 Function<E0>(E0, E0 Function(int, E0))",
            "E1 Function<E1>(E1, E1 Function(num, E1))",
        );

        is_not_subtype2(
            &t,
            "E0 Function(F0) Function<E0, F0>(E0)",
            "E1 Function<F1>(F1) Function<E1>(E1)",
        );

        is_not_subtype2(
            &t,
            "E0 Function(F0) Function<E0, F0>(E0)",
            "E1 Function(F1) Function<F1, E1>(E1)",
        );
    }

    #[test]
    fn function_type_generic_required() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "int Function<E>(E)", "num Function<E>(E)");

        is_subtype2(&t, "E Function<E>(num)", "E Function<E>(int)");

        is_subtype2(&t, "E Function<E>(E, num)", "E Function<E>(E, int)");

        is_not_subtype2(&t, "E Function<E>(E, num)", "E Function<E>(E, E)");
    }

    #[test]
    fn function_type_not_generic_function_return_type() {
        let t = TypeSystemTest::new();
        is_subtype2(
            &t,
            "num Function(num) Function(num)",
            "num Function(int) Function(num)",
        );

        is_not_subtype2(
            &t,
            "int Function(int) Function(int)",
            "num Function(num) Function(num)",
        );
    }

    #[test]
    fn function_type_not_generic_named() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "num Function({num x})", "num Function({int x})");

        is_subtype2(
            &t,
            "num Function(num, {num x})",
            "num Function(int, {int x})",
        );

        is_subtype2(&t, "int Function({num x})", "num Function({num x})");

        is_not_subtype2(&t, "int Function({int x})", "num Function({num x})");
    }

    #[test]
    fn function_type_not_generic_required() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "num Function(num)", "num Function(int)");

        is_subtype2(&t, "int Function(num)", "num Function(num)");

        is_subtype2(&t, "int Function(num)", "num Function(int)");

        is_not_subtype2(&t, "int Function(int)", "num Function(num)");

        is_subtype2(&t, "Null", "num Function(int)?");
    }

    #[test]
    fn function_type_required_named_parameter_01() {
        let t = TypeSystemTest::new();
        is_subtype2(
            &t,
            "void Function({int a})",
            "void Function({required int a})",
        );

        is_not_subtype2(
            &t,
            "void Function({required int a})",
            "void Function({int a})",
        );
    }

    #[test]
    fn function_type_required_named_parameter_02() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "void Function({required int a})", "void Function()");

        is_not_subtype2(
            &t,
            "void Function({required int a, int b})",
            "void Function({int b})",
        );
    }

    #[test]
    fn function_type_required_named_parameter_03() {
        let t = TypeSystemTest::new();
        is_subtype2(
            &t,
            "void Function({int? a})",
            "void Function({required int a})",
        );

        is_not_subtype2(
            &t,
            "void Function({required int a})",
            "void Function({int? a})",
        );
    }

    #[test]
    fn future_or_01() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "int", "FutureOr<int>");
    }

    #[test]
    fn future_or_02() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "int", "FutureOr<num>");
    }

    #[test]
    fn future_or_03() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "Future<int>", "FutureOr<int>");
    }

    #[test]
    fn future_or_04() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "Future<int>", "FutureOr<num>");
    }

    #[test]
    fn future_or_05() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "Future<int>", "FutureOr<Object>");
    }

    #[test]
    fn future_or_06() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "FutureOr<int>", "FutureOr<int>");
    }

    #[test]
    fn future_or_07() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "FutureOr<int>", "FutureOr<num>");
    }

    #[test]
    fn future_or_08() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "FutureOr<int>", "Object");
    }

    #[test]
    fn future_or_09() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "int", "FutureOr<double>");
    }

    #[test]
    fn future_or_10() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "FutureOr<double>", "int");
    }

    #[test]
    fn future_or_11() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "FutureOr<int>", "Future<num>");
    }

    #[test]
    fn future_or_12() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "FutureOr<int>", "num");
    }

    #[test]
    fn future_or_13() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "Null", "FutureOr<int>");
    }

    #[test]
    fn future_or_14() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("Null"), t.parse_type("Future<int>?"));
    }

    #[test]
    fn future_or_15() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "dynamic", "FutureOr<dynamic>");
    }

    #[test]
    fn future_or_16() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "dynamic", "FutureOr<String>");
    }

    #[test]
    fn future_or_17() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "void", "FutureOr<void>");
    }

    #[test]
    fn future_or_18() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "void", "FutureOr<String>");
    }

    #[test]
    fn future_or_19() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("E", |scope| {
            is_subtype(&t, scope.parse_type("E"), scope.parse_type("FutureOr<E>"));
        });
    }

    #[test]
    fn future_or_20() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("E", |scope| {
            is_not_subtype(&t, scope.parse_type("E"), t.parse_type("FutureOr<String>"));
        });
    }

    #[test]
    fn future_or_21() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "String Function()", "FutureOr<void Function()>");
    }

    #[test]
    fn future_or_22() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "void Function()", "FutureOr<String Function()>");
    }

    #[test]
    fn future_or_23() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "FutureOr<num>", "FutureOr<int>");
    }

    #[test]
    fn future_or_24() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_subtype(
                &t,
                scope.parse_type("T & int"),
                t.parse_type("FutureOr<num>"),
            );
        });
    }

    #[test]
    fn future_or_25() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_subtype(
                &t,
                scope.parse_type("T & Future<num>"),
                t.parse_type("FutureOr<num>"),
            );
        });
    }

    #[test]
    fn future_or_26() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_subtype(
                &t,
                scope.parse_type("T & Future<int>"),
                t.parse_type("FutureOr<num>"),
            );
        });
    }

    #[test]
    fn future_or_27() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_not_subtype(
                &t,
                scope.parse_type("T & num"),
                t.parse_type("FutureOr<int>"),
            );
        });
    }

    #[test]
    fn future_or_28() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_not_subtype(
                &t,
                scope.parse_type("T & Future<num>"),
                t.parse_type("FutureOr<int>"),
            );
        });
    }

    #[test]
    fn future_or_29() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_not_subtype(
                &t,
                scope.parse_type("T & FutureOr<num>"),
                t.parse_type("FutureOr<int>"),
            );
        });
    }

    #[test]
    fn future_or_30() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "FutureOr<Object>", "FutureOr<FutureOr<Object>>");
    }

    #[test]
    fn interface_type_01() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("int"), t.parse_type("int"));
    }

    #[test]
    fn interface_type_02() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("int"), t.parse_type("num"));
    }

    #[test]
    fn interface_type_03() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("int"), t.parse_type("Comparable<num>"));
    }

    #[test]
    fn interface_type_04() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("int"), t.parse_type("Object"));
    }

    #[test]
    fn interface_type_05() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("double"), t.parse_type("num"));
    }

    #[test]
    fn interface_type_06() {
        let t = TypeSystemTest::new();
        is_not_subtype(&t, t.parse_type("int"), t.parse_type("double"));
    }

    #[test]
    fn interface_type_07() {
        let t = TypeSystemTest::new();
        is_not_subtype(&t, t.parse_type("int"), t.parse_type("Comparable<int>"));
    }

    #[test]
    fn interface_type_08() {
        let t = TypeSystemTest::new();
        is_not_subtype(&t, t.parse_type("int"), t.parse_type("Iterable<int>"));
    }

    #[test]
    fn interface_type_09() {
        let t = TypeSystemTest::new();
        is_not_subtype(
            &t,
            t.parse_type("Comparable<int>"),
            t.parse_type("Iterable<int>"),
        );
    }

    #[test]
    fn interface_type_10() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("List<int>"), t.parse_type("List<int>"));
    }

    #[test]
    fn interface_type_11() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("List<int>"), t.parse_type("Iterable<int>"));
    }

    #[test]
    fn interface_type_12() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("List<int>"), t.parse_type("List<num>"));
    }

    #[test]
    fn interface_type_13() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("List<int>"), t.parse_type("Iterable<num>"));
    }

    #[test]
    fn interface_type_14() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("List<int>"), t.parse_type("List<Object>"));
    }

    #[test]
    fn interface_type_15() {
        let t = TypeSystemTest::new();
        is_subtype(
            &t,
            t.parse_type("List<int>"),
            t.parse_type("Iterable<Object>"),
        );
    }

    #[test]
    fn interface_type_16() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("List<int>"), t.parse_type("Object"));
    }

    #[test]
    fn interface_type_17() {
        let t = TypeSystemTest::new();
        is_subtype(
            &t,
            t.parse_type("List<int>"),
            t.parse_type("List<Comparable<Object>>"),
        );
    }

    #[test]
    fn interface_type_18() {
        let t = TypeSystemTest::new();
        is_subtype(
            &t,
            t.parse_type("List<int>"),
            t.parse_type("List<Comparable<num>>"),
        );
    }

    #[test]
    fn interface_type_19() {
        let t = TypeSystemTest::new();
        is_subtype(
            &t,
            t.parse_type("List<int>"),
            t.parse_type("List<Comparable<Comparable<num>>>"),
        );
    }

    #[test]
    fn interface_type_20() {
        let t = TypeSystemTest::new();
        is_not_subtype(&t, t.parse_type("List<int>"), t.parse_type("List<double>"));
    }

    #[test]
    fn interface_type_21() {
        let t = TypeSystemTest::new();
        is_not_subtype(
            &t,
            t.parse_type("List<int>"),
            t.parse_type("Iterable<double>"),
        );
    }

    #[test]
    fn interface_type_22() {
        let t = TypeSystemTest::new();
        is_not_subtype(
            &t,
            t.parse_type("List<int>"),
            t.parse_type("Comparable<int>"),
        );
    }

    #[test]
    fn interface_type_23() {
        let t = TypeSystemTest::new();
        is_not_subtype(
            &t,
            t.parse_type("List<int>"),
            t.parse_type("List<Comparable<int>>"),
        );
    }

    #[test]
    fn interface_type_24() {
        let t = TypeSystemTest::new();
        is_not_subtype(
            &t,
            t.parse_type("List<int>"),
            t.parse_type("List<Comparable<Comparable<int>>>"),
        );
    }

    #[test]
    fn interface_type_25_interfaces() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: vec![
                ClassSpec::new("class I"),
                ClassSpec::new("class A implements I"),
            ],
            ..LibrarySpec::test()
        });

        is_subtype(&t, t.parse_type("A"), t.parse_type("I"));
        is_not_subtype(&t, t.parse_type("I"), t.parse_type("A"));
    }

    #[test]
    fn interface_type_26_mixins() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: vec![ClassSpec::new("class M"), ClassSpec::new("class A with M")],
            ..LibrarySpec::test()
        });

        is_subtype(&t, t.parse_type("A"), t.parse_type("M"));
        is_not_subtype(&t, t.parse_type("M"), t.parse_type("A"));
    }

    #[test]
    fn interface_type_27() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("num"), t.parse_type("Object"));
    }

    #[test]
    fn interface_type_28() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("num"), t.parse_type("Object"));
    }

    #[test]
    fn interface_type_39() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object?", |scope| {
            is_subtype(
                &t,
                scope.parse_type("List<T & int>"),
                scope.parse_type("List<T>"),
            );
        });
    }

    #[test]
    fn interface_type_40() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object?", |scope| {
            is_subtype(
                &t,
                scope.parse_type("List<T & int?>"),
                scope.parse_type("List<T>"),
            );
        });
    }

    #[test]
    fn interface_type_contravariant() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: vec![ClassSpec::new("class A<in T>")],
            ..LibrarySpec::test()
        });

        is_subtype(&t, t.parse_type("A<num>"), t.parse_type("A<int>"));
        is_subtype(&t, t.parse_type("A<num>"), t.parse_type("A<num>"));
        is_not_subtype(&t, t.parse_type("A<int>"), t.parse_type("A<num>"));
    }

    #[test]
    fn interface_type_covariant() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: vec![ClassSpec::new("class A<out T>")],
            ..LibrarySpec::test()
        });

        is_subtype(&t, t.parse_type("A<int>"), t.parse_type("A<num>"));
        is_subtype(&t, t.parse_type("A<num>"), t.parse_type("A<num>"));
        is_not_subtype(&t, t.parse_type("A<num>"), t.parse_type("A<int>"));
    }

    #[test]
    fn interface_type_invariant() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: vec![ClassSpec::new("class A<inout T>")],
            ..LibrarySpec::test()
        });

        is_subtype(&t, t.parse_type("A<num>"), t.parse_type("A<num>"));
        is_not_subtype(&t, t.parse_type("A<int>"), t.parse_type("A<num>"));
        is_not_subtype(&t, t.parse_type("A<num>"), t.parse_type("A<int>"));
    }

    #[test]
    fn invalid_type() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "InvalidType", "int");
        is_subtype2(&t, "int", "InvalidType");
    }

    #[test]
    fn multi_function_non_generic_one_argument() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "num Function(num)", "num Function(int)");
        is_subtype2(&t, "int Function(num)", "num Function(num)");
        is_subtype2(&t, "int Function(num)", "num Function(int)");

        is_not_subtype2(&t, "int Function(int)", "num Function(num)");

        is_not_subtype2(&t, "Null", "num Function(int)");
        is_subtype2(&t, "Null", "num Function(int)?");

        is_subtype2(&t, "Never", "num Function(int)");
        is_subtype2(&t, "Never", "num Function(int)?");
        is_not_subtype2(&t, "num Function(int)", "Never");

        is_subtype2(&t, "num Function(num)", "Object");
        is_not_subtype2(&t, "num Function(num)?", "Object");

        is_not_subtype2(&t, "num", "num Function(num)");
        is_not_subtype2(&t, "Object", "num Function(num)");
        is_not_subtype2(&t, "Object?", "num Function(num)");
        is_not_subtype2(&t, "dynamic", "num Function(num)");

        is_subtype2(&t, "num Function(num)", "num Function(num)?");
        is_not_subtype2(&t, "num Function(num)?", "num Function(num)");

        is_subtype2(&t, "num Function(num)", "num? Function(num)");
        is_subtype2(&t, "num Function(num?)", "num Function(num)");
        is_subtype2(&t, "num Function(num?)", "num? Function(num)");
        is_not_subtype2(&t, "num Function(num)", "num? Function(num?)");

        is_subtype2(&t, "num Function({num x})", "num? Function({num x})");
        is_subtype2(&t, "num Function({num? x})", "num Function({num x})");
        is_subtype2(&t, "num Function({num? x})", "num? Function({num x})");
        is_not_subtype2(&t, "num Function({num x})", "num? Function({num? x})");

        is_subtype2(&t, "num Function([num])", "num? Function([num])");
        is_subtype2(&t, "num Function([num?])", "num Function([num])");
        is_subtype2(&t, "num Function([num?])", "num? Function([num])");
        is_not_subtype2(&t, "num Function([num])", "num? Function([num?])");
    }

    #[test]
    fn multi_function_non_generic_zero_arguments() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "int Function()", "Function");
        is_subtype2(&t, "int Function()", "Function?");

        is_not_subtype2(&t, "int Function()?", "Function");
        is_subtype2(&t, "int Function()?", "Function?");

        is_subtype2(&t, "int Function()", "Object");
        is_subtype2(&t, "int Function()", "Object?");

        is_not_subtype2(&t, "int Function()?", "Object");
        is_subtype2(&t, "int Function()?", "Object?");
    }

    #[test]
    fn multi_future_or() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "int", "FutureOr<int>");
        is_subtype2(&t, "int", "FutureOr<num>");
        is_subtype2(&t, "Future<int>", "FutureOr<int>");
        is_subtype2(&t, "Future<int>", "FutureOr<num>");
        is_subtype2(&t, "Future<int>", "FutureOr<Object>");
        is_subtype2(&t, "FutureOr<int>", "FutureOr<int>");
        is_subtype2(&t, "FutureOr<int>", "FutureOr<num>");
        is_subtype2(&t, "FutureOr<int>", "Object");
        is_subtype2(&t, "Null", "FutureOr<num?>");
        is_subtype2(&t, "Null", "FutureOr<num>?");
        is_subtype2(&t, "num?", "FutureOr<num?>");
        is_subtype2(&t, "num?", "FutureOr<num>?");
        is_subtype2(&t, "Future<num>", "FutureOr<num?>");
        is_subtype2(&t, "Future<num>", "FutureOr<num>?");
        is_subtype2(&t, "Future<num>", "FutureOr<num?>?");
        is_subtype2(&t, "Future<num?>", "FutureOr<num?>");
        is_not_subtype2(&t, "Future<num?>", "FutureOr<num>?");
        is_subtype2(&t, "Future<num?>", "FutureOr<num?>?");

        is_subtype2(&t, "num?", "FutureOr<FutureOr<FutureOr<num>>?>");
        is_subtype2(&t, "Future<num>?", "FutureOr<FutureOr<FutureOr<num>>?>");
        is_subtype2(
            &t,
            "Future<Future<num>>?",
            "FutureOr<FutureOr<FutureOr<num>>?>",
        );
        is_subtype2(
            &t,
            "Future<Future<Future<num>>>?",
            "FutureOr<FutureOr<FutureOr<num>>?>",
        );
        is_subtype2(&t, "Future<num>?", "FutureOr<FutureOr<FutureOr<num?>>>");
        is_subtype2(
            &t,
            "Future<Future<num>>?",
            "FutureOr<FutureOr<FutureOr<num?>>>",
        );
        is_subtype2(
            &t,
            "Future<Future<Future<num>>>?",
            "FutureOr<FutureOr<FutureOr<num?>>>",
        );
        is_subtype2(&t, "Future<num?>?", "FutureOr<FutureOr<FutureOr<num?>>>");
        is_subtype2(
            &t,
            "Future<Future<num?>?>?",
            "FutureOr<FutureOr<FutureOr<num?>>>",
        );
        is_subtype2(
            &t,
            "Future<Future<Future<num?>?>?>?",
            "FutureOr<FutureOr<FutureOr<num?>>>",
        );

        is_subtype2(&t, "FutureOr<num>?", "FutureOr<num?>");
        is_not_subtype2(&t, "FutureOr<num?>", "FutureOr<num>?");

        is_subtype2(&t, "dynamic", "FutureOr<Object?>");
        is_subtype2(&t, "dynamic", "FutureOr<Object>?");
        is_subtype2(&t, "void", "FutureOr<Object?>");
        is_subtype2(&t, "void", "FutureOr<Object>?");
        is_subtype2(&t, "Object?", "FutureOr<Object?>");
        is_subtype2(&t, "Object?", "FutureOr<Object>?");
        is_subtype2(&t, "Object", "FutureOr<Object?>");
        is_subtype2(&t, "Object", "FutureOr<Object>?");
        is_not_subtype2(&t, "dynamic", "FutureOr<Object>");
        is_not_subtype2(&t, "void", "FutureOr<Object>");
        is_not_subtype2(&t, "Object?", "FutureOr<Object>");
        is_subtype2(&t, "Object", "FutureOr<Object>");

        is_subtype2(&t, "FutureOr<int>", "Object");
        is_subtype2(&t, "FutureOr<int>", "Object?");

        is_subtype2(&t, "FutureOr<int>", "Object");
        is_subtype2(&t, "FutureOr<int>", "Object?");

        is_not_subtype2(&t, "FutureOr<int>?", "Object");
        is_subtype2(&t, "FutureOr<int>?", "Object?");

        is_subtype2(&t, "FutureOr<int>", "Object");
        is_subtype2(&t, "FutureOr<int>", "Object?");

        is_not_subtype2(&t, "FutureOr<int?>", "Object");
        is_subtype2(&t, "FutureOr<int?>", "Object?");

        is_subtype2(&t, "FutureOr<Future<Object>>", "Future<Object>");
        is_not_subtype2(&t, "FutureOr<Future<Object>>?", "Future<Object>");
        is_not_subtype2(&t, "FutureOr<Future<Object>?>", "Future<Object>");
        is_not_subtype2(&t, "FutureOr<Future<Object>?>?", "Future<Object>");

        is_subtype2(&t, "FutureOr<num>", "Object");
        is_not_subtype2(&t, "FutureOr<num>?", "Object");
    }

    #[test]
    fn multi_future_or_function_type() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "String Function()", "FutureOr<void Function()>");

        is_subtype2(&t, "String Function()", "FutureOr<void Function()>");

        is_subtype2(&t, "String Function()", "FutureOr<void Function()?>");

        is_subtype2(&t, "String Function()", "FutureOr<void Function()>?");

        is_subtype2(&t, "String Function()?", "FutureOr<void Function()?>");

        is_subtype2(&t, "String Function()?", "FutureOr<void Function()>?");

        is_not_subtype2(&t, "String Function()?", "FutureOr<void Function()>");

        is_not_subtype2(&t, "void Function()", "FutureOr<String Function()>");
    }

    #[test]
    fn multi_future_or_type_parameter() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("E extends Object", |scope| {
            is_subtype(&t, scope.parse_type("E"), scope.parse_type("FutureOr<E>"));
        });

        t.with_type_parameter_scope("E extends Object", |scope| {
            is_subtype(&t, scope.parse_type("E?"), scope.parse_type("FutureOr<E>?"));
            is_subtype(&t, scope.parse_type("E?"), scope.parse_type("FutureOr<E?>"));
            is_not_subtype(&t, scope.parse_type("E?"), scope.parse_type("FutureOr<E>"));
        });

        t.with_type_parameter_scope("E extends Object?", |scope| {
            is_subtype(&t, scope.parse_type("E"), scope.parse_type("FutureOr<E>?"));
            is_subtype(&t, scope.parse_type("E"), scope.parse_type("FutureOr<E?>"));
            is_subtype(&t, scope.parse_type("E"), scope.parse_type("FutureOr<E>"));
        });

        t.with_type_parameter_scope("E extends Object", |scope| {
            is_not_subtype(&t, scope.parse_type("E"), t.parse_type("FutureOr<String>"));
        });

        t.with_type_parameter_scope("E extends String", |scope| {
            is_subtype(
                &t,
                scope.parse_type("E?"),
                t.parse_type("FutureOr<String>?"),
            );
            is_subtype(
                &t,
                scope.parse_type("E?"),
                t.parse_type("FutureOr<String?>"),
            );
            is_not_subtype(&t, scope.parse_type("E?"), t.parse_type("FutureOr<String>"));
        });

        t.with_type_parameter_scope("E extends String?", |scope| {
            is_subtype(&t, scope.parse_type("E"), t.parse_type("FutureOr<String>?"));
            is_subtype(&t, scope.parse_type("E"), t.parse_type("FutureOr<String?>"));
            is_not_subtype(&t, scope.parse_type("E"), t.parse_type("FutureOr<String>"));
        });
    }

    #[test]
    fn multi_future_or_type_parameter_promotion() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object", |scope| {
            is_subtype(
                &t,
                scope.parse_type("T & int"),
                t.parse_type("FutureOr<num>"),
            );
            is_subtype(
                &t,
                scope.parse_type("T & int"),
                t.parse_type("FutureOr<num?>"),
            );
            is_subtype(
                &t,
                scope.parse_type("T & int"),
                t.parse_type("FutureOr<num>?"),
            );
        });

        t.with_type_parameter_scope("T extends Object?", |scope| {
            is_subtype(
                &t,
                scope.parse_type("T & int"),
                t.parse_type("FutureOr<num>"),
            );
            is_subtype(
                &t,
                scope.parse_type("T & int"),
                t.parse_type("FutureOr<num?>"),
            );
            is_subtype(
                &t,
                scope.parse_type("T & int"),
                t.parse_type("FutureOr<num>?"),
            );
        });

        t.with_type_parameter_scope("T extends Object?", |scope| {
            is_not_subtype(
                &t,
                scope.parse_type("T & int?"),
                t.parse_type("FutureOr<num>"),
            );
            is_subtype(
                &t,
                scope.parse_type("T & int?"),
                t.parse_type("FutureOr<num?>"),
            );
            is_subtype(
                &t,
                scope.parse_type("T & int?"),
                t.parse_type("FutureOr<num>?"),
            );
        });

        t.with_type_parameter_scope("T extends Object?, S extends T", |scope| {
            is_not_subtype(
                &t,
                scope.parse_type("T & S"),
                t.parse_type("FutureOr<Object>"),
            );
            is_subtype(
                &t,
                scope.parse_type("T & S"),
                t.parse_type("FutureOr<Object?>"),
            );
            is_subtype(
                &t,
                scope.parse_type("T & S"),
                t.parse_type("FutureOr<Object>?"),
            );
        });

        t.with_type_parameter_scope("T extends Object", |scope| {
            is_subtype(
                &t,
                scope.parse_type("T & Future<num>"),
                t.parse_type("FutureOr<num>"),
            );
            is_subtype(
                &t,
                scope.parse_type("T & Future<int>"),
                t.parse_type("FutureOr<num>"),
            );
        });

        t.with_type_parameter_scope("T extends Object", |scope| {
            is_subtype(
                &t,
                scope.parse_type("T & Future<int>"),
                t.parse_type("FutureOr<num>"),
            );
            is_subtype(
                &t,
                scope.parse_type("T & Future<int>"),
                t.parse_type("FutureOr<num?>"),
            );
            is_subtype(
                &t,
                scope.parse_type("T & Future<int>"),
                t.parse_type("FutureOr<num>?"),
            );
        });

        t.with_type_parameter_scope("T extends Object?", |scope| {
            is_subtype(
                &t,
                scope.parse_type("T & Future<int>"),
                t.parse_type("FutureOr<num>"),
            );
            is_subtype(
                &t,
                scope.parse_type("T & Future<int>"),
                t.parse_type("FutureOr<num?>"),
            );
            is_subtype(
                &t,
                scope.parse_type("T & Future<int>"),
                t.parse_type("FutureOr<num>?"),
            );

            is_not_subtype(
                &t,
                scope.parse_type("T & Future<int>?"),
                t.parse_type("FutureOr<num>"),
            );
            is_subtype(
                &t,
                scope.parse_type("T & Future<int>?"),
                t.parse_type("FutureOr<num?>"),
            );
            is_subtype(
                &t,
                scope.parse_type("T & Future<int>?"),
                t.parse_type("FutureOr<num>?"),
            );
        });

        t.with_type_parameter_scope("T extends Object", |scope| {
            is_not_subtype(
                &t,
                scope.parse_type("T & Future<int?>"),
                t.parse_type("FutureOr<num>"),
            );
            is_subtype(
                &t,
                scope.parse_type("T & Future<int?>"),
                t.parse_type("FutureOr<num?>"),
            );
            is_not_subtype(
                &t,
                scope.parse_type("T & Future<int?>"),
                t.parse_type("FutureOr<num>?"),
            );
        });
    }

    #[test]
    fn multi_list_sub_types_super_types() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "List<int>", "List<int>");
        is_subtype2(&t, "List<int>", "Iterable<int>");
        is_subtype2(&t, "List<int>", "List<num>");
        is_subtype2(&t, "List<int>", "Iterable<num>");
        is_subtype2(&t, "List<int>", "List<Object>");
        is_subtype2(&t, "List<int>", "Iterable<Object>");
        is_subtype2(&t, "List<int>", "Object");
        is_subtype2(&t, "List<int>", "List<Comparable<Object>>");
        is_subtype2(&t, "List<int>", "List<Comparable<num>>");
        is_subtype2(&t, "List<int>", "List<Comparable<Comparable<num>>>");
        is_subtype2(&t, "List<int>", "Object");
        is_not_subtype2(&t, "Null", "List<int>");
        is_subtype2(&t, "Null", "List<int>?");
        is_subtype2(&t, "Never", "List<int>");
        is_subtype2(&t, "Never", "List<int>?");

        is_subtype2(&t, "List<int>", "List<int>");
        is_subtype2(&t, "List<int>", "List<int>?");
        is_not_subtype2(&t, "List<int>?", "List<int>");
        is_subtype2(&t, "List<int>?", "List<int>?");

        is_subtype2(&t, "List<int>", "List<int?>");
        is_not_subtype2(&t, "List<int?>", "List<int>");
        is_subtype2(&t, "List<int?>", "List<int?>");
    }

    #[test]
    fn multi_never() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "Never", "FutureOr<num>");
        is_subtype2(&t, "Never", "FutureOr<num?>");
        is_subtype2(&t, "Never", "FutureOr<num>?");
        is_not_subtype2(&t, "FutureOr<num>", "Never");
    }

    #[test]
    fn multi_num_sub_types_super_types() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "int", "num");
        is_subtype2(&t, "int", "Comparable<num>");
        is_subtype2(&t, "int", "Comparable<Object>");
        is_subtype2(&t, "double", "num");
        is_subtype2(&t, "num", "Object");
        is_subtype2(&t, "Null", "num?");
        is_subtype2(&t, "Never", "num");
        is_subtype2(&t, "Never", "num?");

        is_not_subtype2(&t, "int", "double");
        is_not_subtype2(&t, "int", "Comparable<int>");
        is_not_subtype2(&t, "int", "Iterable<int>");
        is_not_subtype2(&t, "Comparable<int>", "Iterable<int>");
        is_not_subtype2(&t, "num?", "Object");
        is_not_subtype2(&t, "Null", "num");
        is_not_subtype2(&t, "num", "Never");
    }

    #[test]
    fn multi_object_top_and_bottom() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "Never", "Object");
        is_subtype2(&t, "Object", "dynamic");
        is_subtype2(&t, "Object", "void");
        is_subtype2(&t, "Object", "Object?");

        is_not_subtype2(&t, "Object", "Never");
        is_not_subtype2(&t, "Object", "Null");
        is_not_subtype2(&t, "dynamic", "Object");
        is_not_subtype2(&t, "void", "Object");
        is_not_subtype2(&t, "Object?", "Object");
    }

    #[test]
    fn multi_special() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "dynamic", "int");
        is_not_subtype2(&t, "dynamic", "int?");

        is_not_subtype2(&t, "void", "int");
        is_not_subtype2(&t, "void", "int?");

        is_not_subtype2(&t, "Object", "int");
        is_not_subtype2(&t, "Object", "int?");

        is_not_subtype2(&t, "Object?", "int");
        is_not_subtype2(&t, "Object?", "int?");

        is_not_subtype2(&t, "int Function()", "int");
    }

    #[test]
    fn multi_top_and_bottom() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "Null", "Null");
        is_subtype2(&t, "Never", "Null");
        is_subtype2(&t, "Never", "Never");
        is_not_subtype2(&t, "Null", "Never");

        is_subtype2(&t, "Null", "Never?");
        is_subtype2(&t, "Never?", "Null");
        is_subtype2(&t, "Never", "Never?");
        is_not_subtype2(&t, "Never?", "Never");

        is_subtype2(&t, "dynamic", "dynamic");
        is_subtype2(&t, "dynamic", "void");
        is_subtype2(&t, "dynamic", "Object?");
        is_subtype2(&t, "void", "dynamic");
        is_subtype2(&t, "void", "void");
        is_subtype2(&t, "void", "Object?");
        is_subtype2(&t, "Object?", "dynamic");
        is_subtype2(&t, "Object?", "void");
        is_subtype2(&t, "Object?", "Object?");

        is_subtype2(&t, "Never", "Object?");
        is_subtype2(&t, "Never", "dynamic");
        is_subtype2(&t, "Never", "void");
        is_subtype2(&t, "Null", "Object?");
        is_subtype2(&t, "Null", "dynamic");
        is_subtype2(&t, "Null", "void");

        is_not_subtype2(&t, "Object?", "Never");
        is_not_subtype2(&t, "Object?", "Null");
        is_not_subtype2(&t, "dynamic", "Never");
        is_not_subtype2(&t, "dynamic", "Null");
        is_not_subtype2(&t, "void", "Never");
        is_not_subtype2(&t, "void", "Null");
    }

    #[test]
    fn multi_type_parameter_promotion() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends int", |scope| {
            is_subtype(&t, scope.parse_type("T"), scope.parse_type("T & int"));
            is_not_subtype(&t, scope.parse_type("T?"), scope.parse_type("T & int"));
        });

        t.with_type_parameter_scope("T extends int?", |scope| {
            is_not_subtype(&t, scope.parse_type("T"), scope.parse_type("T & int"));
            is_subtype(&t, scope.parse_type("T"), scope.parse_type("T & int?"));
            is_not_subtype(&t, scope.parse_type("T?"), scope.parse_type("T & int?"));
        });

        t.with_type_parameter_scope("T extends num", |scope| {
            is_subtype(&t, scope.parse_type("T"), scope.parse_type("T"));
            is_subtype(&t, scope.parse_type("T?"), scope.parse_type("T?"));
        });

        t.with_type_parameter_scope("T extends num?", |scope| {
            is_subtype(&t, scope.parse_type("T"), scope.parse_type("T"));
            is_subtype(&t, scope.parse_type("T?"), scope.parse_type("T?"));
        });
    }

    #[test]
    fn never_01() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("Never"), t.parse_type("Never"));
    }

    #[test]
    fn never_02() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("Never"), t.parse_type("num"));
    }

    #[test]
    fn never_04() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("Never"), t.parse_type("num?"));
    }

    #[test]
    fn never_05() {
        let t = TypeSystemTest::new();
        is_not_subtype(&t, t.parse_type("num"), t.parse_type("Never"));
    }

    #[test]
    fn never_06() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("Never"), t.parse_type("List<int>"));
    }

    #[test]
    fn never_09() {
        let t = TypeSystemTest::new();
        is_not_subtype(&t, t.parse_type("num"), t.parse_type("Never"));
    }

    #[test]
    fn never_15() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object", |scope| {
            is_subtype(&t, t.parse_type("Never"), scope.parse_type("T & num"));
        });
    }

    #[test]
    fn never_16() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object", |scope| {
            is_not_subtype(&t, scope.parse_type("T & num"), t.parse_type("Never"));
        });
    }

    #[test]
    fn never_17() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Never", |scope| {
            is_subtype(&t, scope.parse_type("T"), t.parse_type("Never"));
        });
    }

    #[test]
    fn never_18() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object", |scope| {
            is_subtype(&t, scope.parse_type("T & Never"), t.parse_type("Never"));
        });
    }

    #[test]
    fn never_19() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object", |scope| {
            is_subtype(&t, t.parse_type("Never"), scope.parse_type("T?"));
        });
    }

    #[test]
    fn never_20() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object?", |scope| {
            is_subtype(&t, t.parse_type("Never"), scope.parse_type("T?"));
        });
    }

    #[test]
    fn never_21() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object", |scope| {
            is_subtype(&t, t.parse_type("Never"), scope.parse_type("T"));
        });
    }

    #[test]
    fn never_22() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object?", |scope| {
            is_subtype(&t, t.parse_type("Never"), scope.parse_type("T"));
        });
    }

    #[test]
    fn never_23() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Never", |scope| {
            is_subtype(&t, scope.parse_type("T"), t.parse_type("Never"));
        });
    }

    #[test]
    fn never_24() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Never?", |scope| {
            is_not_subtype(&t, scope.parse_type("T"), t.parse_type("Never"));
        });
    }

    #[test]
    fn never_25() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Never", |scope| {
            is_not_subtype(&t, scope.parse_type("T?"), t.parse_type("Never"));
        });
    }

    #[test]
    fn never_26() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Never?", |scope| {
            is_not_subtype(&t, scope.parse_type("T?"), t.parse_type("Never"));
        });
    }

    #[test]
    fn never_27() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object", |scope| {
            is_not_subtype(&t, scope.parse_type("T"), t.parse_type("Never"));
        });
    }

    #[test]
    fn never_28() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object?", |scope| {
            is_not_subtype(&t, scope.parse_type("T"), t.parse_type("Never"));
        });
    }

    #[test]
    fn never_29() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("Never"), t.parse_type("Null"));
    }

    #[test]
    fn null_01() {
        let t = TypeSystemTest::new();
        is_not_subtype(&t, t.parse_type("Null"), t.parse_type("Never"));
    }

    #[test]
    fn null_02() {
        let t = TypeSystemTest::new();
        is_not_subtype(&t, t.parse_type("Null"), t.parse_type("Object"));
    }

    #[test]
    fn null_03() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("Null"), t.parse_type("void"));
    }

    #[test]
    fn null_04() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("Null"), t.parse_type("dynamic"));
    }

    #[test]
    fn null_05() {
        let t = TypeSystemTest::new();
        is_not_subtype(&t, t.parse_type("Null"), t.parse_type("double"));
    }

    #[test]
    fn null_06() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("Null"), t.parse_type("double?"));
    }

    #[test]
    fn null_07() {
        let t = TypeSystemTest::new();
        is_not_subtype(&t, t.parse_type("Null"), t.parse_type("Comparable<Object>"));
    }

    #[test]
    fn null_08() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object", |scope| {
            is_not_subtype(&t, t.parse_type("Null"), scope.parse_type("T"));
        });
    }

    #[test]
    fn null_09() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("Null"), t.parse_type("Null"));
    }

    #[test]
    fn null_10() {
        let t = TypeSystemTest::new();
        is_not_subtype(&t, t.parse_type("Null"), t.parse_type("List<int>"));
    }

    #[test]
    fn null_13() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "Null", "num Function(int)");
    }

    #[test]
    fn null_14() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "Null", "num Function(int)");
    }

    #[test]
    fn null_15() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "Null", "num Function(int)?");
    }

    #[test]
    fn null_16() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object", |scope| {
            is_subtype(&t, t.parse_type("Null"), scope.parse_type("(T & num)?"));
        });
    }

    #[test]
    fn null_17() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object?", |scope| {
            is_not_subtype(&t, t.parse_type("Null"), scope.parse_type("T & num"));
        });
    }

    #[test]
    fn null_18() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object?", |scope| {
            is_not_subtype(&t, t.parse_type("Null"), scope.parse_type("T & num?"));
        });
    }

    #[test]
    fn null_19() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object", |scope| {
            is_not_subtype(&t, t.parse_type("Null"), scope.parse_type("T & num"));
        });
    }

    #[test]
    fn null_20() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object?, S extends T", |scope| {
            is_not_subtype(&t, t.parse_type("Null"), scope.parse_type("T & S"));
        });
    }

    #[test]
    fn null_21() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object", |scope| {
            is_subtype(&t, t.parse_type("Null"), scope.parse_type("T?"));
        });
    }

    #[test]
    fn null_22() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object?", |scope| {
            is_subtype(&t, t.parse_type("Null"), scope.parse_type("T?"));
        });
    }

    #[test]
    fn null_23() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object", |scope| {
            is_not_subtype(&t, t.parse_type("Null"), scope.parse_type("T"));
        });
    }

    #[test]
    fn null_24() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object?", |scope| {
            is_not_subtype(&t, t.parse_type("Null"), scope.parse_type("T"));
        });
    }

    #[test]
    fn null_25() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Null", |scope| {
            is_subtype(&t, scope.parse_type("T"), t.parse_type("Null"));
        });
    }

    #[test]
    fn null_26() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Null", |scope| {
            is_subtype(&t, scope.parse_type("T?"), t.parse_type("Null"));
        });
    }

    #[test]
    fn null_27() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object", |scope| {
            is_not_subtype(&t, scope.parse_type("T"), t.parse_type("Null"));
        });
    }

    #[test]
    fn null_28() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object?", |scope| {
            is_not_subtype(&t, scope.parse_type("T"), t.parse_type("Null"));
        });
    }

    #[test]
    fn null_29() {
        let t = TypeSystemTest::new();
        is_subtype(
            &t,
            t.parse_type("Null"),
            t.parse_type("Comparable<Object>?"),
        );
    }

    #[test]
    fn null_30() {
        let t = TypeSystemTest::new();
        is_not_subtype(&t, t.parse_type("Null"), t.parse_type("Object"));
    }

    #[test]
    fn nullability_suffix_01() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("int"), t.parse_type("int"));
        is_subtype(&t, t.parse_type("int"), t.parse_type("int?"));

        is_not_subtype(&t, t.parse_type("int?"), t.parse_type("int"));
        is_subtype(&t, t.parse_type("int?"), t.parse_type("int?"));

        is_subtype(&t, t.parse_type("int"), t.parse_type("int"));
        is_subtype(&t, t.parse_type("int"), t.parse_type("int?"));
    }

    #[test]
    fn nullability_suffix_05() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "void Function(int)", "Object");
    }

    #[test]
    fn nullability_suffix_11() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("int?"), t.parse_type("int?"));
    }

    #[test]
    fn nullability_suffix_12() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("int"), t.parse_type("int"));
    }

    #[test]
    fn nullability_suffix_13() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "int Function(int)?", "int Function(int)?");
    }

    #[test]
    fn nullability_suffix_14() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "int Function(int)", "int Function(int)");
    }

    #[test]
    fn nullability_suffix_15() {
        let t = TypeSystemTest::new();
        is_subtype2(
            &t,
            "int? Function(int, int, int?)",
            "int? Function(int, int, int?)",
        );
    }

    #[test]
    fn nullability_suffix_16() {
        let t = TypeSystemTest::new();
        let ty = t.parse_type("List<int>?");
        is_subtype(&t, ty, ty);
    }

    #[test]
    fn nullability_suffix_17() {
        let t = TypeSystemTest::new();
        let ty = t.parse_type("List<int?>?");
        is_subtype(&t, ty, ty);
    }

    #[test]
    fn nullability_suffix_18() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object", |scope| {
            let ty = scope.parse_type("T & int?");
            is_subtype(&t, ty, ty);
        });
    }

    #[test]
    fn nullability_suffix_19() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object", |scope| {
            let ty = scope.parse_type("(T & int?)?");
            is_subtype(&t, ty, ty);
        });
    }

    #[test]
    fn record_function_type() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "({int f1})", "void Function()");
    }

    #[test]
    fn record_interface_type() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "({int f1})", "int");
        is_not_subtype2(&t, "int", "({int f1})");
    }

    #[test]
    fn record_never() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "({int f1})", "Never");
        is_subtype2(&t, "Never", "({int f1})");
    }

    #[test]
    fn record_record2_different_shape() {
        let t = TypeSystemTest::new();
        let check = |t1: &str, t2: &str| {
            is_not_subtype2(&t, t1, t2);
            is_not_subtype2(&t, t2, t1);
        };

        check("(int,)", "(int, String)");
        check("(int,)", "({int $1})");

        check("({int f1, String f2})", "({int f1})");
        check("({int f1})", "({int f2})");
    }

    #[test]
    fn record_record2_same_shape_mixed() {
        let t = TypeSystemTest::new();
        let check = |sub_type: &str, super_type: &str| {
            is_subtype2(&t, sub_type, super_type);
            is_not_subtype2(&t, super_type, sub_type);
        };

        check("(int, {String f2})", "(int, {Object f2})");
    }

    #[test]
    fn record_record2_same_shape_named() {
        let t = TypeSystemTest::new();
        let check = |sub_type: &str, super_type: &str| {
            is_subtype2(&t, sub_type, super_type);
            is_not_subtype2(&t, super_type, sub_type);
        };

        check("({int f1})", "({num f1})");

        is_subtype2(&t, "({int f1, String f2})", "({int f1, String f2})");
        check("({int f1, String f2})", "({int f1, Object f2})");
        check("({int f1, String f2})", "({num f1, String f2})");
        check("({int f1, String f2})", "({num f1, Object f2})");
    }

    #[test]
    fn record_record2_same_shape_named_order() {
        let t = TypeSystemTest::new();
        let check = |sub_type: TypeId, super_type: TypeId| {
            is_subtype(&t, sub_type, super_type);
            is_subtype(&t, super_type, sub_type);
        };

        check(
            t.parse_record_type("({int f1, int f2, int f3, int f4})"),
            t.parse_record_type("({int f4, int f3, int f2, int f1})"),
        );
    }

    #[test]
    fn record_record2_same_shape_positional() {
        let t = TypeSystemTest::new();
        let check = |sub_type: &str, super_type: &str| {
            is_subtype2(&t, sub_type, super_type);
            is_not_subtype2(&t, super_type, sub_type);
        };

        check("(int,)", "(num,)");

        is_subtype2(&t, "(int, String)", "(int, String)");
        check("(int, String)", "(num, String)");
        check("(int, String)", "(num, Object)");
        check("(int, String)", "(int, Object)");
    }

    #[test]
    fn record_top() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "({int f1})", "dynamic");
        is_subtype2(&t, "({int f1})", "Object");
        is_subtype2(&t, "({int f1})", "Record");
    }

    /// The class `Record` is a subtype of `Object` and `dynamic`, and a
    /// supertype of `Never`.
    #[test]
    fn record_class() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("Record"), t.parse_type("Object"));

        is_subtype(&t, t.parse_type("Record"), t.parse_type("dynamic"));

        is_subtype(&t, t.parse_type("Never"), t.parse_type("Record"));
    }

    #[test]
    fn special_01() {
        let t = TypeSystemTest::new();
        is_not_subtype(&t, t.parse_type("dynamic"), t.parse_type("int"));
    }

    #[test]
    fn special_02() {
        let t = TypeSystemTest::new();
        is_not_subtype(&t, t.parse_type("void"), t.parse_type("int"));
    }

    #[test]
    fn special_03() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "int Function()", "int");
    }

    #[test]
    fn special_04() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "int", "int Function()");
    }

    #[test]
    fn special_06() {
        let t = TypeSystemTest::new();
        is_subtype2(&t, "int Function()", "Object");
    }

    #[test]
    fn special_07() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("Object"), t.parse_type("Object"));
    }

    #[test]
    fn special_08() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("Object"), t.parse_type("dynamic"));
    }

    #[test]
    fn special_09() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("Object"), t.parse_type("void"));
    }

    #[test]
    fn special_10() {
        let t = TypeSystemTest::new();
        is_not_subtype(&t, t.parse_type("dynamic"), t.parse_type("Object"));
    }

    #[test]
    fn special_11() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("dynamic"), t.parse_type("dynamic"));
    }

    #[test]
    fn special_12() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("dynamic"), t.parse_type("void"));
    }

    #[test]
    fn special_13() {
        let t = TypeSystemTest::new();
        is_not_subtype(&t, t.parse_type("void"), t.parse_type("Object"));
    }

    #[test]
    fn special_14() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("void"), t.parse_type("dynamic"));
    }

    #[test]
    fn special_15() {
        let t = TypeSystemTest::new();
        is_subtype(&t, t.parse_type("void"), t.parse_type("void"));
    }

    #[test]
    fn top_03() {
        let t = TypeSystemTest::new();
        let f0 = t.parse_function_type("T0 Function<T0 extends dynamic>()");
        let f1 = t.parse_function_type("T2 Function<T2 extends void>()");

        is_subtype(&t, f0, f1);
        is_subtype(&t, f1, f0);
    }

    #[test]
    fn top_04() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "dynamic", "dynamic Function()");
    }

    #[test]
    fn top_05() {
        let t = TypeSystemTest::new();
        is_not_subtype2(&t, "FutureOr<void Function()>", "void Function()");
    }

    #[test]
    fn top_06() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_subtype(
                &t,
                scope.parse_type("T & void Function()"),
                t.parse_function_type("void Function()"),
            );
        });
    }

    #[test]
    fn top_07() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_subtype(
                &t,
                scope.parse_type("T & void Function()"),
                t.parse_function_type("dynamic Function()"),
            );
        });
    }

    #[test]
    fn top_08() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_not_subtype(
                &t,
                scope.parse_type("T & void Function()"),
                t.parse_function_type("Object Function()"),
            );
        });
    }

    #[test]
    fn top_09() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_subtype(
                &t,
                scope.parse_type("T & void Function(void)"),
                t.parse_function_type("void Function(void)"),
            );
        });
    }

    #[test]
    fn top_10() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_subtype(
                &t,
                scope.parse_type("T & void Function(void)"),
                t.parse_function_type("dynamic Function(dynamic)"),
            );
        });
    }

    #[test]
    fn top_11() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_not_subtype(
                &t,
                scope.parse_type("T & void Function(void)"),
                t.parse_function_type("Object Function(Object)"),
            );
        });
    }

    #[test]
    fn top_12() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_subtype(
                &t,
                scope.parse_type("T & void Function(void)"),
                t.parse_function_type("dynamic Function(Iterable<int>)"),
            );
        });
    }

    #[test]
    fn top_13() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_not_subtype(
                &t,
                scope.parse_type("T & void Function(void)"),
                t.parse_function_type("Object Function(int)"),
            );
        });
    }

    #[test]
    fn top_14() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_not_subtype(
                &t,
                scope.parse_type("T & void Function(void)"),
                t.parse_function_type("int Function(int)"),
            );
        });
    }

    #[test]
    fn top_15() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends void Function()", |scope| {
            is_subtype(
                &t,
                scope.parse_type("T"),
                t.parse_function_type("void Function()"),
            );
        });
    }

    #[test]
    fn top_16() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_not_subtype(
                &t,
                scope.parse_type("T"),
                t.parse_function_type("void Function()"),
            );
        });
    }

    #[test]
    fn top_17() {
        let t = TypeSystemTest::new();
        is_not_subtype(
            &t,
            t.parse_type("void"),
            t.parse_function_type("void Function()"),
        );
    }

    #[test]
    fn top_18() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_not_subtype(&t, t.parse_type("dynamic"), scope.parse_type("T"));
        });
    }

    #[test]
    fn top_19() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_not_subtype(&t, scope.parse_type("Iterable<T>"), scope.parse_type("T"));
        });
    }

    #[test]
    fn top_21() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_not_subtype(
                &t,
                t.parse_function_type("void Function()"),
                scope.parse_type("T"),
            );
        });
    }

    #[test]
    fn top_22() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_not_subtype(&t, scope.parse_type("FutureOr<T>"), scope.parse_type("T"));
        });
    }

    #[test]
    fn top_23() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_not_subtype(&t, t.parse_type("void"), scope.parse_type("T"));
        });
    }

    #[test]
    fn top_24() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_not_subtype(&t, t.parse_type("void"), scope.parse_type("T & void"));
        });
    }

    #[test]
    fn top_25() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends void", |scope| {
            is_not_subtype(&t, t.parse_type("void"), scope.parse_type("T & void"));
        });
    }

    #[test]
    fn type_parameter_01() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_subtype(&t, scope.parse_type("T & int"), scope.parse_type("T & int"));
        });
    }

    #[test]
    fn type_parameter_02() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_subtype(&t, scope.parse_type("T & int"), scope.parse_type("T & num"));
        });
    }

    #[test]
    fn type_parameter_03() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_subtype(&t, scope.parse_type("T & num"), scope.parse_type("T & num"));
        });
    }

    #[test]
    fn type_parameter_04() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_not_subtype(&t, scope.parse_type("T & num"), scope.parse_type("T & int"));
        });
    }

    #[test]
    fn type_parameter_05() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_not_subtype(&t, t.parse_type("Null"), scope.parse_type("T & num"));
        });
    }

    #[test]
    fn type_parameter_06() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends int", |scope| {
            is_subtype(&t, scope.parse_type("T & int"), scope.parse_type("T"));
        });
    }

    #[test]
    fn type_parameter_07() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends num", |scope| {
            is_subtype(&t, scope.parse_type("T & int"), scope.parse_type("T"));
        });
    }

    #[test]
    fn type_parameter_08() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends num", |scope| {
            is_subtype(&t, scope.parse_type("T & num"), scope.parse_type("T"));
        });
    }

    #[test]
    fn type_parameter_09() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends int", |scope| {
            is_subtype(&t, scope.parse_type("T"), scope.parse_type("T & int"));
        });
    }

    #[test]
    fn type_parameter_10() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends int", |scope| {
            is_subtype(&t, scope.parse_type("T"), scope.parse_type("T & num"));
        });
    }

    #[test]
    fn type_parameter_11() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends num", |scope| {
            is_not_subtype(&t, scope.parse_type("T"), scope.parse_type("T & int"));
        });
    }

    #[test]
    fn type_parameter_12() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends num", |scope| {
            is_subtype(&t, scope.parse_type("T"), scope.parse_type("T"));
        });
    }

    #[test]
    fn type_parameter_13() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_subtype(&t, scope.parse_type("T"), scope.parse_type("T"));
        });
    }

    #[test]
    fn type_parameter_14() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("S, T", |scope| {
            is_not_subtype(&t, scope.parse_type("S"), scope.parse_type("T"));
        });
    }

    #[test]
    fn type_parameter_15() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object", |scope| {
            is_subtype(&t, scope.parse_type("T"), scope.parse_type("T"));
        });
    }

    #[test]
    fn type_parameter_16() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("S extends Object, T extends Object", |scope| {
            is_not_subtype(&t, scope.parse_type("S"), scope.parse_type("T"));
        });
    }

    #[test]
    fn type_parameter_17() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends dynamic", |scope| {
            is_subtype(&t, scope.parse_type("T"), scope.parse_type("T"));
        });
    }

    #[test]
    fn type_parameter_18() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("S extends dynamic, T extends dynamic", |scope| {
            is_not_subtype(&t, scope.parse_type("S"), scope.parse_type("T"));
        });
    }

    #[test]
    fn type_parameter_19() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("S, T extends S", |scope| {
            is_not_subtype(&t, scope.parse_type("S"), scope.parse_type("T"));

            is_subtype(&t, scope.parse_type("T"), scope.parse_type("S"));
        });
    }

    #[test]
    fn type_parameter_20() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_subtype(&t, scope.parse_type("T & int"), t.parse_type("int"));
        });
    }

    #[test]
    fn type_parameter_21() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_subtype(&t, scope.parse_type("T & int"), t.parse_type("num"));
        });
    }

    #[test]
    fn type_parameter_22() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_subtype(&t, scope.parse_type("T & num"), t.parse_type("num"));
        });
    }

    #[test]
    fn type_parameter_23() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_not_subtype(&t, scope.parse_type("T & num"), t.parse_type("int"));
        });
    }

    #[test]
    fn type_parameter_24() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("S, T", |scope| {
            is_not_subtype(&t, scope.parse_type("S & num"), scope.parse_type("T"));
        });
    }

    #[test]
    fn type_parameter_25() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("S, T", |scope| {
            is_not_subtype(&t, scope.parse_type("S & num"), scope.parse_type("T & num"));
        });
    }

    #[test]
    fn type_parameter_26() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("S extends int", |scope| {
            is_subtype(&t, scope.parse_type("S"), t.parse_type("int"));
        });
    }

    #[test]
    fn type_parameter_27() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("S extends int", |scope| {
            is_subtype(&t, scope.parse_type("S"), t.parse_type("num"));
        });
    }

    #[test]
    fn type_parameter_28() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("S extends num", |scope| {
            is_subtype(&t, scope.parse_type("S"), t.parse_type("num"));
        });
    }

    #[test]
    fn type_parameter_29() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("S extends num", |scope| {
            is_not_subtype(&t, scope.parse_type("S"), t.parse_type("int"));
        });
    }

    #[test]
    fn type_parameter_30() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("S extends num, T", |scope| {
            is_not_subtype(&t, scope.parse_type("S"), scope.parse_type("T"));
        });
    }

    #[test]
    fn type_parameter_31() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("S extends num, T", |scope| {
            is_not_subtype(&t, scope.parse_type("S"), scope.parse_type("T & num"));
        });
    }

    #[test]
    fn type_parameter_32() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends dynamic", |scope| {
            is_not_subtype(&t, t.parse_type("dynamic"), scope.parse_type("T & dynamic"));
        });
    }

    #[test]
    fn type_parameter_33() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            let t_function = scope.parse_type("T Function()");
            is_not_subtype(&t, t_function, scope.parse_type("T & T Function()"));
        });
    }

    #[test]
    fn type_parameter_34() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_not_subtype(
                &t,
                scope.parse_type("FutureOr<T & String>"),
                scope.parse_type("T & String"),
            );
        });
    }

    #[test]
    fn type_parameter_35() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_not_subtype(&t, t.parse_type("Null"), scope.parse_type("T"));
        });
    }

    #[test]
    fn type_parameter_36() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends num", |scope| {
            is_subtype(&t, scope.parse_type("T"), t.parse_type("num"));
        });
    }

    #[test]
    fn type_parameter_37() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object?", |scope| {
            let ty = scope.parse_type("T & num?");

            is_not_subtype(&t, ty, t.parse_type("num"));
            is_subtype(&t, ty, t.parse_type("num?"));
        });
    }

    #[test]
    fn type_parameter_38() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends num", |scope| {
            is_subtype(&t, scope.parse_type("T"), t.parse_type("Object"));
        });
    }

    #[test]
    fn type_parameter_39() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends num", |scope| {
            is_subtype(&t, scope.parse_type("T"), t.parse_type("Object"));
        });
    }

    #[test]
    fn type_parameter_40() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends num", |scope| {
            is_not_subtype(&t, scope.parse_type("T?"), t.parse_type("Object"));
        });
    }

    #[test]
    fn type_parameter_41() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends num?", |scope| {
            is_not_subtype(&t, scope.parse_type("T"), t.parse_type("Object"));
        });
    }

    #[test]
    fn type_parameter_42() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends num?", |scope| {
            is_not_subtype(&t, scope.parse_type("T?"), t.parse_type("Object"));
        });
    }

    #[test]
    fn type_parameter_43() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            is_not_subtype(&t, scope.parse_type("T"), t.parse_type("Object"));
        });
    }

    // @FailingTest(issue: 'https://github.com/dart-lang/language/issues/433')
    // Dart: `T <: FutureOr<T>` recurses without end (`T <: Future<T>` ->
    // bound `FutureOr<T> <: Future<T>` -> `T <: Future<T>`), the test fails
    // with a StackOverflowError. In Rust the stack overflow aborts the test
    // process, so the test cannot be a `#[should_panic]` test.
    #[test]
    #[ignore = "Dart @FailingTest (language issue 433): infinite recursion, stack overflow aborts the process"]
    fn type_parameter_44() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends FutureOr<T>", |scope| {
            is_subtype(&t, scope.parse_type("T"), scope.parse_type("FutureOr<T>"));
        });
    }
}

mod subtyping_compound_test {
    use super::*;

    #[test]
    fn double() {
        let t = TypeSystemTest::new();
        let equivalents = vec![t.parse_type("double")];
        let supertypes = vec![t.parse_type("num")];
        let unrelated = vec![t.parse_type("int")];
        check_groups(
            &t,
            t.parse_type("double"),
            Groups {
                equivalents: Some(&equivalents),
                supertypes: Some(&supertypes),
                unrelated: Some(&unrelated),
                ..Groups::default()
            },
        );
    }

    #[test]
    fn dynamic() {
        let t = TypeSystemTest::new();
        let equivalents = vec![t.parse_type("void"), t.parse_type("Object?")];

        let subtypes = vec![
            t.parse_type("Never"),
            t.parse_type("Null"),
            t.parse_type("Object"),
        ];

        check_groups(
            &t,
            t.parse_type("dynamic"),
            Groups {
                equivalents: Some(&equivalents),
                subtypes: Some(&subtypes),
                ..Groups::default()
            },
        );
    }

    #[test]
    fn dynamic_is_top() {
        let t = TypeSystemTest::new();
        let equivalents = vec![
            t.parse_type("dynamic"),
            t.parse_type("Object?"),
            t.parse_type("void"),
        ];

        let subtypes = vec![
            t.parse_type("int"),
            t.parse_type("double"),
            t.parse_type("num"),
            t.parse_type("String"),
            t.parse_type("Function"),
        ];

        check_groups(
            &t,
            t.parse_type("dynamic"),
            Groups {
                equivalents: Some(&equivalents),
                subtypes: Some(&subtypes),
                ..Groups::default()
            },
        );
    }

    #[test]
    fn future_or_top_types() {
        let t = TypeSystemTest::new();
        let future_or_object = t.parse_type("FutureOr<Object>");
        let future_or_object_question = t.parse_type("FutureOr<Object?>");

        let future_or_question_object = t.parse_type("FutureOr<Object>?");
        let future_or_question_object_question = t.parse_type("FutureOr<Object?>?");

        // FutureOr<Object> <: FutureOr*<Object?>
        check_groups(
            &t,
            future_or_object,
            Groups {
                equivalents: Some(&[t.parse_type("Object")]),
                subtypes: Some(&[]),
                supertypes: Some(&[
                    t.parse_type("Object?"),
                    future_or_question_object,
                    future_or_object_question,
                    future_or_question_object,
                    future_or_question_object_question,
                ]),
                ..Groups::default()
            },
        );
    }

    #[test]
    fn int_none() {
        let t = TypeSystemTest::new();
        let equivalents = vec![t.parse_type("int")];

        let subtypes = vec![t.parse_type("Never")];

        let supertypes = vec![
            t.parse_type("int?"),
            t.parse_type("Object"),
            t.parse_type("Object?"),
        ];

        let unrelated = vec![
            t.parse_type("double"),
            t.parse_type("Null"),
            t.parse_type("Never?"),
        ];

        check_groups(
            &t,
            t.parse_type("int"),
            Groups {
                equivalents: Some(&equivalents),
                supertypes: Some(&supertypes),
                unrelated: Some(&unrelated),
                subtypes: Some(&subtypes),
            },
        );
    }

    #[test]
    fn int_question() {
        let t = TypeSystemTest::new();
        let equivalents = vec![t.parse_type("int?")];

        let subtypes = vec![
            t.parse_type("int"),
            t.parse_type("Null"),
            t.parse_type("Never"),
            t.parse_type("Never?"),
        ];

        let supertypes = vec![t.parse_type("num?"), t.parse_type("Object?")];

        let unrelated = vec![
            t.parse_type("double"),
            t.parse_type("num"),
            t.parse_type("Object"),
        ];

        check_groups(
            &t,
            t.parse_type("int?"),
            Groups {
                equivalents: Some(&equivalents),
                supertypes: Some(&supertypes),
                unrelated: Some(&unrelated),
                subtypes: Some(&subtypes),
            },
        );
    }

    #[test]
    fn null() {
        let t = TypeSystemTest::new();
        let equivalents = vec![t.parse_type("Null"), t.parse_type("Never?")];

        let supertypes = vec![
            t.parse_type("int?"),
            t.parse_type("Object?"),
            t.parse_type("dynamic"),
            t.parse_type("void"),
        ];

        let subtypes = vec![t.parse_type("Never")];

        let unrelated = vec![
            t.parse_type("double"),
            t.parse_type("int"),
            t.parse_type("num"),
            t.parse_type("Object"),
        ];

        for &form_of_null in &equivalents {
            check_groups(
                &t,
                form_of_null,
                Groups {
                    equivalents: Some(&equivalents),
                    supertypes: Some(&supertypes),
                    unrelated: Some(&unrelated),
                    subtypes: Some(&subtypes),
                },
            );
        }
    }

    #[test]
    fn num_none() {
        let t = TypeSystemTest::new();
        let equivalents = vec![t.parse_type("num")];
        let supertypes = vec![t.parse_type("Object")];
        let unrelated = vec![t.parse_type("String")];
        let subtypes = vec![t.parse_type("int"), t.parse_type("double")];
        check_groups(
            &t,
            t.parse_type("num"),
            Groups {
                equivalents: Some(&equivalents),
                supertypes: Some(&supertypes),
                unrelated: Some(&unrelated),
                subtypes: Some(&subtypes),
            },
        );
    }

    #[test]
    fn object() {
        let t = TypeSystemTest::new();
        let equivalents = vec![];

        let supertypes = vec![
            t.parse_type("Object?"),
            t.parse_type("dynamic"),
            t.parse_type("void"),
        ];

        let subtypes = vec![t.parse_type("Never")];

        let unrelated = vec![
            t.parse_type("double?"),
            t.parse_type("num?"),
            t.parse_type("int?"),
            t.parse_type("Null"),
        ];

        check_groups(
            &t,
            t.parse_type("Object"),
            Groups {
                equivalents: Some(&equivalents),
                supertypes: Some(&supertypes),
                unrelated: Some(&unrelated),
                subtypes: Some(&subtypes),
            },
        );
    }

    /// The named parameters of `_checkGroups`.
    #[derive(Default)]
    struct Groups<'a> {
        equivalents: Option<&'a [TypeId]>,
        unrelated: Option<&'a [TypeId]>,
        subtypes: Option<&'a [TypeId]>,
        supertypes: Option<&'a [TypeId]>,
    }

    /// `_checkEquivalent(type1, type2)`.
    fn check_equivalent(t: &TypeSystemTest, type1: TypeId, type2: TypeId) {
        check_is_subtype_of(t, type1, type2);
        check_is_subtype_of(t, type2, type1);
    }

    /// `_checkGroups(t1, ...)`.
    fn check_groups(t: &TypeSystemTest, t1: TypeId, groups: Groups<'_>) {
        if let Some(equivalents) = groups.equivalents {
            for &t2 in equivalents {
                check_equivalent(t, t1, t2);
            }
        }
        if let Some(unrelated) = groups.unrelated {
            for &t2 in unrelated {
                check_unrelated(t, t1, t2);
            }
        }
        if let Some(subtypes) = groups.subtypes {
            for &t2 in subtypes {
                check_is_strict_subtype_of(t, t2, t1);
            }
        }
        if let Some(supertypes) = groups.supertypes {
            for &t2 in supertypes {
                check_is_strict_subtype_of(t, t1, t2);
            }
        }
    }

    /// `_checkIsNotSubtypeOf(type1, type2)`.
    fn check_is_not_subtype_of(t: &TypeSystemTest, type1: TypeId, type2: TypeId) {
        assert!(
            !t.type_system().is_subtype_of(type1, type2),
            "{} was not supposed to be a subtype of {}",
            t.display(type1),
            t.display(type2)
        );
    }

    /// `_checkIsStrictSubtypeOf(type1, type2)`.
    fn check_is_strict_subtype_of(t: &TypeSystemTest, type1: TypeId, type2: TypeId) {
        check_is_subtype_of(t, type1, type2);
        check_is_not_subtype_of(t, type2, type1);
    }

    /// `_checkIsSubtypeOf(type1, type2)`.
    fn check_is_subtype_of(t: &TypeSystemTest, type1: TypeId, type2: TypeId) {
        assert!(
            t.type_system().is_subtype_of(type1, type2),
            "{} is not a subtype of {}",
            t.display(type1),
            t.display(type2)
        );
    }

    /// `_checkUnrelated(type1, type2)`.
    fn check_unrelated(t: &TypeSystemTest, type1: TypeId, type2: TypeId) {
        check_is_not_subtype_of(t, type1, type2);
        check_is_not_subtype_of(t, type2, type1);
    }
}
