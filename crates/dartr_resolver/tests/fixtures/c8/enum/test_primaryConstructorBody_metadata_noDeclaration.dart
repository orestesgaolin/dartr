// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_primaryConstructorBody_metadata_noDeclaration).

enum E {
  v;
  @deprecated
  this;
//^^^^
// [diag.primaryConstructorBodyWithoutDeclaration] A primary constructor body requires a primary constructor declaration.
}
