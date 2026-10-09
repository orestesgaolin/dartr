// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_noReceiver_method_superClass).

class A {
  void foo(int _) {}
}

class B extends A {
  void bar() {
    foo(0);
  }
}
