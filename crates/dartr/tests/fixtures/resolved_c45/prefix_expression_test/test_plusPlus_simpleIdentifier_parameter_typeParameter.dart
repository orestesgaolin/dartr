
void f<T extends num>(T x) {
  ++x;
//^^^
// [diag.invalidAssignment] A value of type 'num' can't be assigned to a variable of type 'T'.
}
