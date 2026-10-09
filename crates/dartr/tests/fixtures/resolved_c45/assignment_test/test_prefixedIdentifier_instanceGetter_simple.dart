
class A {
  int get x => 0;
}

void f(A a) {
  a.x = 2;
//  ^
// [diag.assignmentToFinalNoSetter] There isn't a setter named 'x' in class 'A'.
}
