// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_functionExpression_call_argument).

class C {
  static C member() => C();
  C call(int a) => this;
}

void main() {
  final C _ = .member()(1);
}
