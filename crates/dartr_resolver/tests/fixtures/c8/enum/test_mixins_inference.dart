// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_mixins_inference).

mixin M1<T> {}
mixin M2<T> on M1<T> {}
enum E with M1<int>, M2 {
  v;
}
