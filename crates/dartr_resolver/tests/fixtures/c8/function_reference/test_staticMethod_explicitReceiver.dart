// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_staticMethod_explicitReceiver).

class A {
  static void foo<T>(T a) {}
}

bar() {
  A.foo<int>;
}
