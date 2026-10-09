// Ported from pkg/analyzer/test/src/dart/resolution/constructor_field_initializer_test.dart (ConstructorFieldInitializerResolutionTest.test_functionExpressionInvocation_expressionBody).

class A {
  final int x;
  A(int a) : x = (() => a + 1)();
}
