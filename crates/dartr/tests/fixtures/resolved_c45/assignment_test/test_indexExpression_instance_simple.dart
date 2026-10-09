
class A {
  operator[]=(int index, num _) {}
}

void f(A a) {
  a[0] = 2;
}
