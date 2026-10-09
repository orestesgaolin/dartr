// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_property_access_test.dart (DotShorthandPropertyAccessResolutionTest.test_chain_property).

class C {
  static C get member => C(1);
  int x;
  C(this.x);
  C get property => C(1);
}

void main() {
  C c = .member.property;
  print(c);
}
