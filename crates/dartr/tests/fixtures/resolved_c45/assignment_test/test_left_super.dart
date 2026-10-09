
class A {
  void f() {
    super = 0;
//  ^^^^^
// [diag.missingAssignableSelector] Missing selector such as '.identifier' or '[0]'.
// [diag.illegalAssignmentToNonAssignable] Illegal assignment to non-assignable expression.
  }
}
