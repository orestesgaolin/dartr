// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_chain_method).

class C {
  static C member() => C(1);
  int x;
  C(this.x);
  C method() => C(1);
}

void main() {
  C c = .member().method();
  print(c);
}
