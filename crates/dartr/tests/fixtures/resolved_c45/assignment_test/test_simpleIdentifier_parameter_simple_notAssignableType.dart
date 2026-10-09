
void f(int x) {
  x = true;
//    ^^^^
// [diag.invalidAssignment] A value of type 'bool' can't be assigned to a variable of type 'int'.
}
