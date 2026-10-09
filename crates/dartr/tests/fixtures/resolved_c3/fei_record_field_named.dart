// Ported from pkg/analyzer/test/src/dart/resolution/function_expression_invocation_test.dart (test_record_field_named).

void f(({void Function(int) foo}) r) {
  r.foo(0);
}
