// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceMethod_targetOfFunctionCall_enum_self).

extension on Function {
  void bar() {}
}
enum A {
  v;
  void foo<T>(T a) {}
  void f() {
    foo<int>.bar();
  }
}
