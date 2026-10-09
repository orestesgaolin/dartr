// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_localVariable_call_tooManyTypeArgs).

void foo<T>(T a) {}

void bar() {
  void Function(int) fn = foo;
  fn.call<int>;
//       ^^^^^
// [diag.wrongNumberOfTypeArgumentsFunction] The type of this function is 'void Function(int)', which has 0 type parameters, but 1 type arguments were given.
}
