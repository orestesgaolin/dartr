// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_implicitCallTearoff_class_staticGetter).

class C {
  static const v = C();
  const C();
  T call<T>(T t) => t;
}

void f() {
  C.v<int>;
}
