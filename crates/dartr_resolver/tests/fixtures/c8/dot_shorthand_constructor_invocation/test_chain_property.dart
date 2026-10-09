// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_chain_property).

class C {
  int x;
  C(this.x);
  C get property => C(1);
}

void main() {
  C c = .new(1).property;
  print(c);
}
