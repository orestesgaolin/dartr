// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceMethod_targetOfFunctionCall_extensionType).

extension on Function {
  void bar() {}
}
extension type A(int it) {
  void foo<T>(T a) {}
  void f() {
    foo<int>.bar();
  }
}
