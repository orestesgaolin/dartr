// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_constructor_generic_noTypeArguments_unnamed).

enum E<T> {
  v(42);
  const E(T a);
}
