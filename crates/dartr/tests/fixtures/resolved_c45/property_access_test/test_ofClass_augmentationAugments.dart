
class A {
  int get foo;
}

void f(A a) {
  (a).foo;
}

augment class A {
  augment int get foo => 0;
}
