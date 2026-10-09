// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_implicitCallTearoff_extensionType).

extension type A(int it) {
  void call() {}
}

void g(Function f) {}

void f(A a) {
  g(a);
}
