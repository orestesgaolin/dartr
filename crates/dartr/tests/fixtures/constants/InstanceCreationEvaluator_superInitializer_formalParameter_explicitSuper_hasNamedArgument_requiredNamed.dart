
class A {
  final int a;
  final int b;
  const A({required this.a, required this.b});
}

class B extends A {
  final int c;
  const B(this.c, {required super.b}) : super(a: 1);
}

const x = B(3, b: 2);
