// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest_genericFunctionInstantiation.test_cascadeExpression).

void Function(int) foo(void Function<T>(T) f) {
  return f..toString();
}
