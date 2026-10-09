// Ported from pkg/analyzer/test/src/dart/resolution/function_expression_invocation_test.dart (test_dynamic_withTypeArguments).

main() {
  (main as dynamic)<bool, int>(0);
}
