
class A<T> {
  void operator[]=(int index, T value) {}
}

void f(A<double> a) {
  a[0] = 1.2;
}
