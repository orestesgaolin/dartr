// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_postfixOperator).

class C {
  static C member() => C(1);
  int x;
  C(this.x);
}

void main() {
  C c = .member()++;
//       ^^^^^^
// [diag.dotShorthandUndefinedInvocation] The static method or constructor 'member' isn't defined for the context type '_'.
//               ^^
// [diag.illegalAssignmentToNonAssignable] Illegal assignment to non-assignable expression.
  print(c);
}
