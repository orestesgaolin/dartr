
void f(int a, int b, double c) {
  a + b += c;
//^^^^^
// [diag.missingAssignableSelector] Missing selector such as '.identifier' or '[0]'.
// [diag.illegalAssignmentToNonAssignable] Illegal assignment to non-assignable expression.
}
