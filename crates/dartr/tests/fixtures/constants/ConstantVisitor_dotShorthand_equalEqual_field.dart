
class A {
  const A();
  static const A field = A();
}

const v = A() == .field;
