
class A {
  int get foo;

  void f() {
    this.foo;
  }
}

augment class A {
  augment int get foo => 0;
}
