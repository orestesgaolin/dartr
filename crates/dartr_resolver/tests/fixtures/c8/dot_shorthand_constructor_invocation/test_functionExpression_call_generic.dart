// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_functionExpression_call_generic).

class C {
  C call<T>(T t) => this;
}

void main() {
  C _ = .new()<int>(0);
}
