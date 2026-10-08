// Dart source: pkg/analyzer/test/src/dart/constant/value_test.dart
//
// A port of `DartObjectImplTest`: one Rust test per Dart test, same names.
// Generated from the Dart file by a converter for the `_assert*` calls, then
// the 25 tests with closures or special types were ported by hand.

#![allow(non_snake_case)]

mod support;

use dartr_constant::{DartObjectImpl, InstanceState, IntState};
use dartr_element::{FunctionTypeData, Nullability, TypeKind};
use support::{LONG_MAX_VALUE, T};

#[test]
fn test_add_knownDouble_knownDouble() {
    let t = T::new();
    t.assert_add(
        Some(t.double_value(Some(3.0))),
        t.double_value(Some(1.0)),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_add_knownDouble_knownInt() {
    let t = T::new();
    t.assert_add(
        Some(t.double_value(Some(3.0))),
        t.double_value(Some(1.0)),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_add_knownDouble_unknownDouble() {
    let t = T::new();
    t.assert_add(
        Some(t.double_value(None)),
        t.double_value(Some(1.0)),
        t.double_value(None),
    )
    .unwrap();
}

#[test]
fn test_add_knownDouble_unknownInt() {
    let t = T::new();
    t.assert_add(
        Some(t.double_value(None)),
        t.double_value(Some(1.0)),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_add_knownInt_knownInt() {
    let t = T::new();
    t.assert_add(
        Some(t.int_value(Some(3))),
        t.int_value(Some(1)),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_add_knownInt_knownString() {
    let t = T::new();
    t.assert_add(None, t.int_value(Some(1)), t.string_value(Some("2")))
        .unwrap();
}

#[test]
fn test_add_knownInt_unknownDouble() {
    let t = T::new();
    t.assert_add(
        Some(t.double_value(None)),
        t.int_value(Some(1)),
        t.double_value(None),
    )
    .unwrap();
}

#[test]
fn test_add_knownInt_unknownInt() {
    let t = T::new();
    t.assert_add(
        Some(t.int_value(None)),
        t.int_value(Some(1)),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_add_knownString_knownInt() {
    let t = T::new();
    t.assert_add(None, t.string_value(Some("1")), t.int_value(Some(2)))
        .unwrap();
}

#[test]
fn test_add_knownString_knownString() {
    let t = T::new();
    t.assert_add(
        Some(t.string_value(Some("ab"))),
        t.string_value(Some("a")),
        t.string_value(Some("b")),
    )
    .unwrap();
}

#[test]
fn test_add_knownString_unknownString() {
    let t = T::new();
    t.assert_add(
        Some(t.string_value(None)),
        t.string_value(Some("a")),
        t.string_value(None),
    )
    .unwrap();
}

#[test]
fn test_add_unknownDouble_knownDouble() {
    let t = T::new();
    t.assert_add(
        Some(t.double_value(None)),
        t.double_value(None),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_add_unknownDouble_knownInt() {
    let t = T::new();
    t.assert_add(
        Some(t.double_value(None)),
        t.double_value(None),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_add_unknownInt_knownDouble() {
    let t = T::new();
    t.assert_add(
        Some(t.double_value(None)),
        t.int_value(None),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_add_unknownInt_knownInt() {
    let t = T::new();
    t.assert_add(
        Some(t.int_value(None)),
        t.int_value(None),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_add_unknownString_knownString() {
    let t = T::new();
    t.assert_add(
        Some(t.string_value(None)),
        t.string_value(None),
        t.string_value(Some("b")),
    )
    .unwrap();
}

#[test]
fn test_add_unknownString_unknownString() {
    let t = T::new();
    t.assert_add(
        Some(t.string_value(None)),
        t.string_value(None),
        t.string_value(None),
    )
    .unwrap();
}

#[test]
fn test_bitAnd_knownInt_knownInt() {
    let t = T::new();
    t.assert_eager_and(
        Some(t.int_value(Some(2))),
        t.int_value(Some(6)),
        t.int_value(Some(3)),
    )
    .unwrap();
}

#[test]
fn test_bitAnd_knownInt_knownString() {
    let t = T::new();
    t.assert_eager_and(None, t.int_value(Some(6)), t.string_value(Some("3")))
        .unwrap();
}

#[test]
fn test_bitAnd_knownInt_unknownInt() {
    let t = T::new();
    t.assert_eager_and(
        Some(t.int_value(None)),
        t.int_value(Some(6)),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_bitAnd_knownString_knownInt() {
    let t = T::new();
    t.assert_eager_and(None, t.string_value(Some("6")), t.int_value(Some(3)))
        .unwrap();
}

#[test]
fn test_bitAnd_unknownInt_knownInt() {
    let t = T::new();
    t.assert_eager_and(
        Some(t.int_value(None)),
        t.int_value(None),
        t.int_value(Some(3)),
    )
    .unwrap();
}

#[test]
fn test_bitAnd_unknownInt_unknownInt() {
    let t = T::new();
    t.assert_eager_and(
        Some(t.int_value(None)),
        t.int_value(None),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_bitNot_knownInt() {
    let t = T::new();
    t.assert_bit_not(Some(t.int_value(Some(-4))), t.int_value(Some(3)))
        .unwrap();
}

#[test]
fn test_bitNot_knownString() {
    let t = T::new();
    t.assert_bit_not(None, t.string_value(Some("6"))).unwrap();
}

#[test]
fn test_bitNot_unknownInt() {
    let t = T::new();
    t.assert_bit_not(Some(t.int_value(None)), t.int_value(None))
        .unwrap();
}

#[test]
fn test_bitOr_knownInt_knownInt() {
    let t = T::new();
    t.assert_eager_or(
        Some(t.int_value(Some(7))),
        t.int_value(Some(6)),
        t.int_value(Some(3)),
    )
    .unwrap();
}

#[test]
fn test_bitOr_knownInt_knownString() {
    let t = T::new();
    t.assert_eager_or(None, t.int_value(Some(6)), t.string_value(Some("3")))
        .unwrap();
}

#[test]
fn test_bitOr_knownInt_unknownInt() {
    let t = T::new();
    t.assert_eager_or(
        Some(t.int_value(None)),
        t.int_value(Some(6)),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_bitOr_knownString_knownInt() {
    let t = T::new();
    t.assert_eager_or(None, t.string_value(Some("6")), t.int_value(Some(3)))
        .unwrap();
}

#[test]
fn test_bitOr_unknownInt_knownInt() {
    let t = T::new();
    t.assert_eager_or(
        Some(t.int_value(None)),
        t.int_value(None),
        t.int_value(Some(3)),
    )
    .unwrap();
}

#[test]
fn test_bitOr_unknownInt_unknownInt() {
    let t = T::new();
    t.assert_eager_or(
        Some(t.int_value(None)),
        t.int_value(None),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_bitXor_knownInt_knownInt() {
    let t = T::new();
    t.assert_eager_xor(
        Some(t.int_value(Some(5))),
        t.int_value(Some(6)),
        t.int_value(Some(3)),
    )
    .unwrap();
}

#[test]
fn test_bitXor_knownInt_knownString() {
    let t = T::new();
    t.assert_eager_xor(None, t.int_value(Some(6)), t.string_value(Some("3")))
        .unwrap();
}

#[test]
fn test_bitXor_knownInt_unknownInt() {
    let t = T::new();
    t.assert_eager_xor(
        Some(t.int_value(None)),
        t.int_value(Some(6)),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_bitXor_knownString_knownInt() {
    let t = T::new();
    t.assert_eager_xor(None, t.string_value(Some("6")), t.int_value(Some(3)))
        .unwrap();
}

#[test]
fn test_bitXor_unknownInt_knownInt() {
    let t = T::new();
    t.assert_eager_xor(
        Some(t.int_value(None)),
        t.int_value(None),
        t.int_value(Some(3)),
    )
    .unwrap();
}

#[test]
fn test_bitXor_unknownInt_unknownInt() {
    let t = T::new();
    t.assert_eager_xor(
        Some(t.int_value(None)),
        t.int_value(None),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_concatenate_knownInt_knownString() {
    let t = T::new();
    t.assert_concatenate(None, t.int_value(Some(2)), t.string_value(Some("def")))
        .unwrap();
}

#[test]
fn test_concatenate_knownString_knownInt() {
    let t = T::new();
    t.assert_concatenate(None, t.string_value(Some("abc")), t.int_value(Some(3)))
        .unwrap();
}

#[test]
fn test_concatenate_knownString_knownString() {
    let t = T::new();
    t.assert_concatenate(
        Some(t.string_value(Some("abcdef"))),
        t.string_value(Some("abc")),
        t.string_value(Some("def")),
    )
    .unwrap();
}

#[test]
fn test_concatenate_knownString_unknownString() {
    let t = T::new();
    t.assert_concatenate(
        Some(t.string_value(None)),
        t.string_value(Some("abc")),
        t.string_value(None),
    )
    .unwrap();
}

#[test]
fn test_concatenate_unknownString_knownString() {
    let t = T::new();
    t.assert_concatenate(
        Some(t.string_value(None)),
        t.string_value(None),
        t.string_value(Some("def")),
    )
    .unwrap();
}

#[test]
fn test_divide_knownDouble_knownDouble() {
    let t = T::new();
    t.assert_divide(
        Some(t.double_value(Some(3.0))),
        t.double_value(Some(6.0)),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_divide_knownDouble_knownInt() {
    let t = T::new();
    t.assert_divide(
        Some(t.double_value(Some(3.0))),
        t.double_value(Some(6.0)),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_divide_knownDouble_unknownDouble() {
    let t = T::new();
    t.assert_divide(
        Some(t.double_value(None)),
        t.double_value(Some(6.0)),
        t.double_value(None),
    )
    .unwrap();
}

#[test]
fn test_divide_knownDouble_unknownInt() {
    let t = T::new();
    t.assert_divide(
        Some(t.double_value(None)),
        t.double_value(Some(6.0)),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_divide_knownInt_knownInt() {
    let t = T::new();
    t.assert_divide(
        Some(t.double_value(Some(3.0))),
        t.int_value(Some(6)),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_divide_knownInt_knownString() {
    let t = T::new();
    t.assert_divide(None, t.int_value(Some(6)), t.string_value(Some("2")))
        .unwrap();
}

#[test]
fn test_divide_knownInt_unknownDouble() {
    let t = T::new();
    t.assert_divide(
        Some(t.double_value(None)),
        t.int_value(Some(6)),
        t.double_value(None),
    )
    .unwrap();
}

#[test]
fn test_divide_knownInt_unknownInt() {
    let t = T::new();
    t.assert_divide(
        Some(t.double_value(None)),
        t.int_value(Some(6)),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_divide_knownString_knownInt() {
    let t = T::new();
    t.assert_divide(None, t.string_value(Some("6")), t.int_value(Some(2)))
        .unwrap();
}

#[test]
fn test_divide_unknownDouble_knownDouble() {
    let t = T::new();
    t.assert_divide(
        Some(t.double_value(None)),
        t.double_value(None),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_divide_unknownDouble_knownInt() {
    let t = T::new();
    t.assert_divide(
        Some(t.double_value(None)),
        t.double_value(None),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_divide_unknownInt_knownDouble() {
    let t = T::new();
    t.assert_divide(
        Some(t.double_value(None)),
        t.int_value(None),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_divide_unknownInt_knownInt() {
    let t = T::new();
    t.assert_divide(
        Some(t.double_value(None)),
        t.int_value(None),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_eagerAnd_knownBool_knownBool() {
    let t = T::new();
    let check = |left: bool, right: bool, expected: bool| {
        t.assert_eager_and(
            Some(t.bool_value(Some(expected))),
            t.bool_value(Some(left)),
            t.bool_value(Some(right)),
        )
        .unwrap();
    };
    check(false, false, false);
    check(true, false, false);
    check(false, true, false);
    check(true, true, true);
}

#[test]
fn test_eagerAnd_knownBool_knownInt() {
    let t = T::new();
    t.assert_eager_and(None, t.bool_value(Some(true)), t.int_value(Some(0)))
        .unwrap();
}

#[test]
fn test_eagerAnd_knownBool_unknownBool() {
    let t = T::new();
    t.assert_eager_and(
        Some(t.bool_value(None)),
        t.bool_value(Some(true)),
        t.bool_value(None),
    )
    .unwrap();
}

#[test]
fn test_eagerAnd_unknownBool_knownBool() {
    let t = T::new();
    t.assert_eager_and(
        Some(t.bool_value(None)),
        t.bool_value(None),
        t.bool_value(Some(true)),
    )
    .unwrap();
}

#[test]
fn test_eagerOr_knownBool_knownBool() {
    let t = T::new();
    let check = |left: bool, right: bool, expected: bool| {
        t.assert_eager_or(
            Some(t.bool_value(Some(expected))),
            t.bool_value(Some(left)),
            t.bool_value(Some(right)),
        )
        .unwrap();
    };
    check(false, false, false);
    check(true, false, true);
    check(false, true, true);
    check(true, true, true);
}

#[test]
fn test_eagerOr_knownBool_knownInt() {
    let t = T::new();
    t.assert_eager_or(None, t.bool_value(Some(true)), t.int_value(Some(0)))
        .unwrap();
}

#[test]
fn test_eagerOr_knownBool_unknownBool() {
    let t = T::new();
    t.assert_eager_or(
        Some(t.bool_value(None)),
        t.bool_value(Some(true)),
        t.bool_value(None),
    )
    .unwrap();
}

#[test]
fn test_eagerOr_unknownBool_knownBool() {
    let t = T::new();
    t.assert_eager_or(
        Some(t.bool_value(None)),
        t.bool_value(None),
        t.bool_value(Some(true)),
    )
    .unwrap();
}

#[test]
fn test_eagerXor_knownBool_knownBool() {
    let t = T::new();
    let check = |left: bool, right: bool, expected: bool| {
        t.assert_eager_xor(
            Some(t.bool_value(Some(expected))),
            t.bool_value(Some(left)),
            t.bool_value(Some(right)),
        )
        .unwrap();
    };
    check(false, false, false);
    check(true, false, true);
    check(false, true, true);
    check(true, true, false);
}

#[test]
fn test_eagerXor_knownBool_knownInt() {
    let t = T::new();
    t.assert_eager_xor(None, t.bool_value(Some(true)), t.int_value(Some(0)))
        .unwrap();
}

#[test]
fn test_eagerXor_knownBool_unknownBool() {
    let t = T::new();
    t.assert_eager_xor(
        Some(t.bool_value(None)),
        t.bool_value(Some(true)),
        t.bool_value(None),
    )
    .unwrap();
}

#[test]
fn test_eagerXor_unknownBool_knownBool() {
    let t = T::new();
    t.assert_eager_xor(
        Some(t.bool_value(None)),
        t.bool_value(None),
        t.bool_value(Some(true)),
    )
    .unwrap();
}

#[test]
fn test_equalEqual_bool_false() {
    let t = T::new();
    t.assert_equal_equal(
        Some(t.bool_value(Some(false))),
        t.bool_value(Some(false)),
        t.bool_value(Some(true)),
    )
    .unwrap();
}

#[test]
fn test_equalEqual_bool_true() {
    let t = T::new();
    t.assert_equal_equal(
        Some(t.bool_value(Some(true))),
        t.bool_value(Some(true)),
        t.bool_value(Some(true)),
    )
    .unwrap();
}

#[test]
fn test_equalEqual_bool_unknown() {
    let t = T::new();
    t.assert_equal_equal(
        Some(t.bool_value(None)),
        t.bool_value(None),
        t.bool_value(Some(false)),
    )
    .unwrap();
}

#[test]
fn test_equalEqual_double_false() {
    let t = T::new();
    t.assert_equal_equal(
        Some(t.bool_value(Some(false))),
        t.double_value(Some(2.0)),
        t.double_value(Some(4.0)),
    )
    .unwrap();
}

#[test]
fn test_equalEqual_double_false_language219() {
    let mut t = T::new();
    t.set_language_2_19();
    t.assert_equal_equal(
        Some(t.bool_value(Some(false))),
        t.double_value(Some(2.0)),
        t.double_value(Some(4.0)),
    )
    .unwrap();
}

#[test]
fn test_equalEqual_double_false_zeros() {
    let t = T::new();
    t.assert_equal_equal(
        Some(t.bool_value(Some(false))),
        t.double_value(Some(0.0)),
        t.double_value(Some(-0.0)),
    )
    .unwrap();
}

#[test]
fn test_equalEqual_double_true() {
    let t = T::new();
    t.assert_equal_equal(
        Some(t.bool_value(Some(true))),
        t.double_value(Some(2.0)),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_equalEqual_double_true_language219() {
    let mut t = T::new();
    t.set_language_2_19();
    t.assert_equal_equal(
        Some(t.bool_value(Some(true))),
        t.double_value(Some(2.0)),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_equalEqual_double_unknown() {
    let t = T::new();
    t.assert_equal_equal(
        Some(t.bool_value(None)),
        t.double_value(Some(1.0)),
        t.double_value(None),
    )
    .unwrap();
}

#[test]
fn test_equalEqual_double_unknown_language219() {
    let mut t = T::new();
    t.set_language_2_19();
    t.assert_equal_equal(
        Some(t.bool_value(None)),
        t.double_value(Some(1.0)),
        t.double_value(None),
    )
    .unwrap();
}

#[test]
fn test_equalEqual_int_false() {
    let t = T::new();
    t.assert_equal_equal(
        Some(t.bool_value(Some(false))),
        t.int_value(Some(-5)),
        t.int_value(Some(5)),
    )
    .unwrap();
}

#[test]
fn test_equalEqual_int_true() {
    let t = T::new();
    t.assert_equal_equal(
        Some(t.bool_value(Some(true))),
        t.int_value(Some(5)),
        t.int_value(Some(5)),
    )
    .unwrap();
}

#[test]
fn test_equalEqual_int_unknown() {
    let t = T::new();
    t.assert_equal_equal(
        Some(t.bool_value(None)),
        t.int_value(None),
        t.int_value(Some(3)),
    )
    .unwrap();
}

#[test]
fn test_equalEqual_list_error_language219() {
    let mut t = T::new();
    t.set_language_2_19();
    t.assert_equal_equal(
        None,
        t.list_value(t.tp().int_type(), vec![]),
        t.list_value(t.tp().int_type(), vec![]),
    )
    .unwrap();
}

#[test]
fn test_equalEqual_list_true() {
    let t = T::new();
    t.assert_equal_equal(
        Some(t.bool_value(Some(true))),
        t.list_value(t.tp().int_type(), vec![]),
        t.list_value(t.tp().int_type(), vec![]),
    )
    .unwrap();
}

#[test]
fn test_equalEqual_map_error_language219() {
    let mut t = T::new();
    t.set_language_2_19();
    t.assert_equal_equal(
        None,
        t.map_value(t.tp().int_type(), t.tp().string_type(), vec![]),
        t.map_value(t.tp().int_type(), t.tp().string_type(), vec![]),
    )
    .unwrap();
}

#[test]
fn test_equalEqual_map_true() {
    let t = T::new();
    t.assert_equal_equal(
        Some(t.bool_value(Some(true))),
        t.map_value(t.tp().int_type(), t.tp().string_type(), vec![]),
        t.map_value(t.tp().int_type(), t.tp().string_type(), vec![]),
    )
    .unwrap();
}

#[test]
fn test_equalEqual_null() {
    let t = T::new();
    t.assert_equal_equal(
        Some(t.bool_value(Some(true))),
        t.null_value(),
        t.null_value(),
    )
    .unwrap();
}

#[test]
fn test_equalEqual_string_false() {
    let t = T::new();
    t.assert_equal_equal(
        Some(t.bool_value(Some(false))),
        t.string_value(Some("abc")),
        t.string_value(Some("def")),
    )
    .unwrap();
}

#[test]
fn test_equalEqual_string_true() {
    let t = T::new();
    t.assert_equal_equal(
        Some(t.bool_value(Some(true))),
        t.string_value(Some("abc")),
        t.string_value(Some("abc")),
    )
    .unwrap();
}

#[test]
fn test_equalEqual_string_unknown() {
    let t = T::new();
    t.assert_equal_equal(
        Some(t.bool_value(None)),
        t.string_value(None),
        t.string_value(Some("def")),
    )
    .unwrap();
}

#[test]
fn test_equals_list_false_differentSizes() {
    let t = T::new();
    let bool_type = t.tp().bool_type();
    assert!(!t.eq(
        &t.list_value(bool_type, vec![t.bool_value(Some(true))]),
        &t.list_value(
            bool_type,
            vec![t.bool_value(Some(true)), t.bool_value(Some(false))]
        ),
    ));
}

#[test]
fn test_equals_list_false_sameSize() {
    let t = T::new();
    let bool_type = t.tp().bool_type();
    assert!(!t.eq(
        &t.list_value(bool_type, vec![t.bool_value(Some(true))]),
        &t.list_value(bool_type, vec![t.bool_value(Some(false))]),
    ));
}

#[test]
fn test_equals_list_true_empty() {
    let t = T::new();
    t.assert_eq(
        &t.list_value(t.tp().int_type(), vec![]),
        &t.list_value(t.tp().int_type(), vec![]),
    );
}

#[test]
fn test_equals_list_true_nonEmpty() {
    let t = T::new();
    t.assert_eq(
        &t.list_value(t.tp().bool_type(), vec![t.bool_value(Some(true))]),
        &t.list_value(t.tp().bool_type(), vec![t.bool_value(Some(true))]),
    );
}

#[test]
fn test_equals_map_true_empty() {
    let t = T::new();
    t.assert_eq(
        &t.map_value(t.tp().int_type(), t.tp().string_type(), vec![]),
        &t.map_value(t.tp().int_type(), t.tp().string_type(), vec![]),
    );
}

#[test]
fn test_equals_symbol_false() {
    let t = T::new();
    assert!(!t.eq(&t.symbol_value("a"), &t.symbol_value("b")));
}

#[test]
fn test_equals_symbol_true() {
    let t = T::new();
    t.assert_eq(&t.symbol_value("a"), &t.symbol_value("a"));
}

#[test]
fn test_getField_record() {
    let t = T::new();
    let record = t.record_value(
        vec![t.int_value(Some(0)), t.int_value(Some(1))],
        vec![("a", t.int_value(Some(2))), ("b", t.int_value(Some(3)))],
    );
    t.assert_eq(record.get_field("$1").unwrap(), &t.int_value(Some(0)));
    t.assert_eq(record.get_field("$2").unwrap(), &t.int_value(Some(1)));
    assert!(record.get_field("$3").is_none());
    assert!(record.get_field("$-2").is_none());
    t.assert_eq(record.get_field("a").unwrap(), &t.int_value(Some(2)));
    t.assert_eq(record.get_field("b").unwrap(), &t.int_value(Some(3)));
    assert!(record.get_field("c").is_none());
}

#[test]
fn test_getValue_bool_false() {
    let t = T::new();
    assert_eq!(t.bool_value(Some(false)).to_bool_value(), Some(false));
}

#[test]
fn test_getValue_bool_true() {
    let t = T::new();
    assert_eq!(t.bool_value(Some(true)).to_bool_value(), Some(true));
}

#[test]
fn test_getValue_bool_unknown() {
    let t = T::new();
    assert_eq!(t.bool_value(None).to_bool_value(), None);
}

#[test]
fn test_getValue_double_known() {
    let t = T::new();
    let value = 2.3;
    assert_eq!(t.double_value(Some(value)).to_double_value(), Some(value));
}

#[test]
fn test_getValue_double_unknown() {
    let t = T::new();
    assert_eq!(t.double_value(None).to_double_value(), None);
}

#[test]
fn test_getValue_int_known() {
    let t = T::new();
    let value = 23;
    assert_eq!(t.int_value(Some(value)).to_int_value(), Some(value));
}

#[test]
fn test_getValue_int_unknown() {
    let t = T::new();
    assert_eq!(t.int_value(None).to_int_value(), None);
}

#[test]
fn test_getValue_list_empty() {
    let t = T::new();
    let list = t.list_value(t.tp().int_type(), vec![]);
    assert_eq!(list.to_list_value().unwrap().len(), 0);
}

#[test]
fn test_getValue_list_valid() {
    let t = T::new();
    let list = t.list_value(t.tp().int_type(), vec![t.int_value(Some(23))]);
    assert_eq!(list.to_list_value().unwrap().len(), 1);
}

#[test]
fn test_getValue_map_empty() {
    let t = T::new();
    let map = t.map_value(t.tp().int_type(), t.tp().string_type(), vec![]);
    assert_eq!(map.to_map_value().unwrap().len(), 0);
}

#[test]
fn test_getValue_map_valid() {
    let t = T::new();
    let map = t.map_value(
        t.tp().string_type(),
        t.tp().string_type(),
        vec![t.string_value(Some("key")), t.string_value(Some("value"))],
    );
    assert_eq!(map.to_map_value().unwrap().len(), 1);
}

#[test]
fn test_getValue_null() {
    let t = T::new();
    assert!(t.null_value().is_null());
}

#[test]
fn test_getValue_set_empty() {
    let t = T::new();
    let object = t.set_value(t.tp().int_type(), None);
    assert_eq!(object.to_set_value().unwrap().len(), 0);
}

#[test]
fn test_getValue_set_valid() {
    let t = T::new();
    let object = t.set_value(t.tp().int_type(), Some(vec![t.int_value(Some(23))]));
    assert_eq!(object.to_set_value().unwrap().len(), 1);
}

#[test]
fn test_getValue_string_known() {
    let t = T::new();
    let value = "twenty-three";
    assert_eq!(t.string_value(Some(value)).to_string_value(), Some(value));
}

#[test]
fn test_getValue_string_unknown() {
    let t = T::new();
    assert_eq!(t.string_value(None).to_string_value(), None);
}

#[test]
fn test_greaterThan_knownDouble_knownDouble_false() {
    let t = T::new();
    t.assert_greater_than(
        Some(t.bool_value(Some(false))),
        t.double_value(Some(1.0)),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_greaterThan_knownDouble_knownDouble_true() {
    let t = T::new();
    t.assert_greater_than(
        Some(t.bool_value(Some(true))),
        t.double_value(Some(2.0)),
        t.double_value(Some(1.0)),
    )
    .unwrap();
}

#[test]
fn test_greaterThan_knownDouble_knownInt_false() {
    let t = T::new();
    t.assert_greater_than(
        Some(t.bool_value(Some(false))),
        t.double_value(Some(1.0)),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_greaterThan_knownDouble_knownInt_true() {
    let t = T::new();
    t.assert_greater_than(
        Some(t.bool_value(Some(true))),
        t.double_value(Some(2.0)),
        t.int_value(Some(1)),
    )
    .unwrap();
}

#[test]
fn test_greaterThan_knownDouble_unknownDouble() {
    let t = T::new();
    t.assert_greater_than(
        Some(t.bool_value(None)),
        t.double_value(Some(1.0)),
        t.double_value(None),
    )
    .unwrap();
}

#[test]
fn test_greaterThan_knownDouble_unknownInt() {
    let t = T::new();
    t.assert_greater_than(
        Some(t.bool_value(None)),
        t.double_value(Some(1.0)),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_greaterThan_knownInt_knownInt_false() {
    let t = T::new();
    t.assert_greater_than(
        Some(t.bool_value(Some(false))),
        t.int_value(Some(1)),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_greaterThan_knownInt_knownInt_true() {
    let t = T::new();
    t.assert_greater_than(
        Some(t.bool_value(Some(true))),
        t.int_value(Some(2)),
        t.int_value(Some(1)),
    )
    .unwrap();
}

#[test]
fn test_greaterThan_knownInt_knownString() {
    let t = T::new();
    t.assert_greater_than(None, t.int_value(Some(1)), t.string_value(Some("2")))
        .unwrap();
}

#[test]
fn test_greaterThan_knownInt_unknownDouble() {
    let t = T::new();
    t.assert_greater_than(
        Some(t.bool_value(None)),
        t.int_value(Some(1)),
        t.double_value(None),
    )
    .unwrap();
}

#[test]
fn test_greaterThan_knownInt_unknownInt() {
    let t = T::new();
    t.assert_greater_than(
        Some(t.bool_value(None)),
        t.int_value(Some(1)),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_greaterThan_knownString_knownInt() {
    let t = T::new();
    t.assert_greater_than(None, t.string_value(Some("1")), t.int_value(Some(2)))
        .unwrap();
}

#[test]
fn test_greaterThan_unknownDouble_knownDouble() {
    let t = T::new();
    t.assert_greater_than(
        Some(t.bool_value(None)),
        t.double_value(None),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_greaterThan_unknownDouble_knownInt() {
    let t = T::new();
    t.assert_greater_than(
        Some(t.bool_value(None)),
        t.double_value(None),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_greaterThan_unknownInt_knownDouble() {
    let t = T::new();
    t.assert_greater_than(
        Some(t.bool_value(None)),
        t.int_value(None),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_greaterThan_unknownInt_knownInt() {
    let t = T::new();
    t.assert_greater_than(
        Some(t.bool_value(None)),
        t.int_value(None),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_greaterThanOrEqual_knownDouble_knownDouble_false() {
    let t = T::new();
    t.assert_greater_than_or_equal(
        Some(t.bool_value(Some(false))),
        t.double_value(Some(1.0)),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_greaterThanOrEqual_knownDouble_knownDouble_true() {
    let t = T::new();
    t.assert_greater_than_or_equal(
        Some(t.bool_value(Some(true))),
        t.double_value(Some(2.0)),
        t.double_value(Some(1.0)),
    )
    .unwrap();
}

#[test]
fn test_greaterThanOrEqual_knownDouble_knownInt_false() {
    let t = T::new();
    t.assert_greater_than_or_equal(
        Some(t.bool_value(Some(false))),
        t.double_value(Some(1.0)),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_greaterThanOrEqual_knownDouble_knownInt_true() {
    let t = T::new();
    t.assert_greater_than_or_equal(
        Some(t.bool_value(Some(true))),
        t.double_value(Some(2.0)),
        t.int_value(Some(1)),
    )
    .unwrap();
}

#[test]
fn test_greaterThanOrEqual_knownDouble_unknownDouble() {
    let t = T::new();
    t.assert_greater_than_or_equal(
        Some(t.bool_value(None)),
        t.double_value(Some(1.0)),
        t.double_value(None),
    )
    .unwrap();
}

#[test]
fn test_greaterThanOrEqual_knownDouble_unknownInt() {
    let t = T::new();
    t.assert_greater_than_or_equal(
        Some(t.bool_value(None)),
        t.double_value(Some(1.0)),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_greaterThanOrEqual_knownInt_knownInt_false() {
    let t = T::new();
    t.assert_greater_than_or_equal(
        Some(t.bool_value(Some(false))),
        t.int_value(Some(1)),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_greaterThanOrEqual_knownInt_knownInt_true() {
    let t = T::new();
    t.assert_greater_than_or_equal(
        Some(t.bool_value(Some(true))),
        t.int_value(Some(2)),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_greaterThanOrEqual_knownInt_knownString() {
    let t = T::new();
    t.assert_greater_than_or_equal(None, t.int_value(Some(1)), t.string_value(Some("2")))
        .unwrap();
}

#[test]
fn test_greaterThanOrEqual_knownInt_unknownDouble() {
    let t = T::new();
    t.assert_greater_than_or_equal(
        Some(t.bool_value(None)),
        t.int_value(Some(1)),
        t.double_value(None),
    )
    .unwrap();
}

#[test]
fn test_greaterThanOrEqual_knownInt_unknownInt() {
    let t = T::new();
    t.assert_greater_than_or_equal(
        Some(t.bool_value(None)),
        t.int_value(Some(1)),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_greaterThanOrEqual_knownString_knownInt() {
    let t = T::new();
    t.assert_greater_than_or_equal(None, t.string_value(Some("1")), t.int_value(Some(2)))
        .unwrap();
}

#[test]
fn test_greaterThanOrEqual_unknownDouble_knownDouble() {
    let t = T::new();
    t.assert_greater_than_or_equal(
        Some(t.bool_value(None)),
        t.double_value(None),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_greaterThanOrEqual_unknownDouble_knownInt() {
    let t = T::new();
    t.assert_greater_than_or_equal(
        Some(t.bool_value(None)),
        t.double_value(None),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_greaterThanOrEqual_unknownInt_knownDouble() {
    let t = T::new();
    t.assert_greater_than_or_equal(
        Some(t.bool_value(None)),
        t.int_value(None),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_greaterThanOrEqual_unknownInt_knownInt() {
    let t = T::new();
    t.assert_greater_than_or_equal(
        Some(t.bool_value(None)),
        t.int_value(None),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_hasKnownValue_bool_false() {
    let t = T::new();
    assert!(t.bool_value(Some(false)).has_known_value());
}

#[test]
fn test_hasKnownValue_bool_true() {
    let t = T::new();
    assert!(t.bool_value(Some(true)).has_known_value());
}

#[test]
fn test_hasKnownValue_bool_unknown() {
    let t = T::new();
    assert!(!t.bool_value(None).has_known_value());
}

#[test]
fn test_hasKnownValue_double_known() {
    let t = T::new();
    assert!(t.double_value(Some(2.3)).has_known_value());
}

#[test]
fn test_hasKnownValue_double_unknown() {
    let t = T::new();
    assert!(!t.double_value(None).has_known_value());
}

#[test]
fn test_hasKnownValue_int_known() {
    let t = T::new();
    assert!(t.int_value(Some(23)).has_known_value());
}

#[test]
fn test_hasKnownValue_int_unknown() {
    let t = T::new();
    assert!(!t.int_value(None).has_known_value());
}

#[test]
fn test_hasKnownValue_list_empty() {
    let t = T::new();
    assert!(t.list_value(t.tp().int_type(), vec![]).has_known_value());
}

#[test]
fn test_hasKnownValue_list_valid() {
    let t = T::new();
    assert!(
        t.list_value(t.tp().int_type(), vec![t.int_value(Some(23))])
            .has_known_value()
    );
}

#[test]
fn test_hasKnownValue_map_empty() {
    let t = T::new();
    assert!(
        t.map_value(t.tp().int_type(), t.tp().string_type(), vec![])
            .has_known_value()
    );
}

#[test]
fn test_hasKnownValue_map_valid() {
    let t = T::new();
    assert!(
        t.map_value(
            t.tp().string_type(),
            t.tp().string_type(),
            vec![t.string_value(Some("key")), t.string_value(Some("value"))]
        )
        .has_known_value()
    );
}

#[test]
fn test_hasKnownValue_null() {
    let t = T::new();
    assert!(t.null_value().has_known_value());
}

#[test]
fn test_hasKnownValue_string_known() {
    let t = T::new();
    assert!(t.string_value(Some("twenty-three")).has_known_value());
}

#[test]
fn test_hasKnownValue_string_unknown() {
    let t = T::new();
    assert!(!t.string_value(None).has_known_value());
}

#[test]
fn test_identical_bool_false() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(false)),
        t.bool_value(Some(false)),
        t.bool_value(Some(true)),
    );
}

#[test]
fn test_identical_bool_true() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(true)),
        t.bool_value(Some(true)),
        t.bool_value(Some(true)),
    );
}

#[test]
fn test_identical_bool_unknown() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(None),
        t.bool_value(None),
        t.bool_value(Some(false)),
    );
}

#[test]
fn test_identical_double_false() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(false)),
        t.double_value(Some(2.0)),
        t.double_value(Some(4.0)),
    );
}

#[test]
fn test_identical_double_true() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(true)),
        t.double_value(Some(2.0)),
        t.double_value(Some(2.0)),
    );
}

#[test]
fn test_identical_double_unknown() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(None),
        t.double_value(Some(1.0)),
        t.double_value(None),
    );
}

#[test]
fn test_identical_int_false() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(false)),
        t.int_value(Some(-5)),
        t.int_value(Some(5)),
    );
}

#[test]
fn test_identical_int_true() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(true)),
        t.int_value(Some(5)),
        t.int_value(Some(5)),
    );
}

#[test]
fn test_identical_int_unknown() {
    let t = T::new();
    t.assert_identical(t.bool_value(None), t.int_value(None), t.int_value(Some(3)));
}

#[test]
fn test_identical_intZero_doubleZero() {
    // Used in Flutter:
    // const bool kIsWeb = identical(0, 0.0);
    let t = T::new();
    t.assert_identical(
        t.bool_value(None),
        t.int_value(Some(0)),
        t.double_value(Some(0.0)),
    );
    t.assert_identical(
        t.bool_value(None),
        t.double_value(Some(0.0)),
        t.int_value(Some(0)),
    );
}

#[test]
fn test_identical_list_empty() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(true)),
        t.list_value(t.tp().int_type(), vec![]),
        t.list_value(t.tp().int_type(), vec![]),
    );
}

#[test]
fn test_identical_list_false_differentTypes() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(false)),
        t.list_value(t.tp().int_type(), vec![]),
        t.list_value(t.tp().double_type(), vec![]),
    );
}

#[test]
fn test_identical_list_false_differentValues() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(false)),
        t.list_value(t.tp().int_type(), vec![]),
        t.list_value(t.tp().int_type(), vec![t.int_value(Some(3))]),
    );
}

#[test]
fn test_identical_list_false_equalTypes_differentValues() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(false)),
        t.list_value(t.tp().int_type(), vec![t.int_value(Some(0))]),
        t.list_value(t.tp().int_type(), vec![t.int_value(Some(1))]),
    );
}

#[test]
fn test_identical_list_false_equalTypes_value_unknown() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(false)),
        t.list_value(t.tp().int_type(), vec![t.int_value(Some(0))]),
        t.list_value(t.tp().int_type(), vec![t.int_value(None)]),
    );
}

#[test]
fn test_identical_list_true_equalTypes_empty() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(true)),
        t.list_value(t.tp().int_type(), vec![]),
        t.list_value(t.tp().int_type(), vec![]),
    );
}

#[test]
fn test_identical_list_true_equalTypes_unknown_unknown() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(true)),
        t.list_value(t.tp().int_type(), vec![t.int_value(None)]),
        t.list_value(t.tp().int_type(), vec![t.int_value(None)]),
    );
}

#[test]
fn test_identical_list_true_equalTypesRuntime() {
    let t = T::new();
    let object_type = t.tp().object_type();
    t.assert_identical(
        t.bool_value(Some(true)),
        t.list_value(object_type, vec![]),
        t.list_value(t.future_or(object_type), vec![]),
    );
}

#[test]
fn test_identical_map_empty() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(true)),
        t.map_value(t.tp().int_type(), t.tp().string_type(), vec![]),
        t.map_value(t.tp().int_type(), t.tp().string_type(), vec![]),
    );
}

#[test]
fn test_identical_map_false_differentEntries() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(false)),
        t.map_value(t.tp().int_type(), t.tp().int_type(), vec![]),
        t.map_value(
            t.tp().int_type(),
            t.tp().int_type(),
            vec![t.int_value(Some(1)), t.int_value(Some(2))],
        ),
    );
}

#[test]
fn test_identical_map_false_differentTypes() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(false)),
        t.map_value(t.tp().bool_type(), t.tp().int_type(), vec![]),
        t.map_value(t.tp().int_type(), t.tp().int_type(), vec![]),
    );
    t.assert_identical(
        t.bool_value(Some(false)),
        t.map_value(t.tp().int_type(), t.tp().bool_type(), vec![]),
        t.map_value(t.tp().int_type(), t.tp().int_type(), vec![]),
    );
}

#[test]
fn test_identical_map_true_equalTypesRuntime() {
    let t = T::new();
    let object_type = t.tp().object_type();
    let int_type = t.tp().int_type();
    t.assert_identical(
        t.bool_value(Some(true)),
        t.map_value(int_type, object_type, vec![]),
        t.map_value(int_type, t.future_or(object_type), vec![]),
    );
}

#[test]
fn test_identical_null() {
    let t = T::new();
    t.assert_identical(t.bool_value(Some(true)), t.null_value(), t.null_value());
}

#[test]
fn test_identical_record_mixed_true() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(None),
        t.record_value(
            vec![t.int_value(Some(1))],
            vec![("a", t.int_value(Some(2)))],
        ),
        t.record_value(
            vec![t.int_value(Some(1))],
            vec![("a", t.int_value(Some(2)))],
        ),
    );
}

#[test]
fn test_identical_record_named_false_differentKeys() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(false)),
        t.record_value(vec![], vec![("a", t.int_value(Some(1)))]),
        t.record_value(vec![], vec![("b", t.int_value(Some(1)))]),
    );
}

#[test]
fn test_identical_record_named_false_differentKeysAndValues() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(false)),
        t.record_value(vec![], vec![("a", t.int_value(Some(1)))]),
        t.record_value(vec![], vec![("b", t.int_value(Some(2)))]),
    );
}

#[test]
fn test_identical_record_named_false_differentLength() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(false)),
        t.record_value(vec![], vec![("a", t.int_value(Some(1)))]),
        t.record_value(
            vec![],
            vec![("a", t.int_value(Some(1))), ("b", t.int_value(Some(1)))],
        ),
    );
}

#[test]
fn test_identical_record_named_false_differentValues() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(false)),
        t.record_value(vec![], vec![("a", t.int_value(Some(1)))]),
        t.record_value(vec![], vec![("a", t.int_value(Some(2)))]),
    );
}

#[test]
fn test_identical_record_named_false_value_unknown() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(false)),
        t.record_value(vec![], vec![("a", t.int_value(Some(1)))]),
        t.record_value(vec![], vec![("a", t.int_value(None))]),
    );
}

#[test]
fn test_identical_record_named_true() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(None),
        t.record_value(vec![], vec![("a", t.int_value(Some(1)))]),
        t.record_value(vec![], vec![("a", t.int_value(Some(1)))]),
    );
}

#[test]
fn test_identical_record_named_true_unknown_unknown() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(None),
        t.record_value(vec![], vec![("a", t.int_value(None))]),
        t.record_value(vec![], vec![("a", t.int_value(None))]),
    );
}

#[test]
fn test_identical_record_positional_false_differentLength() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(false)),
        t.record_value(
            vec![
                t.int_value(Some(1)),
                t.int_value(Some(2)),
                t.int_value(Some(3)),
            ],
            vec![],
        ),
        t.record_value(vec![t.int_value(Some(1)), t.int_value(Some(2))], vec![]),
    );
}

#[test]
fn test_identical_record_positional_false_differentOrder() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(false)),
        t.record_value(vec![t.int_value(Some(1)), t.int_value(Some(2))], vec![]),
        t.record_value(vec![t.int_value(Some(2)), t.int_value(Some(1))], vec![]),
    );
}

#[test]
fn test_identical_record_positional_false_value_unknown() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(false)),
        t.record_value(vec![t.int_value(Some(0)), t.int_value(Some(1))], vec![]),
        t.record_value(vec![t.int_value(Some(0)), t.int_value(None)], vec![]),
    );
}

#[test]
fn test_identical_record_positional_true() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(None),
        t.record_value(vec![t.int_value(Some(1)), t.int_value(Some(2))], vec![]),
        t.record_value(vec![t.int_value(Some(1)), t.int_value(Some(2))], vec![]),
    );
}

#[test]
fn test_identical_record_positional_true_unknown_unknown() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(None),
        t.record_value(vec![t.int_value(Some(0)), t.int_value(None)], vec![]),
        t.record_value(vec![t.int_value(Some(0)), t.int_value(None)], vec![]),
    );
}

#[test]
fn test_identical_string_false() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(false)),
        t.string_value(Some("abc")),
        t.string_value(Some("def")),
    );
}

#[test]
fn test_identical_string_true() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(true)),
        t.string_value(Some("abc")),
        t.string_value(Some("abc")),
    );
}

