
void f(int x) {
  !x;
// ^
// [diag.nonBoolNegationExpression] A negation operand must have a static type of 'bool'.
}
