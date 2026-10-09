// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_interfaceQ_Function_call_unchecked).

void f(Function? foo) {
  foo.call();
//    ^^^^
// [diag.uncheckedMethodInvocationOfNullableValue] The method 'call' can't be unconditionally invoked because the receiver can be 'null'.
}
