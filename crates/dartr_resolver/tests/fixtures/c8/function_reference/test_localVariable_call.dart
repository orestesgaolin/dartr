// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_localVariable_call).

void foo<T>(T a) {}

void bar() {
  var fn = foo;
  fn.call<int>;
}
