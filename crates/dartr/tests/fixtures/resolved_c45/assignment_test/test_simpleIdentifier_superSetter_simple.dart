
class A {
  set x(num _) {}
}

class B extends A {
  void f() {
    x = 2;
  }
}
