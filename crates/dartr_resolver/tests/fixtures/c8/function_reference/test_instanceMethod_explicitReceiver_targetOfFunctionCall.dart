// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceMethod_explicitReceiver_targetOfFunctionCall).

extension on Function {
  void m() {}
}
class A {
  void foo<T>(T a) {}
}

bar(A a) {
  a.foo<int>.m();
}
