
void f(int x) {
  ++ ++ x;
//      ^
// [diag.missingAssignableSelector] Missing selector such as '.identifier' or '[0]'.
}
