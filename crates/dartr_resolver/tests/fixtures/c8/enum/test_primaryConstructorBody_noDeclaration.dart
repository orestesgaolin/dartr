// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_primaryConstructorBody_noDeclaration).

enum A/*(bool x, int y)*/ {
  v();
  this : assert(x) {
//^^^^
// [diag.primaryConstructorBodyWithoutDeclaration] A primary constructor body requires a primary constructor declaration.
//              ^
// [diag.undefinedIdentifier] Undefined name 'x'.
    y;
//  ^
// [diag.undefinedIdentifier] Undefined name 'y'.
  }
}
