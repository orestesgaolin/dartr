
class A {
  int operator +(int other) => 0;
}

class B extends A {
  int operator +(int other) => 0;

  void f() {
    super + 0;
  }
}
