// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_functionTyped).

void foo(int _) {}

main() {
  foo.call(0);
}
