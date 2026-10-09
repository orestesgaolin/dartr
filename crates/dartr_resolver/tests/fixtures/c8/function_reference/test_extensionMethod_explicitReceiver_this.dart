// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_extensionMethod_explicitReceiver_this).

class A {}

extension E on A {
  void foo<T>(T a) {}

  bar() {
    this.foo<int>;
  }
}
