// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_functionExpression_call_extension).

class C {
  static C member() => C();
}

extension CallC on C {
  C call() => this;
}

void main() {
  final C _ = .member()();
}
