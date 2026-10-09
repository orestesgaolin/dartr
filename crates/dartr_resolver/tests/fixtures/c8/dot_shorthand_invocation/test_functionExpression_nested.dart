// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_functionExpression_nested).

class C {
  static C member(C c) => C();
  static C one() => C(); 
}

void main() {
  C _ = .member(.one())();
//      ^^^^^^^^^^^^^^^
// [diag.invocationOfNonFunctionExpression] The expression doesn't evaluate to a function, so it can't be invoked.
}
