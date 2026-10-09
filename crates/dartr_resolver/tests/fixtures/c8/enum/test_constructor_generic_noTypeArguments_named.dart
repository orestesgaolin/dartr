// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_constructor_generic_noTypeArguments_named).

enum E<T> {
  v.named(42);
  const E.named(T a);
}
