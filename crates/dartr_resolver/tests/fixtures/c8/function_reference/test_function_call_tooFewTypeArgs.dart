// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_function_call_tooFewTypeArgs).

void foo<T, U>(T a, U b) {}

void bar() {
  foo.call<int>;
//        ^^^^^
// [diag.wrongNumberOfTypeArgumentsFunction] The type of this function is 'void Function<T, U>(T, U)', which has 2 type parameters, but 1 type arguments were given.
}
