class A {
  A(int p1, {required int n1, int? n2});
}
class B(super.p1, {super.n2}) extends A {
  this : super(n1: 1);
}
