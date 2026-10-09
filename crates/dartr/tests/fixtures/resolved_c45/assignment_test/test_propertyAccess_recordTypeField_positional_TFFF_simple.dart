
void f((int, String) r) {
  r.$1 = 0;
//  ^^
// [diag.undefinedSetter] The setter '$1' isn't defined for the type '(int, String)'.
}
