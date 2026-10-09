// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceMethod_explicitReceiver_field).

class A {
  void foo<T>(T a) {}
}

class B {
  A a;
  B(this.a);
  bar() {
    a.foo<int>;
  }
}
