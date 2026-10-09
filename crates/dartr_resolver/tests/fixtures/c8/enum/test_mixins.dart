// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_mixins).

mixin M {}
enum E with M {
  v;
}
