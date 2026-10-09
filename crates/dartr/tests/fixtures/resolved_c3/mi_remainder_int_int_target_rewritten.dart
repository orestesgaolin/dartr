// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_remainder_int_int_target_rewritten).

f(int Function() a, int b) {
  a().remainder(b);
}
