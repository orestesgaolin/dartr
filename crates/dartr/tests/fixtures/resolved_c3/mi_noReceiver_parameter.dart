// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_noReceiver_parameter).

void f(void Function(int) foo) {
  foo(0);
}
