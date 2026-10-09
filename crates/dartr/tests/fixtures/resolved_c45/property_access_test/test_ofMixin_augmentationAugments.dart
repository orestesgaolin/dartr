
mixin A {
  int get foo;
}

void f(A a) {
  (a).foo;
}

augment mixin A {
  augment int get foo => 0;
}
