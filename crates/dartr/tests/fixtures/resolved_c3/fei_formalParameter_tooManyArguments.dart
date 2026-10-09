// Ported from pkg/analyzer/test/src/dart/resolution/function_expression_invocation_test.dart (test_formalParameter_tooManyArguments).

void f(int Function() g, int a) {
  g(a);
//  ^
// [diag.extraPositionalArguments] Too many positional arguments: 0 expected, but 1 found.
}
