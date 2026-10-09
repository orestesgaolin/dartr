// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_primaryInitializerScope_fieldInitializer_static).

enum A(int foo) {
  v(0);
  static var bar = foo;
//                 ^^^
// [diag.undefinedIdentifier] Undefined name 'foo'.
}
