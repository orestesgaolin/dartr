// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_clamp_int_context_double).

T f<T>() => throw Error();
g(int a) {
  h(a.clamp(f(), f()));
//  ^^^^^^^^^^^^^^^^^
// [diag.argumentTypeNotAssignable] The argument type 'num' can't be assigned to the parameter type 'double'.
}
h(double x) {}
