// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_property_access_test.dart (DotShorthandPropertyAccessResolutionTest.test_extensionType).

extension type C(int integer) {
  static C get one => C(1);
}

void main() {
  C c = .one;
  print(c);
}
