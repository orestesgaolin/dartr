
extension type A(int it) {
  int get foo => 0;
}

void f(A a) {
  (a).foo;
}
