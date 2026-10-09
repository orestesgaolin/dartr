// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_getter).

enum E<T> {
  v;
  T get foo => throw 0;
}
