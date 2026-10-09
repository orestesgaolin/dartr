
class A {
  void f(Object a) {
    a = super;
//      ^^^^^
// [diag.missingAssignableSelector] Missing selector such as '.identifier' or '[0]'.
  }
}
