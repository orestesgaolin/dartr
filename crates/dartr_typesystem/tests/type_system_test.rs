// Dart source: pkg/analyzer/test/src/dart/element/type_system_test.dart

use dartr_element::TypeId;
use dartr_typesystem::test_support::*;

fn assert_not_valid(t: &TypeSystemTest, ty: TypeId) {
    assert!(!t.type_system().is_valid_extension_type_superinterface(ty));
}

fn assert_valid(t: &TypeSystemTest, ty: TypeId) {
    assert!(t.type_system().is_valid_extension_type_superinterface(ty));
}

mod is_valid_extension_type_superinterface_test {
    use super::*;

    #[test]
    fn function_type() {
        let t = TypeSystemTest::new();
        assert_not_valid(&t, t.parse_type("void Function()"));
    }

    #[test]
    fn interface_type() {
        let t = TypeSystemTest::new();
        assert_valid(&t, t.parse_type("num"));
    }

    #[test]
    fn interface_type_extension_type() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            extension_types: strs(&["extension type A(int it)"]),
            ..LibrarySpec::test()
        });
        assert_valid(&t, t.parse_type("A"));
    }

    #[test]
    fn interface_type_function() {
        let t = TypeSystemTest::new();
        assert_not_valid(&t, t.parse_type("Function"));
    }

    #[test]
    fn interface_type_future_or() {
        let t = TypeSystemTest::new();
        assert_not_valid(&t, t.parse_type("FutureOr<int>"));
    }

    #[test]
    fn interface_type_null() {
        let t = TypeSystemTest::new();
        assert_not_valid(&t, t.parse_type("Null"));
    }

    #[test]
    fn interface_type_nullable() {
        let t = TypeSystemTest::new();
        assert_not_valid(&t, t.parse_type("num?"));
    }

    #[test]
    fn interface_type_record() {
        let t = TypeSystemTest::new();
        assert_not_valid(&t, t.parse_type("Record"));
    }

    #[test]
    fn record_type() {
        let t = TypeSystemTest::new();
        assert_not_valid(&t, t.parse_type("(int, String)"));
    }

    #[test]
    fn top_type() {
        let t = TypeSystemTest::new();
        assert_not_valid(&t, t.parse_type("dynamic"));
        assert_not_valid(&t, t.parse_type("void"));
        assert_not_valid(&t, t.parse_type("Object?"));
    }

    #[test]
    fn type_parameter_type() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            assert_not_valid(&t, scope.parse_type("T"));
        });
    }
}
