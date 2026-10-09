// Ported from pkg/analyzer/test/src/dart/resolution/function_expression_invocation_test.dart (test_neverQ).

void f(Never? x) {
  x<int>(1 + 2);
//^
// [diag.uncheckedInvocationOfNullableValue] The function can't be unconditionally invoked because it can be 'null'.
}
