// Ported from pkg/analyzer/test/src/dart/resolution/function_expression_invocation_test.dart (test_record_field_positional_rewrite).

void f((void Function(int),) r) {
  r.$1(0);
}
