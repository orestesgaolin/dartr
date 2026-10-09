// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_property_access_test.dart (DotShorthandPropertyAccessResolutionTest.test_postfixOperator).

class C {
  static C get member => C(1);
  int x;
  C(this.x);
}

void main() {
  C c = .member++;
//      ^^^^^^^
// [diag.dotShorthandMissingContext] A dot shorthand can't be used where there is no context type.
//             ^^
// [diag.illegalAssignmentToNonAssignable] Illegal assignment to non-assignable expression.
  print(c);
}
