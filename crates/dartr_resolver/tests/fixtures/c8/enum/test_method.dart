// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_method).

enum E<T> {
  v;
  int foo<U>(T t, U u) => 0;
}
