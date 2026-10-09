// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest_genericFunctionInstantiation.test_awaitExpression).

Future<void Function(int)> foo(Future<void Function<T>(T)> f) async {
  return await f;
}
