
extension E on ({int bar}) {
  int get foo => 0;
  set foo(int _) {}
}

void f(({int bar}) r) {
  r.foo = 0;
}
