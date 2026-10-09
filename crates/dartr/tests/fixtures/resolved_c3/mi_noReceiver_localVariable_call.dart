// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_noReceiver_localVariable_call).

class C {
  void call(int _) {}
}

void f(C c) {
  c(0);
}
