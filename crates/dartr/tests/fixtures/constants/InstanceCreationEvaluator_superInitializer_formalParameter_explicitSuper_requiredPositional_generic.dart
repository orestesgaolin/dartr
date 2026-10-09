
class A {
  final int a;
  const A(this.a);
}

class B<T> extends A {
  final int b;
  const B(super.a, this.b) : super();
}

const x = B<int>(1, 2);
