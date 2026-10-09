// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_emptyBody_language310).

// @dart = 3.10
enum E;
//   ^
// [diag.enumWithoutConstants] The enum must have at least one enum constant.
//    ^
// [diag.experimentNotEnabled] This requires the 'primary-constructors' language feature to be enabled.
