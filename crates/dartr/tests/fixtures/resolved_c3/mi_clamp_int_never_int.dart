// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_clamp_int_never_int).

f(int a, Never b, int c) {
  a.clamp(b, c);
//           ^^^
// [diag.deadCode] Dead code.
}
