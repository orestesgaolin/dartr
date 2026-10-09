class A {
  A(int p1, {int? n1, required int n2});
}
class B(super.p1, {super.n1, required super.n2}) extends A;
