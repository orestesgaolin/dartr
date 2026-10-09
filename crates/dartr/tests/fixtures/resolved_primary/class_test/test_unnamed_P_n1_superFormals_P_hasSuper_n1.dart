class A {
  A(int p1, {int? n1});
}
class B(super.p1) extends A {
  this : super(n1: 1);
}
