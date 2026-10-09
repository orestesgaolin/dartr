// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_extensionTypeName).

extension type A(int it) {
  static void foo() {}
}

void f() {
  A.foo();
}
