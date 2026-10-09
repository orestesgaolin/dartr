// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_postfixOperator).

class C {}

void main() {
  C c = .new()++;
//       ^^^
// [diag.dotShorthandUndefinedInvocation] The static method or constructor 'new' isn't defined for the context type '_'.
//            ^^
// [diag.illegalAssignmentToNonAssignable] Illegal assignment to non-assignable expression.
  print(c);
}
