class A {
  A({int? n1, int? n2, int? n3});
}
class B({super.n1, super.n2}) extends A {
  this : super(n3: 3);
}
