// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceMethod_call).

class C {
  void foo<T>(T a) {}

  void bar() {
    foo.call<int>;
  }
}
