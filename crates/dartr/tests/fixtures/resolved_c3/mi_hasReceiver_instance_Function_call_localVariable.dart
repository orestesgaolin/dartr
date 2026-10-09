// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_instance_Function_call_localVariable).

void f(Function getFunction()) {
  Function foo = getFunction();

  foo.call(0);
}
