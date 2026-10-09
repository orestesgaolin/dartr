// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_property_access_test.dart (DotShorthandPropertyAccessResolutionTest.test_error_unresolved_new).

class C {
  C.named();
}

void main() {
  C c = .new;
//       ^^^
// [diag.dotShorthandUndefinedGetter] The static getter 'new' isn't defined for the context type 'C'.
  print(c);
}
