
void f(int a, int c) {
  a.b += c;
//  ^
// [diag.undefinedGetter] The getter 'b' isn't defined for the type 'int'.
// [diag.undefinedSetter] The setter 'b' isn't defined for the type 'int'.
}
