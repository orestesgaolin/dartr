// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_extension_staticMethod).

extension A on int {
  static void foo(int _) {}
}

void f() {
  A.foo(0);
}
