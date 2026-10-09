
class A {
  final int a;
  const A(this.a);
}

class B extends A {
  final int b;
  const B(super.a, this.b) : super();
}

const x = B(1, 2);
