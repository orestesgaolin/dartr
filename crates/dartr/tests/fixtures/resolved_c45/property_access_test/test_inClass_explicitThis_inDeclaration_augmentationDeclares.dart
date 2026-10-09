
int get foo => 0;

class A {
  void f() {
    this.foo;
  }
}

augment class A {
  int get foo => 0;
}
