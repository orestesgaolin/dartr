
class A {
  const A();
}

class MyClass {
  final A a;
  const MyClass(Object o) : a = o as A;
}
