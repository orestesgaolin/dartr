
class A {
  static int get x => 0;
}

void f() {
  A.x = 2;
//  ^
// [diag.assignmentToFinalNoSetter] There isn't a setter named 'x' in class 'A'.
}
