// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_interfaceType_extensionType_declared).

extension type A(int it) {
  void foo() {}
}

void f(A a) {
  a.foo();
}
