
class A {
  A operator >>>(int amount) => this;
}

void f(A a) {
  a >>> 3;
}
