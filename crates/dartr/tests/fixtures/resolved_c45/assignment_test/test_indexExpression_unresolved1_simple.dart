
void f(int c) {
  a[b] = c;
//^
// [diag.undefinedIdentifier] Undefined name 'a'.
//  ^
// [diag.undefinedIdentifier] Undefined name 'b'.
}
