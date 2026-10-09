// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest_genericFunctionInstantiation.test_indexExpression).

void Function(int) foo(List<void Function<T>(T)> f) {
  return f[0];
}
