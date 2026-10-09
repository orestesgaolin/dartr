class A {
  A.named(int p1, {int? n1});
}
class B.named(super.p1) extends A {
  this : super.named(n1: 1);
}
