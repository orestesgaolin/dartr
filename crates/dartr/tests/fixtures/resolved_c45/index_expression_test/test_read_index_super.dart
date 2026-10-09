
class A {
  void f() {
    this[super];
//       ^^^^^
// [diag.missingAssignableSelector] Missing selector such as '.identifier' or '[0]'.
  }

  int operator[](Object index) => 0;
}
