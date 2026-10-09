// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_topLevelFunction_targetOfFunctionCall).

void foo<T>(T arg) {}

extension on Function {
  void m() {}
}
void bar() {
  foo<int>.m();
}
