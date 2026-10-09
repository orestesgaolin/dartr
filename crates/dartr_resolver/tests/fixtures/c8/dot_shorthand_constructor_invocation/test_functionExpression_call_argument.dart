// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_functionExpression_call_argument).

class C {
  C call(int x) => this;
}

void main() {
  C _ = .new()(0);
}
