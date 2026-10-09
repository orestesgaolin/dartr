// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_property_access_test.dart (DotShorthandPropertyAccessResolutionTest.test_enum_basic).

enum C { red }

void main() {
  C c = .red;
  print(c);
}
