
final num x = 0;

void f() {
  x = 2;
//^
// [diag.assignmentToFinal] 'x' can't be used as a setter because it's final.
}
