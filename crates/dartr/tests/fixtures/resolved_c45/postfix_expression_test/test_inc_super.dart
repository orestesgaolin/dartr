
class A {
  void f() {
    super++;
//       ^^
// [diag.illegalAssignmentToNonAssignable] Illegal assignment to non-assignable expression.
  }
}
