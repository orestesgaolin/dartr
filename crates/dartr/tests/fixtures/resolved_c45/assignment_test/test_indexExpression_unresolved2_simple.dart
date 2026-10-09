
void f(int a, int c) {
  a[b] = c;
// ^^^
// [diag.undefinedOperator] The operator '[]=' isn't defined for the type 'int'.
//  ^
// [diag.undefinedIdentifier] Undefined name 'b'.
}
