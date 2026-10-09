// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_remainder_int_context_int_target_rewritten).

T f<T>() => throw Error();
g(int Function() a) {
  h(a().remainder(f()));
}
h(int x) {}
