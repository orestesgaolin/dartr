
void f((int, String) r) {
  r.$3;
//  ^^
// [diag.undefinedGetter] The getter '$3' isn't defined for the type '(int, String)'.
}
