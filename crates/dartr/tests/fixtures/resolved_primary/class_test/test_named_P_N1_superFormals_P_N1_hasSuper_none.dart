class A {
  A.named(int p1, {required int n1});
}
class B.named(super.p1, {required super.n1}) extends A {
  this : super.named();
}
