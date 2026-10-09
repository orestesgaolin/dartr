// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_instance_method).

class C {
  void foo(int _) {}
}

void f(C c) {
  c.foo(0);
}
