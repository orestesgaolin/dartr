// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_equality).

class C {
  static C member(int x) => C(x);
  int x;
  C(this.x);
}

void main() {
  C lhs = C.member(2);
  bool b = lhs == .member(1);
  print(b);
}
