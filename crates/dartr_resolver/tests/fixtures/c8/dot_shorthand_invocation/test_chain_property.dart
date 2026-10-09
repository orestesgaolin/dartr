// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_chain_property).

class C {
  static C member() => C(1);
  int x;
  C(this.x);
  C get property => C(1);
}

void main() {
  C c = .member().property;
  print(c);
}
