// Dart source: pkg/analyzer/test/src/dart/element/assignable_test.dart

use dartr_element::TypeId;
use dartr_typesystem::test_support::*;

/// `analysisContext.analysisOptions.strictCasts`: false in the default
/// analysis options of the test context.
const STRICT_CASTS: bool = false;

fn is_assignable(t: &TypeSystemTest, from: TypeId, to: TypeId) {
    assert!(t.type_system().is_assignable_to(from, to, STRICT_CASTS));
}

fn is_not_assignable(t: &TypeSystemTest, from: TypeId, to: TypeId) {
    assert!(!t.type_system().is_assignable_to(from, to, STRICT_CASTS));
}

#[test]
fn dynamic_type() {
    let t = TypeSystemTest::new();
    is_assignable(&t, t.parse_type("dynamic"), t.parse_type("dynamic"));
    is_assignable(&t, t.parse_type("dynamic"), t.parse_type("InvalidType"));
    is_assignable(&t, t.parse_type("dynamic"), t.parse_type("int"));
}

#[test]
fn interface_type() {
    let t = TypeSystemTest::new();
    is_assignable(&t, t.parse_type("int"), t.parse_type("num"));
    is_assignable(&t, t.parse_type("double"), t.parse_type("num"));

    is_not_assignable(&t, t.parse_type("num"), t.parse_type("int"));
}

#[test]
fn invalid_type() {
    let t = TypeSystemTest::new();
    is_assignable(&t, t.parse_type("InvalidType"), t.parse_type("InvalidType"));
    is_assignable(&t, t.parse_type("InvalidType"), t.parse_type("dynamic"));
    is_assignable(&t, t.parse_type("InvalidType"), t.parse_type("int"));
}
