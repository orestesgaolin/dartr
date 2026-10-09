// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_tooManyTypeArguments).

class A {
  void foo<T>(T a) {}

  bar() {
    foo<int, int>;
//     ^^^^^^^^^^
// [diag.wrongNumberOfTypeArgumentsElement] The method 'foo' is declared with 1 type parameters, but 2 type arguments are given.
  }
}
