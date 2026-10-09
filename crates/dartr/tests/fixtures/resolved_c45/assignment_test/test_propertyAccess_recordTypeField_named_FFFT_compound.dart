
extension E on ({int bar}) {
  set foo(int _) {}
}

void f(({int bar}) r) {
  r.foo += 0;
//  ^^^
// [diag.undefinedGetter] The getter 'foo' isn't defined for the type '({int bar})'.
}
