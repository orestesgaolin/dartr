
void f<T>() {
  T++;
//^
// [diag.assignmentToType] Types can't be assigned a value.
}
