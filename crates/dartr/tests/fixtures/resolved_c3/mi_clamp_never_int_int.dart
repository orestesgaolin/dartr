// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_clamp_never_int_int).

f(Never a, int b, int c) {
  a.clamp(b, c);
//^
// [diag.receiverOfTypeNever] The receiver is of type 'Never', and will never complete with a value.
//       ^^^^^^^
// [diag.deadCode] Dead code.
}
