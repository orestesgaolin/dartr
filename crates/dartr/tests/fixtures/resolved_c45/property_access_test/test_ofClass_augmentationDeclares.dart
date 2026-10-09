
class A {}

void f(A a) {
  (a).foo;
}

augment class A {
  int get foo => 0;
}
