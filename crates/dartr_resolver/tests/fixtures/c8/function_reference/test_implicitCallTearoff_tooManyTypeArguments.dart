// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_implicitCallTearoff_tooManyTypeArguments).

class C {
  int call(int t) => t;
}

foo() {
  C()<int>;
//   ^^^^^
// [diag.wrongNumberOfTypeArgumentsElement] The method 'call' is declared with 0 type parameters, but 1 type arguments are given.
}
