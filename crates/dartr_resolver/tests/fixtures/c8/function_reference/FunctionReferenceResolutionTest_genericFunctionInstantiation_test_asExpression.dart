// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest_genericFunctionInstantiation.test_asExpression).

void Function(int) foo(void Function<T>(T) f) {
  return (f as dynamic) as void Function<T>(T);
}
