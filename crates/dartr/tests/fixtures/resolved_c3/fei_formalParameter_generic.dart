// Ported from pkg/analyzer/test/src/dart/resolution/function_expression_invocation_test.dart (test_formalParameter_generic).

void f(T Function<T>(T a) g) {
  g(0);
}
