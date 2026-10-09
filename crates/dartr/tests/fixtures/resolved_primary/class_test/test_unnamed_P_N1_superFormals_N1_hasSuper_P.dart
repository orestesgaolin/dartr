class A {
  A(int p1, {int? n1});
}
class B({super.n1}) extends A {
  this : super(0);
}
