// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_interfaceTypeQ_defined_extension).

class A {
  void foo() {}
}

extension E on A {
  void foo() {}
}

void f(A? a) {
  a.foo();
//  ^^^
// [diag.uncheckedMethodInvocationOfNullableValue] The method 'foo' can't be unconditionally invoked because the receiver can be 'null'.
}
