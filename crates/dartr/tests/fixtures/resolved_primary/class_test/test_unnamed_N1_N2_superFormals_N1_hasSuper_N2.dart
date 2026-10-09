class A {
  A({int? n1, int? n2});
}
class B({super.n1}) extends A {
  this : super(n2: 2);
}
