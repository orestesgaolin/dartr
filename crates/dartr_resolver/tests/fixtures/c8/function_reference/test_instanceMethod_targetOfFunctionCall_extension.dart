// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceMethod_targetOfFunctionCall_extension).

extension on Function {
  void m() {}
}
extension E on int {
  void foo<T>(T a) {}
  void bar() {
    foo<int>.m();
  }
}
