
class A {
  final int x;
  const A({required this.x});
}
class const B({required super.x}) extends A;
const b = B(x: 1);
