// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_instance_getter).

class C {
  double Function(int) get foo => throw Error();
}

void f(C c) {
  c.foo(0);
}
