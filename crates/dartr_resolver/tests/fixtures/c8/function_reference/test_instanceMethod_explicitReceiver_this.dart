// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceMethod_explicitReceiver_this).

class A {
  void foo<T>(T a) {}

  bar() {
    this.foo<int>;
  }
}
