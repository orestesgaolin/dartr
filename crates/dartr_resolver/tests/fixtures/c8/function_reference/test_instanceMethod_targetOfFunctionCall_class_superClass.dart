// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceMethod_targetOfFunctionCall_class_superClass).

extension on Function {
  void m() {}
}
class A {
  void foo<T>(T a) {}
}
class B extends A {
  bar() {
    foo<int>.m();
  }
}
