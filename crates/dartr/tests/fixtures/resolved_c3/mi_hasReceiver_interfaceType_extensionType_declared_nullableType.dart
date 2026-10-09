// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_interfaceType_extensionType_declared_nullableType).

extension type A(int it) {
  int foo() => 0;
}

void f(A? a) {
  a.foo();
//  ^^^
// [diag.uncheckedMethodInvocationOfNullableValue] The method 'foo' can't be unconditionally invoked because the receiver can be 'null'.
}
