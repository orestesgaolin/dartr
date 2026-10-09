
void f((int, String) r) {
  r.$0a;
//  ^^^
// [diag.undefinedGetter] The getter '$0a' isn't defined for the type '(int, String)'.
}
