// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_nonGenericFunction).

class A {
  void foo() {}

  bar() {
    foo<int>;
//     ^^^^^
// [diag.wrongNumberOfTypeArgumentsElement] The method 'foo' is declared with 0 type parameters, but 1 type arguments are given.
  }
}
