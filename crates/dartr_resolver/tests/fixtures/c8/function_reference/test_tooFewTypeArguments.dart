// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_tooFewTypeArguments).

class A {
  void foo<T, U>(T a, U b) {}

  bar() {
    foo<int>;
//     ^^^^^
// [diag.wrongNumberOfTypeArgumentsElement] The method 'foo' is declared with 2 type parameters, but 1 type arguments are given.
  }
}
