// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_noReceiver_topGetter).

double Function(int) get foo => throw Error();

main() {
  foo(0);
}
