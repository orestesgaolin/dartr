// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_constructor_argumentList_namedType).

enum E {
  v(<void Function(double)>[]);
  const E(Object a);
}
