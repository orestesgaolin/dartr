// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_primaryConstructorBody_metadata).

enum E(int a) {
  v(0);
  @deprecated
  this;
}
