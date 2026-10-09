// Ported from pkg/analyzer/test/src/dart/resolution/function_expression_invocation_test.dart (test_never).

void f(Never x) {
  x<int>(1 + 2);
//^
// [diag.receiverOfTypeNever] The receiver is of type 'Never', and will never complete with a value.
//      ^^^^^^^^
// [diag.deadCode] Dead code.
}
