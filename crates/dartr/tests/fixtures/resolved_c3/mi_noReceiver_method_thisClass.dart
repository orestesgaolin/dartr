// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_noReceiver_method_thisClass).

class C {
  void foo(int _) {}

  void bar() {
    foo(0);
  }
}
