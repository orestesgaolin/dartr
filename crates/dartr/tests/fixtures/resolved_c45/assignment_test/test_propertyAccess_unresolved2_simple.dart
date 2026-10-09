
void f(int a, int c) {
  (a).b = c;
//    ^
// [diag.undefinedSetter] The setter 'b' isn't defined for the type 'int'.
}
