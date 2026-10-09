// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_instance_Function_call_topVariable).

Function foo = throw Error();

void main() {
  foo.call(0);
}
