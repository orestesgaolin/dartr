// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceMethod_explicitReceiver_otherExpression).

class A {
  void foo<T>(T a) {}
}

void f(A? a, A b) {
  (a ?? b).foo<int>;
}
