
extension E on ({int bar}) {
  set foo(int _) {}
}

void f(({int bar}) r) {
  r.foo = 0;
}
