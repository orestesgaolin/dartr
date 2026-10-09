
extension E on ({int foo, String bar}) {
  set foo(int _) {}
}

void f(({int foo, String bar}) r) {
  r.foo += 0;
//  ^^^
// [diag.undefinedSetter] The setter 'foo' isn't defined for the type '({String bar, int foo})'.
}
