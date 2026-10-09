// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_remainder_other_context_int_via_extension_explicit).

class A {}
extension E on A {
  String remainder(num x) => '';
}
T f<T>() => throw Error();
g(A a) {
  h(E(a).remainder(f()));
//  ^^^^^^^^^^^^^^^^^^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'int'.
}
h(int x) {}
