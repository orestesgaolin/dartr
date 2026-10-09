
extension E on ({int bar}) {
  int get foo => 0;
}

void f(({int bar}) r) {
  r.foo += 0;
//  ^^^
// [diag.assignmentToFinalNoSetter] There isn't a setter named 'foo' in class 'E'.
}
