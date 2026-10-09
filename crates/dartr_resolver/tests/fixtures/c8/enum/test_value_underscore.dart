// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_value_underscore).

enum E { _ }

void f() {
  E._.index;
}
