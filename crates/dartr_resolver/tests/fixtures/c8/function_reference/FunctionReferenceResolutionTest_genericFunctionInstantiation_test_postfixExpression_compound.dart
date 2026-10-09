// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest_genericFunctionInstantiation.test_postfixExpression_compound).

extension on void Function<T>(T) {
  void Function<T>(T) operator +(int i) {
    return this;
  }
}

void Function(int) foo(void Function<T>(T) f) {
  return f++;
}
