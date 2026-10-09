// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_otherExpression).

void f(void Function<T>(T a) foo, void Function<T>(T a) bar) {
  (1 == 2 ? foo : bar)<int>;
}
