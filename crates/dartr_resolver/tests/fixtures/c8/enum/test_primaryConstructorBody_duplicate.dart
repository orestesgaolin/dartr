// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_primaryConstructorBody_duplicate).

enum A(bool x, bool y) {
  v(true, true);
  this : assert(x) {
//                 ^
// [diag.constPrimaryConstructorWithBlockBody] The body part of a constant primary constructor can't have a block body.
    y;
  }
  this : assert(!x) {
//^^^^
// [diag.multiplePrimaryConstructorBodyDeclarations] Only one primary constructor body declaration is allowed.
    !y;
  }
}
