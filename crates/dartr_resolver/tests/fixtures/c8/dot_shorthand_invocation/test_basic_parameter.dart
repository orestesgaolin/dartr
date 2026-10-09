// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_basic_parameter).

class C {
  static C member(int x) => C(x);
  int x;
  C(this.x);
}

void main() {
  C c = .member(1);
  print(c);
}
