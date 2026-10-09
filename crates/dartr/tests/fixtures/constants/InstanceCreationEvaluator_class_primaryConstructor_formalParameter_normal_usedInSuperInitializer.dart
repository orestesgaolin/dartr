
class A {
  final int x;
  const A(this.x);
}
class const B(int y) extends A {
  this : super(y + 1);
}
const b = B(1);
