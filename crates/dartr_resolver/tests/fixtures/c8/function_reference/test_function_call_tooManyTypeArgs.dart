// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_function_call_tooManyTypeArgs).

void foo(String a) {}

void bar() {
  foo.call<int>;
//        ^^^^^
// [diag.wrongNumberOfTypeArgumentsFunction] The type of this function is 'void Function(String)', which has 0 type parameters, but 1 type arguments were given.
}
