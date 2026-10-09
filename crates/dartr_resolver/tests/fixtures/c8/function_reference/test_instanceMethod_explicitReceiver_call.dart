// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceMethod_explicitReceiver_call).

class C {
  void foo<T>(T a) {}
}

void bar(C c) {
  c.foo.call<int>;
}
