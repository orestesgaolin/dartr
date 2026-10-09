// Ported from pkg/analyzer/test/src/dart/resolution/enum_test.dart (EnumDeclarationResolutionTest.test_primaryConstructor_declaringFormalParameter_functionTyped_final).

enum A(final int a(String x)) { v(foo) }
int foo(String _) => 0;
