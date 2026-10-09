// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_clamp_int_int_never).

f(int a, int b, Never c) {
  a.clamp(b, c);
}
