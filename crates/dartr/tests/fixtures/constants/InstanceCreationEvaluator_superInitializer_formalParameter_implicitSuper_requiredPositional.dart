
class A {
  final int a;
  const A(this.a);
}

class B extends A {
  final int b;
  const B(super.a, this.b);
}

const x = B(1, 2);
