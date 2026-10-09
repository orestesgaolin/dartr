// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_instance_method_generic).

class C {
  T foo<T>(T a) {
    return a;
  }
}

void f(C c) {
  c.foo(0);
}
