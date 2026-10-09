// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceMethod_explicitReceiver_topLevelVariable).

class A {
  void foo<T>(T a) {}
}
var a = A();

void bar() {
  a.foo<int>;
}
