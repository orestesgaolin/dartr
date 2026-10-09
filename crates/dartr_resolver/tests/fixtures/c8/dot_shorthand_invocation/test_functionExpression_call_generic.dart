// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_functionExpression_call_generic).

class C {
  static C member() => C();
  C call<T>(T t) => this;
}

void main() {
  final C _ = .member()<int>(1);
}
