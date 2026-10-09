// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_primaryInitializerScope_fieldInitializer_instance_declaringFormal).

enum A(final int foo) {
  v(0);
  final bar = foo;
}
