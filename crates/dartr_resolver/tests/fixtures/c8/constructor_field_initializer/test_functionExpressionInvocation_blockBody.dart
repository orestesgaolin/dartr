// Ported from pkg/analyzer/test/src/dart/resolution/constructor_field_initializer_test.dart (ConstructorFieldInitializerResolutionTest.test_functionExpressionInvocation_blockBody).

class A {
  final x;
  A(int a) : x = (() {return a + 1;})();
}
