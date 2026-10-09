// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceMethod_targetOfFunctionCall_enum_mixin).

extension on Function {
  void bar() {}
}
mixin A {
  void foo<T>(T a) {}
}
enum B with A {
  v;
  void f() {
    foo<int>.bar();
  }
}
