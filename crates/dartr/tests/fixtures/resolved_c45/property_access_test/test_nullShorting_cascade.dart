
class A {
  int get foo => 0;
  int get bar => 0;
}

void f(A? a) {
  a?..foo..bar;
}
