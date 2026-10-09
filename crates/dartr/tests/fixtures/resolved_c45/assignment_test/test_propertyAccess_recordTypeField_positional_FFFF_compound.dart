
void f((int, String) r) {
  r.$4 += 0;
//  ^^
// [diag.undefinedGetter] The getter '$4' isn't defined for the type '(int, String)'.
// [diag.undefinedSetter] The setter '$4' isn't defined for the type '(int, String)'.
}