#[test]
fn test_identical_string_unknown() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(None),
        t.string_value(None),
        t.string_value(Some("def")),
    );
}

#[test]
fn test_identical_Type_functionType() {
    let t = T::new();
    // The type of `Object.toString`: `String Function()`.
    let ctx = t.ctx();
    let to_string_type = ctx.intern(TypeKind::Function(FunctionTypeData {
        type_params: ctx.intern_list(&[]),
        params: ctx.intern_list(&[]),
        required_positional: 0,
        ret: t.tp().string_type(),
        nullability: Nullability::None,
        alias: None,
    }));
    t.assert_identical(
        t.bool_value(Some(true)),
        t.type_value(to_string_type),
        t.type_value(to_string_type),
    );
    t.assert_identical(
        t.bool_value(Some(false)),
        t.type_value(to_string_type),
        t.type_value(t.make_nullable(to_string_type)),
    );
}

#[test]
fn test_identical_Type_interfaceType() {
    let t = T::new();
    let int_type = t.tp().int_type();
    t.assert_identical(
        t.bool_value(Some(true)),
        t.type_value(int_type),
        t.type_value(int_type),
    );
    t.assert_identical(
        t.bool_value(Some(false)),
        t.type_value(int_type),
        t.type_value(t.make_nullable(int_type)),
    );
    t.assert_identical(
        t.bool_value(Some(false)),
        t.type_value(int_type),
        t.type_value(t.tp().num_type()),
    );
    t.assert_identical(
        t.bool_value(Some(true)),
        t.type_value(t.future_or(t.tp().object_type())),
        t.type_value(t.tp().object_type()),
    );
}

