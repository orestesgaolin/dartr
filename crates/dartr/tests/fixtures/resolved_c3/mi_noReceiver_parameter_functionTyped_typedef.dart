// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_noReceiver_parameter_functionTyped_typedef).

typedef F = void Function();

void f(F a) {
  a();
}
