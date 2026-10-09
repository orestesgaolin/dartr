// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_equality).

class C {
  int x;
  C.named(this.x);
}

void main() {
  C lhs = C.named(2);
  bool b = lhs == .named(1);
  print(b);
}
