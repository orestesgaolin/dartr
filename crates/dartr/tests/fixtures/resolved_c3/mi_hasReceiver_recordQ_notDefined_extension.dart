// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_recordQ_notDefined_extension).

extension E on (int, String) {
  void foo(int a) {}
}

void f((int, String)? r) {
  r.foo(0);
//  ^^^
// [diag.uncheckedMethodInvocationOfNullableValue] The method 'foo' can't be unconditionally invoked because the receiver can be 'null'.
}
