// Dart source: pkg/analyzer/test/src/dart/element/is_known_test.dart

//! `IsKnownTest`.

use dartr_element::TypeId;
use dartr_typesystem::test_support::*;
use dartr_typesystem::type_schema::{is_known, is_unknown};

/// `_checkKnown(type)`.
fn check_known(t: &TypeSystemTest, ty: TypeId) {
    assert!(is_known(&t.ctx(), ty), "isKnown({})", t.display(ty));
    assert!(!is_unknown(&t.ctx(), ty), "isUnknown({})", t.display(ty));
}

/// `_checkUnknown(type)`.
fn check_unknown(t: &TypeSystemTest, ty: TypeId) {
    assert!(!is_known(&t.ctx(), ty), "isKnown({})", t.display(ty));
    assert!(is_unknown(&t.ctx(), ty), "isUnknown({})", t.display(ty));
}

#[test]
fn dynamic() {
    let t = TypeSystemTest::new();
    check_known(&t, t.parse_type("dynamic"));
}

#[test]
fn function() {
    let t = TypeSystemTest::new();
    check_known(&t, t.parse_type("void Function()"));

    check_unknown(&t, t.parse_type("UnknownInferredType Function()"));

    check_unknown(&t, t.parse_type("void Function(UnknownInferredType)"));
}

#[test]
fn interface() {
    let t = TypeSystemTest::new();
    check_known(&t, t.parse_type("int"));
    check_known(&t, t.parse_type("List<int>"));
    check_unknown(&t, t.parse_type("List<UnknownInferredType>"));
}

#[test]
fn never() {
    let t = TypeSystemTest::new();
    check_known(&t, t.parse_type("Never"));
}

#[test]
fn null() {
    let t = TypeSystemTest::new();
    check_known(&t, t.parse_type("Null"));
}

#[test]
fn record() {
    let t = TypeSystemTest::new();
    check_known(&t, t.parse_record_type("(int,)"));

    check_unknown(&t, t.parse_type("(UnknownInferredType,)"));

    check_known(&t, t.parse_record_type("({int x})"));

    check_unknown(&t, t.parse_type("({UnknownInferredType x})"));
}

#[test]
fn unknown_inferred_type() {
    let t = TypeSystemTest::new();
    check_unknown(&t, t.parse_type("UnknownInferredType"));
}

#[test]
fn void() {
    let t = TypeSystemTest::new();
    check_known(&t, t.parse_type("void"));
}
