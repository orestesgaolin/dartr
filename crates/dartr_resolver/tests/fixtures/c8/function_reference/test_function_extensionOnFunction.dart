// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_function_extensionOnFunction).

void foo<T>(T a) {}

void bar() {
  foo.m<int>;
}

extension on Function {
  void m<T>(T t) {}
}
