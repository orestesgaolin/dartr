// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_primaryConstructor_typeParameters).

enum E<T extends U, U extends num>(T t, U u) {
  v(0, 0);
}
