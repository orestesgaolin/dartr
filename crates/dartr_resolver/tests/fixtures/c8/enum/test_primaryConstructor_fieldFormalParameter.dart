// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_primaryConstructor_fieldFormalParameter).

enum A(int this.a) {
  v(0);
  final int a;
}
