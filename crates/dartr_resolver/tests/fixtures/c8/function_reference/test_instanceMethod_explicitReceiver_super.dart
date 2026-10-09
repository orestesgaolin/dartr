// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceMethod_explicitReceiver_super).

class A {
  void foo<T>(T a) {}
}
class B extends A {
  bar() {
    super.foo<int>;
  }
}
