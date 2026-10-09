// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_localVariable).

void bar(void Function<T>(T a) foo) {
  foo<int>;
}
