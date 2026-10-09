// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_remainder_int_context_int_via_extension_explicit).

extension E on int {
  String remainder(num x) => '';
}
T f<T>() => throw Error();
g(int a) {
  h(E(a).remainder(f()));
//  ^^^^^^^^^^^^^^^^^^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'int'.
}
h(int x) {}
