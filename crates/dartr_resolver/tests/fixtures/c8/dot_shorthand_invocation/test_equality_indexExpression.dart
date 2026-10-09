// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_equality_indexExpression).

class C {
  int x;
  C(this.x);
  static List<C> instances() => [C(1)];
}

void main() {
  print(C(1) == .instances()[0]);
}
