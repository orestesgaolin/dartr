// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_property_access_test.dart (DotShorthandPropertyAccessResolutionTest.test_error_context_none).

void main() {
  var c = .member;
//        ^^^^^^^
// [diag.dotShorthandMissingContext] A dot shorthand can't be used where there is no context type.
  print(c);
}
