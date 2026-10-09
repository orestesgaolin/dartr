// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_neverQuestion_nullAware).

void f(Never? a) {
  a?.foo();
//   ^^^^^
// [diag.deadCode] Dead code.
}
