
void f(int x) {
  x ++ ++;
//     ^^
// [diag.illegalAssignmentToNonAssignable] Illegal assignment to non-assignable expression.
}
