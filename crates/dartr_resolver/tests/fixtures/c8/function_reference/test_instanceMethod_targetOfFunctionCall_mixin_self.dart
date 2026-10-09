// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceMethod_targetOfFunctionCall_mixin_self).

extension on Function {
  void m() {}
}
mixin M {
  void foo<T>(T a) {}
  void bar() {
    foo<int>.m();
  }
}
