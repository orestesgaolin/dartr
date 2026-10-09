// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest_genericFunctionInstantiation.test_assignmentExpression).

late void Function<T>(T) g;
void Function(int) foo(void Function<T>(T) f) {
  return g = f;
}
