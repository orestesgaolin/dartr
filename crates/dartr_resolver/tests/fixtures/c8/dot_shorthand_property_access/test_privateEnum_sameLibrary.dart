// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_property_access_test.dart (DotShorthandPropertyAccessResolutionTest.test_privateEnum_sameLibrary).

enum _Private { one, two }

typedef Public = _Private;
final Public p = _Private.one;

void main() {
  var x = p;
  x = .two;
  print(x);
}
