// Ported from pkg/analyzer/test/src/dart/resolution/function_expression_invocation_test.dart (test_expression_recordType_noCall).

void f((String,) a) {
  a();
//^
// [diag.invocationOfNonFunctionExpression] The expression doesn't evaluate to a function, so it can't be invoked.
}
