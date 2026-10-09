
class A {
  bool operator [](int index) => false;
  operator []=(String index, bool value) {}
}

void f(A a) {
  a[ g() ];
}

T g<T>() => throw 0;
