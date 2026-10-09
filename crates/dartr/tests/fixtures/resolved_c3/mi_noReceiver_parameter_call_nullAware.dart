// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_noReceiver_parameter_call_nullAware).

double Function(int)? foo;

main() {
  foo?.call(1);
}
    