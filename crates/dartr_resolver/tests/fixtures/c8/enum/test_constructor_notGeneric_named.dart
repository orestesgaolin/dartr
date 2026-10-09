// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_constructor_notGeneric_named).

enum E {
  v.named(42);
  const E.named(int a);
}