#[test]
fn test_identical_Type_notType() {
    let t = T::new();
    t.assert_identical(
        t.bool_value(Some(false)),
        t.type_value(t.tp().int_type()),
        t.int_value(Some(0)),
    );
}

#[test]
fn test_integerDivide_infinity_knownDouble() {
    let t = T::new();
    t.assert_integer_divide(
        None,
        t.double_value(Some(f64::INFINITY)),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_integerDivide_infinity_knownInt() {
    let t = T::new();
    t.assert_integer_divide(
        None,
        t.double_value(Some(f64::INFINITY)),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_integerDivide_knownDouble_knownDouble() {
    let t = T::new();
    t.assert_integer_divide(
        Some(t.int_value(Some(3))),
        t.double_value(Some(6.0)),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_integerDivide_knownDouble_knownInt() {
    let t = T::new();
    t.assert_integer_divide(
        Some(t.int_value(Some(3))),
        t.double_value(Some(6.0)),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_integerDivide_knownDouble_unknownDouble() {
    let t = T::new();
    t.assert_integer_divide(
        Some(t.int_value(None)),
        t.double_value(Some(6.0)),
        t.double_value(None),
    )
    .unwrap();
}

#[test]
fn test_integerDivide_knownDouble_unknownInt() {
    let t = T::new();
    t.assert_integer_divide(
        Some(t.int_value(None)),
        t.double_value(Some(6.0)),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_integerDivide_knownInt_knownDoubleZero() {
    let t = T::new();
    t.assert_integer_divide(None, t.int_value(Some(6)), t.double_value(Some(0.0)))
        .unwrap();
}

#[test]
fn test_integerDivide_knownInt_knownInt() {
    let t = T::new();
    t.assert_integer_divide(
        Some(t.int_value(Some(3))),
        t.int_value(Some(6)),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_integerDivide_knownInt_knownString() {
    let t = T::new();
    t.assert_integer_divide(None, t.int_value(Some(6)), t.string_value(Some("2")))
        .unwrap();
}

#[test]
fn test_integerDivide_knownInt_unknownDouble() {
    let t = T::new();
    t.assert_integer_divide(
        Some(t.int_value(None)),
        t.int_value(Some(6)),
        t.double_value(None),
    )
    .unwrap();
}

#[test]
fn test_integerDivide_knownInt_unknownInt() {
    let t = T::new();
    t.assert_integer_divide(
        Some(t.int_value(None)),
        t.int_value(Some(6)),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_integerDivide_knownInt_zero() {
    let t = T::new();
    t.assert_integer_divide(None, t.int_value(Some(2)), t.int_value(Some(0)))
        .unwrap();
}

#[test]
fn test_integerDivide_knownString_knownInt() {
    let t = T::new();
    t.assert_integer_divide(None, t.string_value(Some("6")), t.int_value(Some(2)))
        .unwrap();
}

#[test]
fn test_integerDivide_NaN_knownDouble() {
    let t = T::new();
    t.assert_integer_divide(
        None,
        t.double_value(Some(f64::NAN)),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_integerDivide_NaN_knownInt() {
    let t = T::new();
    t.assert_integer_divide(None, t.double_value(Some(f64::NAN)), t.int_value(Some(2)))
        .unwrap();
}

#[test]
fn test_integerDivide_negativeInfinity_knownDouble() {
    let t = T::new();
    t.assert_integer_divide(
        None,
        t.double_value(Some(f64::NEG_INFINITY)),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_integerDivide_negativeInfinity_knownInt() {
    let t = T::new();
    t.assert_integer_divide(
        None,
        t.double_value(Some(f64::NEG_INFINITY)),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_integerDivide_unknownDouble_knownDouble() {
    let t = T::new();
    t.assert_integer_divide(
        Some(t.int_value(None)),
        t.double_value(None),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_integerDivide_unknownDouble_knownInt() {
    let t = T::new();
    t.assert_integer_divide(
        Some(t.int_value(None)),
        t.double_value(None),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_integerDivide_unknownInt_knownDouble() {
    let t = T::new();
    t.assert_integer_divide(
        Some(t.int_value(None)),
        t.int_value(None),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_integerDivide_unknownInt_knownInt() {
    let t = T::new();
    t.assert_integer_divide(
        Some(t.int_value(None)),
        t.int_value(None),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_isBoolNumStringOrNull_bool_false() {
    let t = T::new();
    assert!(t.bool_value(Some(false)).is_bool_num_string_or_null());
}

#[test]
fn test_isBoolNumStringOrNull_bool_true() {
    let t = T::new();
    assert!(t.bool_value(Some(true)).is_bool_num_string_or_null());
}

#[test]
fn test_isBoolNumStringOrNull_bool_unknown() {
    let t = T::new();
    assert!(t.bool_value(None).is_bool_num_string_or_null());
}

#[test]
fn test_isBoolNumStringOrNull_double_known() {
    let t = T::new();
    assert!(t.double_value(Some(2.3)).is_bool_num_string_or_null());
}

#[test]
fn test_isBoolNumStringOrNull_double_unknown() {
    let t = T::new();
    assert!(t.double_value(None).is_bool_num_string_or_null());
}

#[test]
fn test_isBoolNumStringOrNull_int_known() {
    let t = T::new();
    assert!(t.int_value(Some(23)).is_bool_num_string_or_null());
}

#[test]
fn test_isBoolNumStringOrNull_int_unknown() {
    let t = T::new();
    assert!(t.int_value(None).is_bool_num_string_or_null());
}

#[test]
fn test_isBoolNumStringOrNull_list() {
    let t = T::new();
    assert!(
        !t.list_value(t.tp().int_type(), vec![])
            .is_bool_num_string_or_null()
    );
}

#[test]
fn test_isBoolNumStringOrNull_null() {
    let t = T::new();
    assert!(t.null_value().is_bool_num_string_or_null());
}

#[test]
fn test_isBoolNumStringOrNull_string_known() {
    let t = T::new();
    assert!(
        t.string_value(Some("twenty-three"))
            .is_bool_num_string_or_null()
    );
}

#[test]
fn test_isBoolNumStringOrNull_string_unknown() {
    let t = T::new();
    assert!(t.string_value(None).is_bool_num_string_or_null());
}

#[test]
fn test_lessThan_knownDouble_knownDouble_false() {
    let t = T::new();
    t.assert_less_than(
        Some(t.bool_value(Some(false))),
        t.double_value(Some(2.0)),
        t.double_value(Some(1.0)),
    )
    .unwrap();
}

#[test]
fn test_lessThan_knownDouble_knownDouble_true() {
    let t = T::new();
    t.assert_less_than(
        Some(t.bool_value(Some(true))),
        t.double_value(Some(1.0)),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_lessThan_knownDouble_knownInt_false() {
    let t = T::new();
    t.assert_less_than(
        Some(t.bool_value(Some(false))),
        t.double_value(Some(2.0)),
        t.int_value(Some(1)),
    )
    .unwrap();
}

#[test]
fn test_lessThan_knownDouble_knownInt_true() {
    let t = T::new();
    t.assert_less_than(
        Some(t.bool_value(Some(true))),
        t.double_value(Some(1.0)),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_lessThan_knownDouble_unknownDouble() {
    let t = T::new();
    t.assert_less_than(
        Some(t.bool_value(None)),
        t.double_value(Some(1.0)),
        t.double_value(None),
    )
    .unwrap();
}

#[test]
fn test_lessThan_knownDouble_unknownInt() {
    let t = T::new();
    t.assert_less_than(
        Some(t.bool_value(None)),
        t.double_value(Some(1.0)),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_lessThan_knownInt_knownInt_false() {
    let t = T::new();
    t.assert_less_than(
        Some(t.bool_value(Some(false))),
        t.int_value(Some(2)),
        t.int_value(Some(1)),
    )
    .unwrap();
}

#[test]
fn test_lessThan_knownInt_knownInt_true() {
    let t = T::new();
    t.assert_less_than(
        Some(t.bool_value(Some(true))),
        t.int_value(Some(1)),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_lessThan_knownInt_knownString() {
    let t = T::new();
    t.assert_less_than(None, t.int_value(Some(1)), t.string_value(Some("2")))
        .unwrap();
}

#[test]
fn test_lessThan_knownInt_unknownDouble() {
    let t = T::new();
    t.assert_less_than(
        Some(t.bool_value(None)),
        t.int_value(Some(1)),
        t.double_value(None),
    )
    .unwrap();
}

#[test]
fn test_lessThan_knownInt_unknownInt() {
    let t = T::new();
    t.assert_less_than(
        Some(t.bool_value(None)),
        t.int_value(Some(1)),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_lessThan_knownString_knownInt() {
    let t = T::new();
    t.assert_less_than(None, t.string_value(Some("1")), t.int_value(Some(2)))
        .unwrap();
}

#[test]
fn test_lessThan_unknownDouble_knownDouble() {
    let t = T::new();
    t.assert_less_than(
        Some(t.bool_value(None)),
        t.double_value(None),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_lessThan_unknownDouble_knownInt() {
    let t = T::new();
    t.assert_less_than(
        Some(t.bool_value(None)),
        t.double_value(None),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_lessThan_unknownInt_knownDouble() {
    let t = T::new();
    t.assert_less_than(
        Some(t.bool_value(None)),
        t.int_value(None),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_lessThan_unknownInt_knownInt() {
    let t = T::new();
    t.assert_less_than(
        Some(t.bool_value(None)),
        t.int_value(None),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_lessThanOrEqual_knownDouble_knownDouble_false() {
    let t = T::new();
    t.assert_less_than_or_equal(
        Some(t.bool_value(Some(false))),
        t.double_value(Some(2.0)),
        t.double_value(Some(1.0)),
    )
    .unwrap();
}

#[test]
fn test_lessThanOrEqual_knownDouble_knownDouble_true() {
    let t = T::new();
    t.assert_less_than_or_equal(
        Some(t.bool_value(Some(true))),
        t.double_value(Some(1.0)),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_lessThanOrEqual_knownDouble_knownInt_false() {
    let t = T::new();
    t.assert_less_than_or_equal(
        Some(t.bool_value(Some(false))),
        t.double_value(Some(2.0)),
        t.int_value(Some(1)),
    )
    .unwrap();
}

#[test]
fn test_lessThanOrEqual_knownDouble_knownInt_true() {
    let t = T::new();
    t.assert_less_than_or_equal(
        Some(t.bool_value(Some(true))),
        t.double_value(Some(1.0)),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_lessThanOrEqual_knownDouble_unknownDouble() {
    let t = T::new();
    t.assert_less_than_or_equal(
        Some(t.bool_value(None)),
        t.double_value(Some(1.0)),
        t.double_value(None),
    )
    .unwrap();
}

#[test]
fn test_lessThanOrEqual_knownDouble_unknownInt() {
    let t = T::new();
    t.assert_less_than_or_equal(
        Some(t.bool_value(None)),
        t.double_value(Some(1.0)),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_lessThanOrEqual_knownInt_knownInt_false() {
    let t = T::new();
    t.assert_less_than_or_equal(
        Some(t.bool_value(Some(false))),
        t.int_value(Some(2)),
        t.int_value(Some(1)),
    )
    .unwrap();
}

#[test]
fn test_lessThanOrEqual_knownInt_knownInt_true() {
    let t = T::new();
    t.assert_less_than_or_equal(
        Some(t.bool_value(Some(true))),
        t.int_value(Some(1)),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_lessThanOrEqual_knownInt_knownString() {
    let t = T::new();
    t.assert_less_than_or_equal(None, t.int_value(Some(1)), t.string_value(Some("2")))
        .unwrap();
}

#[test]
fn test_lessThanOrEqual_knownInt_unknownDouble() {
    let t = T::new();
    t.assert_less_than_or_equal(
        Some(t.bool_value(None)),
        t.int_value(Some(1)),
        t.double_value(None),
    )
    .unwrap();
}

#[test]
fn test_lessThanOrEqual_knownInt_unknownInt() {
    let t = T::new();
    t.assert_less_than_or_equal(
        Some(t.bool_value(None)),
        t.int_value(Some(1)),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_lessThanOrEqual_knownString_knownInt() {
    let t = T::new();
    t.assert_less_than_or_equal(None, t.string_value(Some("1")), t.int_value(Some(2)))
        .unwrap();
}

#[test]
fn test_lessThanOrEqual_unknownDouble_knownDouble() {
    let t = T::new();
    t.assert_less_than_or_equal(
        Some(t.bool_value(None)),
        t.double_value(None),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_lessThanOrEqual_unknownDouble_knownInt() {
    let t = T::new();
    t.assert_less_than_or_equal(
        Some(t.bool_value(None)),
        t.double_value(None),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_lessThanOrEqual_unknownInt_knownDouble() {
    let t = T::new();
    t.assert_less_than_or_equal(
        Some(t.bool_value(None)),
        t.int_value(None),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_lessThanOrEqual_unknownInt_knownInt() {
    let t = T::new();
    t.assert_less_than_or_equal(
        Some(t.bool_value(None)),
        t.int_value(None),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_logicalAnd_false_false() {
    let t = T::new();
    t.assert_lazy_and(
        Some(t.bool_value(Some(false))),
        t.bool_value(Some(false)),
        t.bool_value(Some(false)),
    )
    .unwrap();
}

#[test]
fn test_logicalAnd_false_null() {
    let t = T::new();
    t.assert_lazy_and(
        Some(t.bool_value(Some(false))),
        t.bool_value(Some(false)),
        t.null_value(),
    )
    .unwrap();
}

#[test]
fn test_logicalAnd_false_string() {
    let t = T::new();
    t.assert_lazy_and(
        Some(t.bool_value(Some(false))),
        t.bool_value(Some(false)),
        t.string_value(Some("false")),
    )
    .unwrap();
}

#[test]
fn test_logicalAnd_false_true() {
    let t = T::new();
    t.assert_lazy_and(
        Some(t.bool_value(Some(false))),
        t.bool_value(Some(false)),
        t.bool_value(Some(true)),
    )
    .unwrap();
}

#[test]
fn test_logicalAnd_null_false() {
    let t = T::new();
    assert!(
        t.assert_lazy_and(
            Some(t.bool_value(Some(false))),
            t.null_value(),
            t.bool_value(Some(false))
        )
        .is_err()
    );
}

#[test]
fn test_logicalAnd_null_true() {
    let t = T::new();
    assert!(
        t.assert_lazy_and(
            Some(t.bool_value(Some(false))),
            t.null_value(),
            t.bool_value(Some(true))
        )
        .is_err()
    );
}

#[test]
fn test_logicalAnd_string_false() {
    let t = T::new();
    assert!(
        t.assert_lazy_and(
            Some(t.bool_value(Some(false))),
            t.string_value(Some("true")),
            t.bool_value(Some(false))
        )
        .is_err()
    );
}

#[test]
fn test_logicalAnd_string_true() {
    let t = T::new();
    assert!(
        t.assert_lazy_and(
            Some(t.bool_value(Some(false))),
            t.string_value(Some("false")),
            t.bool_value(Some(true))
        )
        .is_err()
    );
}

#[test]
fn test_logicalAnd_true_false() {
    let t = T::new();
    t.assert_lazy_and(
        Some(t.bool_value(Some(false))),
        t.bool_value(Some(true)),
        t.bool_value(Some(false)),
    )
    .unwrap();
}

#[test]
fn test_logicalAnd_true_null() {
    let t = T::new();
    t.assert_lazy_and(None, t.bool_value(Some(true)), t.null_value())
        .unwrap();
}

#[test]
fn test_logicalAnd_true_string() {
    let t = T::new();
    assert!(
        t.assert_lazy_and(
            Some(t.bool_value(Some(false))),
            t.bool_value(Some(true)),
            t.string_value(Some("true"))
        )
        .is_err()
    );
}

#[test]
fn test_logicalAnd_true_true() {
    let t = T::new();
    t.assert_lazy_and(
        Some(t.bool_value(Some(true))),
        t.bool_value(Some(true)),
        t.bool_value(Some(true)),
    )
    .unwrap();
}

#[test]
fn test_logicalNot_false() {
    let t = T::new();
    t.assert_logical_not(Some(t.bool_value(Some(true))), t.bool_value(Some(false)))
        .unwrap();
}

#[test]
fn test_logicalNot_null() {
    let t = T::new();
    t.assert_logical_not(None, t.null_value()).unwrap();
}

#[test]
fn test_logicalNot_string() {
    let t = T::new();
    assert!(
        t.assert_logical_not(Some(t.bool_value(Some(true))), t.string_value(None))
            .is_err()
    );
}

#[test]
fn test_logicalNot_true() {
    let t = T::new();
    t.assert_logical_not(Some(t.bool_value(Some(false))), t.bool_value(Some(true)))
        .unwrap();
}

#[test]
fn test_logicalNot_unknown() {
    let t = T::new();
    t.assert_logical_not(Some(t.bool_value(None)), t.bool_value(None))
        .unwrap();
}

#[test]
fn test_logicalOr_false_false() {
    let t = T::new();
    t.assert_lazy_or(
        Some(t.bool_value(Some(false))),
        t.bool_value(Some(false)),
        t.bool_value(Some(false)),
    )
    .unwrap();
}

#[test]
fn test_logicalOr_false_null() {
    let t = T::new();
    t.assert_lazy_or(None, t.bool_value(Some(false)), t.null_value())
        .unwrap();
}

#[test]
fn test_logicalOr_false_string() {
    let t = T::new();
    assert!(
        t.assert_lazy_or(
            Some(t.bool_value(Some(false))),
            t.bool_value(Some(false)),
            t.string_value(Some("false"))
        )
        .is_err()
    );
}

#[test]
fn test_logicalOr_false_true() {
    let t = T::new();
    t.assert_lazy_or(
        Some(t.bool_value(Some(true))),
        t.bool_value(Some(false)),
        t.bool_value(Some(true)),
    )
    .unwrap();
}

#[test]
fn test_logicalOr_null_false() {
    let t = T::new();
    assert!(
        t.assert_lazy_or(
            Some(t.bool_value(Some(false))),
            t.null_value(),
            t.bool_value(Some(false))
        )
        .is_err()
    );
}

#[test]
fn test_logicalOr_null_true() {
    let t = T::new();
    assert!(
        t.assert_lazy_or(
            Some(t.bool_value(Some(true))),
            t.null_value(),
            t.bool_value(Some(true))
        )
        .is_err()
    );
}

#[test]
fn test_logicalOr_string_false() {
    let t = T::new();
    assert!(
        t.assert_lazy_or(
            Some(t.bool_value(Some(false))),
            t.string_value(Some("true")),
            t.bool_value(Some(false))
        )
        .is_err()
    );
}

#[test]
fn test_logicalOr_string_true() {
    let t = T::new();
    assert!(
        t.assert_lazy_or(
            Some(t.bool_value(Some(true))),
            t.string_value(Some("false")),
            t.bool_value(Some(true))
        )
        .is_err()
    );
}

#[test]
fn test_logicalOr_true_false() {
    let t = T::new();
    t.assert_lazy_or(
        Some(t.bool_value(Some(true))),
        t.bool_value(Some(true)),
        t.bool_value(Some(false)),
    )
    .unwrap();
}

#[test]
fn test_logicalOr_true_null() {
    let t = T::new();
    t.assert_lazy_or(
        Some(t.bool_value(Some(true))),
        t.bool_value(Some(true)),
        t.null_value(),
    )
    .unwrap();
}

#[test]
fn test_logicalOr_true_string() {
    let t = T::new();
    t.assert_lazy_or(
        Some(t.bool_value(Some(true))),
        t.bool_value(Some(true)),
        t.string_value(Some("true")),
    )
    .unwrap();
}

#[test]
fn test_logicalOr_true_true() {
    let t = T::new();
    t.assert_lazy_or(
        Some(t.bool_value(Some(true))),
        t.bool_value(Some(true)),
        t.bool_value(Some(true)),
    )
    .unwrap();
}

#[test]
fn test_logicalShiftRight_knownInt_knownInt() {
    let t = T::new();
    t.assert_logical_shift_right(
        Some(t.int_value(Some(16))),
        t.int_value(Some(64)),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_logicalShiftRight_knownInt_unknownInt() {
    let t = T::new();
    t.assert_logical_shift_right(
        Some(t.int_value(None)),
        t.int_value(Some(64)),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_logicalShiftRight_unknownInt_knownInt() {
    let t = T::new();
    t.assert_logical_shift_right(
        Some(t.int_value(None)),
        t.int_value(None),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_logicalShiftRight_unknownInt_unknownInt() {
    let t = T::new();
    t.assert_logical_shift_right(
        Some(t.int_value(None)),
        t.int_value(None),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_minus_knownDouble_knownDouble() {
    let t = T::new();
    t.assert_minus(
        Some(t.double_value(Some(1.0))),
        t.double_value(Some(4.0)),
        t.double_value(Some(3.0)),
    )
    .unwrap();
}

#[test]
fn test_minus_knownDouble_knownInt() {
    let t = T::new();
    t.assert_minus(
        Some(t.double_value(Some(1.0))),
        t.double_value(Some(4.0)),
        t.int_value(Some(3)),
    )
    .unwrap();
}

#[test]
fn test_minus_knownDouble_unknownDouble() {
    let t = T::new();
    t.assert_minus(
        Some(t.double_value(None)),
        t.double_value(Some(4.0)),
        t.double_value(None),
    )
    .unwrap();
}

#[test]
fn test_minus_knownDouble_unknownInt() {
    let t = T::new();
    t.assert_minus(
        Some(t.double_value(None)),
        t.double_value(Some(4.0)),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_minus_knownInt_knownInt() {
    let t = T::new();
    t.assert_minus(
        Some(t.int_value(Some(1))),
        t.int_value(Some(4)),
        t.int_value(Some(3)),
    )
    .unwrap();
}

#[test]
fn test_minus_knownInt_knownString() {
    let t = T::new();
    t.assert_minus(None, t.int_value(Some(4)), t.string_value(Some("3")))
        .unwrap();
}

#[test]
fn test_minus_knownInt_unknownDouble() {
    let t = T::new();
    t.assert_minus(
        Some(t.double_value(None)),
        t.int_value(Some(4)),
        t.double_value(None),
    )
    .unwrap();
}

#[test]
fn test_minus_knownInt_unknownInt() {
    let t = T::new();
    t.assert_minus(
        Some(t.int_value(None)),
        t.int_value(Some(4)),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_minus_knownString_knownInt() {
    let t = T::new();
    t.assert_minus(None, t.string_value(Some("4")), t.int_value(Some(3)))
        .unwrap();
}

#[test]
fn test_minus_unknownDouble_knownDouble() {
    let t = T::new();
    t.assert_minus(
        Some(t.double_value(None)),
        t.double_value(None),
        t.double_value(Some(3.0)),
    )
    .unwrap();
}

#[test]
fn test_minus_unknownDouble_knownInt() {
    let t = T::new();
    t.assert_minus(
        Some(t.double_value(None)),
        t.double_value(None),
        t.int_value(Some(3)),
    )
    .unwrap();
}

#[test]
fn test_minus_unknownInt_knownDouble() {
    let t = T::new();
    t.assert_minus(
        Some(t.double_value(None)),
        t.int_value(None),
        t.double_value(Some(3.0)),
    )
    .unwrap();
}

#[test]
fn test_minus_unknownInt_knownInt() {
    let t = T::new();
    t.assert_minus(
        Some(t.int_value(None)),
        t.int_value(None),
        t.int_value(Some(3)),
    )
    .unwrap();
}

#[test]
fn test_negated_double_known() {
    let t = T::new();
    t.assert_negated(Some(t.double_value(Some(2.0))), t.double_value(Some(-2.0)))
        .unwrap();
}

#[test]
fn test_negated_double_unknown() {
    let t = T::new();
    t.assert_negated(Some(t.double_value(None)), t.double_value(None))
        .unwrap();
}

#[test]
fn test_negated_int_known() {
    let t = T::new();
    t.assert_negated(Some(t.int_value(Some(-3))), t.int_value(Some(3)))
        .unwrap();
}

#[test]
fn test_negated_int_unknown() {
    let t = T::new();
    t.assert_negated(Some(t.int_value(None)), t.int_value(None))
        .unwrap();
}

#[test]
fn test_negated_string() {
    let t = T::new();
    t.assert_negated(None, t.string_value(None)).unwrap();
}

#[test]
fn test_notEqual_bool_false() {
    let t = T::new();
    t.assert_not_equal(
        Some(t.bool_value(Some(false))),
        t.bool_value(Some(true)),
        t.bool_value(Some(true)),
    )
    .unwrap();
}

#[test]
fn test_notEqual_bool_true() {
    let t = T::new();
    t.assert_not_equal(
        Some(t.bool_value(Some(true))),
        t.bool_value(Some(false)),
        t.bool_value(Some(true)),
    )
    .unwrap();
}

#[test]
fn test_notEqual_bool_unknown() {
    let t = T::new();
    t.assert_not_equal(
        Some(t.bool_value(None)),
        t.bool_value(None),
        t.bool_value(Some(false)),
    )
    .unwrap();
}

#[test]
fn test_notEqual_double_double_false() {
    let t = T::new();
    t.assert_not_equal(
        Some(t.bool_value(Some(false))),
        t.double_value(Some(2.0)),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_notEqual_double_double_false_language219() {
    let mut t = T::new();
    t.set_language_2_19();
    t.assert_not_equal(
        Some(t.bool_value(Some(false))),
        t.double_value(Some(2.0)),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_notEqual_double_double_true() {
    let t = T::new();
    t.assert_not_equal(
        Some(t.bool_value(Some(true))),
        t.double_value(Some(2.0)),
        t.double_value(Some(4.0)),
    )
    .unwrap();
}

#[test]
fn test_notEqual_double_double_true_language219() {
    let mut t = T::new();
    t.set_language_2_19();
    t.assert_not_equal(
        Some(t.bool_value(Some(true))),
        t.double_value(Some(2.0)),
        t.double_value(Some(4.0)),
    )
    .unwrap();
}

#[test]
fn test_notEqual_double_unknown() {
    let t = T::new();
    t.assert_not_equal(
        Some(t.bool_value(None)),
        t.double_value(Some(1.0)),
        t.double_value(None),
    )
    .unwrap();
}

#[test]
fn test_notEqual_double_unknown_language219() {
    let mut t = T::new();
    t.set_language_2_19();
    t.assert_not_equal(
        Some(t.bool_value(None)),
        t.double_value(Some(1.0)),
        t.double_value(None),
    )
    .unwrap();
}

#[test]
fn test_notEqual_int_false() {
    let t = T::new();
    t.assert_not_equal(
        Some(t.bool_value(Some(false))),
        t.int_value(Some(5)),
        t.int_value(Some(5)),
    )
    .unwrap();
}

#[test]
fn test_notEqual_int_true() {
    let t = T::new();
    t.assert_not_equal(
        Some(t.bool_value(Some(true))),
        t.int_value(Some(-5)),
        t.int_value(Some(5)),
    )
    .unwrap();
}

#[test]
fn test_notEqual_int_unknown() {
    let t = T::new();
    t.assert_not_equal(
        Some(t.bool_value(None)),
        t.int_value(None),
        t.int_value(Some(3)),
    )
    .unwrap();
}

#[test]
fn test_notEqual_null() {
    let t = T::new();
    t.assert_not_equal(
        Some(t.bool_value(Some(false))),
        t.null_value(),
        t.null_value(),
    )
    .unwrap();
}

#[test]
fn test_notEqual_string_false() {
    let t = T::new();
    t.assert_not_equal(
        Some(t.bool_value(Some(false))),
        t.string_value(Some("abc")),
        t.string_value(Some("abc")),
    )
    .unwrap();
}

#[test]
fn test_notEqual_string_true() {
    let t = T::new();
    t.assert_not_equal(
        Some(t.bool_value(Some(true))),
        t.string_value(Some("abc")),
        t.string_value(Some("def")),
    )
    .unwrap();
}

#[test]
fn test_notEqual_string_unknown() {
    let t = T::new();
    t.assert_not_equal(
        Some(t.bool_value(None)),
        t.string_value(None),
        t.string_value(Some("def")),
    )
    .unwrap();
}

#[test]
fn test_performToString_bool_false() {
    let t = T::new();
    t.assert_perform_to_string(
        Some(t.string_value(Some("false"))),
        t.bool_value(Some(false)),
    )
    .unwrap();
}

#[test]
fn test_performToString_bool_true() {
    let t = T::new();
    t.assert_perform_to_string(Some(t.string_value(Some("true"))), t.bool_value(Some(true)))
        .unwrap();
}

#[test]
fn test_performToString_bool_unknown() {
    let t = T::new();
    t.assert_perform_to_string(Some(t.string_value(None)), t.bool_value(None))
        .unwrap();
}

#[test]
fn test_performToString_double_known() {
    let t = T::new();
    t.assert_perform_to_string(Some(t.string_value(Some("2.0"))), t.double_value(Some(2.0)))
        .unwrap();
}

#[test]
fn test_performToString_double_unknown() {
    let t = T::new();
    t.assert_perform_to_string(Some(t.string_value(None)), t.double_value(None))
        .unwrap();
}

#[test]
fn test_performToString_int_known() {
    let t = T::new();
    t.assert_perform_to_string(Some(t.string_value(Some("5"))), t.int_value(Some(5)))
        .unwrap();
}

#[test]
fn test_performToString_int_unknown() {
    let t = T::new();
    t.assert_perform_to_string(Some(t.string_value(None)), t.int_value(None))
        .unwrap();
}

#[test]
fn test_performToString_null() {
    let t = T::new();
    t.assert_perform_to_string(Some(t.string_value(Some("null"))), t.null_value())
        .unwrap();
}

#[test]
fn test_performToString_string_known() {
    let t = T::new();
    t.assert_perform_to_string(
        Some(t.string_value(Some("abc"))),
        t.string_value(Some("abc")),
    )
    .unwrap();
}

#[test]
fn test_performToString_string_unknown() {
    let t = T::new();
    t.assert_perform_to_string(Some(t.string_value(None)), t.string_value(None))
        .unwrap();
}

#[test]
fn test_remainder_knownDouble_knownDouble() {
    let t = T::new();
    t.assert_remainder(
        Some(t.double_value(Some(1.0))),
        t.double_value(Some(7.0)),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_remainder_knownDouble_knownInt() {
    let t = T::new();
    t.assert_remainder(
        Some(t.double_value(Some(1.0))),
        t.double_value(Some(7.0)),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_remainder_knownDouble_unknownDouble() {
    let t = T::new();
    t.assert_remainder(
        Some(t.double_value(None)),
        t.double_value(Some(7.0)),
        t.double_value(None),
    )
    .unwrap();
}

#[test]
fn test_remainder_knownDouble_unknownInt() {
    let t = T::new();
    t.assert_remainder(
        Some(t.double_value(None)),
        t.double_value(Some(6.0)),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_remainder_knownInt_knownInt() {
    let t = T::new();
    t.assert_remainder(
        Some(t.int_value(Some(1))),
        t.int_value(Some(7)),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_remainder_knownInt_knownInt_zero() {
    let t = T::new();
    t.assert_remainder(None, t.int_value(Some(7)), t.int_value(Some(0)))
        .unwrap();
}

#[test]
fn test_remainder_knownInt_knownString() {
    let t = T::new();
    t.assert_remainder(None, t.int_value(Some(7)), t.string_value(Some("2")))
        .unwrap();
}

#[test]
fn test_remainder_knownInt_unknownDouble() {
    let t = T::new();
    t.assert_remainder(
        Some(t.double_value(None)),
        t.int_value(Some(7)),
        t.double_value(None),
    )
    .unwrap();
}

#[test]
fn test_remainder_knownInt_unknownInt() {
    let t = T::new();
    t.assert_remainder(
        Some(t.int_value(None)),
        t.int_value(Some(7)),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_remainder_knownString_knownInt() {
    let t = T::new();
    t.assert_remainder(None, t.string_value(Some("7")), t.int_value(Some(2)))
        .unwrap();
}

#[test]
fn test_remainder_unknownDouble_knownDouble() {
    let t = T::new();
    t.assert_remainder(
        Some(t.double_value(None)),
        t.double_value(None),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_remainder_unknownDouble_knownInt() {
    let t = T::new();
    t.assert_remainder(
        Some(t.double_value(None)),
        t.double_value(None),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_remainder_unknownInt_knownDouble() {
    let t = T::new();
    t.assert_remainder(
        Some(t.double_value(None)),
        t.int_value(None),
        t.double_value(Some(2.0)),
    )
    .unwrap();
}

#[test]
fn test_remainder_unknownInt_knownInt() {
    let t = T::new();
    t.assert_remainder(
        Some(t.int_value(None)),
        t.int_value(None),
        t.int_value(Some(2)),
    )
    .unwrap();
}

#[test]
fn test_shiftLeft_knownInt_knownInt() {
    let t = T::new();
    t.assert_shift_left(
        Some(t.int_value(Some(48))),
        t.int_value(Some(6)),
        t.int_value(Some(3)),
    )
    .unwrap();
}

#[test]
fn test_shiftLeft_knownInt_knownInt_negative() {
    let t = T::new();
    t.assert_shift_left(None, t.int_value(Some(1)), t.int_value(Some(-1)))
        .unwrap();
}

#[test]
fn test_shiftLeft_knownInt_knownString() {
    let t = T::new();
    t.assert_shift_left(None, t.int_value(Some(6)), t.string_value(None))
        .unwrap();
}

#[test]
fn test_shiftLeft_knownInt_tooLarge() {
    let t = T::new();
    t.assert_shift_left(
        Some(t.int_value(None)),
        t.int_value(Some(6)),
        DartObjectImpl::new(
            &t.ts(),
            t.tp().int_type(),
            InstanceState::Int(IntState::new(Some(LONG_MAX_VALUE))),
        ),
    )
    .unwrap();
}

#[test]
fn test_shiftLeft_knownInt_unknownInt() {
    let t = T::new();
    t.assert_shift_left(
        Some(t.int_value(None)),
        t.int_value(Some(6)),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_shiftLeft_knownString_knownInt() {
    let t = T::new();
    t.assert_shift_left(None, t.string_value(None), t.int_value(Some(3)))
        .unwrap();
}

#[test]
fn test_shiftLeft_unknownInt_knownInt() {
    let t = T::new();
    t.assert_shift_left(
        Some(t.int_value(None)),
        t.int_value(None),
        t.int_value(Some(3)),
    )
    .unwrap();
}

#[test]
fn test_shiftLeft_unknownInt_unknownInt() {
    let t = T::new();
    t.assert_shift_left(
        Some(t.int_value(None)),
        t.int_value(None),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_shiftRight_knownInt_knownInt() {
    let t = T::new();
    t.assert_shift_right(
        Some(t.int_value(Some(6))),
        t.int_value(Some(48)),
        t.int_value(Some(3)),
    )
    .unwrap();
}

#[test]
fn test_shiftRight_knownInt_knownInt_negative() {
    let t = T::new();
    t.assert_shift_right(None, t.int_value(Some(1)), t.int_value(Some(-1)))
        .unwrap();
}

#[test]
fn test_shiftRight_knownInt_knownString() {
    let t = T::new();
    t.assert_shift_right(None, t.int_value(Some(48)), t.string_value(None))
        .unwrap();
}

#[test]
fn test_shiftRight_knownInt_tooLarge() {
    let t = T::new();
    t.assert_shift_right(
        Some(t.int_value(None)),
        t.int_value(Some(48)),
        DartObjectImpl::new(
            &t.ts(),
            t.tp().int_type(),
            InstanceState::Int(IntState::new(Some(LONG_MAX_VALUE))),
        ),
    )
    .unwrap();
}

#[test]
fn test_shiftRight_knownInt_unknownInt() {
    let t = T::new();
    t.assert_shift_right(
        Some(t.int_value(None)),
        t.int_value(Some(48)),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_shiftRight_knownString_knownInt() {
    let t = T::new();
    t.assert_shift_right(None, t.string_value(None), t.int_value(Some(3)))
        .unwrap();
}

#[test]
fn test_shiftRight_unknownInt_knownInt() {
    let t = T::new();
    t.assert_shift_right(
        Some(t.int_value(None)),
        t.int_value(None),
        t.int_value(Some(3)),
    )
    .unwrap();
}

#[test]
fn test_shiftRight_unknownInt_unknownInt() {
    let t = T::new();
    t.assert_shift_right(
        Some(t.int_value(None)),
        t.int_value(None),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_stringLength_int() {
    let t = T::new();
    assert!(
        t.assert_string_length(Some(t.int_value(None)), t.int_value(Some(0)))
            .is_err()
    );
}

#[test]
fn test_stringLength_knownString() {
    let t = T::new();
    t.assert_string_length(Some(t.int_value(Some(3))), t.string_value(Some("abc")))
        .unwrap();
}

#[test]
fn test_stringLength_unknownString() {
    let t = T::new();
    t.assert_string_length(Some(t.int_value(None)), t.string_value(None))
        .unwrap();
}

#[test]
fn test_times_knownDouble_knownDouble() {
    let t = T::new();
    t.assert_times(
        Some(t.double_value(Some(6.0))),
        t.double_value(Some(2.0)),
        t.double_value(Some(3.0)),
    )
    .unwrap();
}

#[test]
fn test_times_knownDouble_knownInt() {
    let t = T::new();
    t.assert_times(
        Some(t.double_value(Some(6.0))),
        t.double_value(Some(2.0)),
        t.int_value(Some(3)),
    )
    .unwrap();
}

#[test]
fn test_times_knownDouble_unknownDouble() {
    let t = T::new();
    t.assert_times(
        Some(t.double_value(None)),
        t.double_value(Some(2.0)),
        t.double_value(None),
    )
    .unwrap();
}

#[test]
fn test_times_knownDouble_unknownInt() {
    let t = T::new();
    t.assert_times(
        Some(t.double_value(None)),
        t.double_value(Some(2.0)),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_times_knownInt_knownInt() {
    let t = T::new();
    t.assert_times(
        Some(t.int_value(Some(6))),
        t.int_value(Some(2)),
        t.int_value(Some(3)),
    )
    .unwrap();
}

#[test]
fn test_times_knownInt_knownString() {
    let t = T::new();
    t.assert_times(None, t.int_value(Some(2)), t.string_value(Some("3")))
        .unwrap();
}

#[test]
fn test_times_knownInt_unknownDouble() {
    let t = T::new();
    t.assert_times(
        Some(t.double_value(None)),
        t.int_value(Some(2)),
        t.double_value(None),
    )
    .unwrap();
}

#[test]
fn test_times_knownInt_unknownInt() {
    let t = T::new();
    t.assert_times(
        Some(t.int_value(None)),
        t.int_value(Some(2)),
        t.int_value(None),
    )
    .unwrap();
}

#[test]
fn test_times_knownString_knownInt() {
    let t = T::new();
    t.assert_times(None, t.string_value(Some("2")), t.int_value(Some(3)))
        .unwrap();
}

#[test]
fn test_times_unknownDouble_knownDouble() {
    let t = T::new();
    t.assert_times(
        Some(t.double_value(None)),
        t.double_value(None),
        t.double_value(Some(3.0)),
    )
    .unwrap();
}

#[test]
fn test_times_unknownDouble_knownInt() {
    let t = T::new();
    t.assert_times(
        Some(t.double_value(None)),
        t.double_value(None),
        t.int_value(Some(3)),
    )
    .unwrap();
}

#[test]
fn test_times_unknownInt_knownDouble() {
    let t = T::new();
    t.assert_times(
        Some(t.double_value(None)),
        t.int_value(None),
        t.double_value(Some(3.0)),
    )
    .unwrap();
}

#[test]
fn test_times_unknownInt_knownInt() {
    let t = T::new();
    t.assert_times(
        Some(t.int_value(None)),
        t.int_value(None),
        t.int_value(Some(3)),
    )
    .unwrap();
}

#[test]
fn test_toRecordValue_notARecord() {
    let t = T::new();
    let list = t.list_value(
        t.tp().bool_type(),
        vec![t.bool_value(Some(true)), t.bool_value(Some(false))],
    );
    assert!(list.to_record_value().is_none());
}

#[test]
fn test_toRecordValue_null() {
    let t = T::new();
    assert!(t.null_value().to_record_value().is_none());
}

#[test]
fn test_toRecordValue_record() {
    let t = T::new();
    let constant = t.record_value(
        vec![t.int_value(Some(1))],
        vec![("bool", t.bool_value(Some(true)))],
    );
    let (positional, named) = constant.to_record_value().unwrap();
    assert_eq!(positional.len(), 1);
    t.assert_eq(&positional[0], &t.int_value(Some(1)));
    assert_eq!(named.len(), 1);
    t.assert_eq(&named["bool"], &t.bool_value(Some(true)));
}
