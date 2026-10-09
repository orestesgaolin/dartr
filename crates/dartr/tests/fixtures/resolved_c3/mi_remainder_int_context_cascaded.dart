// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_remainder_int_context_cascaded).

T f<T>() => throw Error();
g(int a) {
  h(a..remainder(f()));
}
h(int x) {}
