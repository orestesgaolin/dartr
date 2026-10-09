// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_primaryConstructor_formalParameters_bodyScope_type).

enum A(int x) {
//     ^^^
// [diag.notAType] int isn't a type.
  v(0);
  static const String int = '';
}
