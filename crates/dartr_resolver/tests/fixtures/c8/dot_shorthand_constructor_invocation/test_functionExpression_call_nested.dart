// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_functionExpression_call_nested).

class C {
  C(C c);
  C.a();
  C call() => this;
}

void main() {
  C _ = .new(.a())();
}
