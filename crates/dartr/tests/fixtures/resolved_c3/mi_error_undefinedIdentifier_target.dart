// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_undefinedIdentifier_target).

main() {
  bar.foo(0);
//^^^
// [diag.undefinedIdentifier] Undefined name 'bar'.
}
