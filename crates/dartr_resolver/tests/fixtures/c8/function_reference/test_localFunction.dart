// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_localFunction).

void bar() {
  void foo<T>(T a) {}

  foo<int>;
}
