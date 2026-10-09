// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_functionTyped_generic).

void foo<T>(T _) {}

main() {
  foo.call(0);
}
