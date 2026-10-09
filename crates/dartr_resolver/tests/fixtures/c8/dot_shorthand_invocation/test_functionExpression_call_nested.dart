// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_functionExpression_call_nested).

class C {
  static C member(C c) => C();
  static C one() => C(); 
  C call() => this;
}

void main() {
  C _ = .member(.one())();
}
