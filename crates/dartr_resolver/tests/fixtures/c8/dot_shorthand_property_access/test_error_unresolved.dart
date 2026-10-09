// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_property_access_test.dart (DotShorthandPropertyAccessResolutionTest.test_error_unresolved).

class C { }

void main() {
  C c = .getter;
//       ^^^^^^
// [diag.dotShorthandUndefinedGetter] The static getter 'getter' isn't defined for the context type 'C'.
  print(c);
}
