// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_noReceiver_localVariable_promoted).

main() {
  var foo;
  if (foo is void Function(int)) {
    foo(0);
  }
}
