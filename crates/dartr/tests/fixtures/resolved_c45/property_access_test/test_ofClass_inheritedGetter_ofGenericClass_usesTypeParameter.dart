
class A<T> {
  T get foo => throw 0;
}

class B extends A<int> {}

void f(B b) {
  (b).foo;
}
