// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_interfaceType_enum).

enum E {
  v;
  void foo() {}
}

void f(E e) {
  e.foo();
}
