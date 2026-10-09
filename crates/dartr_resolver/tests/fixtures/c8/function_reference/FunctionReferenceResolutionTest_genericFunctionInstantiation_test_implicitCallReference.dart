// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest_genericFunctionInstantiation.test_implicitCallReference).

class C {
  void call<T>(T a) {}
}

void Function(int) foo(C c) {
  return c;
}
