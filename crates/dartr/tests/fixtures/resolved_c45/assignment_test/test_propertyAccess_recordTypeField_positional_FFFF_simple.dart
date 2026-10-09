
void f((int, String) r) {
  r.$4 = 0;
//  ^^
// [diag.undefinedSetter] The setter '$4' isn't defined for the type '(int, String)'.
}
