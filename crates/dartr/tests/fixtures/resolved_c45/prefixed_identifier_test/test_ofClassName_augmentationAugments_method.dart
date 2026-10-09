
class A {
  static void foo();
}

augment class A {
  augment static void foo() {}
}

void f() {
  A.foo;
}
