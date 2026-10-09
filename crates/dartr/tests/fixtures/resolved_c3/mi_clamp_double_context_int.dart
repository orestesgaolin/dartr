// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_clamp_double_context_int).

T f<T>() => throw Error();
g(double a) {
  h(a.clamp(f(), f()));
//  ^^^^^^^^^^^^^^^^^
// [diag.argumentTypeNotAssignable] The argument type 'num' can't be assigned to the parameter type 'int'.
}
h(int x) {}
