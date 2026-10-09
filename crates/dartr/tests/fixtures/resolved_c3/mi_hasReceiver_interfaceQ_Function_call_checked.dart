// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_interfaceQ_Function_call_checked).

void f(Function? foo) {
  foo?.call();
}
