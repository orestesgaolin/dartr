// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_clamp_other_context_int).

abstract class A {
  num clamp(String x, String y);
}
T f<T>() => throw Error();
g(A a) {
  h(a.clamp(f(), f()));
//  ^^^^^^^^^^^^^^^^^
// [diag.argumentTypeNotAssignable] The argument type 'num' can't be assigned to the parameter type 'int'.
}
h(int x) {}
