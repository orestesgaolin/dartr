// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_extensionMethod).

class A {}

extension E on A {
  void foo<T>(T a) {}

  bar() {
    foo<int>;
  }
}
