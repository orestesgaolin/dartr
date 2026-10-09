// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_functionExpression_call_extension).

class C {}

extension CallC on C {
  C call() => this;
}

void main() {
  final C _ = .new()();
}
