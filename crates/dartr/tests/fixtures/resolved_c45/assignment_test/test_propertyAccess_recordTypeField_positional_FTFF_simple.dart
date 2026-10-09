
extension E on (int, String) {
  int get $3 => 0;
}

void f((int, String) r) {
  r.$3 = 0;
//  ^^
// [diag.assignmentToFinalNoSetter] There isn't a setter named '$3' in class 'E'.
}
