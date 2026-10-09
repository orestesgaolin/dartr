
extension type A(int it) {
  int get foo => 0;
  set foo(int _) {}
}

void f(A a) {
  ++a.foo;
}
