
class A {
  static int get foo => 0;
}

typedef B = A;

void f() {
  B.foo;
}
