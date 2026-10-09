// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_functionExpression_call).

class C {
  C call() => this;
}

void main() {
  final C _ = .new()();
}
