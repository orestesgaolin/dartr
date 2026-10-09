
void f((int, String) r) {
  r.$zero;
//  ^^^^^
// [diag.undefinedGetter] The getter '$zero' isn't defined for the type '(int, String)'.
}
