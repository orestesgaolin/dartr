// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_clamp_other_int_int).

abstract class A {
  String clamp(int x, int y);
}
f(A a, int b, int c) {
  a.clamp(b, c);
}
