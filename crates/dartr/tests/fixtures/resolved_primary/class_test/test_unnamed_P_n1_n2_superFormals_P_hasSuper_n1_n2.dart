class A {
  A(int p1, {int? n1, int? n2});
}
class B(super.p1) extends A {
  this : super(n1: 1, n2: 2);
}
