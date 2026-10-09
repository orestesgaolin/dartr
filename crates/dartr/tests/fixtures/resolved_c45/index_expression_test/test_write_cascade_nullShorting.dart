
class A {
  void operator[]=(int index, A a) {}
}

void f(A? a) {
  a?..[0] = a..[1] = a;
}
