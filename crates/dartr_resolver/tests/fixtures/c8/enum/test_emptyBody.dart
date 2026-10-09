// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_emptyBody).

enum E;
//   ^
// [diag.enumWithoutConstants] The enum must have at least one enum constant.
