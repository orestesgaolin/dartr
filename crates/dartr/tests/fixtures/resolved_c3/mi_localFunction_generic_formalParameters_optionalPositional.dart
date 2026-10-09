// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_localFunction_generic_formalParameters_optionalPositional).

void f() {
  T g<T>([T? a]) => throw 0;
  g(0);
}
