// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_clamp_int_int_int_via_extension_explicit).

extension E on int {
  String clamp(int x, int y) => '';
}
f(int a, int b, int c) {
  E(a).clamp(b, c);
}
