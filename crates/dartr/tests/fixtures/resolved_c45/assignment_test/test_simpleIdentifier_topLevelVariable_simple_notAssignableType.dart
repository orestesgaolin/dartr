
int x = 0;

void f() {
  x = true;
//    ^^^^
// [diag.invalidAssignment] A value of type 'bool' can't be assigned to a variable of type 'int'.
}
