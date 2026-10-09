// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_functionExpression).

class C {
  static C member() => C();
}

void main() {
  final C _ = .member()();
//            ^^^^^^^^^
// [diag.invocationOfNonFunctionExpression] The expression doesn't evaluate to a function, so it can't be invoked.
}
