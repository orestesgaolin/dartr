// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceMethod_targetOfFunctionCall_class_self).

extension on Function {
  void m() {}
}
class A {
  void foo<T>(T a) {}

  bar() {
    foo<int>.m();
  }
}
