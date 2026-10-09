
class A {
  bool operator[](int index) => false;
}

void f(A? a) {
  a?..[0]..[1];
}
