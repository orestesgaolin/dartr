// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest_genericFunctionInstantiation.test_methodInvocation).

class C {
  late void Function<T>(T) f;
  void Function<T>(T) m() => f;
}

void Function(int) foo(C c) {
  return c.m();
}
