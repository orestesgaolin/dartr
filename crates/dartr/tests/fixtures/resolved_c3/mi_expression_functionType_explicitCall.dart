// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_expression_functionType_explicitCall).

void f(double Function(int p) g) {
  g.call(0);
}
