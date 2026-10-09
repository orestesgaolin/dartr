// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest_genericFunctionInstantiation.test_binaryExpression).

class C {
  void Function<T>(T) operator +(int i) {
    return <T>(T a) {};
  }
}

void Function(int) foo(C c) {
  return c + 1;
}
