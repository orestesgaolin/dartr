
void f(int x) {
  x += 1.2;
//     ^^^
// [diag.invalidAssignment] A value of type 'double' can't be assigned to a variable of type 'int'.
  x -= 1.2;
//     ^^^
// [diag.invalidAssignment] A value of type 'double' can't be assigned to a variable of type 'int'.
  x *= 1.2;
//     ^^^
// [diag.invalidAssignment] A value of type 'double' can't be assigned to a variable of type 'int'.
  x %= 1.2;
//     ^^^
// [diag.invalidAssignment] A value of type 'double' can't be assigned to a variable of type 'int'.
}
