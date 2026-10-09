
class A {
  static set foo(int _);
}

augment class A {
  augment static set foo(int _) {}
}

void f() {
  A.foo = 0;
}
