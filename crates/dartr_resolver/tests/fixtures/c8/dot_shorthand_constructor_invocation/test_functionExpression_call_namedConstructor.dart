// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_functionExpression_call_namedConstructor).

class C {
  C.named();
  C call() => this;
}

void main() {
  final C _ = .named()();
}
