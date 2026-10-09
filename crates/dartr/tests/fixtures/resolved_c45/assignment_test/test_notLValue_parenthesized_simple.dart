
void f(int a, double b) {
  (a + 0) = b;
// ^
// [diag.patternTypeMismatchInIrrefutableContext] The matched value of type 'double' isn't assignable to the required type 'int'.
//   ^
// [diag.expectedToken] Expected to find ')'.
}
