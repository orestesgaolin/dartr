// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_clamp_other_int_int_via_extension_explicit).

class A {}
extension E on A {
  String clamp(int x, int y) => '';
}
f(A a, int b, int c) {
  E(a).clamp(b, c);
}
