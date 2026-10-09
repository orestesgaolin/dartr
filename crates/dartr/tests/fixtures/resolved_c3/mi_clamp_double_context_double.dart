// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_clamp_double_context_double).

T f<T>() => throw Error();
g(double a) {
  h(a.clamp(f(), f()));
}
h(double x) {}
