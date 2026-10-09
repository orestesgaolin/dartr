class A {
  A(int p1, {int? n1, int? n2});
}
class B(super.p1, {super.n1}) extends A {
  this : super(n2: 2);
}
