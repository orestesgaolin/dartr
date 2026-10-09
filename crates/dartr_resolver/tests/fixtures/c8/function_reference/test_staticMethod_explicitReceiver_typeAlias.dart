// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_staticMethod_explicitReceiver_typeAlias).

class A {
  static void foo<T>(T a) {}
}
typedef TA = A;

bar() {
  TA.foo<int>;
}
