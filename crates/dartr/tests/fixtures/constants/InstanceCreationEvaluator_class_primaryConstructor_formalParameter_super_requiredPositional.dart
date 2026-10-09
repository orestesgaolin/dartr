
class A {
  final int x;
  const A(this.x);
}
class const B(super.x) extends A;
const b = B(1);
