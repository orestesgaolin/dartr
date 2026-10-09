// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_primaryConstructor_scopes).

const foo = 0;
enum A<@foo T>([@foo int x = foo]) {
  v;
  static const foo = 1;
}
