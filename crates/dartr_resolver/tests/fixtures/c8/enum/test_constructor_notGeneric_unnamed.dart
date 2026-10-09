// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_constructor_notGeneric_unnamed).

enum E {
  v(42);
  const E(int a);
}
