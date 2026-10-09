// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_implicitCallTearoff).

class C {
  T call<T>(T t) => t;
}

foo() {
  C()<int>;
}
