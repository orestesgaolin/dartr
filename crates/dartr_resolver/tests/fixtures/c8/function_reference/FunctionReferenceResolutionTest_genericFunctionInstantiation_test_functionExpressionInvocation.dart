// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest_genericFunctionInstantiation.test_functionExpressionInvocation).

void Function(int) foo(void Function<T>(T) Function() f) {
  return (f)();
}
