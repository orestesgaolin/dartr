// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceMethod_targetOfFunctionCall_mixin_constraint).

extension on Function {
  void m() {}
}
class A {
  void foo<T>(T a) {}
}
mixin M on A {
  void bar() {
    foo<int>.m();
  }
}
