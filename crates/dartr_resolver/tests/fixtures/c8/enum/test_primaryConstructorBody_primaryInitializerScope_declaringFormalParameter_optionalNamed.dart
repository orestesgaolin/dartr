// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_primaryConstructorBody_primaryInitializerScope_declaringFormalParameter_optionalNamed).

enum A({final x = false}) {
  v(x: true);
  this : assert(x);
}
