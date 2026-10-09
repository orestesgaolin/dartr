
class A {
  final int a;
  const A({required this.a});
}

class B<T> extends A {
  final int b;
  const B(this.b, {required super.a}) : super();
}

const x = B<int>(2, a: 1);
