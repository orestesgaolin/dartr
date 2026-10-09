// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_inClass_inInstanceMethod_staticMethod).

class A {
  static void foo(int p) {}

  void f() {
    foo(0);
  }
}
