
class A {
  static int get foo;
}

augment class A {
  augment static int get foo => 0;
}

void f() {
  A.foo;
}
