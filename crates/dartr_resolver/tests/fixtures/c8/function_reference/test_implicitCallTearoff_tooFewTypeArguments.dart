// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_implicitCallTearoff_tooFewTypeArguments).

class C {
  void call<T, U>(T t, U u) {}
}

foo() {
  C()<int>;
//   ^^^^^
// [diag.wrongNumberOfTypeArgumentsElement] The method 'call' is declared with 2 type parameters, but 1 type arguments are given.
}
