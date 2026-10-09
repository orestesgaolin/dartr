// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_primaryConstructorBody_primaryInitializerScope_fieldFormalParameter).

enum A(this.x) {
  v(true);
  final bool x;
  this : assert(x);
}
