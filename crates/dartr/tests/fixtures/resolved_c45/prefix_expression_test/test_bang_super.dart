
class A {
  void f() {
    !super;
//   ^^^^^
// [diag.missingAssignableSelector] Missing selector such as '.identifier' or '[0]'.
// [diag.nonBoolNegationExpression] A negation operand must have a static type of 'bool'.
  }
}
