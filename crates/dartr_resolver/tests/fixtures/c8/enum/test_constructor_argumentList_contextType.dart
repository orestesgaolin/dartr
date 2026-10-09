// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_constructor_argumentList_contextType).

enum E {
  v([]);
  const E(List<int> a);
}
