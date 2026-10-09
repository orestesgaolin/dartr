// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_localFunction_generic_formalParameters_requiredNamed).

void f() {
  T g<T>({required T a}) => throw 0;
  g(a: 0);
}
