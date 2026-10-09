// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_class_staticMethod).

class C {
  static void foo(int _) {}
}

main() {
  C.foo(0);
}
