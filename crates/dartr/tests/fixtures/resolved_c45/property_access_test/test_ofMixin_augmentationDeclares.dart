
mixin A {}

void f(A a) {
  (a).foo;
}

augment mixin A {
  int get foo => 0;
}
