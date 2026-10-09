// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_noReceiver_topVariable).

void Function(int) foo = throw Error();

main() {
  foo(0);
}
