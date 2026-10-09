// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_function_call).

void foo<T>(T a) {}

void bar() {
  foo.call<int>;
}
