// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_clamp_int_context_none).

T f<T>() => throw Error();
g(int a) {
  a.clamp(f(), f());
}
