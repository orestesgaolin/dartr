// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_property_access_test.dart (DotShorthandPropertyAccessResolutionTest.test_const_enum).

enum Color { red, green, blue }

void main() {
  const Color c = .blue;
  print(c);
}
