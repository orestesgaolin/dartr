
class A {
  Object operator +(int _) => this;
}

void f(Object x) {
  if (x is A) {
    ++x;
  }
}
