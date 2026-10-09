// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_primaryConstructorBody_primaryInitializerScope_declaringFormalParameter_requiredPositional).

enum A(final bool a) {
  v(true);
  this : assert(a);
}
