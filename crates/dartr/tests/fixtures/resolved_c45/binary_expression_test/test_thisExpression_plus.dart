
class A {
  int operator +(int other) => 0;

  void f() {
    this + 0;
  }
}
