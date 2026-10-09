
void f(Object? x) {
  (switch (x) {
    _ => 0,
  }++);
// ^^
// [diag.illegalAssignmentToNonAssignable] Illegal assignment to non-assignable expression.
}
